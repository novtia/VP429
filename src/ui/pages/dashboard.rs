use crate::config::RotationMode;
use crate::ui::anim::{lerp_rgb, render_loading_bars, render_pill_toggle};
use crate::ui::flags::{detect_country, render_flag};
use crate::ui::icons::icon;
use crate::ui::{theme, MainView};
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::f32::consts::PI;

#[derive(Clone, Copy)]
struct StageGeom {
    w: f32,
    h: f32,
    cx: f32,
    cy: f32,
    r_cut: f32,
    r_dial: f32,
    x_top_l: f32,
    x_top_r: f32,
    x_bot_l: f32,
    x_bot_r: f32,
    y_left_t: f32,
    y_left_b: f32,
    y_right_t: f32,
    y_right_b: f32,
}

impl StageGeom {
    fn compute(w: f32, h: f32) -> Self {
        let cx = w * 0.5;
        let cy = h * 0.5;
        let r_cut = (h * 0.33).min(w * 0.228).max(108.0);
        let r_dial = r_cut - 11.0;
        let gap = 10.0;

        let x_top_l = cx + 26.0;
        let x_top_r = x_top_l + gap;
        let x_bot_l = cx - 36.0;
        let x_bot_r = x_bot_l + gap;
        let y_left_t = cy - 14.0;
        let y_left_b = y_left_t + gap;
        let y_right_t = cy + 14.0;
        let y_right_b = y_right_t + gap;

        Self {
            w,
            h,
            cx,
            cy,
            r_cut,
            r_dial,
            x_top_l,
            x_top_r,
            x_bot_l,
            x_bot_r,
            y_left_t,
            y_left_b,
            y_right_t,
            y_right_b,
        }
    }
}

struct FilletPts {
    lx: f32,
    ly: f32,
    tx: f32,
    ty: f32,
}

fn h_line_circle_fillet(
    cx: f32,
    cy: f32,
    r_cut: f32,
    f: f32,
    y_line: f32,
    side_x: f32,
    side_y: f32,
) -> FilletPts {
    let fcy = y_line + side_y * f;
    let dy = fcy - cy;
    let dist = r_cut + f;
    let dx = (dist * dist - dy * dy).max(1.0).sqrt();
    let fcx = cx + side_x * dx;
    let t = r_cut / dist;
    FilletPts {
        lx: fcx,
        ly: y_line,
        tx: cx + (fcx - cx) * t,
        ty: cy + (fcy - cy) * t,
    }
}

fn v_line_circle_fillet(
    cx: f32,
    cy: f32,
    r_cut: f32,
    f: f32,
    x_line: f32,
    side_x: f32,
    side_y: f32,
) -> FilletPts {
    let fcx = x_line + side_x * f;
    let dx = fcx - cx;
    let dist = r_cut + f;
    let dy = (dist * dist - dx * dx).max(1.0).sqrt();
    let fcy = cy + side_y * dy;
    let t = r_cut / dist;
    FilletPts {
        lx: x_line,
        ly: fcy,
        tx: cx + (fcx - cx) * t,
        ty: cy + (fcy - cy) * t,
    }
}

fn build_tl_panel(mut b: PathBuilder, ox: f32, oy: f32, g: StageGeom) -> PathBuilder {
    let r = 12.0;
    let f = 12.0;
    let (x0, y0, x1, y1) = (1.0, 1.0, g.x_top_l, g.y_left_t);
    let fv = v_line_circle_fillet(g.cx, g.cy, g.r_cut, f, x1, -1.0, -1.0);
    let fh = h_line_circle_fillet(g.cx, g.cy, g.r_cut, f, y1, -1.0, -1.0);
    let pt = |x: f32, y: f32| point(px(ox + x), px(oy + y));

    b.move_to(pt(x0 + r, y0));
    b.line_to(pt(x1 - r, y0));
    b.arc_to(point(px(r), px(r)), px(0.0), false, true, pt(x1, y0 + r));
    b.line_to(pt(fv.lx, fv.ly));
    b.arc_to(point(px(f), px(f)), px(0.0), false, true, pt(fv.tx, fv.ty));
    b.arc_to(
        point(px(g.r_cut), px(g.r_cut)),
        px(0.0),
        false,
        false,
        pt(fh.tx, fh.ty),
    );
    b.arc_to(point(px(f), px(f)), px(0.0), false, true, pt(fh.lx, fh.ly));
    b.line_to(pt(x0 + r, y1));
    b.arc_to(point(px(r), px(r)), px(0.0), false, true, pt(x0, y1 - r));
    b.line_to(pt(x0, y0 + r));
    b.arc_to(point(px(r), px(r)), px(0.0), false, true, pt(x0 + r, y0));
    b.close();
    b
}

fn build_tr_panel(mut b: PathBuilder, ox: f32, oy: f32, g: StageGeom) -> PathBuilder {
    let r = 12.0;
    let f = 12.0;
    let (x0, y0, x1, y1) = (g.x_top_r, 1.0, g.w - 1.0, g.y_right_t);
    let fh = h_line_circle_fillet(g.cx, g.cy, g.r_cut, f, y1, 1.0, -1.0);
    let fv = v_line_circle_fillet(g.cx, g.cy, g.r_cut, f, x0, 1.0, -1.0);
    let pt = |x: f32, y: f32| point(px(ox + x), px(oy + y));

    b.move_to(pt(x0 + r, y0));
    b.line_to(pt(x1 - r, y0));
    b.arc_to(point(px(r), px(r)), px(0.0), false, true, pt(x1, y0 + r));
    b.line_to(pt(x1, y1 - r));
    b.arc_to(point(px(r), px(r)), px(0.0), false, true, pt(x1 - r, y1));
    b.line_to(pt(fh.lx, fh.ly));
    b.arc_to(point(px(f), px(f)), px(0.0), false, true, pt(fh.tx, fh.ty));
    b.arc_to(
        point(px(g.r_cut), px(g.r_cut)),
        px(0.0),
        false,
        false,
        pt(fv.tx, fv.ty),
    );
    b.arc_to(point(px(f), px(f)), px(0.0), false, true, pt(fv.lx, fv.ly));
    b.line_to(pt(x0, y0 + r));
    b.arc_to(point(px(r), px(r)), px(0.0), false, true, pt(x0 + r, y0));
    b.close();
    b
}

fn build_bl_panel(mut b: PathBuilder, ox: f32, oy: f32, g: StageGeom) -> PathBuilder {
    let r = 12.0;
    let f = 12.0;
    let (x0, y0, x1, y1) = (1.0, g.y_left_b, g.x_bot_l, g.h - 1.0);
    let fh = h_line_circle_fillet(g.cx, g.cy, g.r_cut, f, y0, -1.0, 1.0);
    let fv = v_line_circle_fillet(g.cx, g.cy, g.r_cut, f, x1, -1.0, 1.0);
    let pt = |x: f32, y: f32| point(px(ox + x), px(oy + y));

    b.move_to(pt(x0 + r, y0));
    b.line_to(pt(fh.lx, fh.ly));
    b.arc_to(point(px(f), px(f)), px(0.0), false, true, pt(fh.tx, fh.ty));
    b.arc_to(
        point(px(g.r_cut), px(g.r_cut)),
        px(0.0),
        false,
        false,
        pt(fv.tx, fv.ty),
    );
    b.arc_to(point(px(f), px(f)), px(0.0), false, true, pt(fv.lx, fv.ly));
    b.line_to(pt(x1, y1 - r));
    b.arc_to(point(px(r), px(r)), px(0.0), false, true, pt(x1 - r, y1));
    b.line_to(pt(x0 + r, y1));
    b.arc_to(point(px(r), px(r)), px(0.0), false, true, pt(x0, y1 - r));
    b.line_to(pt(x0, y0 + r));
    b.arc_to(point(px(r), px(r)), px(0.0), false, true, pt(x0 + r, y0));
    b.close();
    b
}

