use crate::ui::anim::{lerp_rgb, render_loading_bars, render_mini_toggle, render_shimmer_band};
use crate::ui::icons::icon;
use crate::ui::{theme, MainView, NavPage};
use gpui::prelude::FluentBuilder;
use gpui::*;

impl MainView {
    pub(in crate::ui) fn apply_scroll_drag(handle: &ScrollHandle, mouse_y: Pixels) {
        let b = handle.bounds();
        let top_y: f32 = b.origin.y.into();
        let h: f32 = b.size.height.into();
        let click_y: f32 = mouse_y.into();
        let max_off: f32 = handle.max_offset().height.into();
        if h > 1.0 && max_off > 1.0 {
            let total_h = h + max_off;
            let thumb_h = ((h / total_h) * h).max(28.0).min(h - 4.0);
            let travel = (h - thumb_h).max(1.0);
            let ratio = ((click_y - top_y - thumb_h * 0.5) / travel).clamp(0.0, 1.0);
            handle.set_offset(point(px(0.0), px(-ratio * max_off)));
        }
    }

    /// Renders a crisp, draggable custom vertical scrollbar synced with a GPUI ScrollHandle.
    pub(in crate::ui) fn render_scrollbar(
        &self,
        id: &'static str,
        handle: &ScrollHandle,
        dark: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let max_y: f32 = handle.max_offset().height.into();
        let view_h: f32 = handle.bounds().size.height.into();
        let track_bg = if dark {
            rgb(0x222320)
        } else {
            theme::card_header_bg()
        };
        let is_dragging = self
            .dragging_scroll
            .as_ref()
            .map(|(drag_id, _)| *drag_id == id)
            .unwrap_or(false);
        let thumb_bg = if is_dragging {
            theme::braun_orange()
        } else if dark {
            rgb(0x55544e)
        } else {
            theme::border_strong()
        };

        if max_y <= 1.0 || view_h <= 1.0 {
            return div().w(px(10.0)).h_full().bg(track_bg).into_any_element();
        }

        let offset_y: f32 = handle.offset().y.into();
        let cur_y = (-offset_y).clamp(0.0, max_y);
        let total_h = view_h + max_y;
        let thumb_h = ((view_h / total_h) * view_h).max(28.0).min(view_h - 4.0);
        let travel = (view_h - thumb_h - 4.0).max(0.0);
        let thumb_top = 2.0 + (cur_y / max_y) * travel;
        let handle_down = handle.clone();

        div()
            .id(id)
            .relative()
            .w(px(10.0))
            .h_full()
            .bg(track_bg)
            .border_l_1()
            .border_color(if dark {
                theme::lcd_divider()
            } else {
                theme::divider_color()
            })
            .cursor(CursorStyle::ResizeUpDown)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    Self::apply_scroll_drag(&handle_down, ev.position.y);
                    this.dragging_scroll = Some((id, handle_down.clone()));
                    cx.notify();
                }),
            )
            .child(
                div()
                    .absolute()
                    .left(px(2.0))
                    .right(px(2.0))
                    .top(px(thumb_top))
                    .h(px(thumb_h))
                    .rounded(px(3.0))
                    .bg(thumb_bg),
            )
            .into_any_element()
    }

    pub(in crate::ui) fn render_header(
        &self,
        _running: bool,
        local_port: u16,
        tun_on: bool,
        ca_ok: bool,
        _is_maximized: bool,
    ) -> impl IntoElement {
        div()
            .h(px(36.0))
            .bg(theme::titlebar_bg())
            .border_b_1()
            .border_color(theme::border_color())
            .pl_4()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .id("titlebar-drag")
                    .window_control_area(WindowControlArea::Drag)
                    .flex_1()
                    .h_full()
                    .pr_4()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        // Crisp Icon 02 (Swiss V-Reroute) vector logo only (no left text)
                        div()
                            .relative()
                            .w(px(22.0))
                            .h(px(22.0))
                            .rounded(px(5.0))
                            .bg(theme::chassis_bg())
                            .border_1()
                            .border_color(theme::border_strong())
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon("icons/logo-dark.svg", 20.0, theme::ink_primary())),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon(
                                        "icons/logo-orange.svg",
                                        20.0,
                                        theme::braun_orange(),
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .text_xs()
                            .text_color(theme::ink_secondary())
                            .when(self.is_power_busy(), |el| {
                                el.child(
                                    div()
                                        .px_2()
                                        .py(px(1.5))
                                        .rounded_full()
                                        .bg(rgb(0xfce9e3))
                                        .border_1()
                                        .border_color(theme::braun_orange())
                                        .flex()
                                        .items_center()
                                        .gap_1p5()
                                        .text_color(theme::braun_orange())
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(render_loading_bars(
                                            self.loading_phase,
                                            theme::braun_orange(),
                                        ))
                                        .child(if self.power_starting {
                                            "正在启动..."
                                        } else {
                                            "正在停止..."
                                        }),
                                )
                            })
                            .child(format!("SYS :{}", local_port))
                            .child("|")
                            .child(
                                div()
                                    .text_color(if tun_on {
                                        theme::status_green()
                                    } else {
                                        theme::ink_muted()
                                    })
                                    .child(if tun_on { "TUN ON" } else { "TUN OFF" }),
                            )
                            .child("|")
                            .child(
                                div()
                                    .text_color(if ca_ok {
                                        theme::status_green()
                                    } else {
                                        theme::braun_orange()
                                    })
                                    .child(if ca_ok { "CA OK" } else { "CA 未安装" }),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .h_full()
                    .items_center()
                    .child(
                        div()
                            .id("win-min")
                            .w(px(46.0))
                            .h_full()
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(|s| s.bg(theme::sidebar_active()))
                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                window.minimize_window();
                            })
                            .child(icon("icons/win-min.svg", 11.0, theme::ink_primary())),
                    )
                    .child(
                        div()
                            .id("win-close")
                            .w(px(46.0))
                            .h_full()
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(|s| s.bg(theme::braun_orange()))
                            .on_mouse_down(MouseButton::Left, |_, _, _| {
                                crate::tray::hide_to_tray();
                            })
                            .child(icon("icons/win-close.svg", 11.0, theme::ink_primary())),
                    ),
            )
    }

    pub(in crate::ui) fn render_sidebar(
        &self,
        running: bool,
        sys_proxy: bool,
        tun_on: bool,
        auto_retry: bool,
        node_count: usize,
        profile_count: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let width_px = self.anims.get("sidebar_w", 192.0);
        let nav_y = self.anims.get("sidebar_nav_y", self.page.index() as f32 * 40.0);
        let p_sys = self.anims.get("tog_sysproxy", if sys_proxy { 1.0 } else { 0.0 });
        let p_tun = self.anims.get("tog_tun", if tun_on { 1.0 } else { 0.0 });
        let p_retry = self.anims.get("tog_retry", if auto_retry { 1.0 } else { 0.0 });
        let p_power = self.anims.get("tog_power", if running { 1.0 } else { 0.0 });
        let show_labels = width_px > 125.0;
        let collapsed = !show_labels;

        let nav_item = |icon_path: &'static str,
                        label: &'static str,
                        badge: Option<String>,
                        target: NavPage,
                        current: NavPage| {
            let active = current == target;
            let fg = if active {
                theme::ink_primary()
            } else {
                theme::ink_secondary()
            };
            div()
                .id(label)
                .relative()
                .w_full()
                .h(px(36.0))
                .px(if collapsed { px(0.0) } else { px(12.0) })
                .rounded(px(4.0))
                .cursor_pointer()
                .bg(if active {
                    theme::sidebar_active()
                } else {
                    theme::sidebar_bg()
                })
                .hover(|s| s.bg(theme::sidebar_active()))
                .active(|s| s.opacity(0.75))
                .flex()
                .items_center()
                .when(collapsed, |el| el.justify_center())
                .when(!collapsed, |el| el.justify_between())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.switch_page(target, cx);
                    }),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2p5()
                        .child(icon(
                            icon_path,
                            if collapsed { 17.0 } else { 15.0 },
                            if active {
                                theme::braun_orange()
                            } else {
                                theme::ink_secondary()
                            },
                        ))
                        .when(!collapsed, |el| {
                            el.child(
                                div()
                                    .text_sm()
                                    .font_weight(if active {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .text_color(fg)
                                    .whitespace_nowrap()
                                    .child(label),
                            )
                        }),
                )
                .when(!collapsed, |el| {
                    el.when_some(badge, |el, b| {
                        el.child(
                            div()
                                .px_1p5()
                                .py(px(1.0))
                                .rounded(px(3.0))
                                .bg(theme::card_bg())
                                .border_1()
                                .border_color(theme::border_color())
                                .text_xs()
                                .text_color(theme::ink_muted())
                                .child(b),
                        )
                    })
                })
        };

        let toggle_row = |id: &'static str, label: &'static str, progress: f32, accent_orange: bool| {
            let active_hex = if accent_orange { 0xe84c1e } else { 0x222320 };
            div()
                .id(id)
                .px_2()
                .py_1()
                .rounded(px(4.0))
                .cursor_pointer()
                .hover(|s| s.bg(theme::sidebar_active()))
                .active(|s| s.opacity(0.8))
                .flex()
                .items_center()
                .justify_between()
                .text_xs()
                .text_color(theme::ink_primary())
                .child(label)
                .child(render_mini_toggle(progress, active_hex))
        };

        div()
            .w(px(width_px))
            .flex_shrink_0()
            .overflow_hidden()
            .bg(theme::sidebar_bg())
            .border_r_1()
            .border_color(theme::border_color())
            .px(if collapsed { px(8.0) } else { px(10.0) })
            .py_3()
            .flex()
            .flex_col()
            .justify_between()
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(nav_item(
                        "icons/dashboard.svg",
                        "仪表盘",
                        None,
                        NavPage::Dashboard,
                        self.page,
                    ))
                    .child(nav_item(
                        "icons/nodes.svg",
                        "节点列表",
                        if node_count > 0 {
                            Some(node_count.to_string())
                        } else {
                            None
                        },
                        NavPage::Nodes,
                        self.page,
                    ))
                    .child(nav_item(
                        "icons/profiles.svg",
                        "配置文件",
                        if profile_count > 0 {
                            Some(profile_count.to_string())
                        } else {
                            None
                        },
                        NavPage::Profiles,
                        self.page,
                    ))
                    .child(nav_item(
                        "icons/shield.svg",
                        "429 策略",
                        None,
                        NavPage::StrategyAndCa,
                        self.page,
                    ))
                    .child(nav_item(
                        "icons/logs.svg",
                        "连接日志",
                        None,
                        NavPage::Logs,
                        self.page,
                    ))
                    .child(nav_item(
                        "icons/settings.svg",
                        "系统设置",
                        None,
                        NavPage::Settings,
                        self.page,
                    ))
                    .child(
                        // Smoothly gliding Braun orange active indicator bar
                        div()
                            .absolute()
                            .left_0()
                            .top(px(7.0 + nav_y))
                            .h(px(22.0))
                            .w(px(3.0))
                            .rounded_full()
                            .bg(theme::braun_orange()),
                    ),
            )
            .child(
                div()
                    .pt_3()
                    .border_t_1()
                    .border_color(theme::border_color())
                    .flex()
                    .flex_col()
                    .gap_2()
                    .when(!collapsed, |el| {
                        el.child(
                            toggle_row("chk-sysproxy", "系统代理", p_sys, false).on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    let engine = this.engine.clone();
                                    this.rt.spawn(async move {
                                        engine.toggle_system_proxy(!sys_proxy).await;
                                    });
                                    cx.notify();
                                }),
                            ),
                        )
                        .child(
                            toggle_row("chk-tun", "TUN 虚拟网卡", p_tun, false).on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    {
                                        let mut cfg = this.engine.config.write().unwrap();
                                        cfg.enable_tun = !tun_on;
                                    }
                                    this.engine.save_config();
                                    this.engine.bus.info(format!(
                                        "TUN 虚拟网卡已设为 {}（重启代理生效）",
                                        if !tun_on { "开启" } else { "关闭" }
                                    ));
                                    cx.notify();
                                }),
                            ),
                        )
                        .child(
                            toggle_row("chk-retry", "429 透明重试", p_retry, true).on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    let (mode, cd, max_r) = {
                                        let cfg = this.engine.config.read().unwrap();
                                        (
                                            cfg.rotation_mode.clone(),
                                            cfg.cooldown_seconds,
                                            cfg.max_retries,
                                        )
                                    };
                                    this.engine
                                        .update_rotation_policy(mode, cd, !auto_retry, max_r);
                                    cx.notify();
                                }),
                            ),
                        )
                    })
                    .child({
                        let power_busy = self.is_power_busy();
                        div()
                            .id("btn-power")
                            .relative()
                            .overflow_hidden()
                            .mt_1()
                            .w_full()
                            .py_2()
                            .rounded(px(4.0))
                            .border_1()
                            .border_b_2()
                            .border_color(if power_busy {
                                theme::braun_orange_border()
                            } else {
                                theme::ink_primary()
                            })
                            .bg(if power_busy {
                                rgb(0xd96b1f)
                            } else {
                                lerp_rgb(0xe84c1e, 0x222320, p_power)
                            })
                            .hover(|s| {
                                s.bg(if power_busy {
                                    rgb(0xd96b1f)
                                } else if running {
                                    theme::dark_btn_hover()
                                } else {
                                    theme::braun_orange_border()
                                })
                            })
                            .active(|s| s.border_b_1().mt(px(5.0)).opacity(0.9))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::white())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| this.toggle_power(cx)),
                            )
                            .when(power_busy, |el| {
                                el.child(render_shimmer_band(self.loading_phase, 190.0))
                                    .child(render_loading_bars(self.loading_phase, theme::white()))
                                    .when(!collapsed, |el| {
                                        el.child(if self.power_starting {
                                            "正在启动..."
                                        } else {
                                            "正在停止..."
                                        })
                                    })
                            })
                            .when(!power_busy && collapsed, |el| {
                                el.child(icon(
                                    "icons/power.svg",
                                    16.0,
                                    if running {
                                        theme::lcd_green()
                                    } else {
                                        theme::white()
                                    },
                                ))
                            })
                            .when(!power_busy && !collapsed, |el| {
                                el.child(if running {
                                    "运行中 · 点击停止"
                                } else {
                                    "启动代理服务"
                                })
                            })
                    })
                    .child(
                        // Sidebar Expand / Collapse toggle button
                        div()
                            .id("btn-toggle-sidebar")
                            .w_full()
                            .py_1p5()
                            .px(if collapsed { px(0.0) } else { px(8.0) })
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(theme::sidebar_active()))
                            .active(|s| s.opacity(0.75))
                            .flex()
                            .items_center()
                            .when(collapsed, |el| el.justify_center())
                            .when(!collapsed, |el| el.justify_between())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.sidebar_collapsed = !this.sidebar_collapsed;
                                    cx.notify();
                                }),
                            )
                            .child(icon("icons/menu.svg", 15.0, theme::ink_secondary()))
                            .when(!collapsed, |el| {
                                el.child(
                                    div()
                                        .text_xs()
                                        .whitespace_nowrap()
                                        .text_color(theme::ink_muted())
                                        .child("收起侧边栏"),
                                )
                            }),
                    ),
            )
    }

}