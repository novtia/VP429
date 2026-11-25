use crate::config::AppConfig;
use crate::ui::anim::render_pill_toggle;
use crate::ui::{theme, MainView};
use gpui::*;

impl MainView {
    pub(in crate::ui) fn render_settings_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let cfg = self.engine.config.read().unwrap().clone();
        let allow_lan = cfg.allow_lan;
        let data_dir = AppConfig::data_dir().to_string_lossy().to_string();

        let setting_row = |title: &'static str, desc: String, right: AnyElement| {
            div()
                .px_4()
                .py_3()
                .border_b_1()
                .border_color(theme::divider_color())
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::ink_primary())
                                .child(title),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::ink_muted())
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .child(desc),
                        ),
                )
                .child(div().flex_shrink_0().child(right))
        };

        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .child(
                div()
                    .id("settings-scroll")
                    .track_scroll(&self.settings_scroll)
                    .flex_1()
                    .min_w_0()
                    .overflow_y_scroll()
                    .pr_2()
                    .flex()
                    .flex_col()
                    .gap_4()
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
                                    .child("系统与端口设置"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("btn-reset-ports")
                                            .px_3()
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
                                                    this.reset_default_ports(cx)
                                                }),
                                            )
                                            .child("恢复默认端口"),
                                    )
                                    .child(
                                        div()
                                            .id("btn-save-settings")
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
                                                    this.save_system_settings(cx)
                                                }),
                                            )
                                            .child("保存系统设置"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .bg(theme::card_bg())
                            .border_1()
                            .border_color(theme::border_color())
                            .rounded(px(5.0))
                            .flex()
                            .flex_col()
                            .child(setting_row(
                                "本地代理端口 (HTTP / HTTPS)",
                                "对外监听的主代理端口，系统代理与 HTTPS_PROXY 环境变量使用此端口".to_string(),
                                div()
                                    .w(px(110.0))
                                    .child(self.local_port_input.clone())
                                    .into_any_element(),
                            ))
                            .child(setting_row(
                                "允许局域网 / WSL 连接",
                                "开启后主代理端口将监听 0.0.0.0，允许同一局域网设备或 WSL2 虚拟机接入".to_string(),
                                div()
                                    .id("chk-allow-lan")
                                    .cursor_pointer()
                                    .active(|s| s.opacity(0.8))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            {
                                                let mut c = this.engine.config.write().unwrap();
                                                c.allow_lan = !allow_lan;
                                            }
                                            this.engine.save_config();
                                            cx.notify();
                                        }),
                                    )
                                    .child(render_pill_toggle(
                                        self.anims.get(
                                            "tog_allow_lan",
                                            if allow_lan { 1.0 } else { 0.0 },
                                        ),
                                        0xe84c1e,
                                    ))
                                    .into_any_element(),
                            ))
                            .child(setting_row(
                                "Mihomo Mixed 内部端口",
                                "非监听域名直通流量使用的 Mihomo 内部混合端口".to_string(),
                                div()
                                    .w(px(110.0))
                                    .child(self.mixed_port_input.clone())
                                    .into_any_element(),
                            ))
                            .child(setting_row(
                                "TUN 回流 MITM 端口",
                                "TUN 全局模式下，目标监听域名回流至本地 429 拦截器的内部端口".to_string(),
                                div()
                                    .w(px(110.0))
                                    .child(self.tun_mitm_port_input.clone())
                                    .into_any_element(),
                            ))
                            .child(setting_row(
                                "Mihomo 无回环直出端口",
                                "MITM 解密后直接绑定 VERTEX-POOL 出站的专用端口（防止路由死循环）".to_string(),
                                div()
                                    .w(px(110.0))
                                    .child(self.bypass_port_input.clone())
                                    .into_any_element(),
                            ))
                            .child(setting_row(
                                "Mihomo API 控制端口",
                                "本地用于毫秒级切换节点与真连接测速的 External Controller 端口".to_string(),
                                div()
                                    .w(px(110.0))
                                    .child(self.api_port_input.clone())
                                    .into_any_element(),
                            ))
                            .child(setting_row(
                                "最大可重试请求体缓冲 (MB)",
                                "单次 Vertex Prompt 请求体积小于该阈值时允许在 429 后自动重放".to_string(),
                                div()
                                    .w(px(110.0))
                                    .child(self.max_body_mb_input.clone())
                                    .into_any_element(),
                            ))
                            .child(
                                div()
                                    .px_4()
                                    .py_3()
                                    .border_b_1()
                                    .border_color(theme::divider_color())
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme::ink_primary())
                                            .child("自定义 Mihomo 可执行文件路径 (可选)"),
                                    )
                                    .child(div().w_full().child(self.mihomo_path_input.clone())),
                            )
                            .child(setting_row(
                                "本地配置与证书目录",
                                data_dir.clone(),
                                div()
                                    .id("btn-open-datadir")
                                    .px_3()
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
                                    .on_mouse_down(MouseButton::Left, move |_, _, _| {
                                        let _ = std::process::Command::new("explorer")
                                            .arg(AppConfig::data_dir())
                                            .spawn();
                                    })
                                    .child("打开数据目录")
                                    .into_any_element(),
                            )),
                    ),
            )
            .child(self.render_scrollbar("sb-settings", &self.settings_scroll, false, cx))
    }
}