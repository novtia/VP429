use crate::config::ChainExitSource;
use crate::mihomo::MihomoManager;
use crate::ui::anim::render_pill_toggle;
use crate::ui::{theme, MainView};
use gpui::*;

impl MainView {
    pub(in crate::ui) fn render_strategy_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let cfg = self.engine.config.read().unwrap().clone();
        let ca_ok = self.engine.is_ca_installed();
        let ca_path = self.engine.ca.ca_pem_path().to_string_lossy().to_string();
        let mihomo_status = MihomoManager::resolve_binary_path(&cfg)
            .map(|p| format!("已就绪: {}", p.to_string_lossy()))
            .unwrap_or_else(|| "未检测到 mihomo.exe，请点击右侧一键下载".to_string());
        let p_chain = self
            .anims
            .get("tog_chain", if cfg.enable_chain_proxy { 1.0 } else { 0.0 });
        let chain_exit_desc = match &cfg.chain_exit_source {
            Some(ChainExitSource::SubscriptionNode { name, .. }) => {
                format!("当前落地: 订阅节点「{}」（可在「节点列表」更换）", name)
            }
            Some(ChainExitSource::ProfilePool { profile_name, .. }) => {
                format!("当前落地池: 「{}」（支持 429 自动轮换出口 IP）", profile_name)
            }
            Some(ChainExitSource::CustomSpec { .. }) => {
                "当前落地: 自定义落地节点（前置池 -> 自定义固定出口）".to_string()
            }
            None => "当前未指定落地节点：请在「节点列表」点「设为落地」或在下方填入自定义落地"
                .to_string(),
        };

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
                    .id("strategy-scroll")
                    .track_scroll(&self.strategy_scroll)
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
                                    .child("429 轮询与证书设置"),
                            )
                            .child(
                                div()
                                    .id("btn-save-strategy")
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
                                        cx.listener(|this, _, _, cx| this.save_strategy_inputs(cx)),
                                    )
                                    .child("保存策略"),
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
                                "节点冷却时长 (秒)",
                                "触发 429 后暂不分配请求的时间".to_string(),
                                div()
                                    .w(px(100.0))
                                    .child(self.cooldown_input.clone())
                                    .into_any_element(),
                            ))
                            .child(setting_row(
                                "最大透明重试次数",
                                "切换节点后自动重放当前请求，不向客户端暴露 429".to_string(),
                                div()
                                    .w(px(100.0))
                                    .child(self.retries_input.clone())
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
                                            .child("选择性 HTTPS MITM 监听域名（逗号分隔，其余域名直通不解密）"),
                                    )
                                    .child(div().w_full().child(self.domains_input.clone())),
                            )
                            .child(setting_row(
                                "本地根证书 (VP429 Local CA)",
                                format!(
                                    "状态: {} · {}",
                                    if ca_ok {
                                        "已安装至 CurrentUser\\Root"
                                    } else {
                                        "未安装"
                                    },
                                    ca_path
                                ),
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("btn-install-ca")
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
                                                    this.engine.install_ca();
                                                    cx.notify();
                                                }),
                                            )
                                            .child("一键安装根证书"),
                                    )
                                    .child(
                                        div()
                                            .id("btn-copy-ca")
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
                                                    let p = this
                                                        .engine
                                                        .ca
                                                        .ca_pem_path()
                                                        .to_string_lossy()
                                                        .to_string();
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(p.clone()),
                                                    );
                                                    this.engine
                                                        .bus
                                                        .ok(format!("已复制根证书路径: {}", p));
                                                    cx.notify();
                                                }),
                                            )
                                            .child("复制 PEM 路径"),
                                    )
                                    .into_any_element(),
                            ))
                            .child(setting_row(
                                "Mihomo 内核管理",
                                mihomo_status,
                                div()
                                    .id("btn-download-mihomo")
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
                                            let eng = this.engine.clone();
                                            this.rt.spawn(async move {
                                                if let Err(e) = eng.download_core().await {
                                                    eng.bus.error(format!("下载 Mihomo 失败: {}", e));
                                                }
                                            });
                                            cx.notify();
                                        }),
                                    )
                                    .child("一键下载 / 更新内核")
                                    .into_any_element(),
                            )),
                    )
                    .child(
                        div()
                            .bg(theme::card_bg())
                            .border_1()
                            .border_color(theme::border_color())
                            .rounded(px(5.0))
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .px_4()
                                    .py_2()
                                    .bg(theme::card_header_bg())
                                    .border_b_1()
                                    .border_color(theme::border_color())
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::ink_secondary())
                                    .child("链式代理（前置中转可切 + 固定落地出口防封）"),
                            )
                            .child(setting_row(
                                "启用链式代理",
                                chain_exit_desc,
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("strategy-toggle-chain")
                                            .cursor_pointer()
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    let eng = this.engine.clone();
                                                    let next = !cfg.enable_chain_proxy;
                                                    this.rt.spawn(async move {
                                                        let _ = eng.toggle_chain_proxy(next).await;
                                                    });
                                                    cx.notify();
                                                }),
                                            )
                                            .child(render_pill_toggle(p_chain, 0xe84c1e)),
                                    )
                                    .child(
                                        div()
                                            .id("btn-clear-chain-exit")
                                            .px_2p5()
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
                                                    this.clear_chain_exit(cx);
                                                }),
                                            )
                                            .child("清除落地"),
                                    )
                                    .into_any_element(),
                            ))
                            .child(setting_row(
                                "链式 429 退避重试间隔 (ms)",
                                "开启链式代理后命中 429 不切出口 IP，等待该时长后原出口重放"
                                    .to_string(),
                                div()
                                    .w(px(100.0))
                                    .child(self.chain_backoff_input.clone())
                                    .into_any_element(),
                            ))
                            .child(
                                div()
                                    .px_4()
                                    .py_3()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme::ink_primary())
                                                    .child("自定义落地节点（SOCKS5/HTTP URL 或单条 Clash 节点 YAML）"),
                                            )
                                            .child(
                                                div()
                                                    .id("btn-apply-custom-exit")
                                                    .px_3()
                                                    .py_1()
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
                                                            this.apply_custom_chain_exit(cx);
                                                        }),
                                                    )
                                                    .child("应用自定义落地"),
                                            ),
                                    )
                                    .child(div().w_full().child(self.chain_exit_input.clone())),
                            ),
                    ),
            )
            .child(self.render_scrollbar("sb-strategy", &self.strategy_scroll, false, cx))
    }

}