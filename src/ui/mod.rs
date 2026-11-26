pub mod anim;
pub mod flags;
pub mod icons;
pub mod input;
mod pages;
mod shell;
pub mod theme;

use crate::config::RotationMode;
use crate::engine::ProxyEngine;
use crate::ui::anim::UiAnims;
use crate::ui::input::TextInput;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavPage {
    Dashboard,
    Nodes,
    Profiles,
    StrategyAndCa,
    Logs,
    Settings,
}

impl NavPage {
    fn index(self) -> usize {
        match self {
            NavPage::Dashboard => 0,
            NavPage::Nodes => 1,
            NavPage::Profiles => 2,
            NavPage::StrategyAndCa => 3,
            NavPage::Logs => 4,
            NavPage::Settings => 5,
        }
    }
}

pub struct MainView {
    engine: Arc<ProxyEngine>,
    rt: tokio::runtime::Handle,
    page: NavPage,
    sidebar_collapsed: bool,
    // Modal state for adding a new subscription profile
    show_add_modal: bool,
    open_profile_menu: Option<String>,
    // Dashboard circular shutter node picker & dial page state
    show_node_picker: bool,
    picker_closed_at: Option<Instant>,
    picker_scroll: ScrollHandle,
    dial_show_speed: bool,
    dial_launch_started: Option<Instant>,
    // Unified 60fps animation registry & async loading states
    anims: UiAnims,
    peak_down_bps: u64,
    power_busy: Arc<AtomicBool>,
    power_starting: bool,
    sim429_flash_until: Option<Instant>,
    loading_epoch: Instant,
    loading_phase: f32,
    modal_name_input: Entity<TextInput>,
    modal_url_input: Entity<TextInput>,
    // Strategy page inputs
    domains_input: Entity<TextInput>,
    cooldown_input: Entity<TextInput>,
    retries_input: Entity<TextInput>,
    // System settings inputs
    local_port_input: Entity<TextInput>,
    mixed_port_input: Entity<TextInput>,
    tun_mitm_port_input: Entity<TextInput>,
    bypass_port_input: Entity<TextInput>,
    api_port_input: Entity<TextInput>,
    max_body_mb_input: Entity<TextInput>,
    mihomo_path_input: Entity<TextInput>,
    // ScrollHandles for visible custom scrollbars
    nodes_scroll: ScrollHandle,
    profiles_scroll: ScrollHandle,
    logs_scroll: ScrollHandle,
    strategy_scroll: ScrollHandle,
    settings_scroll: ScrollHandle,
    dragging_scroll: Option<(&'static str, ScrollHandle)>,
}

impl MainView {
    pub fn new(
        engine: Arc<ProxyEngine>,
        rt: tokio::runtime::Handle,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let cfg = engine.config.read().unwrap().clone();
        let modal_name_input =
            cx.new(|cx| TextInput::new(String::new(), "配置名称 (留空则自动从网址提取)", cx));
        let modal_url_input = cx.new(|cx| {
            TextInput::new(
                String::new(),
                "粘贴 Clash / Mihomo YAML 订阅 URL (支持 Ctrl+V)",
                cx,
            )
        });
        let domains_input = cx.new(|cx| {
            TextInput::new(
                cfg.watch_domains.join(", "),
                "aiplatform.googleapis.com, *.googleapis.com",
                cx,
            )
        });
        let cooldown_input = cx.new(|cx| TextInput::new(cfg.cooldown_seconds.to_string(), "60", cx));
        let retries_input = cx.new(|cx| TextInput::new(cfg.max_retries.to_string(), "2", cx));
        let local_port_input =
            cx.new(|cx| TextInput::new(cfg.local_proxy_port.to_string(), "7890", cx));
        let mixed_port_input =
            cx.new(|cx| TextInput::new(cfg.mihomo_mixed_port.to_string(), "17890", cx));
        let tun_mitm_port_input =
            cx.new(|cx| TextInput::new(cfg.tun_mitm_port.to_string(), "17891", cx));
        let bypass_port_input =
            cx.new(|cx| TextInput::new(cfg.mihomo_bypass_port.to_string(), "17892", cx));
        let api_port_input =
            cx.new(|cx| TextInput::new(cfg.mihomo_api_port.to_string(), "19090", cx));
        let max_body_mb_input = cx.new(|cx| {
            TextInput::new(
                (cfg.max_retry_body_bytes / (1024 * 1024)).max(1).to_string(),
                "16",
                cx,
            )
        });
        let mihomo_path_input = cx.new(|cx| {
            TextInput::new(
                cfg.mihomo_path.clone().unwrap_or_default(),
                "留空则使用默认内置目录 bin\\mihomo.exe",
                cx,
            )
        });

        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| loop {
            cx.background_executor()
                .timer(Duration::from_millis(450))
                .await;
            if !crate::tray::is_window_visible() {
                continue;
            }
            let ok = this.update(cx, |_, cx| {
                cx.notify();
            });
            if ok.is_err() {
                break;
            }
        })
        .detach();

