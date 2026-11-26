pub mod config;
pub mod engine;
pub mod mihomo;
pub mod mitm;
pub mod scheduler;
pub mod state;
pub mod subscription;
pub mod sysproxy;
pub mod tray;

#[cfg(feature = "web")]
pub mod commands;

#[cfg(feature = "lite")]
pub mod ui;

/// Runs the Tauri v2 + React high-fidelity Braun Web desktop edition.
#[cfg(feature = "web")]
pub fn run_web() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(4)
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime");
    let rt_handle = rt.handle().clone();

    let config = config::AppConfig::load();
    let engine = engine::ProxyEngine::new(config).expect("Failed to initialize ProxyEngine");
    let engine_for_shutdown = engine.clone();
    tray::spawn_tray(engine.clone(), rt_handle.clone());

    tauri::Builder::default()
        .manage(commands::AppState {
            engine: engine.clone(),
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::start_proxy,
            commands::stop_proxy,
            commands::select_node,
            commands::rotate_next_node,
            commands::ping_all_nodes,
            commands::check_exit_ip,
            commands::set_outbound_mode,
            commands::toggle_system_proxy,
            commands::toggle_tun,
            commands::toggle_chain_proxy,
            commands::set_chain_exit_node,
            commands::set_rotation_mode,
            commands::add_profile,
            commands::activate_profile,
            commands::refresh_profile,
            commands::delete_profile,
            commands::save_strategy,
            commands::apply_custom_chain_exit,
            commands::install_ca,
            commands::download_core,
            commands::save_system_settings,
            commands::set_sidebar_collapsed,
            commands::trigger_elevate,
            commands::hide_to_tray,
        ])
        .run(tauri::generate_context!())
        .expect("error while running VP429 Tauri application");

    tray::remove_tray_icon();
    rt.block_on(async move {
        engine_for_shutdown.stop().await;
    });
}

/// Runs the Pure Rust + GPUI ultra-low-memory Lite desktop edition.
#[cfg(feature = "lite")]
pub fn run_lite() {
    use gpui::*;

    let _ = rustls::crypto::ring::default_provider().install_default();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(4)
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime");
    let rt_handle = rt.handle().clone();

    let config = config::AppConfig::load();
    let engine = engine::ProxyEngine::new(config).expect("Failed to initialize ProxyEngine");
    let engine_for_shutdown = engine.clone();
    tray::spawn_tray(engine.clone(), rt_handle.clone());

    Application::new()
        .with_assets(ui::icons::EmbeddedAssets)
        .run(move |cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1040.0), px(670.0)), cx);
            let engine_clone = engine.clone();
            let rt_clone = rt_handle.clone();

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(1040.0), px(670.0))),
                    is_resizable: false,
                    is_minimizable: true,
                    titlebar: Some(TitlebarOptions {
                        title: Some("VP429".into()),
                        appears_transparent: true,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |window, cx| cx.new(|cx| ui::MainView::new(engine_clone, rt_clone, window, cx)),
            )
            .expect("Failed to open GPUI window");
        });

    tray::remove_tray_icon();
    rt.block_on(async move {
        engine_for_shutdown.stop().await;
    });
}
