use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::BufRead;
use std::io::{self, Read};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use tauri::{Builder, Wry};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;
use crate::menus;

const NODE_VERSION: &str = "v20.18.1";
const NODE_DIST_BASE_URL: &str = "https://nodejs.org/dist/v20.18.1";

const DEFAULT_SERVER_PORT: u16 = 3179;
const SERVER_ENTRY: &str = include_str!("../resources/node-server/server.mjs");

const EVENT_LOG: &str = "node-runtime://log";
const EVENT_DOWNLOAD_PROGRESS: &str = "node-runtime://download-progress";

/// Integrates Node Runtime lifecycle into the Tauri app builder.
///
/// Groups:
/// - managed state
/// - auto-start on launch
pub fn with_node_runtime(builder: Builder<Wry>) -> Builder<Wry> {
  builder
    .manage(NodeServerState::default())
    .setup(|app| {
      let handle = app.handle().clone();
      tauri::async_runtime::spawn(async move {
        let _ = auto_start_on_launch(handle).await;
      });
      Ok(())
    })
}

pub struct NodeServerState {
  child: Mutex<Option<Child>>,
}

impl Default for NodeServerState {
  fn default() -> Self {
    Self {
      child: Mutex::new(None),
    }
  }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeRuntimeStatus {
  installed: bool,
  node_path: Option<String>,
  version: Option<String>,
  server_running: bool,
  server_port: u16,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
  downloaded: u64,
  total: Option<u64>,
}

fn node_root_dir(app: &AppHandle) -> tauri::Result<PathBuf> {
  let base = app.path().app_data_dir()?;
  Ok(base.join("node-runtime"))
}

fn node_install_dir(app: &AppHandle) -> tauri::Result<PathBuf> {
  Ok(node_root_dir(app)?.join(format!("node-{}", NODE_VERSION)))
}

fn node_bin_path(app: &AppHandle) -> tauri::Result<PathBuf> {
  let artifact = node_artifact()?;
  Ok(node_install_dir(app)?.join(artifact.node_bin_relpath))
}

fn server_dir(app: &AppHandle) -> tauri::Result<PathBuf> {
  Ok(node_root_dir(app)?.join("server"))
}

fn server_entry_path(app: &AppHandle) -> tauri::Result<PathBuf> {
  Ok(server_dir(app)?.join("server.mjs"))
}

fn pid_file_path(app: &AppHandle) -> tauri::Result<PathBuf> {
  Ok(node_root_dir(app)?.join("server.pid"))
}

fn write_pid(app: &AppHandle, pid: u32) {
  if let Ok(path) = pid_file_path(app) {
    let _ = fs::create_dir_all(path.parent().unwrap());
    let _ = fs::write(path, pid.to_string());
  }
}

fn read_pid(app: &AppHandle) -> Option<u32> {
  let path = pid_file_path(app).ok()?;
  let s = fs::read_to_string(path).ok()?;
  s.trim().parse::<u32>().ok()
}

fn clear_pid(app: &AppHandle) {
  if let Ok(path) = pid_file_path(app) {
    let _ = fs::remove_file(path);
  }
}

#[cfg(unix)]
fn kill_pid(pid: u32) -> io::Result<()> {
  // SIGKILL for deterministic stop in dev.
  let res = unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
  if res == 0 {
    return Ok(());
  }
  let err = io::Error::last_os_error();
  // ESRCH means process not found; treat as success.
  if err.raw_os_error() == Some(libc::ESRCH) {
    return Ok(());
  }
  Err(err)
}

#[cfg(windows)]
fn kill_pid(pid: u32) -> io::Result<()> {
  // Best-effort deterministic stop on Windows.
  // Use taskkill to terminate the process tree.
  let status = Command::new("taskkill")
    .arg("/PID")
    .arg(pid.to_string())
    .arg("/T")
    .arg("/F")
    .status()?;
  if status.success() {
    Ok(())
  } else {
    Err(io::Error::new(
      io::ErrorKind::Other,
      format!("taskkill failed with status {}", status),
    ))
  }
}

fn port_available(port: u16) -> bool {
  TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn ensure_server_entry(app: &AppHandle) -> tauri::Result<PathBuf> {
  let dir = server_dir(app)?;
  fs::create_dir_all(&dir).map_err(|e| tauri::Error::Io(e))?;
  let path = server_entry_path(app)?;
  fs::write(&path, SERVER_ENTRY).map_err(|e| tauri::Error::Io(e))?;
  Ok(path)
}

fn compute_sha256_hex(path: &Path) -> io::Result<String> {
  let mut file = fs::File::open(path)?;
  let mut hasher = Sha256::new();
  let mut buf = [0u8; 1024 * 64];
  loop {
    let n = file.read(&mut buf)?;
    if n == 0 {
      break;
    }
    hasher.update(&buf[..n]);
  }
  Ok(hex::encode(hasher.finalize()))
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> tauri::Result<()> {
  let file = fs::File::open(archive).map_err(|e| tauri::Error::Io(e))?;
  let gz = flate2::read::GzDecoder::new(file);
  let mut tar = tar::Archive::new(gz);
  fs::create_dir_all(dest).map_err(|e| tauri::Error::Io(e))?;
  tar.unpack(dest).map_err(|e| tauri::Error::Io(e))?;
  Ok(())
}

fn extract_tar_xz(archive: &Path, dest: &Path) -> tauri::Result<()> {
  let file = fs::File::open(archive).map_err(|e| tauri::Error::Io(e))?;
  let xz = xz2::read::XzDecoder::new(file);
  let mut tar = tar::Archive::new(xz);
  fs::create_dir_all(dest).map_err(|e| tauri::Error::Io(e))?;
  tar.unpack(dest).map_err(|e| tauri::Error::Io(e))?;
  Ok(())
}

fn extract_zip(archive: &Path, dest: &Path) -> tauri::Result<()> {
  let file = fs::File::open(archive).map_err(|e| tauri::Error::Io(e))?;
  let mut zip = zip::ZipArchive::new(file)
    .map_err(|e| tauri::Error::Anyhow(anyhow::anyhow!(e)))?;

  fs::create_dir_all(dest).map_err(|e| tauri::Error::Io(e))?;

  for i in 0..zip.len() {
    let mut entry = zip
      .by_index(i)
      .map_err(|e| tauri::Error::Anyhow(anyhow::anyhow!(e)))?;
    let name = entry.name().to_string();
    let outpath = dest.join(&name);

    if entry.is_dir() {
      fs::create_dir_all(&outpath).map_err(|e| tauri::Error::Io(e))?;
      continue;
    }

    if let Some(parent) = outpath.parent() {
      fs::create_dir_all(parent).map_err(|e| tauri::Error::Io(e))?;
    }

    let mut outfile = fs::File::create(&outpath).map_err(|e| tauri::Error::Io(e))?;
    io::copy(&mut entry, &mut outfile).map_err(|e| tauri::Error::Io(e))?;
  }

  Ok(())
}

fn run_node_version(node_path: &Path) -> Option<String> {
  let out = Command::new(node_path)
    .arg("-v")
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .output()
    .ok()?;
  if !out.status.success() {
    return None;
  }
  let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
  if s.is_empty() { None } else { Some(s) }
}

fn emit_log(app: &AppHandle, message: impl Into<String>) {
  let _ = app.emit(EVENT_LOG, message.into());
}

fn emit_download_progress(app: &AppHandle, downloaded: u64, total: Option<u64>) {
  let _ = app.emit(
    EVENT_DOWNLOAD_PROGRESS,
    DownloadProgress { downloaded, total },
  );
}

#[derive(Clone, Copy, Debug)]
enum ArchiveKind {
  TarGz,
  TarXz,
  Zip,
}

#[derive(Clone, Debug)]
struct NodeArtifact {
  filename: String,
  node_bin_relpath: String,
  kind: ArchiveKind,
  platform_tag: String,
}

fn shasums_url() -> String {
  format!("{}/SHASUMS256.txt", NODE_DIST_BASE_URL)
}

fn node_artifact() -> tauri::Result<NodeArtifact> {
  let os = std::env::consts::OS;
  let arch = std::env::consts::ARCH;

  let (platform_tag, kind) = match (os, arch) {
    ("macos", "aarch64") => ("darwin-arm64", ArchiveKind::TarGz),
    ("macos", "x86_64") => ("darwin-x64", ArchiveKind::TarGz),

    ("linux", "x86_64") => ("linux-x64", ArchiveKind::TarXz),
    ("linux", "aarch64") => ("linux-arm64", ArchiveKind::TarXz),

    ("windows", "x86_64") => ("win-x64", ArchiveKind::Zip),
    ("windows", "aarch64") => ("win-arm64", ArchiveKind::Zip),

    _ => {
      return Err(tauri::Error::Anyhow(anyhow::anyhow!(
        "unsupported platform: os={} arch={}",
        os,
        arch
      )));
    }
  };

  let dirname = format!("node-{}-{}", NODE_VERSION, platform_tag);

  let filename = match kind {
    ArchiveKind::TarGz => format!("{}.tar.gz", dirname),
    ArchiveKind::TarXz => format!("{}.tar.xz", dirname),
    ArchiveKind::Zip => format!("{}.zip", dirname),
  };

  let node_bin_relpath = if os == "windows" {
    format!("{}/node.exe", dirname)
  } else {
    format!("{}/bin/node", dirname)
  };

  Ok(NodeArtifact {
    filename,
    node_bin_relpath,
    kind,
    platform_tag: platform_tag.to_string(),
  })
}

async fn resolve_expected_sha256(artifact_filename: &str) -> tauri::Result<String> {
  let url = shasums_url();
  let body = reqwest::get(&url)
    .await
    .map_err(|e| tauri::Error::Anyhow(anyhow::anyhow!(e)))?
    .text()
    .await
    .map_err(|e| tauri::Error::Anyhow(anyhow::anyhow!(e)))?;

  for line in body.lines() {
    // Format: "<sha>  <filename>"
    let line = line.trim();
    if line.is_empty() {
      continue;
    }
    let Some((sha, name)) = line.split_once("  ") else {
      continue;
    };
    if name.trim() == artifact_filename {
      return Ok(sha.trim().to_string());
    }
  }

  Err(tauri::Error::Anyhow(anyhow::anyhow!(
    "sha256 not found in SHASUMS256.txt for {}",
    artifact_filename
  )))
}

pub fn server_running(state: &NodeServerState) -> bool {
  let mut guard = state.child.lock().unwrap();
  if let Some(child) = guard.as_mut() {
    match child.try_wait() {
      Ok(Some(_status)) => {
        *guard = None;
        false
      }
      Ok(None) => true,
      Err(_) => false,
    }
  } else {
    false
  }
}

#[tauri::command]
pub async fn node_runtime_status(
  app: AppHandle,
  state: tauri::State<'_, NodeServerState>,
) -> tauri::Result<NodeRuntimeStatus> {
  let node_path = node_bin_path(&app)?;
  let installed = node_path.exists();
  let version = if installed { run_node_version(&node_path) } else { None };
  Ok(NodeRuntimeStatus {
    installed,
    node_path: if installed { Some(node_path.display().to_string()) } else { None },
    version,
    server_running: server_running(&state),
    server_port: DEFAULT_SERVER_PORT,
  })
}

#[tauri::command]
pub async fn node_runtime_install(app: AppHandle) -> tauri::Result<NodeRuntimeStatus> {
  let artifact = node_artifact()?;
  emit_log(
    &app,
    format!(
      "Download Node {} ({})",
      NODE_VERSION, artifact.platform_tag
    ),
  );
  let install_dir = node_install_dir(&app)?;
  let downloads_dir = node_root_dir(&app)?.join("downloads");
  fs::create_dir_all(&downloads_dir).map_err(|e| tauri::Error::Io(e))?;

  let archive_path = downloads_dir.join(&artifact.filename);
  if archive_path.exists() {
    let _ = fs::remove_file(&archive_path);
  }

  let archive_url = format!("{}/{}", NODE_DIST_BASE_URL, artifact.filename);
  let resp = reqwest::get(&archive_url)
    .await
    .map_err(|e| tauri::Error::Anyhow(anyhow::anyhow!(e)))?;
  let total = resp.content_length();
  emit_log(
    &app,
    match total {
      Some(t) => format!("Downloading: {} ({} bytes)", archive_url, t),
      None => format!("Downloading: {} (unknown size)", archive_url),
    },
  );

  let mut file = tokio::fs::File::create(&archive_path)
    .await
    .map_err(|e| tauri::Error::Io(e))?;
  let mut downloaded: u64 = 0;
  let mut resp = resp;
  while let Some(chunk) = resp
    .chunk()
    .await
    .map_err(|e| tauri::Error::Anyhow(anyhow::anyhow!(e)))?
  {
    file
      .write_all(&chunk)
      .await
      .map_err(|e| tauri::Error::Io(e))?;
    downloaded += chunk.len() as u64;
    emit_download_progress(&app, downloaded, total);
  }
  file.flush().await.map_err(|e| tauri::Error::Io(e))?;
  emit_log(&app, format!("Download done: {} bytes", downloaded));

  emit_log(&app, "Resolve SHA256 from SHASUMS256.txt…");
  let expected_sha = resolve_expected_sha256(&artifact.filename).await?;
  emit_log(&app, format!("Verify SHA256… expected={}", expected_sha));
  let archive_path_0 = archive_path.clone();
  let sha = tauri::async_runtime::spawn_blocking(move || compute_sha256_hex(&archive_path_0))
    .await
    .map_err(|e| tauri::Error::Anyhow(anyhow::anyhow!(e)))?
    .map_err(|e| tauri::Error::Io(e))?;
  if sha.to_lowercase() != expected_sha.to_lowercase() {
    let _ = fs::remove_file(&archive_path);
    emit_log(
      &app,
      format!(
        "SHA256 mismatch, expected={} got={}",
        expected_sha, sha
      ),
    );
    return Err(tauri::Error::Anyhow(anyhow::anyhow!(
      "node archive sha256 mismatch: expected {}, got {}",
      expected_sha,
      sha
    )));
  }
  emit_log(&app, "SHA256 OK");

  if install_dir.exists() {
    let _ = fs::remove_dir_all(&install_dir);
  }
  fs::create_dir_all(&install_dir).map_err(|e| tauri::Error::Io(e))?;

  emit_log(&app, "Extracting…");
  let archive_path_2 = archive_path.clone();
  let install_dir_2 = install_dir.clone();
  let kind = artifact.kind;
  tauri::async_runtime::spawn_blocking(move || match kind {
    ArchiveKind::TarGz => extract_tar_gz(&archive_path_2, &install_dir_2),
    ArchiveKind::TarXz => extract_tar_xz(&archive_path_2, &install_dir_2),
    ArchiveKind::Zip => extract_zip(&archive_path_2, &install_dir_2),
  })
    .await
    .map_err(|e| tauri::Error::Anyhow(anyhow::anyhow!(e)))??;
  emit_log(&app, "Extract done");

  let node_path = node_bin_path(&app)?;
  if !node_path.exists() {
    return Err(tauri::Error::Anyhow(anyhow::anyhow!(
      "node binary not found after extract: {}",
      node_path.display()
    )));
  }

  if let Some(v) = run_node_version(&node_path) {
    emit_log(&app, format!("Installed: {} ({})", v, node_path.display()));
  }

  Ok(NodeRuntimeStatus {
    installed: true,
    node_path: Some(node_path.display().to_string()),
    version: run_node_version(&node_path),
    server_running: false,
    server_port: DEFAULT_SERVER_PORT,
  })
}

#[tauri::command]
pub async fn node_runtime_uninstall(
  app: AppHandle,
  state: tauri::State<'_, NodeServerState>,
) -> tauri::Result<NodeRuntimeStatus> {
  emit_log(&app, "Uninstall: stopping service (best-effort)...");
  let _ = node_server_stop(app.clone(), state.clone()).await;

  let root = node_root_dir(&app)?;
  if !root.exists() {
    emit_log(&app, "Uninstall: nothing to remove");
    return node_runtime_status(app, state).await;
  }

  emit_log(&app, format!("Uninstall: removing {}", root.display()));
  let root2 = root.clone();
  tauri::async_runtime::spawn_blocking(move || fs::remove_dir_all(root2))
    .await
    .map_err(|e| tauri::Error::Anyhow(anyhow::anyhow!(e)))?
    .map_err(|e| tauri::Error::Io(e))?;

  emit_log(&app, "Uninstall: done");
  node_runtime_status(app, state).await
}

#[tauri::command]
pub async fn node_server_start(
  app: AppHandle,
  state: tauri::State<'_, NodeServerState>,
) -> tauri::Result<NodeRuntimeStatus> {
  if server_running(&state) {
    emit_log(&app, "服务器已在运行");
    return node_runtime_status(app, state).await;
  }

  let node_path = node_bin_path(&app)?;
  if !node_path.exists() {
    return Err(tauri::Error::Anyhow(anyhow::anyhow!(
      "node runtime not installed"
    )));
  }

  let entry = ensure_server_entry(&app)?;
  let workdir = server_dir(&app)?;
  let port = DEFAULT_SERVER_PORT;

  // If port is occupied, try best-effort cleanup using pidfile.
  if !port_available(port) {
    emit_log(&app, format!("端口 {} 已被占用：尝试清理遗留进程…", port));
    if let Some(pid) = read_pid(&app) {
      match kill_pid(pid) {
        Ok(_) => emit_log(&app, format!("已按 PID 结束进程：{}", pid)),
        Err(e) => emit_log(&app, format!("按 PID 结束进程失败：{} ({})", pid, e)),
      }
      clear_pid(&app);
    }
  }

  if !port_available(port) {
    emit_log(&app, format!("端口 {} 仍被占用，启动取消", port));
    return Err(tauri::Error::Anyhow(anyhow::anyhow!(
      "port {} is already in use",
      port
    )));
  }

  emit_log(
    &app,
    format!(
      "启动服务器：{} {} --port {}",
      node_path.display(),
      entry.display(),
      port
    ),
  );

  let mut cmd = Command::new(&node_path);
  cmd.current_dir(&workdir)
    .arg(entry)
    .arg("--port")
    .arg(port.to_string())
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

  let mut child = cmd.spawn().map_err(|e| tauri::Error::Io(e))?;
  write_pid(&app, child.id());
  let _ = menus::refresh_node_runtime_menu_label(&app, Some(port), true);

  if let Some(stdout) = child.stdout.take() {
    let app2 = app.clone();
    std::thread::spawn(move || {
      let reader = io::BufReader::new(stdout);
      for line in reader.lines().flatten() {
        emit_log(&app2, format!("[node][stdout] {}", line));
      }
    });
  }

  if let Some(stderr) = child.stderr.take() {
    let app2 = app.clone();
    std::thread::spawn(move || {
      let reader = io::BufReader::new(stderr);
      for line in reader.lines().flatten() {
        emit_log(&app2, format!("[node][stderr] {}", line));
      }
    });
  }

  *state.child.lock().unwrap() = Some(child);

  node_runtime_status(app, state).await
}

pub async fn auto_start_on_launch(app: AppHandle) -> tauri::Result<()> {
  let node_path = node_bin_path(&app)?;
  if !node_path.exists() {
    emit_log(&app, "应用启动：未安装私有 Node，跳过自动启动（请先安装 Node）");
    return Ok(());
  }

  let state = app.state::<NodeServerState>();
  if server_running(&state) {
    emit_log(&app, "应用启动：服务已在运行，跳过自动启动");
    return Ok(());
  }

  emit_log(&app, "应用启动：自动启动服务…");
  let _ = node_server_start(app.clone(), state).await?;
  Ok(())
}

#[tauri::command]
pub async fn node_server_stop(
  app: AppHandle,
  state: tauri::State<'_, NodeServerState>,
) -> tauri::Result<NodeRuntimeStatus> {
  emit_log(&app, "停止服务器…");
  {
    let mut guard = state.child.lock().unwrap();
    if let Some(mut child) = guard.take() {
      let _ = child.kill();
    }
  }
  // Also try to kill by pid file (covers orphaned process after app restart/crash).
  if let Some(pid) = read_pid(&app) {
    match kill_pid(pid) {
      Ok(_) => emit_log(&app, format!("已按 PID 结束进程：{}", pid)),
      Err(e) => emit_log(&app, format!("按 PID 结束进程失败：{} ({})", pid, e)),
    }
  }
  clear_pid(&app);
  let _ = menus::refresh_node_runtime_menu_label(&app, None, false);
  emit_log(&app, "已停止（如仍可访问 /health，通常是历史遗留进程占用端口）");
  node_runtime_status(app, state).await
}

pub fn shutdown_on_exit(app: &AppHandle) {
  let state = app.state::<NodeServerState>();
  // Best-effort stop.
  let _ = tauri::async_runtime::block_on(async {
    let _ = node_server_stop(app.clone(), state).await;
  });
}

