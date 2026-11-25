use gpui::*;
use std::collections::HashMap;
use std::time::Instant;

struct AnimValue {
    current: f32,
    from: f32,
    target: f32,
    start: Option<Instant>,
    duration_ms: f32,
}

/// Event-driven 60fps UI animation registry with zero idle overhead.
pub struct UiAnims {
    values: HashMap<&'static str, AnimValue>,
}

impl UiAnims {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Synchronizes an animated value with a state-driven target.
    /// Initializes silently on first call so initial render never jumps.
    pub fn sync(&mut self, key: &'static str, target: f32, duration_ms: f32) -> f32 {
        match self.values.get_mut(key) {
            Some(v) => {
                if (v.target - target).abs() > 0.001 {
                    v.from = v.current;
                    v.target = target;
                    v.start = Some(Instant::now());
                    v.duration_ms = duration_ms.max(1.0);
                }
                v.current
            }
            None => {
                self.values.insert(
                    key,
                    AnimValue {
                        current: target,
                        from: target,
                        target,
                        start: None,
                        duration_ms: duration_ms.max(1.0),
                    },
                );
                target
            }
        }
    }

    /// Explicitly starts a transition from `from` to `target` (e.g. page/modal entrance).
    pub fn trigger(&mut self, key: &'static str, from: f32, target: f32, duration_ms: f32) {
        self.values.insert(
            key,
            AnimValue {
                current: from,
                from,
                target,
                start: Some(Instant::now()),
                duration_ms: duration_ms.max(1.0),
            },
        );
    }

    /// Reads the current animated value without mutating.
    pub fn get(&self, key: &'static str, default: f32) -> f32 {
        self.values.get(key).map(|v| v.current).unwrap_or(default)
    }

    /// Advances all running animations. Returns true if at least one animation is still in flight.
    pub fn tick(&mut self) -> bool {
        let mut any_active = false;
        for v in self.values.values_mut() {
            if let Some(start) = v.start {
                let t = (start.elapsed().as_secs_f32() * 1000.0 / v.duration_ms).clamp(0.0, 1.0);
                // Cubic ease-out: snappy initial response, smooth deceleration
                let eased = 1.0 - (1.0 - t).powi(3);
                v.current = v.from + (v.target - v.from) * eased;
                if t < 1.0 {
                    any_active = true;
                } else {
                    v.current = v.target;
                    v.start = None;
                }
            }
        }
        any_active
    }
}

/// Smoothly interpolates between two 0xRRGGBB hex colors.
pub fn lerp_rgb(from_hex: u32, to_hex: u32, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    let r0 = ((from_hex >> 16) & 0xff) as f32;
    let g0 = ((from_hex >> 8) & 0xff) as f32;
    let b0 = (from_hex & 0xff) as f32;

    let r1 = ((to_hex >> 16) & 0xff) as f32;
    let g1 = ((to_hex >> 8) & 0xff) as f32;
    let b1 = (to_hex & 0xff) as f32;

    let r = (r0 + (r1 - r0) * t).round() as u32;
    let g = (g0 + (g1 - g0) * t).round() as u32;
    let b = (b0 + (b1 - b0) * t).round() as u32;
    rgb((r << 16) | (g << 8) | b)
}

/// Animated Win11 Fluent × Braun pill toggle switch (38×21px).
/// Features smooth track color blending, sliding thumb, and subtle mid-flight stretch.
pub fn render_pill_toggle(progress: f32, active_hex: u32) -> Div {
    let p = progress.clamp(0.0, 1.0);
    let track_bg = lerp_rgb(0xcac8bf, active_hex, p);
    let stretch = (p * std::f32::consts::PI).sin() * 3.5;
    let thumb_w = 16.0 + stretch;
    let thumb_left = 2.5 + p * (38.0 - 5.0 - thumb_w);

    div()
        .relative()
        .w(px(38.0))
        .h(px(21.0))
        .rounded_full()
        .bg(track_bg)
        .flex_shrink_0()
        .child(
            div()
                .absolute()
                .top(px(2.5))
                .left(px(thumb_left))
                .w(px(thumb_w))
                .h(px(16.0))
                .rounded_full()
                .bg(rgb(0xffffff))
                .shadow_sm(),
        )
}

/// Compact animated toggle switch (30×17px) for sidebar quick controls.
pub fn render_mini_toggle(progress: f32, active_hex: u32) -> Div {
    let p = progress.clamp(0.0, 1.0);
    let track_bg = lerp_rgb(0xcac8bf, active_hex, p);
    let stretch = (p * std::f32::consts::PI).sin() * 2.5;
    let thumb_w = 13.0 + stretch;
    let thumb_left = 2.0 + p * (30.0 - 4.0 - thumb_w);

    div()
        .relative()
        .w(px(30.0))
        .h(px(17.0))
        .rounded_full()
        .bg(track_bg)
        .flex_shrink_0()
        .child(
            div()
                .absolute()
                .top(px(2.0))
                .left(px(thumb_left))
                .w(px(thumb_w))
                .h(px(13.0))
                .rounded_full()
                .bg(rgb(0xffffff)),
        )
}

/// 4-bar Braun / Teenage Engineering equalizer wave spinner for unmistakable loading feedback.
pub fn render_loading_bars(phase: f32, color: Rgba) -> Div {
    let bar = |idx: usize| {
        let wave = ((phase + idx as f32 * 0.95).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
        let h = 4.0 + wave * 9.0;
        div()
            .w(px(2.5))
            .h(px(h))
            .rounded_full()
            .bg(color)
    };
    div()
        .h(px(13.0))
        .flex()
        .items_center()
        .gap(px(2.0))
        .flex_shrink_0()
        .child(bar(0))
        .child(bar(1))
        .child(bar(2))
        .child(bar(3))
}

/// Sweeping translucent light band overlay across a loading button (parent must be `.relative().overflow_hidden()`).
pub fn render_shimmer_band(phase: f32, container_w: f32) -> Div {
    let band_w = 52.0;
    let cycle = (phase * 0.24) % 1.0;
    let x = -band_w + cycle * (container_w + band_w * 2.0);
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(px(x))
        .w(px(band_w))
        .bg(rgba(0xffffff38))
}
