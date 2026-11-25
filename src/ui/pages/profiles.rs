use crate::state::LogLevel;
use crate::ui::icons::icon;
use crate::ui::{theme, MainView};
use gpui::prelude::FluentBuilder;
use gpui::*;

impl MainView {
    pub(in crate::ui) fn render_profiles_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (profiles, active_id) = {
            let cfg = self.engine.config.read().unwrap();
            (cfg.profiles.clone(), cfg.active_profile_id.clone())
        };
        let latest_log = self.engine.bus.snapshot_logs().pop();
        let now_unix = chrono::Utc::now().timestamp();

        let format_gb = |bytes: u64| -> String {
            let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            if (gb - gb.round()).abs() < 0.05 {
                format!("{:.0}GB", gb)
            } else {
                format!("{:.1}GB", gb)
            }
        };

        let format_relative = |ts: i64, fallback: &str| -> String {
            if ts <= 0 {
                return if fallback.is_empty() {
                    "未更新 · 点击 ⋮ 更新".to_string()
                } else {
                    fallback.to_string()
                };
            }
            let diff = (now_unix - ts).max(0);
            if diff < 60 {
                "刚刚".to_string()
            } else if diff < 3600 {
                format!("{} 分钟前", diff / 60)
            } else if diff < 86400 {
                format!("{} 小时前", diff / 3600)
            } else {
                format!("{} 天前", diff / 86400)
            }
        };

