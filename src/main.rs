#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod engine;
mod mihomo;
mod mitm;
mod scheduler;
mod state;
mod subscription;
mod sysproxy;
mod tray;
mod ui;

use crate::config::AppConfig;
use crate::engine::ProxyEngine;
use crate::ui::MainView;
use gpui::*;

fn main() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(4)
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime");
    let rt_handle = rt.handle().clone();

    let config = AppConfig::load();
    let engine = ProxyEngine::new(config).expect("Failed to initialize ProxyEngine");
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
            move |window, cx| cx.new(|cx| MainView::new(engine_clone, rt_clone, window, cx)),
        )
        .expect("Failed to open GPUI window");
    });

    // Cleanly stop mihomo, remove tray icon, and restore Windows system proxy on exit
    tray::remove_tray_icon();
    rt.block_on(async move {
        engine_for_shutdown.stop().await;
    });
}