fn build_br_panel(mut b: PathBuilder, ox: f32, oy: f32, g: StageGeom) -> PathBuilder {
    let r = 12.0;
    let f = 12.0;
    let (x0, y0, x1, y1) = (g.x_bot_r, g.y_right_b, g.w - 1.0, g.h - 1.0);
    let fv = v_line_circle_fillet(g.cx, g.cy, g.r_cut, f, x0, 1.0, 1.0);
    let fh = h_line_circle_fillet(g.cx, g.cy, g.r_cut, f, y0, 1.0, 1.0);
    let pt = |x: f32, y: f32| point(px(ox + x), px(oy + y));

    b.move_to(pt(fh.lx, fh.ly));
    b.line_to(pt(x1 - r, y0));
    b.arc_to(point(px(r), px(r)), px(0.0), false, true, pt(x1, y0 + r));
    b.line_to(pt(x1, y1 - r));
    b.arc_to(point(px(r), px(r)), px(0.0), false, true, pt(x1 - r, y1));
    b.line_to(pt(x0 + r, y1));
    b.arc_to(point(px(r), px(r)), px(0.0), false, true, pt(x0, y1 - r));
    b.line_to(pt(fv.lx, fv.ly));
    b.arc_to(point(px(f), px(f)), px(0.0), false, true, pt(fv.tx, fv.ty));
    b.arc_to(
        point(px(g.r_cut), px(g.r_cut)),
        px(0.0),
        false,
        false,
        pt(fh.tx, fh.ty),
    );
    b.arc_to(point(px(f), px(f)), px(0.0), false, true, pt(fh.lx, fh.ly));
    b.close();
    b
}

fn paint_arc(
    window: &mut Window,
    cx: f32,
    cy: f32,
    radius: f32,
    start_ang: f32,
    end_ang: f32,
    width: f32,
    color: Rgba,
) {
    let sweep = end_ang - start_ang;
    if sweep.abs() < 0.005 {
        return;
    }
    let steps = ((sweep.abs() / 0.08).ceil() as usize).clamp(4, 96);
    let mut b = PathBuilder::stroke(px(width));
    let x0 = cx + start_ang.cos() * radius;
    let y0 = cy + start_ang.sin() * radius;
    b.move_to(point(px(x0), px(y0)));
    for i in 1..=steps {
        let a = start_ang + sweep * (i as f32 / steps as f32);
        b.line_to(point(px(cx + a.cos() * radius), px(cy + a.sin() * radius)));
    }
    if let Ok(path) = b.build() {
        window.paint_path(path, color);
    }
}

fn paint_speed_dial(
    bounds: Bounds<Pixels>,
    down_norm: f32,
    up_norm: f32,
    running: bool,
    window: &mut Window,
) {
    let ox: f32 = bounds.origin.x.into();
    let oy: f32 = bounds.origin.y.into();
    let w: f32 = bounds.size.width.into();
    let h: f32 = bounds.size.height.into();
    let cx = ox + w * 0.5;
    let cy = oy + h * 0.5;
    let r = w.min(h) * 0.5;

    // 236° sweep (152° -> 388°) leaves a clean open bottom mouth for the single pager pill
    let start_ang = 152.0 * PI / 180.0;
    let end_ang = 388.0 * PI / 180.0;
    let sweep = end_ang - start_ang;

    let r_outer = r * 0.83;
    let r_inner = r * 0.715;

    // 1. Subtle outer hairline ring
    paint_arc(window, cx, cy, r * 0.935, 0.0, PI * 2.0, 1.0, rgba(0xffffff12));

    // 2. Recessed background tracks
    paint_arc(window, cx, cy, r_outer, start_ang, end_ang, 7.0, rgb(0x2a2c29));
    paint_arc(window, cx, cy, r_inner, start_ang, end_ang, 3.5, rgb(0x232522));

    // 3. Precision tick marks (40 steps, 5 major logarithmic stops)
    let total_ticks = 40;
    for i in 0..=total_ticks {
        let t = i as f32 / total_ticks as f32;
        let ang = start_ang + t * sweep;
        let is_major = i % 10 == 0;
        let is_mid = i % 5 == 0 && !is_major;
        let r1 = if is_major {
            r * 0.91
        } else if is_mid {
            r * 0.90
        } else {
            r * 0.89
        };
        let r2 = if is_major {
            r * 0.86
        } else if is_mid {
            r * 0.87
        } else {
            r * 0.875
        };
        let mut tb = PathBuilder::stroke(px(if is_major { 1.6 } else { 0.9 }));
        tb.move_to(point(px(cx + ang.cos() * r1), px(cy + ang.sin() * r1)));
        tb.line_to(point(px(cx + ang.cos() * r2), px(cy + ang.sin() * r2)));
        if let Ok(path) = tb.build() {
            window.paint_path(
                path,
                if is_major {
                    rgb(0x9c9a91)
                } else {
                    rgb(0x4a4c47)
                },
            );
        }
    }

    let dn = if running { down_norm.clamp(0.0, 1.0) } else { 0.0 };
    let un = if running { up_norm.clamp(0.0, 1.0) } else { 0.0 };

    // 4. Active Download Arc (LCD Green)
    if dn > 0.005 {
        paint_arc(
            window,
            cx,
            cy,
            r_outer,
            start_ang,
            start_ang + dn * sweep,
            7.0,
            theme::lcd_green(),
        );
    }

    // 5. Active Upload Arc (LCD Orange)
    if un > 0.005 {
        paint_arc(
            window,
            cx,
            cy,
            r_inner,
            start_ang,
            start_ang + un * sweep,
            3.5,
            theme::lcd_orange(),
        );
    }

    // 6. Braun Orange Precision Indicator Tip
    let tip_ang = start_ang + dn * sweep;
    let mut nb = PathBuilder::stroke(px(2.4));
    nb.move_to(point(
        px(cx + tip_ang.cos() * (r_outer - 6.0)),
        px(cy + tip_ang.sin() * (r_outer - 6.0)),
    ));
    nb.line_to(point(
        px(cx + tip_ang.cos() * (r_outer + 7.0)),
        px(cy + tip_ang.sin() * (r_outer + 7.0)),
    ));
    if let Ok(path) = nb.build() {
        window.paint_path(
            path,
            if running {
                theme::braun_orange()
            } else {
                rgb(0x555752)
            },
        );
    }
}

