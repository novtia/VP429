use crate::ui::theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::time::Instant;

pub struct TextInput {
    value: String,
    placeholder: String,
    focus_handle: FocusHandle,
    scroll_handle: ScrollHandle,
    last_edit: Instant,
}

impl TextInput {
    pub fn new(initial: String, placeholder: impl Into<String>, cx: &mut Context<Self>) -> Self {
        Self {
            value: initial,
            placeholder: placeholder.into(),
            focus_handle: cx.focus_handle(),
            scroll_handle: ScrollHandle::new(),
            last_edit: Instant::now(),
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn set_value(&mut self, val: String, cx: &mut Context<Self>) {
        self.value = val;
        self.last_edit = Instant::now();
        cx.notify();
    }

    pub fn focus(&self, window: &mut Window) {
        self.focus_handle.focus(window);
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        self.last_edit = Instant::now();

        if ks.modifiers.control || ks.modifiers.platform {
            match ks.key.as_str() {
                "v" | "V" => {
                    if let Some(item) = cx.read_from_clipboard() {
                        if let Some(text) = item.text() {
                            self.value.push_str(text.trim());
                            cx.notify();
                        }
                    }
                    return;
                }
                "c" | "C" => {
                    cx.write_to_clipboard(ClipboardItem::new_string(self.value.clone()));
                    return;
                }
                "a" | "A" | "u" | "U" => {
                    self.value.clear();
                    cx.notify();
                    return;
                }
                _ => {}
            }
        }

        match ks.key.as_str() {
            "backspace" => {
                self.value.pop();
                cx.notify();
            }
            "space" => {
                self.value.push(' ');
                cx.notify();
            }
            _ => {
                if let Some(ref ch) = ks.key_char {
                    if !ch.chars().any(|c| c.is_control()) {
                        self.value.push_str(ch);
                        cx.notify();
                    }
                }
            }
        }
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus_handle.is_focused(window);
        let is_empty = self.value.is_empty();
        // Blink caret every 500ms; stay solid immediately after typing
        let elapsed_ms = self.last_edit.elapsed().as_millis();
        let caret_visible = focused && (elapsed_ms < 450 || (elapsed_ms / 500) % 2 == 0);

        // Only scroll horizontally to the end when content genuinely overflows the input width
        let max_x: f32 = self.scroll_handle.max_offset().width.into();
        if focused && max_x > 1.0 {
            self.scroll_handle.set_offset(point(px(-max_x), px(0.0)));
        } else if !focused || max_x <= 1.0 {
            self.scroll_handle.set_offset(point(px(0.0), px(0.0)));
        }

        div()
            .id("vp-text-input")
            .track_focus(&self.focus_handle)
            .track_scroll(&self.scroll_handle)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus_handle.focus(window);
                    this.last_edit = Instant::now();
                    cx.notify();
                }),
            )
            .on_key_down(cx.listener(Self::on_key_down))
            .flex_1()
            .h(px(32.0))
            .px_2p5()
            .bg(theme::white())
            .border_1()
            .border_color(if focused {
                theme::braun_orange()
            } else {
                theme::border_strong()
            })
            .rounded(px(4.0))
            .flex()
            .items_center()
            .overflow_x_scroll()
            .cursor_text()
            .text_xs()
            .child(
                div()
                    .flex()
                    .items_center()
                    .whitespace_nowrap()
                    .when(is_empty && focused, |el| {
                        el.child(
                            div()
                                .w(px(2.0))
                                .h(px(14.0))
                                .mr(px(2.0))
                                .flex_shrink_0()
                                .bg(if caret_visible {
                                    theme::braun_orange()
                                } else {
                                    theme::white()
                                }),
                        )
                    })
                    .child(
                        div()
                            .text_color(if is_empty {
                                theme::ink_muted()
                            } else {
                                theme::ink_primary()
                            })
                            .child(if is_empty {
                                self.placeholder.clone()
                            } else {
                                self.value.clone()
                            }),
                    )
                    .when(!is_empty && focused, |el| {
                        el.child(
                            div()
                                .ml(px(1.5))
                                .w(px(2.0))
                                .h(px(14.0))
                                .flex_shrink_0()
                                .bg(if caret_visible {
                                    theme::braun_orange()
                                } else {
                                    theme::white()
                                }),
                        )
                    }),
            )
    }
}
