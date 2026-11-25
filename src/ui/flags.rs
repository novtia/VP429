use gpui::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountryCode {
    Us,
    Jp,
    Hk,
    Tw,
    Sg,
    Kr,
    Gb,
    De,
    Fr,
    Nl,
    Ca,
    Au,
    Ch,
    In,
    Ru,
    Tr,
    Se,
    Ar,
    Br,
    Ng,
    Ua,
    Sa,
    Aq,
    My,
    Vn,
    Th,
    Ph,
    Id,
    It,
    Es,
    Pl,
    Unknown,
}

impl CountryCode {
    pub fn iso(&self) -> &'static str {
        match self {
            CountryCode::Us => "US",
            CountryCode::Jp => "JP",
            CountryCode::Hk => "HK",
            CountryCode::Tw => "TW",
            CountryCode::Sg => "SG",
            CountryCode::Kr => "KR",
            CountryCode::Gb => "UK",
            CountryCode::De => "DE",
            CountryCode::Fr => "FR",
            CountryCode::Nl => "NL",
            CountryCode::Ca => "CA",
            CountryCode::Au => "AU",
            CountryCode::Ch => "CH",
            CountryCode::In => "IN",
            CountryCode::Ru => "RU",
            CountryCode::Tr => "TR",
            CountryCode::Se => "SE",
            CountryCode::Ar => "AR",
            CountryCode::Br => "BR",
            CountryCode::Ng => "NG",
            CountryCode::Ua => "UA",
            CountryCode::Sa => "SA",
            CountryCode::Aq => "AQ",
            CountryCode::My => "MY",
            CountryCode::Vn => "VN",
            CountryCode::Th => "TH",
            CountryCode::Ph => "PH",
            CountryCode::Id => "ID",
            CountryCode::It => "IT",
            CountryCode::Es => "ES",
            CountryCode::Pl => "PL",
            CountryCode::Unknown => "GL",
        }
    }
}