/// First-person 3D sports-car tunnel acceleration inside the circular dial well.
/// Radial speed streaks shoot outward from the vanishing point, accelerating exponentially
/// and fading out cleanly as the live speedometer emerges.
fn paint_warp_tunnel(bounds: Bounds<Pixels>, launch_p: f32, window: &mut Window) {
    let p = launch_p.clamp(0.0, 1.0);
    let master_alpha = if p < 0.12 {
        p / 0.12
    } else if p < 0.64 {
        1.0
    } else {
        (1.0 - (p - 0.64) / 0.36).max(0.0)
    };
    if master_alpha <= 0.005 {
        return;
    }

    let ox: f32 = bounds.origin.x.into();
    let oy: f32 = bounds.origin.y.into();
    let w: f32 = bounds.size.width.into();
    let h: f32 = bounds.size.height.into();
    let cx = ox + w * 0.5;
    let cy = oy + h * 0.5;
    let max_r = w.min(h) * 0.52;

    // Integrated exponential distance traveled so particles accelerate smoothly forward
    let travel = p * 0.7 + p.powf(3.0) * 4.4;
    let accel = p.powf(2.2);
    let fov = max_r * 0.25;

    const STREAKS: usize = 60;
    for i in 0..STREAKS {
        let fi = i as f32;
        // Deterministic pseudo-random per-streak parameters
        let seed1 = ((fi * 12.9898).sin() * 43758.5453).fract().abs();
        let seed2 = ((fi * 78.233).sin() * 24634.6345).fract().abs();
        let speed_mult = 0.72 + seed1 * 0.68;
        let base_ang = (fi / STREAKS as f32) * PI * 2.0 + (seed2 - 0.5) * 0.24;

        // Depth z wraps cleanly through [0.06 .. 1.0] moving toward viewer
        let z_cycle = 1.0 - ((seed2 + travel * speed_mult) % 0.94);
        let z_head = z_cycle.max(0.06);
        let tail_delta = 0.08 + accel * 0.34;
        let z_tail = (z_head + tail_delta).min(1.04);

        let r_head = (fov / z_head).min(max_r);
        let r_tail = (fov / z_tail).min(max_r * 0.98);
        if r_head < max_r * 0.11 || r_head <= r_tail + 1.5 {
            continue;
        }

        let depth_fade = ((r_head - max_r * 0.11) / (max_r * 0.42)).clamp(0.0, 1.0);
        let alpha = (master_alpha * depth_fade).clamp(0.0, 1.0);
        if alpha < 0.02 {
            continue;
        }

        let (cr, cg, cb) = if i % 5 == 0 {
            (0xff_u32, 0x6a_u32, 0x3d_u32) // Braun orange
        } else if i % 3 == 0 {
            (0x5b_u32, 0xd6_u32, 0x8a_u32) // LCD green
        } else {
            (0xe8_u32, 0xe6_u32, 0xde_u32) // Warm white
        };
        let a_byte = (alpha * 255.0).round() as u32;
        let col = rgba((cr << 24) | (cg << 16) | (cb << 8) | a_byte);

        let thickness = (1.1 + seed1 * 1.4) * (0.45 + depth_fade * 1.15);
        let cos = base_ang.cos();
        let sin = base_ang.sin();
        let mut lb = PathBuilder::stroke(px(thickness));
        lb.move_to(point(px(cx + cos * r_tail), px(cy + sin * r_tail)));
        lb.line_to(point(px(cx + cos * r_head), px(cy + sin * r_head)));
        if let Ok(path) = lb.build() {
            window.paint_path(path, col);
        }
    }
}

impl MainView {
    pub(in crate::ui) fn render_dashboard_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let snap = self.engine.scheduler.snapshot();
        let tel = self.engine.telemetry_snapshot();
        let cfg = self.engine.config.read().unwrap().clone();
        let running = self.engine.is_running();

        let format_rate = |bps: u64| -> String {
            if bps >= 1024 * 1024 {
                format!("{:.2} MB/s", bps as f64 / (1024.0 * 1024.0))
            } else if bps >= 1024 {
                format!("{:.1} KB/s", bps as f64 / 1024.0)
            } else {
                format!("{} B/s", bps)
            }
        };
        let format_rate_parts = |bps: u64| -> (String, &'static str) {
            if bps >= 1024 * 1024 {
                let mb = bps as f64 / (1024.0 * 1024.0);
                (
                    if mb >= 10.0 {
                        format!("{:.1}", mb)
                    } else {
                        format!("{:.2}", mb)
                    },
                    "MB/s",
                )
            } else if bps >= 1024 {
                (format!("{:.1}", bps as f64 / 1024.0), "KB/s")
            } else {
                (format!("{}", bps), "B/s")
            }
        };
        let format_bytes = |bytes: u64| -> (String, &'static str) {
            if bytes >= 1024 * 1024 * 1024 {
                (
                    format!("{:.2}", bytes as f64 / (1024.0 * 1024.0 * 1024.0)),
                    "GB",
                )
            } else if bytes >= 1024 * 1024 {
                (format!("{:.1}", bytes as f64 / (1024.0 * 1024.0)), "MB")
            } else {
                (format!("{:.1}", bytes as f64 / 1024.0), "KB")
            }
        };
        let format_uptime = |secs: u64| -> String {
            let h = secs / 3600;
            let m = (secs % 3600) / 60;
            let s = secs % 60;
            format!("{:02}:{:02}:{:02}", h, m, s)
        };

        let active_name = snap
            .active_node
            .clone()
            .unwrap_or_else(|| "未加载节点".to_string());
        let active_country = detect_country(&active_name);
        let active_ms = snap
            .nodes
            .iter()
            .find(|n| n.is_active)
            .and_then(|n| n.latency_ms)
            .map(|m| {
                if m == 0 {
                    "超时".to_string()
                } else {
                    format!("{}ms", m)
                }
            })
            .unwrap_or_else(|| "--ms".to_string());

        // Full-height sculpted stage (no bottom black bar!)
        let sidebar_w = self.anims.get("sidebar_w", 192.0);
        let stage_w = (860.0 - sidebar_w - 32.0).max(580.0);
        let stage_h = 492.0;
        let g = StageGeom::compute(stage_w, stage_h);

        let left_body_w = (g.cx - g.r_cut).max(168.0);
        let right_body_x = g.cx + g.r_cut;
        let right_body_w = (g.w - right_body_x).max(168.0);
        let top_wing_h = (g.cy - g.r_cut).max(76.0);
        let bot_wing_y = g.cy + g.r_cut;
        let bot_wing_h = (g.h - bot_wing_y).max(76.0);

        let dial_d = g.r_dial * 2.0;
        let dial_left = g.cx - g.r_dial;
        let dial_top = g.cy - g.r_dial;
        let inner_well_d = dial_d - 18.0;
        let well_r = inner_well_d * 0.5;

        let dial_down_norm = self.anims.get("dial_down_norm", 0.0);
        let dial_up_norm = self.anims.get("dial_up_norm", 0.0);
        let picker_slide = self.anims.get("picker_slide", 0.0).clamp(0.0, 1.0);
        let dial_page_p = self.anims.get("dial_page_p", 0.0).clamp(0.0, 1.0);

        let launch_p = self
            .dial_launch_started
            .map(|st| (st.elapsed().as_secs_f32() / 1.55).clamp(0.0, 1.0));
        let is_launching = launch_p.is_some();

        let p_sys = self
            .anims
            .get("tog_sysproxy", if cfg.enable_system_proxy { 1.0 } else { 0.0 });
        let p_tun = self
            .anims
            .get("tog_tun", if cfg.enable_tun { 1.0 } else { 0.0 });
        let p_out_rule = self
            .anims
            .get("out_rule", if tel.outbound_mode == "rule" { 1.0 } else { 0.0 });
        let p_out_global = self.anims.get(
            "out_global",
            if tel.outbound_mode == "global" { 1.0 } else { 0.0 },
        );
        let p_out_direct = self.anims.get(
            "out_direct",
            if tel.outbound_mode == "direct" { 1.0 } else { 0.0 },
        );
        let p_rot_429 = self.anims.get(
            "rot_429",
            if cfg.rotation_mode == RotationMode::On429Only {
                1.0
            } else {
                0.0
            },
        );
        let p_rot_req = self.anims.get(
            "rot_req",
            if cfg.rotation_mode == RotationMode::RoundRobinPerRequest {
                1.0
            } else {
                0.0
            },
        );
        let p_rot_int = self.anims.get(
            "rot_int",
            if matches!(cfg.rotation_mode, RotationMode::RoundRobinInterval { .. }) {
                1.0
            } else {
                0.0
            },
        );

