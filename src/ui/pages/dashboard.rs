use crate::config::RotationMode;
use crate::ui::anim::{lerp_rgb, render_loading_bars, render_pill_toggle, render_shimmer_band};
use crate::ui::flags::{detect_country, render_flag};
use crate::ui::icons::icon;
use crate::ui::{theme, MainView};
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::time::{Duration, Instant};

impl MainView {
    pub(in crate::ui) fn render_dashboard_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let snap = self.engine.scheduler.snapshot();
        let tel = self.engine.telemetry_snapshot();
        let cfg = self.engine.config.read().unwrap().clone();
        let running = self.engine.is_running();

        let format_rate = |bps: u64| -> String {
            if bps >= 1024 * 1024 {
                format!("{:.2} MB/s", bps as f64 / (1024.0 * 1024.0))
            } else if bps >= 1024 {
                format!("{:.1} KB/s", bps as f64 / 1024.0)
            } else {
                format!("{} B/s", bps)
            }
        };
        let format_bytes = |bytes: u64| -> (String, &'static str) {
            if bytes >= 1024 * 1024 * 1024 {
                (
                    format!("{:.2}", bytes as f64 / (1024.0 * 1024.0 * 1024.0)),
                    "GB",
                )
            } else if bytes >= 1024 * 1024 {
                (format!("{:.1}", bytes as f64 / (1024.0 * 1024.0)), "MB")
            } else {
                (format!("{:.1}", bytes as f64 / 1024.0), "KB")
            }
        };
        let format_uptime = |secs: u64| -> String {
            let h = secs / 3600;
            let m = (secs % 3600) / 60;
            let s = secs % 60;
            format!("{:02}:{:02}:{:02}", h, m, s)
        };

        let active_name = snap
            .active_node
            .clone()
            .unwrap_or_else(|| "未加载节点 (请先在配置文件中添加订阅)".to_string());
        let active_country = detect_country(&active_name);
        let active_ms = snap
            .nodes
            .iter()
            .find(|n| n.is_active)
            .and_then(|n| n.latency_ms)
            .map(|m| {
                if m == 0 {
                    "超时".to_string()
                } else {
                    format!("{} ms", m)
                }
            })
            .unwrap_or_else(|| "-- ms".to_string());

        // Real-time 36-bar speed sparkline inside recessed matte-black screen
        let max_rate = tel
            .speed_history
            .iter()
            .map(|(u, d)| (*u).max(*d))
            .max()
            .unwrap_or(1024)
            .max(1024) as f32;

        let spark_bars = tel.speed_history.iter().map(|(u, d)| {
            let total = (*u + *d) as f32;
            let h_px = ((total / (max_rate * 1.15)) * 78.0).clamp(3.0, 78.0);
            let is_up_dominant = *u > *d;
            div()
                .flex_1()
                .h_full()
                .flex()
                .items_end()
                .child(
                    div()
                        .w_full()
                        .h(px(h_px))
                        .rounded_t(px(1.5))
                        .bg(if total <= 1.0 {
                            rgb(0x2c2e2a)
                        } else if is_up_dominant {
                            theme::lcd_orange()
                        } else {
                            theme::lcd_green()
                        }),
                )
        });

        let p_sys = self
            .anims
            .get("tog_sysproxy", if cfg.enable_system_proxy { 1.0 } else { 0.0 });
        let p_tun = self
            .anims
            .get("tog_tun", if cfg.enable_tun { 1.0 } else { 0.0 });
        let p_power = self.anims.get("tog_power", if running { 1.0 } else { 0.0 });
        let p_picker_btn = self
            .anims
            .get("tog_picker_btn", if self.show_node_picker { 1.0 } else { 0.0 });
        let p_out_rule = self
            .anims
            .get("out_rule", if tel.outbound_mode == "rule" { 1.0 } else { 0.0 });
        let p_out_global = self.anims.get(
            "out_global",
            if tel.outbound_mode == "global" { 1.0 } else { 0.0 },
        );
        let p_out_direct = self.anims.get(
            "out_direct",
            if tel.outbound_mode == "direct" { 1.0 } else { 0.0 },
        );
        let p_rot_429 = self.anims.get(
            "rot_429",
            if cfg.rotation_mode == RotationMode::On429Only {
                1.0
            } else {
                0.0
            },
        );
        let p_rot_req = self.anims.get(
            "rot_req",
            if cfg.rotation_mode == RotationMode::RoundRobinPerRequest {
                1.0
            } else {
                0.0
            },
        );
        let p_rot_int = self.anims.get(
            "rot_int",
            if matches!(cfg.rotation_mode, RotationMode::RoundRobinInterval { .. }) {
                1.0
            } else {
                0.0
            },
        );

