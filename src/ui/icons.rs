use std::f32::consts::{FRAC_PI_2, PI, TAU};

use eframe::egui::{Color32, Painter, Pos2, Shape, Stroke, pos2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Snowflake,
    Power,
    Refresh,
    Scan,
    Settings,
    Auto,
    Drop,
    Fan,
    Flame,
    SwingV,
    SwingH,
    Bulb,
    Turbo,
    Quiet,
    Sleep,
    Health,
    Saving,
    Air,
    Freeze,
    Minus,
    Plus,
}

fn arc_points(center: Pos2, r: f32, start: f32, end: f32, steps: usize) -> Vec<Pos2> {
    (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            let a = start + t * (end - start);
            pos2(center.x + r * a.cos(), center.y + r * a.sin())
        })
        .collect()
}

pub fn draw_icon(painter: &Painter, c: Pos2, r: f32, icon: Icon, color: Color32) {
    let sw = (r * 0.19).clamp(1.3, 2.4);
    let stroke = Stroke::new(sw, color);

    match icon {
        Icon::Snowflake => {
            for i in 0..3 {
                let a = (i as f32) * (PI / 3.0) - FRAC_PI_2;
                let dx = r * a.cos();
                let dy = r * a.sin();
                painter.line_segment([pos2(c.x - dx, c.y - dy), pos2(c.x + dx, c.y + dy)], stroke);

                // Small V barbs near both tips
                for sign in [-1.0_f32, 1.0_f32] {
                    let base = pos2(c.x + sign * dx * 0.58, c.y + sign * dy * 0.58);
                    for off in [-0.55_f32, 0.55_f32] {
                        let ba = a + off + if sign < 0.0 { PI } else { 0.0 };
                        let tip = pos2(base.x + r * 0.32 * ba.cos(), base.y + r * 0.32 * ba.sin());
                        painter.line_segment([base, tip], Stroke::new(sw * 0.85, color));
                    }
                }
            }
        }
        Icon::Power => {
            let start = -FRAC_PI_2 + 0.55;
            let end = start + (TAU - 1.10);
            painter.add(Shape::line(arc_points(c, r * 0.85, start, end, 22), stroke));
            painter.line_segment(
                [pos2(c.x, c.y - r * 1.02), pos2(c.x, c.y - r * 0.12)],
                stroke,
            );
        }
        Icon::Refresh => {
            let start = -0.35;
            let end = TAU - 1.05;
            let pts = arc_points(c, r * 0.8, start, end, 20);
            if let Some(&tip) = pts.last() {
                painter.add(Shape::line(pts, stroke));
                painter.line_segment(
                    [tip, pos2(tip.x - r * 0.36, tip.y - r * 0.06)],
                    stroke,
                );
                painter.line_segment(
                    [tip, pos2(tip.x + r * 0.05, tip.y - r * 0.36)],
                    stroke,
                );
            }
        }
        Icon::Scan => {
            // Wi-Fi / radar waves + center dot
            painter.circle_filled(pos2(c.x, c.y + r * 0.35), r * 0.17, color);
            for scale in [0.48_f32, 0.85_f32] {
                let pts = arc_points(
                    pos2(c.x, c.y + r * 0.35),
                    r * scale,
                    -PI * 0.82,
                    -PI * 0.18,
                    14,
                );
                painter.add(Shape::line(pts, stroke));
            }
        }
        Icon::Settings => {
            painter.circle_stroke(c, r * 0.42, stroke);
            for i in 0..8 {
                let a = (i as f32) * (TAU / 8.0);
                let p1 = pos2(c.x + r * 0.62 * a.cos(), c.y + r * 0.62 * a.sin());
                let p2 = pos2(c.x + r * 0.92 * a.cos(), c.y + r * 0.92 * a.sin());
                painter.line_segment([p1, p2], stroke);
            }
        }
        Icon::Auto => {
            // Circular cycle arrows + center dot
            let pts1 = arc_points(c, r * 0.78, -PI * 0.85, -PI * 0.1, 14);
            let pts2 = arc_points(c, r * 0.78, PI * 0.15, PI * 0.9, 14);
            painter.add(Shape::line(pts1, stroke));
            painter.add(Shape::line(pts2, stroke));
            painter.circle_filled(c, r * 0.18, color);
        }
        Icon::Drop => {
            // Water droplet outline
            let mut pts = Vec::with_capacity(20);
            pts.push(pos2(c.x, c.y - r * 0.92));
            let lower = arc_points(pos2(c.x, c.y + r * 0.2), r * 0.62, -0.25, PI + 0.25, 16);
            pts.extend(lower);
            pts.push(pos2(c.x, c.y - r * 0.92));
            painter.add(Shape::line(pts, stroke));
        }
        Icon::Fan => {
            painter.circle_filled(c, r * 0.17, color);
            for i in 0..4 {
                let a = (i as f32) * FRAC_PI_2 + 0.3;
                let blade_c = pos2(c.x + r * 0.48 * a.cos(), c.y + r * 0.48 * a.sin());
                let pts = arc_points(blade_c, r * 0.38, a - 1.35, a + 1.35, 10);
                painter.add(Shape::line(pts, stroke));
            }
        }
        Icon::Flame => {
            let pts = vec![
                pos2(c.x, c.y - r * 0.95),
                pos2(c.x + r * 0.45, c.y - r * 0.25),
                pos2(c.x + r * 0.62, c.y + r * 0.30),
                pos2(c.x + r * 0.35, c.y + r * 0.82),
                pos2(c.x, c.y + r * 0.92),
                pos2(c.x - r * 0.35, c.y + r * 0.82),
                pos2(c.x - r * 0.62, c.y + r * 0.30),
                pos2(c.x - r * 0.42, c.y - r * 0.22),
                pos2(c.x, c.y - r * 0.95),
            ];
            painter.add(Shape::line(pts, stroke));
        }
        Icon::SwingV => {
            painter.line_segment([pos2(c.x, c.y - r * 0.82), pos2(c.x, c.y + r * 0.82)], stroke);
            painter.line_segment(
                [pos2(c.x - r * 0.38, c.y - r * 0.44), pos2(c.x, c.y - r * 0.84)],
                stroke,
            );
            painter.line_segment(
                [pos2(c.x + r * 0.38, c.y - r * 0.44), pos2(c.x, c.y - r * 0.84)],
                stroke,
            );
            painter.line_segment(
                [pos2(c.x - r * 0.38, c.y + r * 0.44), pos2(c.x, c.y + r * 0.84)],
                stroke,
            );
            painter.line_segment(
                [pos2(c.x + r * 0.38, c.y + r * 0.44), pos2(c.x, c.y + r * 0.84)],
                stroke,
            );
        }
        Icon::SwingH => {
            painter.line_segment([pos2(c.x - r * 0.82, c.y), pos2(c.x + r * 0.82, c.y)], stroke);
            painter.line_segment(
                [pos2(c.x - r * 0.44, c.y - r * 0.38), pos2(c.x - r * 0.84, c.y)],
                stroke,
            );
            painter.line_segment(
                [pos2(c.x - r * 0.44, c.y + r * 0.38), pos2(c.x - r * 0.84, c.y)],
                stroke,
            );
            painter.line_segment(
                [pos2(c.x + r * 0.44, c.y - r * 0.38), pos2(c.x + r * 0.84, c.y)],
                stroke,
            );
            painter.line_segment(
                [pos2(c.x + r * 0.44, c.y + r * 0.38), pos2(c.x + r * 0.84, c.y)],
                stroke,
            );
        }
        Icon::Bulb => {
            let bulb_c = pos2(c.x, c.y - r * 0.18);
            let pts = arc_points(bulb_c, r * 0.56, -PI * 1.22, PI * 0.22, 16);
            painter.add(Shape::line(pts, stroke));
            painter.line_segment(
                [pos2(c.x - r * 0.28, c.y + r * 0.55), pos2(c.x + r * 0.28, c.y + r * 0.55)],
                stroke,
            );
            painter.line_segment(
                [pos2(c.x - r * 0.20, c.y + r * 0.82), pos2(c.x + r * 0.20, c.y + r * 0.82)],
                stroke,
            );
        }
        Icon::Turbo => {
            let pts = vec![
                pos2(c.x + r * 0.15, c.y - r * 0.92),
                pos2(c.x - r * 0.48, c.y + r * 0.08),
                pos2(c.x + r * 0.05, c.y + r * 0.08),
                pos2(c.x - r * 0.18, c.y + r * 0.92),
                pos2(c.x + r * 0.48, c.y - r * 0.12),
                pos2(c.x - r * 0.02, c.y - r * 0.12),
                pos2(c.x + r * 0.15, c.y - r * 0.92),
            ];
            painter.add(Shape::line(pts, stroke));
        }
        Icon::Quiet => {
            // Speaker + mute slash
            let pts = vec![
                pos2(c.x - r * 0.72, c.y - r * 0.25),
                pos2(c.x - r * 0.32, c.y - r * 0.25),
                pos2(c.x + r * 0.15, c.y - r * 0.65),
                pos2(c.x + r * 0.15, c.y + r * 0.65),
                pos2(c.x - r * 0.32, c.y + r * 0.25),
                pos2(c.x - r * 0.72, c.y + r * 0.25),
                pos2(c.x - r * 0.72, c.y - r * 0.25),
            ];
            painter.add(Shape::line(pts, stroke));
            painter.line_segment(
                [pos2(c.x - r * 0.78, c.y - r * 0.78), pos2(c.x + r * 0.78, c.y + r * 0.78)],
                stroke,
            );
        }
        Icon::Sleep => {
            // Crescent moon
            let outer = arc_points(c, r * 0.78, 0.45, TAU - 1.15, 18);
            let inner = arc_points(
                pos2(c.x + r * 0.32, c.y - r * 0.22),
                r * 0.62,
                TAU - 1.45,
                0.75,
                14,
            );
            painter.add(Shape::line(outer, stroke));
            painter.add(Shape::line(inner, stroke));
        }
        Icon::Health => {
            // Shield / heart-pulse outline
            let pts = vec![
                pos2(c.x, c.y - r * 0.88),
                pos2(c.x + r * 0.72, c.y - r * 0.52),
                pos2(c.x + r * 0.62, c.y + r * 0.25),
                pos2(c.x, c.y + r * 0.90),
                pos2(c.x - r * 0.62, c.y + r * 0.25),
                pos2(c.x - r * 0.72, c.y - r * 0.52),
                pos2(c.x, c.y - r * 0.88),
            ];
            painter.add(Shape::line(pts, stroke));
            painter.line_segment([pos2(c.x - r * 0.30, c.y), pos2(c.x + r * 0.30, c.y)], stroke);
            painter.line_segment([pos2(c.x, c.y - r * 0.30), pos2(c.x, c.y + r * 0.30)], stroke);
        }
        Icon::Saving => {
            // Eco leaf
            let pts = vec![
                pos2(c.x - r * 0.68, c.y + r * 0.72),
                pos2(c.x - r * 0.55, c.y - r * 0.25),
                pos2(c.x + r * 0.65, c.y - r * 0.75),
                pos2(c.x + r * 0.55, c.y + r * 0.28),
                pos2(c.x - r * 0.35, c.y + r * 0.58),
            ];
            painter.add(Shape::line(pts, stroke));
            painter.line_segment(
                [pos2(c.x - r * 0.68, c.y + r * 0.72), pos2(c.x + r * 0.28, c.y - r * 0.25)],
                stroke,
            );
        }
        Icon::Air => {
            // Three wind lines
            for (dy, len) in [(-0.42_f32, 0.78_f32), (0.0, 0.92), (0.42, 0.65)] {
                painter.line_segment(
                    [
                        pos2(c.x - r * 0.78, c.y + r * dy),
                        pos2(c.x + r * len, c.y + r * dy),
                    ],
                    stroke,
                );
            }
        }
        Icon::Freeze => {
            // Thermometer
            painter.line_segment(
                [pos2(c.x, c.y - r * 0.85), pos2(c.x, c.y + r * 0.28)],
                Stroke::new(sw * 1.4, color),
            );
            painter.circle_filled(pos2(c.x, c.y + r * 0.52), r * 0.34, color);
        }
        Icon::Minus => {
            painter.line_segment(
                [pos2(c.x - r * 0.7, c.y), pos2(c.x + r * 0.7, c.y)],
                stroke,
            );
        }
        Icon::Plus => {
            painter.line_segment(
                [pos2(c.x - r * 0.7, c.y), pos2(c.x + r * 0.7, c.y)],
                stroke,
            );
            painter.line_segment(
                [pos2(c.x, c.y - r * 0.7), pos2(c.x, c.y + r * 0.7)],
                stroke,
            );
        }
    }
}
