use crate::config::{AppConfig, ChainExitSource, RotationMode};
use crate::engine::ProxyEngine;
use crate::mihomo::{is_elevated, relaunch_as_admin};
use crate::state::LogLevel;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

pub struct AppState {
    pub engine: Arc<ProxyEngine>,
}

#[derive(Serialize)]
pub struct NodeDto {
    pub name: String,
    pub proxy_type: String,
    pub server: String,
    pub port: u16,
    pub latency_ms: Option<u64>,
    pub cooldown_remaining_secs: u64,
    pub hit_429_count: u64,
    pub success_count: u64,
    pub is_active: bool,
}

#[derive(Serialize)]
pub struct LogDto {
    pub timestamp: String,
    pub level: &'static str,
    pub message: String,
}

#[derive(Serialize)]
pub struct TelemetryDto {
    pub up_bps: u64,
    pub down_bps: u64,
    pub total_up: u64,
    pub total_down: u64,
    pub speed_history: Vec<(u64, u64)>,
    pub uptime_secs: u64,
    pub local_ip: String,
    pub exit_ip: Option<String>,
    pub exit_country_code: Option<String>,
    pub checking_ip: bool,
    pub outbound_mode: String,
}

#[derive(Serialize)]
pub struct FullStateDto {
    pub running: bool,
    pub testing_latency: bool,
    pub ca_installed: bool,
    pub elevated: bool,
    pub active_node: Option<String>,
    pub nodes: Vec<NodeDto>,
    pub total_429: u64,
    pub total_retries_ok: u64,
    pub ready_count: usize,
    pub cooldown_count: usize,
    pub telemetry: TelemetryDto,
    pub config: AppConfig,
    pub logs: Vec<LogDto>,
}

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> FullStateDto {
    let eng = &state.engine;
    let snap = eng.scheduler.snapshot();
    let tel = eng.telemetry_snapshot();
    let cfg = eng.config.read().unwrap().clone();
    let logs = eng
        .bus
        .snapshot_logs()
        .into_iter()
        .map(|l| LogDto {
            timestamp: l.timestamp,
            level: match l.level {
                LogLevel::Info => "INFO",
                LogLevel::RateLimit429 => "429",
                LogLevel::Success => "OK",
                LogLevel::Error => "ERR",
            },
            message: l.message,
        })
        .collect();

    FullStateDto {
        running: eng.is_running(),
        testing_latency: eng.is_testing_latency(),
        ca_installed: eng.is_ca_installed(),
        elevated: is_elevated(),
        active_node: snap.active_node,
        nodes: snap
            .nodes
            .into_iter()
            .map(|n| NodeDto {
                name: n.name,
                proxy_type: n.proxy_type,
                server: n.server,
                port: n.port,
                latency_ms: n.latency_ms,
                cooldown_remaining_secs: n.cooldown_remaining_secs,
                hit_429_count: n.hit_429_count,
                success_count: n.success_count,
                is_active: n.is_active,
            })
            .collect(),
        total_429: snap.total_429,
        total_retries_ok: snap.total_retries_ok,
        ready_count: snap.ready_count,
        cooldown_count: snap.cooldown_count,
        telemetry: TelemetryDto {
            up_bps: tel.up_bps,
            down_bps: tel.down_bps,
            total_up: tel.total_up,
            total_down: tel.total_down,
            speed_history: tel.speed_history,
            uptime_secs: tel.uptime_secs,
            local_ip: tel.local_ip,
            exit_ip: tel.exit_ip,
            exit_country_code: tel.exit_country_code,
            checking_ip: tel.checking_ip,
            outbound_mode: tel.outbound_mode,
        },
        config: cfg,
        logs,
    }
}