        // Row 1: Speed Sparkline (left) + System Proxy & TUN Toggle Cards (right)
        let row1 = div()
            .flex()
            .gap_3()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .bg(theme::card_bg())
                    .border_1()
                    .border_color(theme::border_color())
                    .rounded(px(6.0))
                    .p_3p5()
                    .flex()
                    .flex_col()
                    .gap_2p5()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::ink_primary())
                                    .child("网络速度"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .text_xs()
                                    .child(
                                        div()
                                            .text_color(theme::braun_orange())
                                            .child(format!("↑ {}", format_rate(tel.up_bps))),
                                    )
                                    .child(
                                        div()
                                            .text_color(theme::status_green())
                                            .child(format!("↓ {}", format_rate(tel.down_bps))),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .h(px(94.0))
                            .bg(theme::lcd_bg())
                            .border_1()
                            .border_color(theme::border_strong())
                            .rounded(px(4.0))
                            .px_2()
                            .pt_2()
                            .pb_1p5()
                            .flex()
                            .items_end()
                            .gap(px(2.0))
                            .children(spark_bars),
                    ),
            )
            .child(
                div()
                    .w(px(230.0))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap_2p5()
                    .child(
                        div()
                            .id("dash-card-sysproxy")
                            .flex_1()
                            .bg(theme::card_bg())
                            .hover(|s| s.bg(theme::row_hover_bg()))
                            .active(|s| s.bg(theme::sidebar_active()))
                            .border_1()
                            .border_color(theme::border_color())
                            .rounded(px(6.0))
                            .px_4()
                            .py_3()
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    let eng = this.engine.clone();
                                    let next = !cfg.enable_system_proxy;
                                    this.rt.spawn(async move {
                                        eng.toggle_system_proxy(next).await;
                                    });
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme::ink_primary())
                                            .child("系统代理"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::ink_muted())
                                            .child(format!("HTTP :{}", cfg.local_proxy_port)),
                                    ),
                            )
                            .child(render_pill_toggle(p_sys, 0xe84c1e)),
                    )
                    .child(
                        div()
                            .id("dash-card-tun")
                            .flex_1()
                            .bg(theme::card_bg())
                            .hover(|s| s.bg(theme::row_hover_bg()))
                            .active(|s| s.bg(theme::sidebar_active()))
                            .border_1()
                            .border_color(theme::border_color())
                            .rounded(px(6.0))
                            .px_4()
                            .py_3()
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    let next = !cfg.enable_tun;
                                    {
                                        let mut c = this.engine.config.write().unwrap();
                                        c.enable_tun = next;
                                    }
                                    this.engine.save_config();
                                    this.engine.bus.info(format!(
                                        "TUN 虚拟网卡已设为 {}（重启代理生效）",
                                        if next { "开启" } else { "关闭" }
                                    ));
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme::ink_primary())
                                            .child("虚拟网卡 (TUN)"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1p5()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme::ink_muted())
                                                    .child(if crate::mihomo::is_elevated() {
                                                        "全局接管 · 已提权"
                                                    } else {
                                                        "全局接管 · 需管理员"
                                                    }),
                                            )
                                            .when(!crate::mihomo::is_elevated(), |el| {
                                                el.child(
                                                    div()
                                                        .id("btn-elevate-tun")
                                                        .px_1p5()
                                                        .py(px(1.0))
                                                        .rounded(px(3.0))
                                                        .bg(theme::braun_orange())
                                                        .hover(|s| s.bg(theme::braun_orange_border()))
                                                        .text_size(px(10.0))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(theme::white())
                                                        .cursor_pointer()
                                                        .on_mouse_down(
                                                            MouseButton::Left,
                                                            cx.listener(|this, _, _, cx| {
                                                                cx.stop_propagation();
                                                                let eng = this.engine.clone();
                                                                let rt = this.rt.clone();
                                                                cx.spawn(async move |_, cx| {
                                                                    let res = rt
                                                                        .spawn_blocking(crate::mihomo::relaunch_as_admin)
                                                                        .await;
                                                                    match res {
                                                                        Ok(Ok(())) => {
                                                                            let _ = rt
                                                                                .spawn(async move {
                                                                                    eng.stop().await;
                                                                                })
                                                                                .await;
                                                                            let _ = cx.update(|cx| cx.quit());
                                                                        }
                                                                        Ok(Err(e)) => {
                                                                            eng.bus.error(format!("{}", e));
                                                                        }
                                                                        Err(e) => {
                                                                            eng.bus.error(format!("提权任务异常: {}", e));
                                                                        }
                                                                    }
                                                                })
                                                                .detach();
                                                            }),
                                                        )
                                                        .child("提权"),
                                                )
                                            }),
                                    ),
                            )
                            .child(render_pill_toggle(p_tun, 0xe84c1e)),
                    ),
            );

        // Row 2: Traffic Stats + Local/Exit IP + Outbound & 429 Mode
        let (up_val, up_unit) = format_bytes(tel.total_up);
        let (dn_val, dn_unit) = format_bytes(tel.total_down);
        let total_sum = (tel.total_up + tel.total_down).max(1) as f32;
        let up_ratio = (tel.total_up as f32 / total_sum).clamp(0.05, 0.95);

        let exit_country = tel
            .exit_country_code
            .as_deref()
            .map(detect_country)
            .unwrap_or(active_country);
        let exit_ip_text = if tel.checking_ip {
            "正在检测出口 IP...".to_string()
        } else {
            tel.exit_ip
                .clone()
                .unwrap_or_else(|| "点击检测出口 IP".to_string())
        };

        let radio_row = |id: &'static str, label: &'static str, mode_val: &'static str, progress: f32| {
            let active = progress > 0.5;
            let dot_sz = (progress * 8.0).clamp(0.0, 8.0);
            div()
                .id(id)
                .py_1()
                .px_2()
                .rounded(px(4.0))
                .cursor_pointer()
                .hover(|s| s.bg(theme::row_hover_bg()))
                .active(|s| s.opacity(0.75))
                .flex()
                .items_center()
                .gap_2p5()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        let eng = this.engine.clone();
                        this.rt.spawn(async move {
                            eng.set_outbound_mode(mode_val).await;
                        });
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .w(px(14.0))
                        .h(px(14.0))
                        .rounded_full()
                        .border_1()
                        .border_color(lerp_rgb(0xb8b5ac, 0xe84c1e, progress))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(dot_sz > 0.5, |el| {
                            el.child(
                                div()
                                    .w(px(dot_sz))
                                    .h(px(dot_sz))
                                    .rounded_full()
                                    .bg(theme::braun_orange()),
                            )
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(if active {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .text_color(theme::ink_primary())
                        .child(label),
                )
        };

        let rot_pill = |id: &'static str, label: &'static str, target: RotationMode, progress: f32| {
            div()
                .id(id)
                .flex_1()
                .py_1()
                .px_1()
                .rounded(px(3.0))
                .cursor_pointer()
                .hover(|s| s.opacity(0.88))
                .active(|s| s.opacity(0.75))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(11.0))
                .whitespace_nowrap()
                .bg(lerp_rgb(0xe3e1d9, 0x1a1a18, progress))
                .text_color(lerp_rgb(0x5b5a54, 0xffffff, progress))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.set_rotation_mode(target.clone(), cx);
                    }),
                )
                .child(label)
        };

        let row2 = div()
            .flex_1()
            .min_h_0()
            .flex()
            .gap_3()
            .child(
                // Card 1: 流量统计 + 429 统计
                div()
                    .flex_1()
                    .min_w_0()
                    .bg(theme::card_bg())
                    .border_1()
                    .border_color(theme::border_color())
                    .rounded(px(6.0))
                    .p_3p5()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::ink_primary())
                                    .child("流量统计"),
                            )
                            .child(
                                // Two-tone ratio bar
                                div()
                                    .w_full()
                                    .h(px(7.0))
                                    .rounded_full()
                                    .overflow_hidden()
                                    .bg(theme::sidebar_active())
                                    .flex()
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative(up_ratio))
                                            .bg(theme::braun_orange()),
                                    )
                                    .child(div().h_full().flex_1().bg(theme::ink_primary())),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1p5()
                                    .text_xs()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .text_color(theme::braun_orange())
                                                    .child("↑ 上传"),
                                            )
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme::ink_primary())
                                                    .child(format!("{} {}", up_val, up_unit)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .text_color(theme::ink_secondary())
                                                    .child("↓ 下载"),
                                            )
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme::ink_primary())
                                                    .child(format!("{} {}", dn_val, dn_unit)),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .pt_2()
                            .border_t_1()
                            .border_color(theme::divider_color())
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .text_size(px(11.0))
                            .child(
                                div()
                                    .whitespace_nowrap()
                                    .text_color(theme::braun_orange())
                                    .child(format!("429 拦截: {}", snap.total_429)),
                            )
                            .child(
                                div()
                                    .whitespace_nowrap()
                                    .text_color(theme::status_green())
                                    .child(format!("重试: {}", snap.total_retries_ok)),
                            ),
                    ),
            )
            .child(
                // Card 2: 内网 IP + 网络检测 (Exit IP)
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .bg(theme::card_bg())
                            .border_1()
                            .border_color(theme::border_color())
                            .rounded(px(6.0))
                            .p_3p5()
                            .flex()
                            .flex_col()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::ink_secondary())
                                    .child("内网 IP"),
                            )
                            .child(
                                div()
                                    .text_size(px(14.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::ink_primary())
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(tel.local_ip),
                            ),
                    )
                    .child(
                        div()
                            .id("dash-check-ip")
                            .flex_1()
                            .bg(theme::card_bg())
                            .hover(|s| s.bg(theme::row_hover_bg()))
                            .active(|s| s.bg(theme::sidebar_active()))
                            .border_1()
                            .border_color(theme::border_color())
                            .rounded(px(6.0))
                            .p_3p5()
                            .cursor_pointer()
                            .flex()
                            .flex_col()
                            .justify_between()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    let eng = this.engine.clone();
                                    this.rt.spawn(async move {
                                        eng.check_exit_ip().await;
                                    });
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1p5()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .child(render_flag(exit_country))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme::ink_secondary())
                                                    .whitespace_nowrap()
                                                    .child("网络检测"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .whitespace_nowrap()
                                            .flex_shrink_0()
                                            .text_color(theme::braun_orange())
                                            .child("刷新"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::ink_primary())
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(exit_ip_text),
                            ),
                    ),
            )
            .child(
                // Card 3: 出站模式 + 429 轮询模式
                div()
                    .w(px(230.0))
                    .flex_shrink_0()
                    .bg(theme::card_bg())
                    .border_1()
                    .border_color(theme::border_color())
                    .rounded(px(6.0))
                    .p_3p5()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::ink_primary())
                                    .mb_1()
                                    .child("出站模式"),
                            )
                            .child(radio_row("om-rule", "规则 (Rule)", "rule", p_out_rule))
                            .child(radio_row("om-global", "全局 (Global)", "global", p_out_global))
                            .child(radio_row("om-direct", "直连 (Direct)", "direct", p_out_direct)),
                    )
                    .child(
                        div()
                            .pt_2p5()
                            .border_t_1()
                            .border_color(theme::divider_color())
                            .flex()
                            .flex_col()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::ink_muted())
                                    .child("429 节点轮询策略"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(rot_pill(
                                        "m-429",
                                        "429切换",
                                        RotationMode::On429Only,
                                        p_rot_429,
                                    ))
                                    .child(rot_pill(
                                        "m-rr",
                                        "逐请求",
                                        RotationMode::RoundRobinPerRequest,
                                        p_rot_req,
                                    ))
                                    .child(rot_pill(
                                        "m-int",
                                        "60s定时",
                                        RotationMode::RoundRobinInterval { seconds: 60 },
                                        p_rot_int,
                                    )),
                            ),
                    ),
            );

        // Bottom Strip: Active Node + Slide-Up Node Picker + Sim429 + Uptime Control Pill
        let picker_cur_h = self.anims.get("picker_h", 0.0);
        let picker_visible = self.show_node_picker || picker_cur_h > 1.0;
        let picker_h = picker_cur_h.max(1.0);
        let picker_opacity = (picker_cur_h / 320.0).clamp(0.0, 1.0);
        let picker_rows: Vec<_> = if picker_visible {
            snap.nodes
                .iter()
                .enumerate()
                .map(|(idx, n)| {
                    let name_click = n.name.clone();
                    let country = detect_country(&n.name);
                    let (lat_text, lat_color) = match n.latency_ms {
                        Some(0) => ("超时".to_string(), theme::braun_orange()),
                        Some(ms) if ms < 150 => (format!("{} ms", ms), theme::status_green()),
                        Some(ms) => (format!("{} ms", ms), theme::ink_secondary()),
                        None => ("--".to_string(), theme::ink_muted()),
                    };
                    div()
                        .id(("picker-node", idx))
                        .px_3p5()
                        .py_2()
                        .border_b_1()
                        .border_color(theme::divider_color())
                        .bg(if n.is_active {
                            theme::row_active_bg()
                        } else {
                            theme::white()
                        })
                        .hover(|s| s.bg(theme::row_hover_bg()))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .text_xs()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                let eng = this.engine.clone();
                                let name = name_click.clone();
                                this.rt.spawn(async move {
                                    let _ = eng.select_node_manual(&name).await;
                                });
                                this.toggle_node_picker(false, cx);
                            }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap_2p5()
                                .overflow_hidden()
                                .child(render_flag(country))
                                .child(
                                    div()
                                        .w(px(7.0))
                                        .h(px(7.0))
                                        .rounded_full()
                                        .flex_shrink_0()
                                        .bg(if n.is_active {
                                            theme::braun_orange()
                                        } else if n.cooldown_remaining_secs > 0 {
                                            theme::status_amber()
                                        } else {
                                            theme::status_green()
                                        }),
                                )
                                .child(
                                    div()
                                        .font_weight(if n.is_active {
                                            FontWeight::SEMIBOLD
                                        } else {
                                            FontWeight::NORMAL
                                        })
                                        .text_color(theme::ink_primary())
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .child(n.name.clone()),
                                )
                                .child(
                                    div()
                                        .px_1p5()
                                        .py(px(1.0))
                                        .rounded(px(3.0))
                                        .bg(theme::sidebar_bg())
                                        .text_color(theme::ink_muted())
                                        .flex_shrink_0()
                                        .child(n.proxy_type.clone()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                .flex_shrink_0()
                                .when(n.cooldown_remaining_secs > 0, |el| {
                                    el.child(
                                        div()
                                            .text_color(theme::braun_orange())
                                            .child(format!("冷却 {}s", n.cooldown_remaining_secs)),
                                    )
                                })
                                .child(
                                    div()
                                        .w(px(48.0))
                                        .text_right()
                                        .text_color(lat_color)
                                        .child(lat_text),
                                ),
                        )
                })
                .collect()
        } else {
            Vec::new()
        };

        let bottom_bar = div()
            .relative()
            .bg(theme::lcd_bg())
            .border_1()
            .border_color(theme::border_strong())
            .rounded(px(6.0))
            .px_4()
            .py_2p5()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .when(picker_visible, |el| {
                el.child(
                    deferred(
                        div()
                            .id("node-picker-popup")
                            .absolute()
                            .bottom(px(54.0))
                            .right(px(0.0))
                            .w(px(440.0))
                            .h(px(picker_h))
                            .opacity(picker_opacity)
                            .bg(theme::white())
                            .border_1()
                            .border_color(theme::border_strong())
                            .rounded(px(8.0))
                            .shadow_lg()
                            .occlude()
                            .overflow_hidden()
                            .flex()
                            .flex_col()
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
                            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                this.toggle_node_picker(false, cx);
                            }))
                            .child(
                                div()
                                    .px_3p5()
                                    .py_2()
                                    .bg(theme::card_header_bg())
                                    .border_b_1()
                                    .border_color(theme::border_color())
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme::ink_primary())
                                            .child(format!(
                                                "选择出口节点 (可用 {}/{})",
                                                snap.ready_count,
                                                snap.nodes.len()
                                            )),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .id("picker-next")
                                                    .px_2()
                                                    .py(px(2.0))
                                                    .rounded(px(3.0))
                                                    .bg(theme::btn_bg())
                                                    .hover(|s| s.bg(theme::btn_hover()))
                                                    .border_1()
                                                    .border_color(theme::border_strong())
                                                    .cursor_pointer()
                                                    .text_xs()
                                                    .text_color(theme::ink_primary())
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(|this, _, _, cx| {
                                                            let eng = this.engine.clone();
                                                            this.rt.spawn(async move {
                                                                let _ = eng
                                                                    .scheduler
                                                                    .rotate_next("快捷轮换节点")
                                                                    .await;
                                                            });
                                                            cx.notify();
                                                        }),
                                                    )
                                                    .child("轮换下一个"),
                                            )
                                            .child(
                                                div()
                                                    .id("picker-close")
                                                    .p_1()
                                                    .rounded(px(3.0))
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(theme::sidebar_active()))
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(|this, _, _, cx| {
                                                            this.toggle_node_picker(false, cx);
                                                        }),
                                                    )
                                                    .child(icon(
                                                        "icons/win-close.svg",
                                                        10.0,
                                                        theme::ink_secondary(),
                                                    )),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .flex()
                                    .child(
                                        div()
                                            .id("picker-scroll")
                                            .track_scroll(&self.picker_scroll)
                                            .flex_1()
                                            .overflow_y_scroll()
                                            .children(picker_rows),
                                    )
                                    .child(self.render_scrollbar(
                                        "sb-picker",
                                        &self.picker_scroll,
                                        false,
                                        cx,
                                    )),
                            ),
                    )
                    .with_priority(2),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_2p5()
                    .overflow_hidden()
                    .when(snap.active_node.is_some(), |el| {
                        el.child(render_flag(active_country))
                    })
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::white())
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(active_name),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::lcd_green())
                            .flex_shrink_0()
                            .child(active_ms),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::lcd_muted())
                            .flex_shrink_0()
                            .child(format!("· 可用 {}/{}", snap.ready_count, snap.nodes.len())),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .flex_shrink_0()
                    .child(
                        div()
                            .id("btn-rotate")
                            .px_3()
                            .py_1p5()
                            .bg(lerp_rgb(0x2b2d2a, 0xe84c1e, p_picker_btn))
                            .hover(|s| {
                                s.bg(if self.show_node_picker {
                                    theme::braun_orange_border()
                                } else {
                                    rgb(0x3a3d38)
                                })
                            })
                            .active(|s| s.opacity(0.78).mt(px(1.0)))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .text_xs()
                            .text_color(theme::white())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    let just_closed = !this.show_node_picker
                                        && this
                                            .picker_closed_at
                                            .map_or(false, |t| t.elapsed() < Duration::from_millis(80));
                                    if !just_closed {
                                        let next = !this.show_node_picker;
                                        this.toggle_node_picker(next, cx);
                                    }
                                }),
                            )
                            .child("切换节点"),
                    )
                    .child({
                        let sim_flash = self.is_sim429_flashing();
                        div()
                            .id("btn-sim429")
                            .relative()
                            .overflow_hidden()
                            .px_3()
                            .py_1p5()
                            .bg(if sim_flash {
                                theme::braun_orange()
                            } else {
                                rgb(0x2b2d2a)
                            })
                            .hover(|s| s.bg(theme::braun_orange()))
                            .active(|s| s.opacity(0.78).mt(px(1.0)))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .text_xs()
                            .font_weight(if sim_flash {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .text_color(if sim_flash {
                                theme::white()
                            } else {
                                theme::lcd_orange()
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.sim429_flash_until =
                                        Some(Instant::now() + Duration::from_millis(700));
                                    let eng = this.engine.clone();
                                    this.rt.spawn(async move {
                                        eng.simulate_429().await;
                                    });
                                    cx.notify();
                                }),
                            )
                            .when(sim_flash, |el| {
                                el.child(render_shimmer_band(self.loading_phase, 100.0))
                                    .child(render_loading_bars(self.loading_phase, theme::white()))
                                    .child("已触发 429")
                            })
                            .when(!sim_flash, |el| el.child("模拟 429"))
                    })
                    .child({
                        let power_busy = self.is_power_busy();
                        div()
                            .id("dash-uptime-btn")
                            .relative()
                            .overflow_hidden()
                            .px_3p5()
                            .py_1p5()
                            .rounded(px(4.0))
                            .bg(if power_busy {
                                rgb(0xd96b1f)
                            } else {
                                lerp_rgb(0x1f8a4c, 0xe84c1e, p_power)
                            })
                            .hover(|s| s.opacity(0.92))
                            .active(|s| s.opacity(0.78).mt(px(1.0)))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::white())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| this.toggle_power(cx)),
                            )
                            .when(power_busy, |el| {
                                el.child(render_shimmer_band(self.loading_phase, 125.0))
                                    .child(render_loading_bars(self.loading_phase, theme::white()))
                                    .child(if self.power_starting {
                                        "正在启动..."
                                    } else {
                                        "正在停止..."
                                    })
                            })
                            .when(!power_busy, |el| {
                                el.child(if running {
                                    format!("■  {}", format_uptime(tel.uptime_secs))
                                } else {
                                    "▶  启动代理".to_string()
                                })
                            })
                    }),
            );

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_3()
            .child(row1)
            .child(row2)
            .child(bottom_bar)
    }

}