        let (main_val, main_unit) = format_rate_parts(if running { tel.down_bps } else { 0 });
        let (up_val, up_unit) = format_bytes(tel.total_up);
        let (dn_val, dn_unit) = format_bytes(tel.total_down);
        let total_sum = (tel.total_up + tel.total_down).max(1) as f32;
        let up_ratio = (tel.total_up as f32 / total_sum).clamp(0.05, 0.95);

        let exit_country = tel
            .exit_country_code
            .as_deref()
            .map(detect_country)
            .unwrap_or(active_country);
        let exit_ip_text = if tel.checking_ip {
            "检测中...".to_string()
        } else {
            tel.exit_ip
                .clone()
                .unwrap_or_else(|| "点击探测出口 IP".to_string())
        };

        // Mini 20-bar sparkline inside lower dial window
        let hist_len = tel.speed_history.len();
        let spark_slice = if hist_len > 20 {
            &tel.speed_history[hist_len - 20..]
        } else {
            &tel.speed_history[..]
        };
        let max_rate = spark_slice
            .iter()
            .map(|(u, d)| (*u).max(*d))
            .max()
            .unwrap_or(1024)
            .max(1024) as f32;

        let mini_spark_bars = spark_slice.iter().map(|(u, d)| {
            let total = if running { (*u + *d) as f32 } else { 0.0 };
            let h_px = ((total / (max_rate * 1.15)) * 15.0).clamp(2.0, 15.0);
            let is_up_dom = *u > *d;
            div()
                .flex_1()
                .h_full()
                .flex()
                .items_end()
                .child(
                    div()
                        .w_full()
                        .h(px(h_px))
                        .rounded_t(px(1.0))
                        .bg(if total <= 1.0 {
                            rgb(0x2c2e2a)
                        } else if is_up_dom {
                            theme::lcd_orange()
                        } else {
                            theme::lcd_green()
                        }),
                )
        });

