use crate::state::LogLevel;
use crate::ui::{theme, MainView};
use gpui::*;

impl MainView {
    pub(in crate::ui) fn render_logs_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let logs = self.engine.bus.snapshot_logs();
        let rows = logs.iter().rev().map(|l| {
            let color = match l.level {
                LogLevel::RateLimit429 => theme::lcd_orange(),
                LogLevel::Success => theme::lcd_green(),
                LogLevel::Error => theme::braun_orange(),
                LogLevel::Info => theme::lcd_text(),
            };
            div()
                .text_xs()
                .text_color(color)
                .child(format!("[{}] [{:<4}] {}", l.timestamp, l.level.tag(), l.message))
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
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::ink_primary())
                            .child("连接与 429 拦截日志"),
                    )
                    .child(
                        div()
                            .id("btn-clear-logs")
                            .px_3()
                            .py_1()
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
                                    this.engine.bus.clear_logs();
                                    cx.notify();
                                }),
                            )
                            .child("清空"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .bg(theme::lcd_bg())
                    .border_1()
                    .border_color(theme::border_strong())
                    .rounded(px(5.0))
                    .flex()
                    .overflow_hidden()
                    .child(
                        div()
                            .id("full-log-scroll")
                            .track_scroll(&self.logs_scroll)
                            .flex_1()
                            .overflow_y_scroll()
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(rows),
                    )
                    .child(self.render_scrollbar("sb-logs", &self.logs_scroll, true, cx)),
            )
    }

}