        let format_expire = |ts: i64| -> Option<String> {
            if ts <= 0 {
                return None;
            }
            chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string())
        };

        let cards: Vec<AnyElement> = profiles
            .into_iter()
            .enumerate()
            .map(|(idx, prof)| {
                let is_active = active_id.as_deref() == Some(prof.id.as_str());
                let menu_open = self.open_profile_menu.as_deref() == Some(prof.id.as_str());
                let id_act = prof.id.clone();
                let id_menu = prof.id.clone();
                let id_ref = prof.id.clone();
                let id_del = prof.id.clone();
                let url_copy = prof.url.clone();

                let ratio = if prof.total_bytes > 0 {
                    (prof.used_bytes as f32 / prof.total_bytes as f32).clamp(0.02, 1.0)
                } else {
                    0.0
                };

                let quota_line = if prof.total_bytes > 0 {
                    let base = format!(
                        "{} / {}",
                        format_gb(prof.used_bytes),
                        format_gb(prof.total_bytes)
                    );
                    if let Some(exp) = format_expire(prof.expire_unix) {
                        format!("{} · {}", base, exp)
                    } else {
                        base
                    }
                } else {
                    format!("{} 个节点 · 点击 ⋮ 更新获取流量", prof.node_count)
                };

                let rel_time = format_relative(prof.updated_unix, &prof.updated_at);

                div()
                    .id(("prof-card", idx))
                    .relative()
                    .w(px(340.0))
                    .min_h(px(126.0))
                    .p_4()
                    .bg(if is_active {
                        theme::row_active_bg()
                    } else {
                        theme::card_bg()
                    })
                    .border_1()
                    .border_color(if is_active {
                        theme::braun_orange()
                    } else {
                        theme::border_color()
                    })
                    .rounded(px(10.0))
                    .flex()
                    .flex_col()
                    .justify_between()
                    .gap_2p5()
                    .child(
                        // Top row: Bold Profile Title + `⋮` button
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .id(("prof-select", idx))
                                    .flex_1()
                                    .min_w_0()
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.open_profile_menu = None;
                                            let eng = this.engine.clone();
                                            let id = id_act.clone();
                                            this.rt.spawn(async move {
                                                if let Err(e) = eng.activate_profile(&id).await {
                                                    eng.bus.error(format!("切换配置失败: {}", e));
                                                }
                                            });
                                            cx.notify();
                                        }),
                                    )
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::ink_primary())
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(prof.name),
                            )
                            .child(
                                div()
                                    .id(("prof-more", idx))
                                    .w(px(26.0))
                                    .h(px(26.0))
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme::sidebar_active()))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            if this.open_profile_menu.as_deref()
                                                == Some(id_menu.as_str())
                                            {
                                                this.open_profile_menu = None;
                                            } else {
                                                this.open_profile_menu = Some(id_menu.clone());
                                            }
                                            cx.notify();
                                        }),
                                    )
                                    .child(icon("icons/more.svg", 16.0, theme::ink_secondary())),
                            ),
                    )
                    .child(
                        // Quota Progress Bar (FlClash style)
                        div()
                            .w_full()
                            .h(px(6.0))
                            .rounded_full()
                            .overflow_hidden()
                            .bg(theme::border_color())
                            .when(prof.total_bytes > 0, |el| {
                                el.child(
                                    div()
                                        .h_full()
                                        .w(relative(ratio))
                                        .rounded_full()
                                        .bg(if is_active {
                                            theme::braun_orange()
                                        } else {
                                            theme::ink_primary()
                                        }),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::ink_primary())
                                    .child(quota_line),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::ink_muted())
                                    .child(rel_time),
                            ),
                    )
                    // Inline `⋮` Action Dropdown Menu (deferred to top z-layer so card borders never draw over it)
                    .when(menu_open, |el| {
                        el.child(
                            deferred(
                                div()
                                    .absolute()
                                    .top(px(42.0))
                                    .right(px(12.0))
                                    .w(px(116.0))
                                    .bg(theme::white())
                                    .border_1()
                                    .border_color(theme::border_strong())
                                    .rounded(px(6.0))
                                    .shadow_md()
                                    .occlude()
                                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                        this.open_profile_menu = None;
                                        cx.notify();
                                    }))
                                    .p_1()
                                    .flex()
                                    .flex_col()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .id(("pm-ref", idx))
                                            .px_2p5()
                                            .py_1p5()
                                            .rounded(px(4.0))
                                            .cursor_pointer()
                                            .hover(|s| s.bg(theme::row_hover_bg()))
                                            .text_xs()
                                            .text_color(theme::ink_primary())
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    this.open_profile_menu = None;
                                                    let eng = this.engine.clone();
                                                    let id = id_ref.clone();
                                                    this.rt.spawn(async move {
                                                        if let Err(e) =
                                                            eng.refresh_profile(&id).await
                                                        {
                                                            eng.bus.error(format!(
                                                                "更新订阅失败: {}",
                                                                e
                                                            ));
                                                        }
                                                    });
                                                    cx.notify();
                                                }),
                                            )
                                            .child("更新订阅"),
                                    )
                                    .child(
                                        div()
                                            .id(("pm-copy", idx))
                                            .px_2p5()
                                            .py_1p5()
                                            .rounded(px(4.0))
                                            .cursor_pointer()
                                            .hover(|s| s.bg(theme::row_hover_bg()))
                                            .text_xs()
                                            .text_color(theme::ink_primary())
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    this.open_profile_menu = None;
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(url_copy.clone()),
                                                    );
                                                    this.engine.bus.ok("已复制订阅 URL");
                                                    cx.notify();
                                                }),
                                            )
                                            .child("复制链接"),
                                    )
                                    .child(
                                        div()
                                            .id(("pm-del", idx))
                                            .px_2p5()
                                            .py_1p5()
                                            .rounded(px(4.0))
                                            .cursor_pointer()
                                            .text_xs()
                                            .text_color(theme::braun_orange())
                                            .hover(|s| s.bg(rgb(0xfce9e3)))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    this.open_profile_menu = None;
                                                    this.engine.delete_profile(&id_del);
                                                    cx.notify();
                                                }),
                                            )
                                            .child("删除配置"),
                                    ),
                            )
                            .with_priority(1),
                        )
                    })
                    .into_any_element()
            })
            .collect();

        div()
            .relative()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::ink_primary())
                            .child("配置"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::ink_muted())
                            .child("点击卡片切换 · 点右上 ⋮ 更新/删除 · 右下角 + 添加"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(
                        div()
                            .id("profiles-scroll")
                            .track_scroll(&self.profiles_scroll)
                            .flex_1()
                            .min_w_0()
                            .overflow_y_scroll()
                            .pr_2()
                            .pb(px(72.0))
                            .flex()
                            .flex_wrap()
                            .content_start()
                            .gap_4()
                            .children(cards),
                    )
                    .child(self.render_scrollbar("sb-profiles", &self.profiles_scroll, false, cx)),
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
            // Floating bottom-right '+' button
            .child(
                div()
                    .id("fab-add-profile")
                    .absolute()
                    .bottom(px(16.0))
                    .right(px(20.0))
                    .w(px(46.0))
                    .h(px(46.0))
                    .rounded_full()
                    .bg(theme::braun_orange())
                    .hover(|s| s.bg(theme::braun_orange_border()))
                    .border_1()
                    .border_b_2()
                    .border_color(theme::braun_orange_border())
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.open_profile_menu = None;
                            this.open_add_profile_modal(window, cx);
                        }),
                    )
                    .child(icon("icons/plus.svg", 20.0, theme::white())),
            )
    }

    /// Modal dialog for adding a new subscription profile URL
    pub(in crate::ui) fn render_add_profile_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let modal_p = self
            .anims
            .get("modal_p", if self.show_add_modal { 1.0 } else { 0.0 });
        div()
            .id("modal-backdrop")
            .absolute()
            .inset_0()
            .bg(rgba(0x18191888))
            .opacity(modal_p)
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.show_add_modal = false;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("modal-dialog-card")
                    .w(px(480.0))
                    .mt(px((1.0 - modal_p) * 20.0))
                    .bg(theme::workspace_bg())
                    .border_1()
                    .border_color(theme::border_strong())
                    .rounded(px(6.0))
                    .shadow_lg()
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::ink_primary())
                                    .child("添加订阅配置文件"),
                            )
                            .child(
                                div()
                                    .id("modal-close-x")
                                    .cursor_pointer()
                                    .p_1()
                                    .rounded(px(3.0))
                                    .hover(|s| s.bg(theme::sidebar_active()))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.show_add_modal = false;
                                            cx.notify();
                                        }),
                                    )
                                    .child(icon("icons/win-close.svg", 11.0, theme::ink_secondary())),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::ink_secondary())
                                    .child("配置名称 (可选)"),
                            )
                            .child(self.modal_name_input.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::ink_secondary())
                                    .child("Clash / Mihomo 订阅 URL 网址"),
                            )
                            .child(self.modal_url_input.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .pt_1()
                            .child(
                                div()
                                    .id("modal-cancel")
                                    .px_4()
                                    .py_1p5()
                                    .bg(theme::btn_bg())
                                    .hover(|s| s.bg(theme::btn_hover()))
                                    .border_1()
                                    .border_b_2()
                                    .border_color(theme::border_strong())
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(theme::ink_primary())
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.show_add_modal = false;
                                            cx.notify();
                                        }),
                                    )
                                    .child("取消"),
                            )
                            .child(
                                div()
                                    .id("modal-submit")
                                    .px_4()
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
                                            this.submit_add_profile(cx);
                                        }),
                                    )
                                    .child("确认并同步"),
                            ),
                    ),
            )
    }

}