        Self {
            engine,
            rt,
            page: NavPage::Dashboard,
            sidebar_collapsed: cfg.sidebar_collapsed,
            show_add_modal: false,
            open_profile_menu: None,
            show_node_picker: false,
            picker_closed_at: None,
            picker_scroll: ScrollHandle::new(),
            dial_show_speed: false,
            dial_launch_started: None,
            anims: UiAnims::new(),
            peak_down_bps: 0,
            power_busy: Arc::new(AtomicBool::new(false)),
            power_starting: false,
            sim429_flash_until: None,
            loading_epoch: Instant::now(),
            loading_phase: 0.0,
            modal_name_input,
            modal_url_input,
            domains_input,
            cooldown_input,
            retries_input,
            local_port_input,
            mixed_port_input,
            tun_mitm_port_input,
            bypass_port_input,
            api_port_input,
            max_body_mb_input,
            mihomo_path_input,
            nodes_scroll: ScrollHandle::new(),
            profiles_scroll: ScrollHandle::new(),
            logs_scroll: ScrollHandle::new(),
            strategy_scroll: ScrollHandle::new(),
            settings_scroll: ScrollHandle::new(),
            dragging_scroll: None,
        }
    }

    pub(in crate::ui) fn switch_page(&mut self, target: NavPage, cx: &mut Context<Self>) {
        if self.page == target {
            return;
        }
        self.page = target;
        self.anims.trigger("page_enter", 0.0, 1.0, 160.0);
        cx.notify();
    }

    fn toggle_node_picker(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.show_node_picker == open {
            return;
        }
        self.show_node_picker = open;
        if open {
            if !self.engine.is_testing_latency() {
                let eng = self.engine.clone();
                self.rt.spawn(async move {
                    eng.ping_all_nodes().await;
                });
            }
        } else {
            self.picker_closed_at = Some(Instant::now());
        }
        cx.notify();
    }

    pub(in crate::ui) fn is_power_busy(&self) -> bool {
        self.power_busy.load(Ordering::Relaxed)
    }

    pub(in crate::ui) fn is_sim429_flashing(&self) -> bool {
        self.sim429_flash_until
            .map_or(false, |until| Instant::now() < until)
    }

    fn toggle_power(&mut self, cx: &mut Context<Self>) {
        if self.power_busy.swap(true, Ordering::SeqCst) {
            return;
        }
        let engine = self.engine.clone();
        let busy = self.power_busy.clone();
        let running = engine.is_running();
        self.power_starting = !running;
        if !running {
            self.dial_launch_started = Some(Instant::now());
        } else {
            self.dial_launch_started = None;
            self.dial_show_speed = false;
        }
        self.rt.spawn(async move {
            let started_at = Instant::now();
            if running {
                engine.stop().await;
            } else if let Err(err) = engine.start().await {
                engine.bus.error(format!("启动失败: {}", err));
            }
            let min_dur = Duration::from_millis(320);
            if let Some(rem) = min_dur.checked_sub(started_at.elapsed()) {
                tokio::time::sleep(rem).await;
            }
            busy.store(false, Ordering::SeqCst);
        });
        cx.notify();
    }

