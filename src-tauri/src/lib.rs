use tauri::{Emitter, Manager};
#[cfg(not(target_os = "android"))]
use tauri_plugin_deep_link::DeepLinkExt;

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

    // Safety: if we crash or exit, always disable system proxy (desktop only)
    #[cfg(not(target_os = "android"))]
    {
        let orig_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            system::proxy_setter::ensure_proxy_disabled();
            orig_hook(info);
        }));
    }

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

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_deep_link::init());

    // Single-instance plugin: desktop only (Android handles this via launchMode="singleTask")
    #[cfg(not(target_os = "android"))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // When a second instance is launched (e.g. via deep link),
            // forward the URL to the existing window
            if let Some(url) = args.into_iter().find(|a| a.starts_with("nexvpn://")) {
                let _ = app.emit("deep-link-received", url);
            }
            // Focus existing window
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
        }));
    }

    builder
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

            // Android: poll file-based deep link written by MainActivity
            #[cfg(target_os = "android")]
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    // Same paths used by CoreManager::read_android_paths / MainActivity
                    let candidates: Vec<std::path::PathBuf> = vec![
                        std::path::PathBuf::from("/data/data/com.horusvpn.nexvpn/files/nexvpn/.deep_link"),
                        std::path::PathBuf::from("/data/user/0/com.horusvpn.nexvpn/files/nexvpn/.deep_link"),
                    ];

                    log::info!("Deep link watcher started, watching: {:?}", candidates);

                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        for path in &candidates {
                            if path.exists() {
                                if let Ok(url) = std::fs::read_to_string(path) {
                                    let url = url.trim().to_string();
                                    if !url.is_empty() {
                                        log::info!("Deep link from file: {}", url);
                                        let _ = handle.emit("deep-link-received", &url);
                                    }
                                }
                                let _ = std::fs::remove_file(path);
                            }
                        }
                    }
                });
            }

            // Desktop: use deep-link plugin API
            #[cfg(not(target_os = "android"))]
            {
                let handle = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    for url in event.urls() {
                        let url_str = url.to_string();
                        log::info!("Deep link received: {}", url_str);
                        let _ = handle.emit("deep-link-received", &url_str);
                    }
                });
            }

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
            commands::open_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running NexVPN");

    // Also cleanup on normal exit (desktop)
    #[cfg(not(target_os = "android"))]
    system::proxy_setter::ensure_proxy_disabled();
}
