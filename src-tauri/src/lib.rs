mod menus;
mod node_runtime;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = node_runtime::with_node_runtime(tauri::Builder::default())
        .setup(|app| Ok(menus::setup_menu(app)?))
        .on_menu_event(|app, event| menus::handle_menu_event(app, event))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
          greet,
          node_runtime::node_runtime_status,
          node_runtime::node_runtime_install,
          node_runtime::node_runtime_uninstall,
          node_runtime::node_server_start,
          node_runtime::node_server_stop
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    builder.run(|app, event| {
      if matches!(event, tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit { .. }) {
        node_runtime::shutdown_on_exit(app);
      }
    });
}