#[tauri::command]
pub async fn start_proxy(state: State<'_, AppState>) -> Result<(), String> {
    state.engine.start().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_proxy(state: State<'_, AppState>) -> Result<(), String> {
    state.engine.stop().await;
    Ok(())
}

#[tauri::command]
pub async fn select_node(
    state: State<'_, AppState>,
    name: String,
) -> Result<Option<String>, String> {
    Ok(state.engine.select_node_manual(&name).await)
}

#[tauri::command]
pub async fn rotate_next_node(state: State<'_, AppState>) -> Result<Option<String>, String> {
    Ok(state.engine.scheduler.rotate_next("手动轮换节点").await)
}

#[tauri::command]
pub async fn ping_all_nodes(state: State<'_, AppState>) -> Result<(), String> {
    state.engine.ping_all_nodes().await;
    Ok(())
}

#[tauri::command]
pub async fn check_exit_ip(state: State<'_, AppState>) -> Result<(), String> {
    state.engine.check_exit_ip().await;
    Ok(())
}

#[tauri::command]
pub async fn set_outbound_mode(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    state.engine.set_outbound_mode(&mode).await;
    Ok(())
}

#[tauri::command]
pub async fn toggle_system_proxy(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    state.engine.toggle_system_proxy(enabled).await;
    Ok(())
}

#[tauri::command]
pub async fn toggle_tun(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    {
        let mut c = state.engine.config.write().unwrap();
        c.enable_tun = enabled;
    }
    state.engine.save_config();

    if state.engine.is_running() {
        if enabled && !is_elevated() {
            state.engine.bus.warn_429(
                "TUN 已设为开启，但当前非管理员权限；请点击「提权」以管理员身份重启后生效",
            );
        } else if let Err(e) = state.engine.reload_mihomo_runtime().await {
            state
                .engine
                .bus
                .error(format!("热切换 TUN 模式失败（请尝试重启代理）: {}", e));
        } else {
            state.engine.bus.ok(format!(
                "TUN 虚拟网卡已实时{}",
                if enabled { "开启" } else { "关闭" }
            ));
        }
    } else {
        state.engine.bus.info(format!(
            "TUN 虚拟网卡已设为 {}",
            if enabled { "开启" } else { "关闭" }
        ));
    }
    state.engine.bus.notify_state();
    Ok(())
}

#[tauri::command]
pub async fn toggle_chain_proxy(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    state
        .engine
        .toggle_chain_proxy(enabled)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_chain_exit_node(
    state: State<'_, AppState>,
    name: Option<String>,
) -> Result<(), String> {
    match name {
        Some(n) => state
            .engine
            .set_chain_exit_subscription_node(&n)
            .await
            .map_err(|e| e.to_string()),
        None => state
            .engine
            .clear_chain_exit()
            .await
            .map_err(|e| e.to_string()),
    }
}

#[tauri::command]
pub async fn set_chain_exit_pool(
    state: State<'_, AppState>,
    profile_id: Option<String>,
) -> Result<(), String> {
    match profile_id {
        Some(pid) => state
            .engine
            .set_chain_exit_profile_pool(&pid)
            .await
            .map_err(|e| e.to_string()),
        None => state
            .engine
            .clear_chain_exit()
            .await
            .map_err(|e| e.to_string()),
    }
}

#[tauri::command]
pub fn set_rotation_mode(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    let parsed = match mode.as_str() {
        "req" => RotationMode::RoundRobinPerRequest,
        "int" => RotationMode::RoundRobinInterval { seconds: 60 },
        _ => RotationMode::On429Only,
    };
    let (cd, retry, max_r) = {
        let cfg = state.engine.config.read().unwrap();
        (cfg.cooldown_seconds, cfg.auto_retry_on_429, cfg.max_retries)
    };
    state
        .engine
        .update_rotation_policy(parsed, cd, retry, max_r);
    Ok(())
}

#[tauri::command]
pub async fn add_profile(
    state: State<'_, AppState>,
    name: String,
    url: String,
) -> Result<usize, String> {
    state
        .engine
        .add_and_sync_profile(name, url)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_proxy_pool(
    state: State<'_, AppState>,
    name: String,
    lines: String,
    protocol: String,
    use_current_relay: Option<bool>,
) -> Result<usize, String> {
    state
        .engine
        .add_proxy_pool_profile(name, lines, protocol, use_current_relay.unwrap_or(false))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn activate_profile(state: State<'_, AppState>, id: String) -> Result<usize, String> {
    state
        .engine
        .activate_profile(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn refresh_profile(state: State<'_, AppState>, id: String) -> Result<usize, String> {
    state
        .engine
        .refresh_profile(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_profile(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.engine.delete_profile(&id);
    Ok(())
}

#[tauri::command]
pub fn reset_profile_traffic(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.engine.reset_profile_traffic(&id);
    Ok(())
}

#[tauri::command]
pub fn set_profile_quota(
    state: State<'_, AppState>,
    id: String,
    quota_gb: f64,
) -> Result<(), String> {
    state.engine.set_profile_quota_gb(&id, quota_gb);
    Ok(())
}

#[tauri::command]
pub async fn save_strategy(
    state: State<'_, AppState>,
    cooldown_seconds: u64,
    max_retries: usize,
    auto_retry_on_429: bool,
    watch_domains: Vec<String>,
    chain_backoff_ms: u64,
) -> Result<(), String> {
    let mode = state.engine.config.read().unwrap().rotation_mode.clone();
    state
        .engine
        .update_rotation_policy(mode, cooldown_seconds, auto_retry_on_429, max_retries);
    state.engine.update_chain_backoff_ms(chain_backoff_ms);
    if !watch_domains.is_empty() {
        state.engine.update_watch_domains(watch_domains);
        if state.engine.is_running() {
            let _ = state.engine.reload_mihomo_runtime().await;
        }
    }
    state.engine.bus.ok("429 轮询与监听域名配置已保存");
    Ok(())
}

#[tauri::command]
pub async fn apply_custom_chain_exit(
    state: State<'_, AppState>,
    raw: String,
    backoff_ms: u64,
) -> Result<(), String> {
    state
        .engine
        .set_chain_exit_custom(raw, backoff_ms)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn install_ca(state: State<'_, AppState>) -> Result<(), String> {
    state.engine.install_ca();
    Ok(())
}

#[tauri::command]
pub async fn download_core(state: State<'_, AppState>) -> Result<(), String> {
    state
        .engine
        .download_core()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_system_settings(
    state: State<'_, AppState>,
    local_proxy_port: u16,
    allow_lan: bool,
    mihomo_mixed_port: u16,
    tun_mitm_port: u16,
    mihomo_bypass_port: u16,
    mihomo_api_port: u16,
    max_body_mb: usize,
    mihomo_path: Option<String>,
) -> Result<(), String> {
    state
        .engine
        .update_system_settings(
            local_proxy_port,
            allow_lan,
            mihomo_mixed_port,
            tun_mitm_port,
            mihomo_bypass_port,
            mihomo_api_port,
            max_body_mb,
            mihomo_path,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_sidebar_collapsed(state: State<'_, AppState>, collapsed: bool) -> Result<(), String> {
    {
        let mut cfg = state.engine.config.write().unwrap();
        cfg.sidebar_collapsed = collapsed;
    }
    state.engine.save_config();
    Ok(())
}

#[tauri::command]
pub async fn trigger_elevate(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    relaunch_as_admin().map_err(|e| e.to_string())?;
    state.engine.shutdown().await;
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub fn hide_to_tray() {
    crate::tray::hide_to_tray();
}

#[allow(dead_code)]
pub fn _chain_exit_source_keep(_: Option<ChainExitSource>) {}