pub fn detect_country(name: &str) -> CountryCode {
    let lower = name.to_ascii_lowercase();
    let has_token = |tok: &str| {
        lower
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|part| part == tok)
    };

    if name.contains("美国") || name.contains("🇺🇸") || has_token("us") || has_token("usa")
        || lower.contains("united states") || lower.contains("oregon") || lower.contains("iowa")
        || lower.contains("virginia") || lower.contains("ashburn") || lower.contains("los angeles")
        || lower.contains("san jose") || lower.contains("seattle") || lower.contains("silicon")
        || lower.contains("dallas") || lower.contains("new york") || lower.contains("chicago")
        || lower.contains("miami") || lower.contains("phoenix")
    {
        return CountryCode::Us;
    }
    if name.contains("日本") || name.contains("🇯🇵") || has_token("jp") || lower.contains("japan") || lower.contains("tokyo") || lower.contains("osaka") {
        return CountryCode::Jp;
    }
    if name.contains("香港") || name.contains("🇭🇰") || has_token("hk") || lower.contains("hong kong") || lower.contains("hongkong") {
        return CountryCode::Hk;
    }
    if name.contains("台湾") || name.contains("台灣") || name.contains("🇹🇼") || has_token("tw") || lower.contains("taiwan") || lower.contains("taipei") || lower.contains("hinet") {
        return CountryCode::Tw;
    }
    if name.contains("新加坡") || name.contains("狮城") || name.contains("🇸🇬") || has_token("sg") || lower.contains("singapore") || lower.contains("jurong") {
        return CountryCode::Sg;
    }
    if name.contains("韩国") || name.contains("韓國") || name.contains("🇰🇷") || has_token("kr") || lower.contains("korea") || lower.contains("seoul") {
        return CountryCode::Kr;
    }
    if name.contains("英国") || name.contains("英國") || name.contains("🇬🇧") || has_token("uk") || has_token("gb") || lower.contains("london") || lower.contains("britain") {
        return CountryCode::Gb;
    }
    if name.contains("德国") || name.contains("德國") || name.contains("🇩🇪") || has_token("de") || lower.contains("germany") || lower.contains("frankfurt") {
        return CountryCode::De;
    }
    if name.contains("法国") || name.contains("法國") || name.contains("🇫🇷") || has_token("fr") || lower.contains("france") || lower.contains("paris") {
        return CountryCode::Fr;
    }
    if name.contains("荷兰") || name.contains("荷蘭") || name.contains("🇳🇱") || has_token("nl") || lower.contains("netherlands") || lower.contains("amsterdam") {
        return CountryCode::Nl;
    }
    if name.contains("加拿大") || name.contains("🇨🇦") || has_token("ca") || lower.contains("canada") || lower.contains("toronto") || lower.contains("vancouver") {
        return CountryCode::Ca;
    }
    if name.contains("澳大利亚") || name.contains("澳洲") || name.contains("🇦🇺") || has_token("au") || lower.contains("australia") || lower.contains("sydney") {
        return CountryCode::Au;
    }
    if name.contains("瑞士") || name.contains("🇨🇭") || has_token("ch") || lower.contains("switzerland") || lower.contains("zurich") {
        return CountryCode::Ch;
    }
    if name.contains("印度") || name.contains("🇮🇳") || has_token("in") || lower.contains("india") || lower.contains("mumbai") {
        return CountryCode::In;
    }
    if name.contains("俄罗斯") || name.contains("🇷🇺") || has_token("ru") || lower.contains("russia") || lower.contains("moscow") {
        return CountryCode::Ru;
    }
    if name.contains("土耳其") || name.contains("🇹🇷") || has_token("tr") || lower.contains("turkey") || lower.contains("istanbul") {
        return CountryCode::Tr;
    }
    if name.contains("瑞典") || name.contains("🇸🇪") || has_token("se") || lower.contains("sweden") || lower.contains("stockholm") {
        return CountryCode::Se;
    }
    if name.contains("阿根廷") || name.contains("🇦🇷") || has_token("ar") || lower.contains("argentina") {
        return CountryCode::Ar;
    }
    if name.contains("巴西") || name.contains("🇧🇷") || has_token("br") || lower.contains("brazil") || lower.contains("sao paulo") {
        return CountryCode::Br;
    }
    if name.contains("尼日利亚") || name.contains("🇳🇬") || has_token("ng") || lower.contains("nigeria") {
        return CountryCode::Ng;
    }
    if name.contains("乌克兰") || name.contains("🇺🇦") || has_token("ua") || lower.contains("ukraine") {
        return CountryCode::Ua;
    }
    if name.contains("沙特") || name.contains("🇸🇦") || has_token("sa") || lower.contains("saudi") {
        return CountryCode::Sa;
    }
    if name.contains("南极") || name.contains("🇦🇶") || has_token("aq") || lower.contains("antarctica") {
        return CountryCode::Aq;
    }
    if name.contains("马来") || name.contains("🇲🇾") || has_token("my") || lower.contains("malaysia") {
        return CountryCode::My;
    }
    if name.contains("越南") || name.contains("🇻🇳") || has_token("vn") || lower.contains("vietnam") {
        return CountryCode::Vn;
    }
    if name.contains("泰国") || name.contains("🇹🇭") || has_token("th") || lower.contains("thailand") {
        return CountryCode::Th;
    }
    if name.contains("菲律宾") || name.contains("🇵🇭") || has_token("ph") || lower.contains("philippines") {
        return CountryCode::Ph;
    }
    if name.contains("印尼") || name.contains("印度尼西亚") || name.contains("🇮🇩") || has_token("id") || lower.contains("indonesia") {
        return CountryCode::Id;
    }
    if name.contains("意大利") || name.contains("🇮🇹") || has_token("it") || lower.contains("italy") {
        return CountryCode::It;
    }
    if name.contains("西班牙") || name.contains("🇪🇸") || has_token("es") || lower.contains("spain") {
        return CountryCode::Es;
    }
    if name.contains("波兰") || name.contains("🇵🇱") || has_token("pl") || lower.contains("poland") {
        return CountryCode::Pl;
    }

    CountryCode::Unknown
}

