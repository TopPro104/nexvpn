use tauri::Manager;

mod commands;
mod core;
mod proxy;
mod system;
mod testing;

use commands::{AppContext, load_state};
use core::manager::CoreManager;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    // Safety: if we crash or exit, always disable system proxy
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        system::proxy_setter::ensure_proxy_disabled();
        orig_hook(info);
    }));

    let state = load_state();
    let core_manager = CoreManager::new();

    // Restore saved core type and ports from settings
    let saved_core = state.selected_core.clone();
    let saved_socks = state.settings.socks_port;
    let saved_http = state.settings.http_port;

    let app_context = AppContext {
        core: core_manager,
        state: Arc::new(Mutex::new(state)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_deep_link::init())
        .manage(app_context)
        .setup(move |app| {
            let resource_dir = app
                .path()
                .resource_dir()
                .unwrap_or_else(|_| {
                    std::env::current_exe()
                        .ok()
                        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                        .unwrap_or_else(|| std::path::PathBuf::from("."))
                });

            log::info!("Resource dir: {:?}", resource_dir);

            let ctx: tauri::State<AppContext> = app.state();
            let dir = resource_dir.clone();
            let core_type = saved_core;
            tauri::async_runtime::block_on(async move {
                ctx.core.set_sidecar_dir(dir).await;
                ctx.core.set_core_type(core_type).await;
                ctx.core.set_ports(saved_socks, saved_http).await;
            });

            Ok(())
        })
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                system::proxy_setter::ensure_proxy_disabled();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_servers,
            commands::add_links,
            commands::add_subscription,
            commands::remove_server,
            commands::connect,
            commands::disconnect,
            commands::get_status,
            commands::set_core_type,
            commands::ping_server,
            commands::ping_all_servers,
            commands::get_subscriptions,
            commands::update_subscription,
            commands::delete_subscription,
            commands::auto_select_server,
            commands::export_config,
            commands::import_config,
            commands::get_traffic_stats,
            commands::get_settings,
            commands::save_settings,
            commands::get_logs,
            commands::clear_logs,
            commands::get_connection_history,
            commands::clear_connection_history,
            commands::get_device_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running NexVPN");

    // Also cleanup on normal exit
    system::proxy_setter::ensure_proxy_disabled();
}
