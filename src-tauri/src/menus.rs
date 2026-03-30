use tauri::image::Image;
use tauri::menu::{AboutMetadata, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{App, AppHandle, Runtime};
use tauri_plugin_opener::OpenerExt;
use tauri::Emitter;

const GITHUB_REPO_URL: &str = "https://github.com/OSpoon/starter-tauri-app";
const ISSUES_URL: &str = "https://github.com/OSpoon/starter-tauri-app/issues";

pub fn setup_menu<R: Runtime>(app: &App<R>) -> tauri::Result<()> {
  // macOS App menu (About / Theme / Check for Updates / Services / Hide / Hide Others / Show All / Quit)
  let about_icon = Image::from_bytes(include_bytes!("../icons/icon.png"))?;
  let about = PredefinedMenuItem::about(
    app,
    None,
    Some(AboutMetadata {
      icon: Some(about_icon),
      copyright: Some("© 2026 OSpoon".to_string()),
      ..Default::default()
    }),
  )?;

  let services = PredefinedMenuItem::services(app, None)?;
  let hide = PredefinedMenuItem::hide(app, None)?;
  let hide_others = PredefinedMenuItem::hide_others(app, None)?;
  let show_all = PredefinedMenuItem::show_all(app, None)?;
  let quit = PredefinedMenuItem::quit(app, None)?;
  let sep = PredefinedMenuItem::separator(app)?;

  // Custom items
  let theme_light = MenuItem::with_id(app, "theme_light", "Light", true, None::<&str>)?;
  let theme_dark = MenuItem::with_id(app, "theme_dark", "Dark", true, None::<&str>)?;
  let theme_system = MenuItem::with_id(app, "theme_system", "System", true, None::<&str>)?;
  let theme_menu = Submenu::with_items(app, "Theme", true, &[&theme_light, &theme_dark, &theme_system])?;
  let node_open = MenuItem::with_id(app, "open_node_runtime", "Open…", true, None::<&str>)?;
  let node_start = MenuItem::with_id(app, "node_server_start", "Start Service", true, None::<&str>)?;
  let node_sep = PredefinedMenuItem::separator(app)?;
  let node_items: Vec<&dyn IsMenuItem<R>> = vec![&node_open, &node_sep, &node_start];
  let node_menu = Submenu::with_items(
    app,
    "Node Runtime",
    true,
    node_items.as_slice(),
  )?;
  let check_updates = MenuItem::with_id(app, "check_updates", "Check for Updates…", true, None::<&str>)?;

  let app_menu = Submenu::with_items(
    app,
    app.package_info().name.clone(),
    true,
    &[
      &about,
      &sep,
      &theme_menu,
      &node_menu,
      &check_updates,
      &sep,
      &services,
      &sep,
      &hide,
      &hide_others,
      &show_all,
      &sep,
      &quit,
    ],
  )?;

  // Help
  let help_github = MenuItem::with_id(app, "help_github", "GitHub", true, None::<&str>)?;
  let help_issues = MenuItem::with_id(app, "help_issues", "Issues", true, None::<&str>)?;
  let help_menu = Submenu::with_items(app, "Help", true, &[&help_github, &help_issues])?;

  // Default menus (File / Edit / View / Window)
  let close_window = PredefinedMenuItem::close_window(app, None)?;
  let minimize = PredefinedMenuItem::minimize(app, None)?;
  let maximize = PredefinedMenuItem::maximize(app, None)?;
  let fullscreen = PredefinedMenuItem::fullscreen(app, None)?;

  let undo = PredefinedMenuItem::undo(app, None)?;
  let redo = PredefinedMenuItem::redo(app, None)?;
  let cut = PredefinedMenuItem::cut(app, None)?;
  let copy = PredefinedMenuItem::copy(app, None)?;
  let paste = PredefinedMenuItem::paste(app, None)?;
  let select_all = PredefinedMenuItem::select_all(app, None)?;

  let file_menu = Submenu::with_items(app, "File", true, &[&close_window])?;
  let edit_menu = Submenu::with_items(
    app,
    "Edit",
    true,
    &[&undo, &redo, &sep, &cut, &copy, &paste, &sep, &select_all],
  )?;
  let view_menu = Submenu::with_items(app, "View", true, &[&fullscreen])?;
  let window_menu = Submenu::with_items(app, "Window", true, &[&minimize, &maximize])?;

  let menu = Menu::with_items(
    app,
    &[&app_menu, &file_menu, &edit_menu, &view_menu, &window_menu, &help_menu],
  )?;

  app.set_menu(menu)?;
  Ok(())
}

pub fn refresh_node_runtime_menu_label<R: Runtime>(
  app: &AppHandle<R>,
  port: Option<u16>,
  running: bool,
) -> tauri::Result<()> {
  // Rebuild the whole menu with updated label (simple & reliable).
  // Note: keep this in sync with setup_menu.
  let app_ref = app;

  let about_icon = Image::from_bytes(include_bytes!("../icons/icon.png"))?;
  let about = PredefinedMenuItem::about(
    app_ref,
    None,
    Some(AboutMetadata {
      icon: Some(about_icon),
      copyright: Some("© 2026 OSpoon".to_string()),
      ..Default::default()
    }),
  )?;

  let services = PredefinedMenuItem::services(app_ref, None)?;
  let hide = PredefinedMenuItem::hide(app_ref, None)?;
  let hide_others = PredefinedMenuItem::hide_others(app_ref, None)?;
  let show_all = PredefinedMenuItem::show_all(app_ref, None)?;
  let quit = PredefinedMenuItem::quit(app_ref, None)?;
  let sep = PredefinedMenuItem::separator(app_ref)?;

  let theme_light = MenuItem::with_id(app_ref, "theme_light", "Light", true, None::<&str>)?;
  let theme_dark = MenuItem::with_id(app_ref, "theme_dark", "Dark", true, None::<&str>)?;
  let theme_system = MenuItem::with_id(app_ref, "theme_system", "System", true, None::<&str>)?;
  let theme_menu = Submenu::with_items(app_ref, "Theme", true, &[&theme_light, &theme_dark, &theme_system])?;

  let node_open = MenuItem::with_id(app_ref, "open_node_runtime", "Open…", true, None::<&str>)?;
  let node_start = MenuItem::with_id(app_ref, "node_server_start", "Start Service", true, None::<&str>)?;
  let node_stop = MenuItem::with_id(app_ref, "node_server_stop", "Stop Service", true, None::<&str>)?;
  let node_sep = PredefinedMenuItem::separator(app_ref)?;
  let mut node_items: Vec<&dyn IsMenuItem<R>> = vec![&node_open, &node_sep];
  if running {
    node_items.push(&node_stop);
  }
  else {
    node_items.push(&node_start);
  }
  let node_menu = Submenu::with_items(
    app_ref,
    match port {
      Some(p) => format!("Node Runtime ({})", p),
      None => "Node Runtime".to_string(),
    },
    true,
    node_items.as_slice(),
  )?;

  let check_updates = MenuItem::with_id(app_ref, "check_updates", "Check for Updates…", true, None::<&str>)?;

  let app_menu = Submenu::with_items(
    app_ref,
    app_ref.package_info().name.clone(),
    true,
    &[
      &about,
      &sep,
      &theme_menu,
      &node_menu,
      &check_updates,
      &sep,
      &services,
      &sep,
      &hide,
      &hide_others,
      &show_all,
      &sep,
      &quit,
    ],
  )?;

  let help_github = MenuItem::with_id(app_ref, "help_github", "GitHub", true, None::<&str>)?;
  let help_issues = MenuItem::with_id(app_ref, "help_issues", "Issues", true, None::<&str>)?;
  let help_menu = Submenu::with_items(app_ref, "Help", true, &[&help_github, &help_issues])?;

  let close_window = PredefinedMenuItem::close_window(app_ref, None)?;
  let minimize = PredefinedMenuItem::minimize(app_ref, None)?;
  let maximize = PredefinedMenuItem::maximize(app_ref, None)?;
  let fullscreen = PredefinedMenuItem::fullscreen(app_ref, None)?;

  let undo = PredefinedMenuItem::undo(app_ref, None)?;
  let redo = PredefinedMenuItem::redo(app_ref, None)?;
  let cut = PredefinedMenuItem::cut(app_ref, None)?;
  let copy = PredefinedMenuItem::copy(app_ref, None)?;
  let paste = PredefinedMenuItem::paste(app_ref, None)?;
  let select_all = PredefinedMenuItem::select_all(app_ref, None)?;

  let file_menu = Submenu::with_items(app_ref, "File", true, &[&close_window])?;
  let edit_menu = Submenu::with_items(
    app_ref,
    "Edit",
    true,
    &[&undo, &redo, &sep, &cut, &copy, &paste, &sep, &select_all],
  )?;
  let view_menu = Submenu::with_items(app_ref, "View", true, &[&fullscreen])?;
  let window_menu = Submenu::with_items(app_ref, "Window", true, &[&minimize, &maximize])?;

  let menu = Menu::with_items(
    app_ref,
    &[&app_menu, &file_menu, &edit_menu, &view_menu, &window_menu, &help_menu],
  )?;
  app_ref.set_menu(menu)?;
  Ok(())
}

pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
  match event.id().as_ref() {
    "check_updates" => {
      let _ = app.emit("app://check-updates", ());
    }
    "theme_light" => {
      let _ = app.emit("app://set-theme", "light");
    }
    "theme_dark" => {
      let _ = app.emit("app://set-theme", "dark");
    }
    "theme_system" => {
      let _ = app.emit("app://set-theme", "auto");
    }
    "open_node_runtime" => {
      let _ = app.emit("app://open-node-runtime", ());
    }
    "node_server_start" => {
      let _ = app.emit("app://node-server-start", ());
    }
    "node_server_stop" => {
      let _ = app.emit("app://node-server-stop", ());
    }
    "help_github" => {
      let _ = app.opener().open_url(GITHUB_REPO_URL, None::<&str>);
    }
    "help_issues" => {
      let _ = app.opener().open_url(ISSUES_URL, None::<&str>);
    }
    _ => {}
  }
}