fn flag_frame() -> Div {
    div()
        .relative()
        .w(px(22.0))
        .h(px(15.0))
        .rounded(px(2.0))
        .overflow_hidden()
        .border_1()
        .border_color(rgba(0x00000028))
        .flex_shrink_0()
}

fn h_bicolor(c1: u32, c2: u32) -> AnyElement {
    flag_frame()
        .flex()
        .flex_col()
        .child(div().flex_1().w_full().bg(rgb(c1)))
        .child(div().flex_1().w_full().bg(rgb(c2)))
        .into_any_element()
}

fn h_tricolor(c1: u32, c2: u32, c3: u32) -> AnyElement {
    flag_frame()
        .flex()
        .flex_col()
        .child(div().flex_1().w_full().bg(rgb(c1)))
        .child(div().flex_1().w_full().bg(rgb(c2)))
        .child(div().flex_1().w_full().bg(rgb(c3)))
        .into_any_element()
}

fn v_tricolor(c1: u32, c2: u32, c3: u32) -> AnyElement {
    flag_frame()
        .flex()
        .flex_row()
        .child(div().flex_1().h_full().bg(rgb(c1)))
        .child(div().flex_1().h_full().bg(rgb(c2)))
        .child(div().flex_1().h_full().bg(rgb(c3)))
        .into_any_element()
}

