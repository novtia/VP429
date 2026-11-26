use crate::state::LogLevel;
use crate::ui::anim::{render_loading_bars, render_shimmer_band};
use crate::ui::flags::{detect_country, render_flag};
use crate::ui::{theme, MainView};
use gpui::prelude::FluentBuilder;
use gpui::*;

impl MainView {
    pub(in crate::ui) fn render_nodes_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let snap = self.engine.scheduler.snapshot();
        let is_testing = self.engine.is_testing_latency();
        let latest_log = self.engine.bus.snapshot_logs().pop();

        let node_rows = snap.nodes.into_iter().enumerate().map(|(idx, n)| {
            let node_name_click = n.name.clone();
            let country = detect_country(&n.name);
            let status_text = if n.cooldown_remaining_secs > 0 {
                format!("429 冷却 {}s", n.cooldown_remaining_secs)
            } else if n.is_active {
                "使用中".to_string()
            } else {
                format!("429: {}", n.hit_429_count)
            };
            let (latency_text, latency_color) = match n.latency_ms {
                Some(0) => ("超时".to_string(), theme::braun_orange()),
                Some(ms) if ms < 150 => (format!("{} ms", ms), theme::status_green()),
                Some(ms) => (format!("{} ms", ms), theme::ink_secondary()),
                None => ("--".to_string(), theme::ink_muted()),
            };

            div()
                .id(("node-row", idx))
                .px_4()
                .py_2p5()
                .border_b_1()
                .border_color(theme::divider_color())
                .bg(if n.is_active {
                    theme::row_active_bg()
                } else {
                    theme::card_bg()
                })
                .hover(|s| s.bg(theme::row_hover_bg()))
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_between()
                .text_xs()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        let eng = this.engine.clone();
                        let name = node_name_click.clone();
                        this.rt.spawn(async move {
                            let _ = eng.select_node_manual(&name).await;
                        });
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .overflow_hidden()
                        .child(render_flag(country))
                        .child(
                            div()
                                .w(px(24.0))
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::ink_muted())
                                .child(country.iso()),
                        )
                        .child(
                            div()
                                .w(px(7.0))
                                .h(px(7.0))
                                .rounded_full()
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
                                    FontWeight::MEDIUM
                                })
                                .text_color(theme::ink_primary())
                                .whitespace_nowrap()
                                .child(n.name),
                        )
                        .child(
                            div()
                                .px_1p5()
                                .py(px(1.0))
                                .rounded(px(3.0))
                                .bg(theme::sidebar_bg())
                                .text_color(theme::ink_muted())
                                .child(n.proxy_type),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_5()
                        .child(
                            div()
                                .text_color(if n.cooldown_remaining_secs > 0 {
                                    theme::braun_orange()
                                } else if n.is_active {
                                    theme::status_green()
                                } else {
                                    theme::ink_muted()
                                })
                                .font_weight(if n.cooldown_remaining_secs > 0 || n.is_active {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::NORMAL
                                })
                                .child(status_text),
                        )
                        .child(
                            div()
                                .w(px(56.0))
                                .text_right()
                                .text_color(latency_color)
                                .child(latency_text),
                        ),
                )
        });

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap_3()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::ink_primary())
                                    .child("节点列表"),
                            )
                            .child(div().text_xs().text_color(theme::ink_muted()).child(
                                format!(
                                    "可用 {} / 冷却 {} / 总计 {}",
                                    snap.ready_count,
                                    snap.cooldown_count,
                                    snap.ready_count + snap.cooldown_count
                                ),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("nodes-ping-all")
                                    .relative()
                                    .overflow_hidden()
                                    .px_3()
                                    .py_1p5()
                                    .bg(if is_testing {
                                        rgb(0xfce9e3)
                                    } else {
                                        theme::btn_bg()
                                    })
                                    .hover(|s| s.bg(theme::btn_hover()))
                                    .active(|s| s.border_b_1().mt(px(1.0)))
                                    .border_1()
                                    .border_b_2()
                                    .border_color(if is_testing {
                                        theme::braun_orange()
                                    } else {
                                        theme::border_strong()
                                    })
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .text_xs()
                                    .text_color(if is_testing {
                                        theme::braun_orange()
                                    } else {
                                        theme::ink_primary()
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            let eng = this.engine.clone();
                                            this.rt.spawn(async move {
                                                eng.ping_all_nodes().await;
                                            });
                                            cx.notify();
                                        }),
                                    )
                                    .when(is_testing, |el| {
                                        el.child(render_shimmer_band(self.loading_phase, 100.0))
                                            .child(render_loading_bars(
                                                self.loading_phase,
                                                theme::braun_orange(),
                                            ))
                                            .child("测速中...")
                                    })
                                    .when(!is_testing, |el| el.child("全部测速")),
                            )
                            .child(
                                div()
                                    .id("nodes-rotate")
                                    .px_3()
                                    .py_1p5()
                                    .bg(theme::braun_orange())
                                    .hover(|s| s.bg(theme::braun_orange_border()))
                                    .border_1()
                                    .border_b_2()
                                    .border_color(theme::braun_orange_border())
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::white())
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            let eng = this.engine.clone();
                                            this.rt.spawn(async move {
                                                let _ = eng.scheduler.rotate_next("手动轮换节点").await;
                                            });
                                            cx.notify();
                                        }),
                                    )
                                    .child("轮换下一节点"),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .bg(theme::card_bg())
                    .border_1()
                    .border_color(theme::border_color())
                    .rounded(px(5.0))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .bg(theme::card_header_bg())
                            .border_b_1()
                            .border_color(theme::border_color())
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_xs()
                            .text_color(theme::ink_muted())
                            .child("地区 / 节点名称 / 协议")
                            .child("状态 / 延迟"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .child(
                                div()
                                    .id("node-scroll")
                                    .track_scroll(&self.nodes_scroll)
                                    .flex_1()
                                    .overflow_y_scroll()
                                    .children(node_rows),
                            )
                            .child(self.render_scrollbar("sb-nodes", &self.nodes_scroll, false, cx)),
                    ),
            )
            .when_some(latest_log, |el, l| {
                let color = match l.level {
                    LogLevel::Error | LogLevel::RateLimit429 => theme::braun_orange(),
                    LogLevel::Success => theme::status_green(),
                    LogLevel::Info => theme::ink_secondary(),
                };
                el.child(
                    div()
                        .px_3()
                        .py_1p5()
                        .bg(theme::tape_bg())
                        .border_1()
                        .border_color(theme::border_color())
                        .rounded(px(4.0))
                        .text_xs()
                        .text_color(color)
                        .child(format!("[{}] {}", l.timestamp, l.message)),
                )
            })
    }

}