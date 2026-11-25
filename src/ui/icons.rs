use anyhow::Result;
use gpui::{px, svg, AssetSource, Rgba, SharedString, Styled, Svg};
use std::borrow::Cow;

pub struct EmbeddedAssets;

const WIN_MIN_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12" fill="none"><line x1="1.5" y1="6" x2="10.5" y2="6" stroke="currentColor" stroke-width="1.2"/></svg>"#;
const WIN_MAX_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12" fill="none"><rect x="2" y="2" width="8" height="8" rx="0.8" stroke="currentColor" stroke-width="1.2"/></svg>"#;
const WIN_RESTORE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12" fill="none"><rect x="1.8" y="3.5" width="6.7" height="6.7" rx="0.6" stroke="currentColor" stroke-width="1.15"/><path d="M3.8 3.5V2.2C3.8 1.9 4.0 1.7 4.3 1.7H9.8C10.1 1.7 10.3 1.9 10.3 2.2V7.7C10.3 8.0 10.1 8.2 9.8 8.2H8.5" stroke="currentColor" stroke-width="1.15"/></svg>"#;
const WIN_CLOSE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12" fill="none"><path d="M2.2 2.2L9.8 9.8M9.8 2.2L2.2 9.8" stroke="currentColor" stroke-width="1.25" stroke-linecap="round"/></svg>"#;

const ICON_DASHBOARD_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none"><rect x="2" y="2" width="5" height="5" rx="1" stroke="currentColor" stroke-width="1.4"/><rect x="9" y="2" width="5" height="5" rx="1" stroke="currentColor" stroke-width="1.4"/><rect x="2" y="9" width="5" height="5" rx="1" stroke="currentColor" stroke-width="1.4"/><rect x="9" y="9" width="5" height="5" rx="1" stroke="currentColor" stroke-width="1.4"/></svg>"#;
const ICON_NODES_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none"><circle cx="8" cy="8" r="6" stroke="currentColor" stroke-width="1.4"/><path d="M2.5 8H13.5M8 2C9.7 3.8 10.5 5.8 10.5 8C10.5 10.2 9.7 12.2 8 14C6.3 12.2 5.5 10.2 5.5 8C5.5 5.8 6.3 3.8 8 2Z" stroke="currentColor" stroke-width="1.25"/></svg>"#;
const ICON_PROFILES_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none"><path d="M3.5 2H9.5L12.5 5V13.5C12.5 13.8 12.3 14 12 14H3.5C3.2 14 3 13.8 3 13.5V2.5C3 2.2 3.2 2 3.5 2Z" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/><path d="M9.5 2V5H12.5M5.5 8H10.5M5.5 10.8H9" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/></svg>"#;
const ICON_SHIELD_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none"><path d="M8 1.8L13.2 3.8V7.6C13.2 10.9 10.9 13.3 8 14.3C5.1 13.3 2.8 10.9 2.8 7.6V3.8L8 1.8Z" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/><path d="M5.8 8.1L7.3 9.6L10.4 6.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const ICON_LOGS_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none"><rect x="2" y="2.5" width="12" height="11" rx="1.5" stroke="currentColor" stroke-width="1.4"/><path d="M4.8 6.2L6.8 8L4.8 9.8M8.2 9.8H11.2" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const ICON_SETTINGS_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none"><circle cx="8" cy="8" r="2.2" stroke="currentColor" stroke-width="1.4"/><path d="M8 1.8V3.2M8 12.8V14.2M1.8 8H3.2M12.8 8H14.2M3.6 3.6L4.6 4.6M11.4 11.4L12.4 12.4M12.4 3.6L11.4 4.6M4.6 11.4L3.6 12.4" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;
const ICON_CHECK_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12" fill="none"><path d="M2.2 6.2L4.8 8.8L9.8 3.2" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const ICON_PLUS_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none"><path d="M8 3V13M3 8H13" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>"#;
const ICON_TRASH_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none"><path d="M3 4.5H13M6 4.5V3H10V4.5M4.5 4.5L5.2 13H10.8L11.5 4.5" stroke="currentColor" stroke-width="1.35" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const ICON_REFRESH_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none"><path d="M13 8A5 5 0 1 1 11.4 4.3M13 3V6H10" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const LOGO_DARK_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128" fill="none"><path d="M32 34 L62 96 L80 60" stroke="currentColor" stroke-width="15" stroke-linecap="round" stroke-linejoin="round"/><circle cx="32" cy="34" r="8" fill="currentColor"/></svg>"#;
const LOGO_ORANGE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128" fill="none"><path d="M62 96 L96 34" stroke="currentColor" stroke-width="15" stroke-linecap="round"/><circle cx="96" cy="34" r="8" fill="currentColor"/></svg>"#;
const ICON_MENU_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none"><path d="M2.5 4.5H13.5M2.5 8H13.5M2.5 11.5H13.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>"#;
const ICON_POWER_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none"><path d="M8 2.2V7.5M5.1 4.1A4.8 4.8 0 1 0 10.9 4.1" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>"#;
const ICON_MORE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none"><circle cx="8" cy="3.5" r="1.35" fill="currentColor"/><circle cx="8" cy="8" r="1.35" fill="currentColor"/><circle cx="8" cy="12.5" r="1.35" fill="currentColor"/></svg>"#;

impl AssetSource for EmbeddedAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let bytes = match path {
            "icons/win-min.svg" => Some(WIN_MIN_SVG),
            "icons/win-max.svg" => Some(WIN_MAX_SVG),
            "icons/win-restore.svg" => Some(WIN_RESTORE_SVG),
            "icons/win-close.svg" => Some(WIN_CLOSE_SVG),
            "icons/dashboard.svg" => Some(ICON_DASHBOARD_SVG),
            "icons/nodes.svg" => Some(ICON_NODES_SVG),
            "icons/profiles.svg" => Some(ICON_PROFILES_SVG),
            "icons/shield.svg" => Some(ICON_SHIELD_SVG),
            "icons/logs.svg" => Some(ICON_LOGS_SVG),
            "icons/settings.svg" => Some(ICON_SETTINGS_SVG),
            "icons/check.svg" => Some(ICON_CHECK_SVG),
            "icons/plus.svg" => Some(ICON_PLUS_SVG),
            "icons/trash.svg" => Some(ICON_TRASH_SVG),
            "icons/refresh.svg" => Some(ICON_REFRESH_SVG),
            "icons/logo-dark.svg" => Some(LOGO_DARK_SVG),
            "icons/logo-orange.svg" => Some(LOGO_ORANGE_SVG),
            "icons/menu.svg" => Some(ICON_MENU_SVG),
            "icons/power.svg" => Some(ICON_POWER_SVG),
            "icons/more.svg" => Some(ICON_MORE_SVG),
            _ => None,
        };
        Ok(bytes.map(Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(vec![])
    }
}

pub fn icon(path: &'static str, size_px: f32, color: Rgba) -> Svg {
    svg()
        .path(path)
        .size(px(size_px))
        .text_color(color)
        .flex_shrink_0()
}