        let radio_row = |id: &'static str, label: &'static str, mode_val: &'static str, progress: f32| {
            let active = progress > 0.5;
            let dot_sz = (progress * 7.0).clamp(0.0, 7.0);
            div()
                .id(id)
                .py(px(3.0))
                .px_1p5()
                .rounded(px(4.0))
                .cursor_pointer()
                .hover(|s| s.bg(theme::row_hover_bg()))
                .active(|s| s.opacity(0.75))
                .flex()
                .items_center()
                .gap_2()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        let eng = this.engine.clone();
                        this.rt.spawn(async move {
                            eng.set_outbound_mode(mode_val).await;
                        });
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .w(px(13.0))
                        .h(px(13.0))
                        .rounded_full()
                        .border_1()
                        .border_color(lerp_rgb(0xb8b5ac, 0xe84c1e, progress))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(dot_sz > 0.5, |el| {
                            el.child(
                                div()
                                    .w(px(dot_sz))
                                    .h(px(dot_sz))
                                    .rounded_full()
                                    .bg(theme::braun_orange()),
                            )
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(if active {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .text_color(if active {
                            theme::ink_primary()
                        } else {
                            theme::ink_secondary()
                        })
                        .child(label),
                )
        };

        let rot_pill = |id: &'static str, label: &'static str, target: RotationMode, progress: f32| {
            div()
                .id(id)
                .flex_1()
                .py(px(3.5))
                .px(px(2.0))
                .rounded(px(3.0))
                .cursor_pointer()
                .hover(|s| s.opacity(0.88))
                .active(|s| s.opacity(0.75))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(10.5))
                .whitespace_nowrap()
                .bg(lerp_rgb(0xe3e1d9, 0x1a1a18, progress))
                .text_color(lerp_rgb(0x5b5a54, 0xffffff, progress))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.set_rotation_mode(target.clone(), cx);
                    }),
                )
                .child(label)
        };

        // Background canvas: 4 asymmetric sculpted concave-cutout panels + radial groove
        let bg_canvas = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let ox: f32 = bounds.origin.x.into();
                let oy: f32 = bounds.origin.y.into();
                let bw: f32 = bounds.size.width.into();
                let bh: f32 = bounds.size.height.into();
                let cg = StageGeom::compute(bw, bh);

                let mut groove = PathBuilder::stroke(px(1.0)).dash_array(&[px(3.0), px(5.0)]);
                let steps = 72;
                for i in 0..=steps {
                    let a = (i as f32 / steps as f32) * PI * 2.0;
                    let p = point(
                        px(ox + cg.cx + a.cos() * cg.r_cut),
                        px(oy + cg.cy + a.sin() * cg.r_cut),
                    );
                    if i == 0 {
                        groove.move_to(p);
                    } else {
                        groove.line_to(p);
                    }
                }
                if let Ok(path) = groove.build() {
                    window.paint_path(path, rgb(0xd4d2c9));
                }

                let builders: [fn(PathBuilder, f32, f32, StageGeom) -> PathBuilder; 4] = [
                    build_tl_panel,
                    build_tr_panel,
                    build_bl_panel,
                    build_br_panel,
                ];
                for build_fn in builders {
                    if let Ok(fill_path) = build_fn(PathBuilder::fill(), ox, oy, cg).build() {
                        window.paint_path(fill_path, theme::card_bg());
                    }
                    if let Ok(stroke_path) =
                        build_fn(PathBuilder::stroke(px(1.25)), ox, oy, cg).build()
                    {
                        window.paint_path(stroke_path, theme::border_color());
                    }
                }
            },
        )
        .size_full();

        // Scale labels positioned inside the 236° circular dial
        let dial_labels = ["0", "100K", "1M", "10M", "100M"]
            .into_iter()
            .enumerate()
            .map(|(idx, lbl)| {
                let t = idx as f32 / 4.0;
                let ang = (152.0 + t * 236.0) * PI / 180.0;
                let rl = well_r * 0.595;
                let lx = well_r + ang.cos() * rl - 16.0;
                let ly = well_r + ang.sin() * rl - 6.0;
                div()
                    .absolute()
                    .left(px(lx))
                    .top(px(ly))
                    .w(px(32.0))
                    .flex()
                    .justify_center()
                    .text_size(px(9.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::lcd_muted())
                    .child(lbl)
            });

        // Reusable recessed digital-watch LCD node selector window (inside upper dial aperture)
        let render_lcd_node_window = |id: &'static str, cx: &mut Context<Self>| {
            let dot_color = if running {
                theme::lcd_green()
            } else {
                theme::lcd_muted()
            };
            div()
                .id(id)
                .px_2p5()
                .py(px(3.5))
                .max_w(px((inner_well_d * 0.66).min(176.0)))
                .rounded(px(5.0))
                .bg(rgb(0x111311))
                .border_1()
                .border_color(rgb(0x30332e))
                .hover(|s| s.border_color(theme::braun_orange()))
                .active(|s| s.opacity(0.85))
                .cursor_pointer()
                .flex()
                .items_center()
                .gap_1p5()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.toggle_node_picker(true, cx);
                    }),
                )
                .child(
                    div()
                        .w(px(5.5))
                        .h(px(5.5))
                        .rounded_full()
                        .flex_shrink_0()
                        .bg(dot_color),
                )
                .when(snap.active_node.is_some(), |el| {
                    el.child(render_flag(active_country))
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(10.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::lcd_text())
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(active_name.clone()),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(theme::lcd_green())
                        .flex_shrink_0()
                        .whitespace_nowrap()
                        .child(active_ms.clone()),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .text_color(theme::lcd_muted())
                        .flex_shrink_0()
                        .child("▾"),
                )
        };

        // Circular Node Picker rows (dark LCD style inside circular dial well)
        let picker_visible = self.show_node_picker || picker_slide > 0.01;
        let picker_rows: Vec<_> = if picker_visible {
            snap.nodes
                .iter()
                .enumerate()
                .map(|(idx, n)| {
                    let name_click = n.name.clone();
                    let country = detect_country(&n.name);
                    let (lat_text, lat_color) = match n.latency_ms {
                        Some(0) => ("超时".to_string(), theme::lcd_orange()),
                        Some(ms) if ms < 150 => (format!("{}ms", ms), theme::lcd_green()),
                        Some(ms) => (format!("{}ms", ms), theme::lcd_muted()),
                        None => ("--".to_string(), theme::lcd_muted()),
                    };
                    div()
                        .id(("circ-picker-node", idx))
                        .px_2p5()
                        .py_1p5()
                        .rounded(px(5.0))
                        .border_1()
                        .border_color(if n.is_active {
                            theme::braun_orange()
                        } else {
                            rgba(0x00000000)
                        })
                        .bg(if n.is_active {
                            rgb(0x2a2c28)
                        } else {
                            rgb(0x1e201d)
                        })
                        .hover(|s| s.bg(rgb(0x272925)))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_1p5()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                let eng = this.engine.clone();
                                let name = name_click.clone();
                                this.rt.spawn(async move {
                                    let _ = eng.select_node_manual(&name).await;
                                });
                                this.toggle_node_picker(false, cx);
                            }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .overflow_hidden()
                                .child(
                                    div()
                                        .w(px(5.5))
                                        .h(px(5.5))
                                        .rounded_full()
                                        .flex_shrink_0()
                                        .bg(if n.is_active {
                                            theme::braun_orange()
                                        } else if n.cooldown_remaining_secs > 0 {
                                            theme::status_amber()
                                        } else {
                                            theme::lcd_green()
                                        }),
                                )
                                .child(render_flag(country))
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_weight(if n.is_active {
                                            FontWeight::SEMIBOLD
                                        } else {
                                            FontWeight::NORMAL
                                        })
                                        .text_color(if n.is_active {
                                            theme::white()
                                        } else {
                                            theme::lcd_text()
                                        })
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .child(n.name.clone()),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .flex_shrink_0()
                                .when(n.cooldown_remaining_secs > 0, |el| {
                                    el.child(
                                        div()
                                            .text_size(px(10.0))
                                            .text_color(theme::lcd_orange())
                                            .child(format!("{}s", n.cooldown_remaining_secs)),
                                    )
                                })
                                .child(
                                    div()
                                        .text_size(px(10.5))
                                        .text_color(lat_color)
                                        .whitespace_nowrap()
                                        .child(lat_text),
                                ),
                        )
                })
                .collect()
        } else {
            Vec::new()
        };

        let mode_badge = match tel.outbound_mode.as_str() {
            "global" => "GLOBAL",
            "direct" => "DIRECT",
            _ => "RULE",
        };
        let uptime_text = if running {
            format_uptime(tel.uptime_secs)
        } else {
            "--:--:--".to_string()
        };

        // Power button fade while accelerating into the 3D tunnel
        let power_btn_opacity = if let Some(lp) = launch_p {
            if lp < 0.62 {
                (1.0 - (lp / 0.60).powf(2.0)).clamp(0.0, 1.0)
            } else {
                0.0
            }
        } else {
            1.0
        };
        let power_btn_sz = if let Some(lp) = launch_p {
            let shrink = 1.0 - (lp / 0.64).clamp(0.0, 1.0).powf(1.8) * 0.25;
            94.0 * shrink
        } else {
            94.0
        };

        let show_speed_face = dial_page_p >= 0.5 && running;
        let pager_enabled = running && !is_launching;
        let pager_label = if !running {
            "未启动"
        } else if self.dial_show_speed {
            "启停页"
        } else {
            "速度盘"
        };

        // Custom sleek draggable white scrollbar for the circular node picker
        let circ_scrollbar = {
            let handle = &self.picker_scroll;
            let max_y: f32 = handle.max_offset().height.into();
            let view_h: f32 = handle.bounds().size.height.into();
            let is_dragging = self
                .dragging_scroll
                .as_ref()
                .map(|(drag_id, _)| *drag_id == "sb-circ-picker")
                .unwrap_or(false);

            if max_y <= 1.0 || view_h <= 1.0 {
                div().w(px(6.0)).h_full().flex_shrink_0().into_any_element()
            } else {
                let offset_y: f32 = handle.offset().y.into();
                let cur_y = (-offset_y).clamp(0.0, max_y);
                let total_h = view_h + max_y;
                let thumb_h = ((view_h / total_h) * view_h).max(22.0).min(view_h - 4.0);
                let travel = (view_h - thumb_h - 4.0).max(0.0);
                let thumb_top = 2.0 + (cur_y / max_y) * travel;
                let handle_down = handle.clone();

                div()
                    .id("sb-circ-picker")
                    .relative()
                    .w(px(8.0))
                    .h_full()
                    .flex_shrink_0()
                    .rounded_full()
                    .bg(rgb(0x232522))
                    .cursor(CursorStyle::ResizeUpDown)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                            Self::apply_scroll_drag(&handle_down, ev.position.y);
                            this.dragging_scroll = Some(("sb-circ-picker", handle_down.clone()));
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(1.5))
                            .right(px(1.5))
                            .top(px(thumb_top))
                            .h(px(thumb_h))
                            .rounded_full()
                            .bg(if is_dragging {
                                theme::white()
                            } else {
                                rgba(0xffffffcc)
                            }),
                    )
                    .into_any_element()
            }
        };

        // Full-Height Sculpted Instrument Stage
        div()
            .flex_1()
            .min_h_0()
            .relative()
            .child(bg_canvas)
            // === PANEL 1 (TOP-LEFT) MAIN BODY: System Proxy & TUN ===
            .child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(0.0))
                    .w(px(left_body_w))
                    .h(px(g.y_left_t))
                    .p_3()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::ink_muted())
                                    .child("接管模式"),
                            )
                            .child(
                                div()
                                    .px_1p5()
                                    .py(px(1.0))
                                    .rounded(px(3.0))
                                    .bg(theme::sidebar_bg())
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::ink_secondary())
                                    .child(format!(":{}", cfg.local_proxy_port)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2p5()
                            .child(
                                div()
                                    .id("dash-card-sysproxy")
                                    .px_1p5()
                                    .py_1()
                                    .mx(px(-6.0))
                                    .rounded(px(5.0))
                                    .hover(|s| s.bg(theme::row_hover_bg()))
                                    .active(|s| s.bg(theme::sidebar_active()))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            let eng = this.engine.clone();
                                            let next = !cfg.enable_system_proxy;
                                            this.rt.spawn(async move {
                                                eng.toggle_system_proxy(next).await;
                                            });
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        div()
                                            .min_w_0()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme::ink_primary())
                                                    .child("系统代理"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(10.0))
                                                    .text_color(theme::ink_muted())
                                                    .child("HTTP / SOCKS5"),
                                            ),
                                    )
                                    .child(render_pill_toggle(p_sys, 0xe84c1e)),
                            )
                            .child(div().h(px(1.0)).w_full().bg(theme::divider_color()))
                            .child(
                                div()
                                    .id("dash-card-tun")
                                    .px_1p5()
                                    .py_1()
                                    .mx(px(-6.0))
                                    .rounded(px(5.0))
                                    .hover(|s| s.bg(theme::row_hover_bg()))
                                    .active(|s| s.bg(theme::sidebar_active()))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_1p5()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            let next = !cfg.enable_tun;
                                            {
                                                let mut c = this.engine.config.write().unwrap();
                                                c.enable_tun = next;
                                            }
                                            this.engine.save_config();
                                            this.engine.bus.info(format!(
                                                "TUN 虚拟网卡已设为 {}（重启代理生效）",
                                                if next { "开启" } else { "关闭" }
                                            ));
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        div()
                                            .min_w_0()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme::ink_primary())
                                                    .child("虚拟网卡 TUN"),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_size(px(10.0))
                                                            .text_color(theme::ink_muted())
                                                            .child(
                                                                if crate::mihomo::is_elevated() {
                                                                    "已提权"
                                                                } else {
                                                                    "需管理员"
                                                                },
                                                            ),
                                                    )
                                                    .when(!crate::mihomo::is_elevated(), |el| {
                                                        el.child(
                                                            div()
                                                                .id("btn-elevate-tun")
                                                                .px_1()
                                                                .py(px(0.5))
                                                                .rounded(px(3.0))
                                                                .bg(theme::braun_orange())
                                                                .hover(|s| {
                                                                    s.bg(theme::braun_orange_border(
                                                                    ))
                                                                })
                                                                .text_size(px(9.5))
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(theme::white())
                                                                .cursor_pointer()
                                                                .on_mouse_down(
                                                                    MouseButton::Left,
                                                                    cx.listener(|this, _, _, cx| {
                                                                        cx.stop_propagation();
                                                                        let eng =
                                                                            this.engine.clone();
                                                                        let rt = this.rt.clone();
                                                                        cx.spawn(async move |_, cx| {
                                                                            let res = rt
                                                                                .spawn_blocking(crate::mihomo::relaunch_as_admin)
                                                                                .await;
                                                                            match res {
                                                                                Ok(Ok(())) => {
                                                                                    let _ = rt
                                                                                        .spawn(async move {
                                                                                            eng.stop().await;
                                                                                        })
                                                                                        .await;
                                                                                    let _ = cx.update(|cx| cx.quit());
                                                                                }
                                                                                Ok(Err(e)) => {
                                                                                    eng.bus.error(format!("{}", e));
                                                                                }
                                                                                Err(e) => {
                                                                                    eng.bus.error(format!("提权任务异常: {}", e));
                                                                                }
                                                                            }
                                                                        })
                                                                        .detach();
                                                                    }),
                                                                )
                                                                .child("提权"),
                                                        )
                                                    }),
                                            ),
                                    )
                                    .child(render_pill_toggle(p_tun, 0xe84c1e)),
                            ),
                    ),
            )
            // === PANEL 1 UPPER WING: Kernel Status + Peak Rate ===
            .child(
                div()
                    .absolute()
                    .left(px(left_body_w))
                    .top(px(0.0))
                    .w(px((g.x_top_l - left_body_w).max(60.0)))
                    .h(px(top_wing_h))
                    .px_2p5()
                    .py_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme::ink_muted())
                                    .child("内核状态"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .w(px(6.0))
                                            .h(px(6.0))
                                            .rounded_full()
                                            .bg(if running {
                                                theme::status_green()
                                            } else {
                                                theme::ink_muted()
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme::ink_primary())
                                            .whitespace_nowrap()
                                            .child(if running { "Mihomo" } else { "已停止" }),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme::ink_muted())
                                    .child("峰值"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::braun_orange())
                                    .whitespace_nowrap()
                                    .child(format_rate(self.peak_down_bps)),
                            ),
                    ),
            )
            // === PANEL 2 (TOP-RIGHT) UPPER WING: Active Mode Badge ===
            .child(
                div()
                    .absolute()
                    .left(px(g.x_top_r))
                    .top(px(0.0))
                    .w(px((right_body_x - g.x_top_r).max(60.0)))
                    .h(px(top_wing_h))
                    .px_2p5()
                    .py_2()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap_0p5()
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(theme::ink_muted())
                            .child("当前策略"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::braun_orange())
                            .child(mode_badge),
                    ),
            )
            // === PANEL 2 (TOP-RIGHT) MAIN BODY: Outbound + 429 Rotation ===
            .child(
                div()
                    .absolute()
                    .left(px(right_body_x))
                    .top(px(0.0))
                    .w(px(right_body_w))
                    .h(px(g.y_right_t))
                    .p_3()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::ink_muted())
                                    .mb_0p5()
                                    .child("出站模式"),
                            )
                            .child(radio_row("om-rule", "规则 (Rule)", "rule", p_out_rule))
                            .child(radio_row(
                                "om-global",
                                "全局 (Global)",
                                "global",
                                p_out_global,
                            ))
                            .child(radio_row(
                                "om-direct",
                                "直连 (Direct)",
                                "direct",
                                p_out_direct,
                            )),
                    )
                    .child(
                        div()
                            .pt_2()
                            .border_t_1()
                            .border_color(theme::divider_color())
                            .flex()
                            .flex_col()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(theme::ink_muted())
                                    .child("429 轮询策略"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(rot_pill(
                                        "m-429",
                                        "429切",
                                        RotationMode::On429Only,
                                        p_rot_429,
                                    ))
                                    .child(rot_pill(
                                        "m-rr",
                                        "逐请求",
                                        RotationMode::RoundRobinPerRequest,
                                        p_rot_req,
                                    ))
                                    .child(rot_pill(
                                        "m-int",
                                        "60s",
                                        RotationMode::RoundRobinInterval { seconds: 60 },
                                        p_rot_int,
                                    )),
                            ),
                    ),
            )
            // === PANEL 3 (BOTTOM-LEFT) MAIN BODY: Traffic Stats ===
            .child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(g.y_left_b))
                    .w(px(left_body_w))
                    .h(px(g.h - g.y_left_b))
                    .p_3()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2p5()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme::ink_muted())
                                            .child("流量统计"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .text_color(theme::ink_muted())
                                            .child("本次会话"),
                                    ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .h(px(7.0))
                                    .rounded_full()
                                    .overflow_hidden()
                                    .bg(theme::sidebar_active())
                                    .flex()
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative(up_ratio))
                                            .bg(theme::braun_orange()),
                                    )
                                    .child(div().h_full().flex_1().bg(theme::ink_primary())),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1p5()
                                    .text_xs()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .text_color(theme::braun_orange())
                                                    .child("↑ 上传"),
                                            )
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme::ink_primary())
                                                    .child(format!("{} {}", up_val, up_unit)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .text_color(theme::ink_secondary())
                                                    .child("↓ 下载"),
                                            )
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme::ink_primary())
                                                    .child(format!("{} {}", dn_val, dn_unit)),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .pt_2()
                            .border_t_1()
                            .border_color(theme::divider_color())
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_size(px(10.5))
                            .child(
                                div()
                                    .text_color(theme::ink_muted())
                                    .child("429 透明重试"),
                            )
                            .child(
                                div()
                                    .px_1p5()
                                    .py(px(1.0))
                                    .rounded(px(3.0))
                                    .bg(theme::sidebar_bg())
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(if cfg.auto_retry_on_429 {
                                        theme::status_green()
                                    } else {
                                        theme::ink_muted()
                                    })
                                    .child(if cfg.auto_retry_on_429 { "已启用" } else { "已关闭" }),
                            ),
                    ),
            )
            // === PANEL 3 LOWER WING: 429 & Retry Counters ===
            .child(
                div()
                    .absolute()
                    .left(px(left_body_w))
                    .top(px(bot_wing_y))
                    .w(px((g.x_bot_l - left_body_w).max(60.0)))
                    .h(px(bot_wing_h))
                    .px_2p5()
                    .py_2()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap_1()
                    .text_size(px(11.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(div().text_color(theme::ink_muted()).child("429"))
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::braun_orange())
                                    .child(format!("{}", snap.total_429)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(div().text_color(theme::ink_muted()).child("重试"))
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::status_green())
                                    .child(format!("{}", snap.total_retries_ok)),
                            ),
                    ),
            )
            // === PANEL 4 (BOTTOM-RIGHT) LOWER WING: Local IP + Uptime ===
            .child(
                div()
                    .absolute()
                    .left(px(g.x_bot_r))
                    .top(px(bot_wing_y))
                    .w(px((right_body_x - g.x_bot_r).max(60.0)))
                    .h(px(bot_wing_h))
                    .px_2p5()
                    .py_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme::ink_muted())
                                    .child("内网 IP"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::ink_primary())
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(tel.local_ip),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap_0p5()
                            .flex_shrink_0()
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme::ink_muted())
                                    .child("运行时长"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if running {
                                        theme::ink_primary()
                                    } else {
                                        theme::ink_muted()
                                    })
                                    .whitespace_nowrap()
                                    .child(uptime_text.clone()),
                            ),
                    ),
            )
            // === PANEL 4 (BOTTOM-RIGHT) MAIN BODY: Clickable Exit IP Detection ===
            .child(
                div()
                    .id("dash-check-ip")
                    .absolute()
                    .left(px(right_body_x))
                    .top(px(g.y_right_b))
                    .w(px(right_body_w))
                    .h(px(g.h - g.y_right_b))
                    .p_3()
                    .rounded_r(px(12.0))
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.92))
                    .active(|s| s.opacity(0.8))
                    .flex()
                    .flex_col()
                    .justify_between()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            let eng = this.engine.clone();
                            this.rt.spawn(async move {
                                eng.check_exit_ip().await;
                            });
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .child(render_flag(exit_country))
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme::ink_muted())
                                            .whitespace_nowrap()
                                            .child("出口检测"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::braun_orange())
                                    .flex_shrink_0()
                                    .child("刷新"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_size(px(14.5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::ink_primary())
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(exit_ip_text),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(theme::ink_muted())
                                    .child("点击区域重新探测"),
                            ),
                    )
                    .child(
                        div()
                            .pt_2()
                            .border_t_1()
                            .border_color(theme::divider_color())
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_size(px(10.5))
                            .child(
                                div()
                                    .text_color(theme::ink_muted())
                                    .child("出口状态"),
                            )
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(if running {
                                        theme::status_green()
                                    } else {
                                        theme::ink_muted()
                                    })
                                    .child(if running { "● 连通" } else { "○ 待机" }),
                            ),
                    ),
            )
            // === CENTER HERO: ALL-IN-ONE 2-PAGE BRAUN CIRCULAR DIAL ===
            .child(
                div()
                    .absolute()
                    .left(px(dial_left))
                    .top(px(dial_top))
                    .w(px(dial_d))
                    .h(px(dial_d))
                    .rounded_full()
                    .bg(rgb(0xe3e2da))
                    .border_1()
                    .border_color(theme::border_strong())
                    .shadow_md()
                    .p(px(9.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .relative()
                            .size_full()
                            .rounded_full()
                            .bg(theme::lcd_bg())
                            .border_1()
                            .border_color(rgb(0x2f312d))
                            .overflow_hidden()
                            // LAYER 1: Main Dial Content (slides DOWN inside circle when picker opens)
                            .child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .right_0()
                                    .top(px(picker_slide * 26.0))
                                    .h(px(inner_well_d))
                                    .opacity((1.0 - picker_slide * 1.45).clamp(0.0, 1.0))
                                    // PAGE 0: Ignition / Power Page
                                    .when(!show_speed_face, |el| {
                                        el.child(
                                            div()
                                                .absolute()
                                                .inset_0()
                                                .flex()
                                                .flex_col()
                                                .items_center()
                                                .justify_center()
                                                .pb_4()
                                                .child(
                                                    div()
                                                        .mb_3()
                                                        .child(render_lcd_node_window(
                                                            "dial-node-win-pwr",
                                                            cx,
                                                        )),
                                                )
                                                .child(
                                                    div()
                                                        .id("dial-power-btn")
                                                        .w(px(power_btn_sz))
                                                        .h(px(power_btn_sz))
                                                        .rounded_full()
                                                        .opacity(power_btn_opacity)
                                                        .bg(if running {
                                                            rgb(0x1f211e)
                                                        } else {
                                                            theme::braun_orange()
                                                        })
                                                        .hover(|s| {
                                                            s.bg(if running {
                                                                rgb(0x2b2d29)
                                                            } else {
                                                                rgb(0xf25829)
                                                            })
                                                        })
                                                        .active(|s| s.opacity(0.88))
                                                        .border_2()
                                                        .border_color(if running {
                                                            theme::lcd_green()
                                                        } else {
                                                            rgb(0xff8c66)
                                                        })
                                                        .shadow_md()
                                                        .cursor_pointer()
                                                        .flex()
                                                        .flex_col()
                                                        .items_center()
                                                        .justify_center()
                                                        .gap_1()
                                                        .on_mouse_down(
                                                            MouseButton::Left,
                                                            cx.listener(|this, _, _, cx| {
                                                                this.toggle_power(cx);
                                                            }),
                                                        )
                                                        .child(icon(
                                                            "icons/power.svg",
                                                            24.0,
                                                            if running {
                                                                theme::lcd_green()
                                                            } else {
                                                                theme::white()
                                                            },
                                                        ))
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(theme::white())
                                                                .whitespace_nowrap()
                                                                .child(if is_launching {
                                                                    "正在加速"
                                                                } else if running {
                                                                    "停止代理"
                                                                } else {
                                                                    "启动代理"
                                                                }),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .mt_2()
                                                        .text_size(px(10.0))
                                                        .text_color(theme::lcd_muted())
                                                        .opacity(power_btn_opacity)
                                                        .child(if is_launching {
                                                            "冲刺建立加密隧道...".to_string()
                                                        } else if running {
                                                            format!("已运行 {}", uptime_text)
                                                        } else {
                                                            "点击点火启动内核".to_string()
                                                        }),
                                                ),
                                        )
                                    })
                                    // PAGE 1: Live Speedometer Page
                                    .when(show_speed_face, |el| {
                                        el.child(
                                            div()
                                                .absolute()
                                                .inset_0()
                                                .child(
                                                    canvas(
                                                        |_, _, _| {},
                                                        move |bounds, _, window, _| {
                                                            paint_speed_dial(
                                                                bounds,
                                                                dial_down_norm,
                                                                dial_up_norm,
                                                                running,
                                                                window,
                                                            );
                                                        },
                                                    )
                                                    .size_full(),
                                                )
                                                .children(dial_labels)
                                                .child(
                                                    div()
                                                        .absolute()
                                                        .inset_0()
                                                        .pb_3()
                                                        .flex()
                                                        .flex_col()
                                                        .items_center()
                                                        .justify_center()
                                                        .child(
                                                            div()
                                                                .mb_1p5()
                                                                .child(render_lcd_node_window(
                                                                    "dial-node-win-spd",
                                                                    cx,
                                                                )),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_baseline()
                                                                .gap_1()
                                                                .child(
                                                                    div()
                                                                        .text_size(px(32.0))
                                                                        .line_height(px(34.0))
                                                                        .font_weight(
                                                                            FontWeight::BOLD,
                                                                        )
                                                                        .text_color(
                                                                            theme::lcd_text(),
                                                                        )
                                                                        .child(main_val),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_size(px(11.0))
                                                                        .font_weight(
                                                                            FontWeight::SEMIBOLD,
                                                                        )
                                                                        .text_color(
                                                                            theme::lcd_green(),
                                                                        )
                                                                        .child(main_unit),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .mt_1()
                                                                .flex()
                                                                .items_center()
                                                                .gap_1p5()
                                                                .text_size(px(10.0))
                                                                .child(
                                                                    div()
                                                                        .text_color(
                                                                            theme::lcd_green(),
                                                                        )
                                                                        .child(format!(
                                                                            "↓ {}",
                                                                            format_rate(
                                                                                tel.down_bps
                                                                            )
                                                                        )),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_color(rgb(0x4a4c47))
                                                                        .child("|"),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_color(
                                                                            theme::lcd_orange(),
                                                                        )
                                                                        .child(format!(
                                                                            "↑ {}",
                                                                            format_rate(tel.up_bps)
                                                                        )),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .mt_2()
                                                                .w(px(88.0))
                                                                .h(px(19.0))
                                                                .rounded(px(3.0))
                                                                .bg(rgb(0x101210))
                                                                .border_1()
                                                                .border_color(rgb(0x2a2c28))
                                                                .px_1()
                                                                .py(px(2.0))
                                                                .flex()
                                                                .items_end()
                                                                .gap(px(1.5))
                                                                .children(mini_spark_bars),
                                                        ),
                                                ),
                                        )
                                    })
                                    // SINGLE MINIMALIST BOTTOM PAGE-FLIP PILL
                                    .child(
                                        div()
                                            .absolute()
                                            .bottom(px(inner_well_d * 0.075))
                                            .left_0()
                                            .right_0()
                                            .flex()
                                            .justify_center()
                                            .child(
                                                div()
                                                    .id("dial-pager-pill")
                                                    .h(px(23.0))
                                                    .px_3()
                                                    .rounded_full()
                                                    .bg(rgb(0x1d1f1c))
                                                    .border_1()
                                                    .border_color(rgb(0x333630))
                                                    .when(pager_enabled, |el| {
                                                        el.cursor_pointer()
                                                            .hover(|s| {
                                                                s.bg(rgb(0x272a26))
                                                                    .border_color(
                                                                        theme::braun_orange(),
                                                                    )
                                                            })
                                                            .active(|s| s.opacity(0.8))
                                                            .on_mouse_down(
                                                                MouseButton::Left,
                                                                cx.listener(|this, _, _, cx| {
                                                                    if this.engine.is_running()
                                                                        && this
                                                                            .dial_launch_started
                                                                            .is_none()
                                                                    {
                                                                        this.dial_show_speed =
                                                                            !this.dial_show_speed;
                                                                        cx.notify();
                                                                    }
                                                                }),
                                                            )
                                                    })
                                                    .when(!pager_enabled, |el| el.opacity(0.48))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .gap_1p5()
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .items_center()
                                                            .gap_1()
                                                            .child(
                                                                div()
                                                                    .w(px(5.0))
                                                                    .h(px(5.0))
                                                                    .rounded_full()
                                                                    .bg(if !show_speed_face {
                                                                        theme::braun_orange()
                                                                    } else {
                                                                        rgb(0x4e504b)
                                                                    }),
                                                            )
                                                            .child(
                                                                div()
                                                                    .w(px(5.0))
                                                                    .h(px(5.0))
                                                                    .rounded_full()
                                                                    .bg(if show_speed_face {
                                                                        theme::lcd_green()
                                                                    } else {
                                                                        rgb(0x4e504b)
                                                                    }),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(10.0))
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(if pager_enabled {
                                                                theme::lcd_text()
                                                            } else {
                                                                theme::lcd_muted()
                                                            })
                                                            .whitespace_nowrap()
                                                            .child(pager_label),
                                                    ),
                                            ),
                                    ),
                            )
                            // LAYER 2: 3D First-Person Sports-Car Tunnel Acceleration Overlay
                            .when_some(launch_p, |el, lp| {
                                el.child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .child(
                                            canvas(
                                                |_, _, _| {},
                                                move |bounds, _, window, _| {
                                                    paint_warp_tunnel(bounds, lp, window);
                                                },
                                            )
                                            .size_full(),
                                        ),
                                )
                            })
                            // LAYER 3: Circular Shutter Node Picker (stays 100% circular inside dial well)
                            .when(picker_visible, |el| {
                                el.child(
                                    div()
                                        .id("dial-circular-picker")
                                        .absolute()
                                        .inset_0()
                                        .rounded_full()
                                        .bg(rgb(0x161715))
                                        .opacity(picker_slide)
                                        .pt(px(22.0 - (1.0 - picker_slide) * 14.0))
                                        .pb_4()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .items_center()
                                                .gap_0p5()
                                                .flex_shrink_0()
                                                .child(
                                                    div()
                                                        .text_size(px(8.5))
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(theme::braun_orange())
                                                        .child("NODE SELECTOR"),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap_1p5()
                                                        .when(
                                                            self.engine.is_testing_latency(),
                                                            |el| {
                                                                el.child(render_loading_bars(
                                                                    self.loading_phase,
                                                                    theme::lcd_green(),
                                                                ))
                                                            },
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(10.5))
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(theme::lcd_text())
                                                                .whitespace_nowrap()
                                                                .child(
                                                                    if self
                                                                        .engine
                                                                        .is_testing_latency()
                                                                    {
                                                                        format!(
                                                                            "正在测速 · {}/{}",
                                                                            snap.ready_count,
                                                                            snap.nodes.len()
                                                                        )
                                                                    } else {
                                                                        format!(
                                                                            "选择出口节点 · {}/{}",
                                                                            snap.ready_count,
                                                                            snap.nodes.len()
                                                                        )
                                                                    },
                                                                ),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .w(px(inner_well_d * 0.72))
                                                .flex_1()
                                                .min_h_0()
                                                .my_2()
                                                .flex()
                                                .gap_1p5()
                                                .child(
                                                    div()
                                                        .id("circ-picker-scroll")
                                                        .track_scroll(&self.picker_scroll)
                                                        .flex_1()
                                                        .min_w_0()
                                                        .overflow_y_scroll()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_1()
                                                        .children(picker_rows),
                                                )
                                                .child(circ_scrollbar),
                                        )
                                        .child(
                                            div()
                                                .id("circ-picker-close")
                                                .flex_shrink_0()
                                                .px_3p5()
                                                .py_1()
                                                .rounded_full()
                                                .bg(rgb(0x1d1f1c))
                                                .hover(|s| {
                                                    s.bg(rgb(0x272a26))
                                                        .border_color(theme::braun_orange())
                                                })
                                                .border_1()
                                                .border_color(rgb(0x333630))
                                                .cursor_pointer()
                                                .text_size(px(10.0))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme::lcd_text())
                                                .whitespace_nowrap()
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(|this, _, _, cx| {
                                                        this.toggle_node_picker(false, cx);
                                                    }),
                                                )
                                                .child("▲ 返回表盘"),
                                        ),
                                )
                            }),
                    ),
            )
    }
}