    fn open_add_profile_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.modal_name_input.update(cx, |inp, cx| inp.set_value(String::new(), cx));
        self.modal_url_input.update(cx, |inp, cx| {
            inp.set_value(String::new(), cx);
            inp.focus(window);
        });
        self.show_add_modal = true;
        cx.notify();
    }

    fn submit_add_profile(&mut self, cx: &mut Context<Self>) {
        let name = self.modal_name_input.read(cx).value().trim().to_string();
        let url = self.modal_url_input.read(cx).value().trim().to_string();
        if url.is_empty() {
            self.engine.bus.error("请输入有效的订阅 URL 网址");
            cx.notify();
            return;
        }
        self.show_add_modal = false;
        let engine = self.engine.clone();
        self.rt.spawn(async move {
            if let Err(err) = engine.add_and_sync_profile(name, url).await {
                engine.bus.error(format!("添加订阅配置失败: {}", err));
            }
        });
        cx.notify();
    }

    fn set_rotation_mode(&mut self, mode: RotationMode, cx: &mut Context<Self>) {
        let (cd, retry, max_r) = {
            let cfg = self.engine.config.read().unwrap();
            (cfg.cooldown_seconds, cfg.auto_retry_on_429, cfg.max_retries)
        };
        self.engine.update_rotation_policy(mode, cd, retry, max_r);
        cx.notify();
    }

    fn save_strategy_inputs(&mut self, cx: &mut Context<Self>) {
        let cd: u64 = self
            .cooldown_input
            .read(cx)
            .value()
            .trim()
            .parse()
            .unwrap_or(60);
        let max_r: usize = self
            .retries_input
            .read(cx)
            .value()
            .trim()
            .parse()
            .unwrap_or(2);
        let domains: Vec<String> = self
            .domains_input
            .read(cx)
            .value()
            .split([',', '\n', ';'])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let (mode, auto_retry) = {
            let cfg = self.engine.config.read().unwrap();
            (cfg.rotation_mode.clone(), cfg.auto_retry_on_429)
        };
        self.engine
            .update_rotation_policy(mode, cd, auto_retry, max_r);
        if !domains.is_empty() {
            self.engine.update_watch_domains(domains);
        }
        self.engine.bus.ok("429 轮询与监听域名配置已保存");
        cx.notify();
    }

    fn save_system_settings(&mut self, cx: &mut Context<Self>) {
        let parse_port = |ent: &Entity<TextInput>, fallback: u16| -> u16 {
            ent.read(cx).value().trim().parse::<u16>().unwrap_or(fallback)
        };
        let local_port = parse_port(&self.local_port_input, 7890);
        let mixed_port = parse_port(&self.mixed_port_input, 17890);
        let tun_mitm_port = parse_port(&self.tun_mitm_port_input, 17891);
        let bypass_port = parse_port(&self.bypass_port_input, 17892);
        let api_port = parse_port(&self.api_port_input, 19090);
        let max_mb = self
            .max_body_mb_input
            .read(cx)
            .value()
            .trim()
            .parse::<usize>()
            .unwrap_or(16);
        let custom_path = self.mihomo_path_input.read(cx).value().trim().to_string();
        let allow_lan = self.engine.config.read().unwrap().allow_lan;

        if let Err(err) = self.engine.update_system_settings(
            local_port,
            allow_lan,
            mixed_port,
            tun_mitm_port,
            bypass_port,
            api_port,
            max_mb,
            if custom_path.is_empty() {
                None
            } else {
                Some(custom_path)
            },
        ) {
            self.engine.bus.error(format!("保存系统设置失败: {}", err));
        }
        cx.notify();
    }

    fn reset_default_ports(&mut self, cx: &mut Context<Self>) {
        self.local_port_input.update(cx, |i, cx| i.set_value("7890".into(), cx));
        self.mixed_port_input.update(cx, |i, cx| i.set_value("17890".into(), cx));
        self.tun_mitm_port_input.update(cx, |i, cx| i.set_value("17891".into(), cx));
        self.bypass_port_input.update(cx, |i, cx| i.set_value("17892".into(), cx));
        self.api_port_input.update(cx, |i, cx| i.set_value("19090".into(), cx));
        self.max_body_mb_input.update(cx, |i, cx| i.set_value("16".into(), cx));
        self.save_system_settings(cx);
    }

}

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let running = self.engine.is_running();
        let ca_ok = self.engine.is_ca_installed();
        let is_maximized = window.is_maximized();
        let node_count = self.engine.scheduler.snapshot().nodes.len();
        let tel_snap = self.engine.telemetry_snapshot();
        let outbound_mode = tel_snap.outbound_mode.clone();
        if running && tel_snap.down_bps > self.peak_down_bps {
            self.peak_down_bps = tel_snap.down_bps;
        }
        let bps_to_norm = |bps: u64| -> f32 {
            if bps <= 1024 {
                return 0.0;
            }
            let kb = (bps as f32 / 1024.0).max(10.0);
            ((kb.log10() - 1.0) / 4.0).clamp(0.0, 1.0)
        };
        let target_down_norm = if running {
            bps_to_norm(tel_snap.down_bps)
        } else {
            0.0
        };
        let target_up_norm = if running {
            bps_to_norm(tel_snap.up_bps)
        } else {
            0.0
        };
        let (
            sys_proxy,
            tun_on,
            auto_retry,
            allow_lan,
            profile_count,
            local_port,
            rotation_mode,
        ) = {
            let cfg = self.engine.config.read().unwrap();
            (
                cfg.enable_system_proxy,
                cfg.enable_tun,
                cfg.auto_retry_on_429,
                cfg.allow_lan,
                cfg.profiles.len(),
                cfg.local_proxy_port,
                cfg.rotation_mode.clone(),
            )
        };

        // Dial 3D launch sprint & page-flip state machine
        let launch_dur_secs = 1.55_f32;
        if let Some(started) = self.dial_launch_started {
            let p = started.elapsed().as_secs_f32() / launch_dur_secs;
            if p >= 0.62 && running {
                self.dial_show_speed = true;
            }
            if p >= 1.0 {
                self.dial_launch_started = None;
                if running {
                    self.dial_show_speed = true;
                }
            }
        } else if !running {
            self.dial_show_speed = false;
        }

        // Sync state-driven UI animations
        self.anims.sync(
            "sidebar_w",
            if self.sidebar_collapsed { 58.0 } else { 192.0 },
            190.0,
        );
        self.anims
            .sync("sidebar_nav_y", self.page.index() as f32 * 40.0, 190.0);
        self.anims.sync(
            "picker_slide",
            if self.show_node_picker { 1.0 } else { 0.0 },
            260.0,
        );
        self.anims.sync(
            "dial_page_p",
            if self.dial_show_speed { 1.0 } else { 0.0 },
            280.0,
        );
        self.anims.sync(
            "modal_p",
            if self.show_add_modal { 1.0 } else { 0.0 },
            170.0,
        );
        self.anims
            .sync("tog_sysproxy", if sys_proxy { 1.0 } else { 0.0 }, 180.0);
        self.anims
            .sync("tog_tun", if tun_on { 1.0 } else { 0.0 }, 180.0);
        self.anims
            .sync("tog_retry", if auto_retry { 1.0 } else { 0.0 }, 180.0);
        self.anims
            .sync("tog_allow_lan", if allow_lan { 1.0 } else { 0.0 }, 180.0);
        self.anims
            .sync("tog_power", if running { 1.0 } else { 0.0 }, 200.0);
        self.anims.sync(
            "tog_picker_btn",
            if self.show_node_picker { 1.0 } else { 0.0 },
            160.0,
        );
        self.anims.sync(
            "out_rule",
            if outbound_mode == "rule" { 1.0 } else { 0.0 },
            150.0,
        );
        self.anims.sync(
            "out_global",
            if outbound_mode == "global" { 1.0 } else { 0.0 },
            150.0,
        );
        self.anims.sync(
            "out_direct",
            if outbound_mode == "direct" { 1.0 } else { 0.0 },
            150.0,
        );
        self.anims.sync(
            "rot_429",
            if matches!(rotation_mode, RotationMode::On429Only) {
                1.0
            } else {
                0.0
            },
            160.0,
        );
        self.anims.sync(
            "rot_req",
            if matches!(rotation_mode, RotationMode::RoundRobinPerRequest) {
                1.0
            } else {
                0.0
            },
            160.0,
        );
        self.anims.sync(
            "rot_int",
            if matches!(rotation_mode, RotationMode::RoundRobinInterval { .. }) {
                1.0
            } else {
                0.0
            },
            160.0,
        );

        self.anims
            .sync("dial_down_norm", target_down_norm, 380.0);
        self.anims.sync("dial_up_norm", target_up_norm, 380.0);

        let any_loading = self.is_power_busy()
            || self.engine.is_testing_latency()
            || self.is_sim429_flashing()
            || self.dial_launch_started.is_some();
        if any_loading {
            self.loading_phase = self.loading_epoch.elapsed().as_secs_f32() * 7.5;
        }

        if self.anims.tick() || any_loading {
            window.request_animation_frame();
        }

        let page_enter = self.anims.get("page_enter", 1.0);
        let modal_p = self.anims.get("modal_p", 0.0);

        let content: AnyElement = match self.page {
            NavPage::Dashboard => self.render_dashboard_page(cx).into_any_element(),
            NavPage::Nodes => self.render_nodes_page(cx).into_any_element(),
            NavPage::Profiles => self.render_profiles_page(cx).into_any_element(),
            NavPage::StrategyAndCa => self.render_strategy_page(cx).into_any_element(),
            NavPage::Logs => self.render_logs_page(cx).into_any_element(),
            NavPage::Settings => self.render_settings_page(cx).into_any_element(),
        };

        div()
            .id("root-window")
            .relative()
            .size_full()
            .bg(theme::chassis_bg())
            .flex()
            .flex_col()
            .overflow_hidden()
            .when(self.dragging_scroll.is_some(), |el| {
                el.cursor(CursorStyle::ResizeUpDown)
            })
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                if let Some((_, ref handle)) = this.dragging_scroll {
                    if ev.dragging() {
                        Self::apply_scroll_drag(handle, ev.position.y);
                        cx.notify();
                    } else {
                        this.dragging_scroll = None;
                        cx.notify();
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.dragging_scroll.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.dragging_scroll.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .child(self.render_header(running, local_port, tun_on, ca_ok, is_maximized))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.render_sidebar(
                        running,
                        sys_proxy,
                        tun_on,
                        auto_retry,
                        node_count,
                        profile_count,
                        cx,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .bg(theme::workspace_bg())
                            .p_4()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .flex()
                                    .flex_col()
                                    .opacity(0.45 + 0.55 * page_enter)
                                    .pt(px((1.0 - page_enter) * 8.0))
                                    .child(content),
                            ),
                    ),
            )
            .when(self.show_add_modal || modal_p > 0.02, |el| {
                el.child(self.render_add_profile_modal(cx))
            })
    }
}