pub fn render_flag(code: CountryCode) -> AnyElement {
    match code {
        CountryCode::Us => flag_frame()
            .bg(rgb(0xffffff))
            .flex()
            .flex_col()
            .child(div().h(px(2.0)).w_full().bg(rgb(0xb22234)))
            .child(div().h(px(2.0)).w_full().bg(rgb(0xffffff)))
            .child(div().h(px(2.0)).w_full().bg(rgb(0xb22234)))
            .child(div().h(px(2.0)).w_full().bg(rgb(0xffffff)))
            .child(div().h(px(2.0)).w_full().bg(rgb(0xb22234)))
            .child(div().h(px(2.0)).w_full().bg(rgb(0xffffff)))
            .child(div().flex_1().w_full().bg(rgb(0xb22234)))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(px(10.0))
                    .h(px(8.0))
                    .bg(rgb(0x3c3b6e))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().w(px(3.0)).h(px(3.0)).rounded_full().bg(rgb(0xffffff))),
            )
            .into_any_element(),

        CountryCode::Jp => flag_frame()
            .bg(rgb(0xffffff))
            .flex()
            .items_center()
            .justify_center()
            .child(div().w(px(7.0)).h(px(7.0)).rounded_full().bg(rgb(0xbc002d)))
            .into_any_element(),

        CountryCode::Hk => flag_frame()
            .bg(rgb(0xde2910))
            .flex()
            .items_center()
            .justify_center()
            .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(rgb(0xffffff)))
            .into_any_element(),

        CountryCode::Tw => flag_frame()
            .bg(rgb(0xfe0000))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(px(10.5))
                    .h(px(7.5))
                    .bg(rgb(0x000095))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().w(px(3.5)).h(px(3.5)).rounded_full().bg(rgb(0xffffff))),
            )
            .into_any_element(),

        CountryCode::Sg => flag_frame()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .bg(rgb(0xef3340))
                    .pl(px(3.0))
                    .flex()
                    .items_center()
                    .child(div().w(px(3.5)).h(px(3.5)).rounded_full().bg(rgb(0xffffff))),
            )
            .child(div().flex_1().w_full().bg(rgb(0xffffff)))
            .into_any_element(),

        CountryCode::Kr => flag_frame()
            .bg(rgb(0xffffff))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(7.0))
                    .h(px(7.0))
                    .rounded_full()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(div().flex_1().w_full().bg(rgb(0xcd2e3a)))
                    .child(div().flex_1().w_full().bg(rgb(0x0047a0))),
            )
            .into_any_element(),

        CountryCode::Gb => flag_frame()
            .bg(rgb(0x012169))
            .child(div().absolute().top(px(5.0)).left_0().right_0().h(px(4.5)).bg(rgb(0xffffff)))
            .child(div().absolute().left(px(8.5)).top_0().bottom_0().w(px(4.5)).bg(rgb(0xffffff)))
            .child(div().absolute().top(px(6.0)).left_0().right_0().h(px(2.5)).bg(rgb(0xc8102e)))
            .child(div().absolute().left(px(9.5)).top_0().bottom_0().w(px(2.5)).bg(rgb(0xc8102e)))
            .into_any_element(),

        CountryCode::De => h_tricolor(0x111111, 0xdd0000, 0xffce00),
        CountryCode::Fr => v_tricolor(0x002395, 0xffffff, 0xed2939),
        CountryCode::It => v_tricolor(0x009246, 0xffffff, 0xce2b37),
        CountryCode::Ng => v_tricolor(0x008751, 0xffffff, 0x008751),
        CountryCode::Nl => h_tricolor(0xae1c28, 0xffffff, 0x21468b),
        CountryCode::Ru => h_tricolor(0xffffff, 0x0039a6, 0xd52b1e),
        CountryCode::Es => h_tricolor(0xaa151b, 0xf1bf00, 0xaa151b),
        CountryCode::Ua => h_bicolor(0x0057b7, 0xffd700),
        CountryCode::Id => h_bicolor(0xce1126, 0xffffff),
        CountryCode::Pl => h_bicolor(0xffffff, 0xdc143c),

        CountryCode::Ar => flag_frame()
            .flex()
            .flex_col()
            .child(div().flex_1().w_full().bg(rgb(0x74acdf)))
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .bg(rgb(0xffffff))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().w(px(3.5)).h(px(3.5)).rounded_full().bg(rgb(0xf6b40e))),
            )
            .child(div().flex_1().w_full().bg(rgb(0x74acdf)))
            .into_any_element(),

        CountryCode::Br => flag_frame()
            .bg(rgb(0x009c3b))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(12.0))
                    .h(px(8.0))
                    .rounded(px(2.0))
                    .bg(rgb(0xffdf00))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().w(px(5.0)).h(px(5.0)).rounded_full().bg(rgb(0x002776))),
            )
            .into_any_element(),

        CountryCode::Sa => flag_frame()
            .bg(rgb(0x006c35))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(1.5))
            .child(div().w(px(10.0)).h(px(2.0)).bg(rgb(0xffffff)))
            .child(div().w(px(8.0)).h(px(1.0)).bg(rgb(0xffffff)))
            .into_any_element(),

        CountryCode::Aq => flag_frame()
            .bg(rgb(0x3a7dce))
            .flex()
            .items_center()
            .justify_center()
            .child(div().w(px(7.0)).h(px(6.0)).rounded_full().bg(rgb(0xffffff)))
            .into_any_element(),

        CountryCode::My => flag_frame()
            .bg(rgb(0xcc0001))
            .flex()
            .flex_col()
            .child(div().h(px(3.0)).w_full().bg(rgb(0xcc0001)))
            .child(div().h(px(3.0)).w_full().bg(rgb(0xffffff)))
            .child(div().h(px(3.0)).w_full().bg(rgb(0xcc0001)))
            .child(div().h(px(3.0)).w_full().bg(rgb(0xffffff)))
            .child(div().flex_1().w_full().bg(rgb(0xcc0001)))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(px(10.0))
                    .h(px(8.0))
                    .bg(rgb(0x010066))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().w(px(3.5)).h(px(3.5)).rounded_full().bg(rgb(0xffcc00))),
            )
            .into_any_element(),

        CountryCode::Vn => flag_frame()
            .bg(rgb(0xda251d))
            .flex()
            .items_center()
            .justify_center()
            .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(rgb(0xffff00)))
            .into_any_element(),

        CountryCode::Th => flag_frame()
            .flex()
            .flex_col()
            .child(div().h(px(2.5)).w_full().bg(rgb(0xa51931)))
            .child(div().h(px(2.0)).w_full().bg(rgb(0xffffff)))
            .child(div().flex_1().w_full().bg(rgb(0x2d2a4a)))
            .child(div().h(px(2.0)).w_full().bg(rgb(0xffffff)))
            .child(div().h(px(2.5)).w_full().bg(rgb(0xa51931)))
            .into_any_element(),

        CountryCode::Ph => flag_frame()
            .flex()
            .flex_col()
            .child(div().flex_1().w_full().bg(rgb(0x0038a8)))
            .child(div().flex_1().w_full().bg(rgb(0xce1126)))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .w(px(7.0))
                    .bg(rgb(0xffffff))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().w(px(3.0)).h(px(3.0)).rounded_full().bg(rgb(0xfcd116))),
            )
            .into_any_element(),

        CountryCode::Ca => flag_frame()
            .flex()
            .flex_row()
            .child(div().w(px(5.5)).h_full().bg(rgb(0xd80621)))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .bg(rgb(0xffffff))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().w(px(4.0)).h(px(4.0)).rounded(px(1.0)).bg(rgb(0xd80621))),
            )
            .child(div().w(px(5.5)).h_full().bg(rgb(0xd80621)))
            .into_any_element(),

        CountryCode::Au => flag_frame()
            .bg(rgb(0x00008b))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(px(9.5))
                    .h(px(7.0))
                    .bg(rgb(0xc8102e))
                    .border_r_1()
                    .border_b_1()
                    .border_color(rgb(0xffffff)),
            )
            .child(
                div()
                    .absolute()
                    .bottom(px(2.5))
                    .right(px(4.0))
                    .w(px(2.5))
                    .h(px(2.5))
                    .rounded_full()
                    .bg(rgb(0xffffff)),
            )
            .into_any_element(),

        CountryCode::Ch => flag_frame()
            .bg(rgb(0xd52b1e))
            .child(div().absolute().top(px(5.5)).left(px(6.5)).w(px(8.0)).h(px(3.0)).bg(rgb(0xffffff)))
            .child(div().absolute().top(px(3.0)).left(px(9.0)).w(px(3.0)).h(px(8.0)).bg(rgb(0xffffff)))
            .into_any_element(),

        CountryCode::In => flag_frame()
            .flex()
            .flex_col()
            .child(div().flex_1().w_full().bg(rgb(0xff9933)))
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .bg(rgb(0xffffff))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().w(px(3.0)).h(px(3.0)).rounded_full().bg(rgb(0x000080))),
            )
            .child(div().flex_1().w_full().bg(rgb(0x138808)))
            .into_any_element(),

        CountryCode::Tr => flag_frame()
            .bg(rgb(0xe30a17))
            .flex()
            .items_center()
            .justify_center()
            .child(div().w(px(5.0)).h(px(5.0)).rounded_full().bg(rgb(0xffffff)))
            .into_any_element(),

        CountryCode::Se => flag_frame()
            .bg(rgb(0x006aa7))
            .child(div().absolute().top(px(5.5)).left_0().right_0().h(px(3.0)).bg(rgb(0xfecc00)))
            .child(div().absolute().left(px(6.5)).top_0().bottom_0().w(px(3.0)).bg(rgb(0xfecc00)))
            .into_any_element(),

        CountryCode::Unknown => flag_frame()
            .bg(rgb(0x4b5563))
            .flex()
            .items_center()
            .justify_center()
            .child(div().w(px(5.0)).h(px(5.0)).rounded_full().bg(rgb(0xd1d5db)))
            .into_any_element(),
    }
}
