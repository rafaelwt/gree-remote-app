use std::f32::consts::PI;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Sense, Shape, Stroke, pos2, vec2,
};

use crate::gree::protocol::{AcMode, FanSpeed};

use super::icons::{Icon, draw_icon};
use super::theme::{
    BORDER, CARD_HOVER, CARD_INNER, POWER_OFF, POWER_ON, TEXT_MUTED, TEXT_PRIMARY, with_alpha,
};

pub fn header_icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    tooltip: &str,
    enabled: bool,
    active: bool,
    accent: Color32,
) -> bool {
    let size = vec2(34.0, 34.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);

    if ui.is_rect_visible(rect) {
        let bg = if active {
            with_alpha(accent, 36)
        } else if enabled && resp.hovered() {
            CARD_HOVER
        } else {
            CARD_INNER
        };
        let border = if active { accent } else { BORDER };
        let fg = if !enabled {
            with_alpha(TEXT_MUTED, 110)
        } else if active {
            accent
        } else {
            TEXT_PRIMARY
        };

        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(9), bg);
        painter.rect_stroke(
            rect,
            CornerRadius::same(9),
            Stroke::new(1.0_f32, border),
            egui::StrokeKind::Inside,
        );
        draw_icon(painter, rect.center(), 8.5, icon, fg);
    }

    let resp = resp.on_hover_text(tooltip);
    enabled && resp.clicked()
}

fn circle_step_button(ui: &mut egui::Ui, icon: Icon, enabled: bool) -> bool {
    let size = vec2(42.0, 42.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);

    if ui.is_rect_visible(rect) {
        let bg = if !enabled {
            with_alpha(CARD_INNER, 130)
        } else if resp.is_pointer_button_down_on() {
            Color32::from_rgb(45, 55, 72)
        } else if resp.hovered() {
            CARD_HOVER
        } else {
            CARD_INNER
        };
        let fg = if enabled {
            TEXT_PRIMARY
        } else {
            with_alpha(TEXT_MUTED, 90)
        };

        let painter = ui.painter();
        painter.circle_filled(rect.center(), 20.0, bg);
        painter.circle_stroke(rect.center(), 20.0, Stroke::new(1.0_f32, BORDER));
        draw_icon(painter, rect.center(), 8.5, icon, fg);
    }

    enabled && resp.clicked()
}

/// Interactive circular arc thermostat dial (16..=30 °C) flanked by `-` and `+` buttons.
/// Returns `Some(new_temp)` when the user finishes changing the temperature.
pub fn thermostat_dial(
    ui: &mut egui::Ui,
    set_temp: &mut i64,
    room_temp: Option<i64>,
    power: bool,
    accent: Color32,
    enabled: bool,
) -> Option<i64> {
    let mut committed: Option<i64> = None;
    let dial_accent = if power { accent } else { TEXT_MUTED };

    ui.horizontal(|ui| {
        let total_w = ui.available_width();
        let content_w = 42.0 + 18.0 + 184.0 + 18.0 + 42.0;
        let pad = ((total_w - content_w) * 0.5).max(0.0);
        ui.add_space(pad);

        // Minus button vertically centered
        ui.vertical(|ui| {
            ui.add_space(70.0);
            if circle_step_button(ui, Icon::Minus, enabled && *set_temp > 16) {
                *set_temp -= 1;
                committed = Some(*set_temp);
            }
        });

        ui.add_space(18.0);

        // Circular arc dial
        let dial_size = vec2(184.0, 178.0);
        let sense = if enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        };
        let (rect, resp) = ui.allocate_exact_size(dial_size, sense);
        let center = pos2(rect.center().x, rect.center().y + 6.0);
        let radius = 74.0_f32;

        // 270-degree sweep: from 135° (bottom-left) clockwise to 405° (bottom-right)
        let start_deg = 135.0_f32;
        let sweep_deg = 270.0_f32;
        let start_rad = start_deg.to_radians();
        let sweep_rad = sweep_deg.to_radians();

        // Pointer drag/click mapping to 16..=30
        if enabled && (resp.dragged() || resp.clicked()) {
            if let Some(pointer) = resp.interact_pointer_pos() {
                let dx = pointer.x - center.x;
                let dy = pointer.y - center.y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > 28.0 {
                    let mut deg = dy.atan2(dx).to_degrees();
                    if deg < 0.0 {
                        deg += 360.0;
                    }
                    let mut rel = deg - start_deg;
                    if rel < 0.0 {
                        rel += 360.0;
                    }
                    if rel <= sweep_deg + 25.0 || rel >= 335.0 {
                        let clamped = if rel >= 335.0 {
                            0.0
                        } else {
                            (rel / sweep_deg).clamp(0.0, 1.0)
                        };
                        let mapped = (16.0 + clamped * 14.0).round() as i64;
                        *set_temp = mapped.clamp(16, 30);
                    }
                }
            }
        }
        if enabled && (resp.drag_stopped() || resp.clicked()) {
            committed = Some(*set_temp);
        }

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let frac = ((*set_temp - 16) as f32 / 14.0).clamp(0.0, 1.0);

            let arc_pts = |end_frac: f32, steps: usize| -> Vec<Pos2> {
                (0..=steps)
                    .map(|i| {
                        let t = (i as f32 / steps as f32) * end_frac;
                        let a = start_rad + t * sweep_rad;
                        pos2(center.x + radius * a.cos(), center.y + radius * a.sin())
                    })
                    .collect()
            };

            // Track background
            painter.add(Shape::line(
                arc_pts(1.0, 56),
                Stroke::new(9.0_f32, Color32::from_rgb(35, 42, 56)),
            ));

            // Active glowing progress arc
            if frac > 0.001 {
                let active_pts = arc_pts(frac, (56.0 * frac).max(8.0) as usize);
                if power {
                    painter.add(Shape::line(
                        active_pts.clone(),
                        Stroke::new(15.0_f32, with_alpha(dial_accent, 42)),
                    ));
                }
                painter.add(Shape::line(active_pts, Stroke::new(9.0_f32, dial_accent)));
            }

            // Thumb knob
            let thumb_angle = start_rad + frac * sweep_rad;
            let thumb_pos = pos2(
                center.x + radius * thumb_angle.cos(),
                center.y + radius * thumb_angle.sin(),
            );
            if power {
                painter.circle_filled(thumb_pos, 11.0, with_alpha(dial_accent, 65));
            }
            painter.circle_filled(thumb_pos, 6.5, Color32::WHITE);
            painter.circle_stroke(thumb_pos, 6.5, Stroke::new(2.0_f32, dial_accent));

            // Center temperature readout
            painter.text(
                pos2(center.x, center.y - 6.0),
                Align2::CENTER_CENTER,
                format!("{}°C", *set_temp),
                FontId::proportional(38.0),
                if power { TEXT_PRIMARY } else { TEXT_MUTED },
            );

            // Ambient room temperature
            let subtitle = match room_temp {
                Some(t) => format!("Ambiente {t}°C"),
                None => "Sin sensor".to_string(),
            };
            painter.text(
                pos2(center.x, center.y + 24.0),
                Align2::CENTER_CENTER,
                subtitle,
                FontId::proportional(13.0),
                TEXT_MUTED,
            );

            // Min / Max endpoint labels (16° and 30°)
            let min_a = start_rad + 0.12;
            let max_a = start_rad + sweep_rad - 0.12;
            let label_r = radius + 16.0;
            painter.text(
                pos2(
                    center.x + label_r * min_a.cos() - 4.0,
                    center.y + label_r * min_a.sin() + 4.0,
                ),
                Align2::CENTER_CENTER,
                "16°",
                FontId::proportional(11.5),
                TEXT_MUTED,
            );
            painter.text(
                pos2(
                    center.x + label_r * max_a.cos() + 4.0,
                    center.y + label_r * max_a.sin() + 4.0,
                ),
                Align2::CENTER_CENTER,
                "30°",
                FontId::proportional(11.5),
                TEXT_MUTED,
            );
            let _ = PI;
        }

        ui.add_space(18.0);

        // Plus button vertically centered
        ui.vertical(|ui| {
            ui.add_space(70.0);
            if circle_step_button(ui, Icon::Plus, enabled && *set_temp < 30) {
                *set_temp += 1;
                committed = Some(*set_temp);
            }
        });
    });

    committed
}

pub fn power_pill_button(ui: &mut egui::Ui, power: bool, enabled: bool) -> bool {
    let mut clicked = false;
    ui.vertical_centered(|ui| {
        let size = vec2(176.0, 42.0);
        let sense = if enabled { Sense::click() } else { Sense::hover() };
        let (rect, resp) = ui.allocate_exact_size(size, sense);

        if ui.is_rect_visible(rect) {
            let base = if power { POWER_ON } else { POWER_OFF };
            let fill = if !enabled {
                base.gamma_multiply(0.6)
            } else if resp.is_pointer_button_down_on() {
                base.gamma_multiply(0.85)
            } else if resp.hovered() {
                base.gamma_multiply(1.1)
            } else {
                base
            };

            let painter = ui.painter();
            if power {
                painter.rect_filled(
                    rect.expand(2.0),
                    CornerRadius::same(23),
                    with_alpha(POWER_ON, 45),
                );
            }
            painter.rect_filled(rect, CornerRadius::same(21), fill);

            let fg = Color32::WHITE;
            let label = if power { "ENCENDIDO" } else { "APAGADO" };
            let icon_x = rect.center().x - 48.0;
            draw_icon(painter, pos2(icon_x, rect.center().y), 8.5, Icon::Power, fg);
            painter.text(
                pos2(icon_x + 18.0, rect.center().y),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(14.5),
                fg,
            );
        }

        if enabled && resp.clicked() {
            clicked = true;
        }
    });
    clicked
}

pub fn mode_segmented_bar(
    ui: &mut egui::Ui,
    current: AcMode,
    accent: Color32,
    enabled: bool,
) -> Option<AcMode> {
    let mut selected_new = None;
    let items: [(AcMode, Icon, &str); 5] = [
        (AcMode::Auto, Icon::Auto, "Auto"),
        (AcMode::Cool, Icon::Snowflake, "Frío"),
        (AcMode::Dry, Icon::Drop, "Seco"),
        (AcMode::Fan, Icon::Fan, "Vent."),
        (AcMode::Heat, Icon::Flame, "Calor"),
    ];

    let total_w = ui.available_width();
    let height = 38.0;
    let (outer, _) = ui.allocate_exact_size(vec2(total_w, height), Sense::hover());

    if ui.is_rect_visible(outer) {
        ui.painter()
            .rect_filled(outer, CornerRadius::same(19), CARD_INNER);
        ui.painter().rect_stroke(
            outer,
            CornerRadius::same(19),
            Stroke::new(1.0_f32, BORDER),
            egui::StrokeKind::Inside,
        );
    }

    let seg_w = total_w / items.len() as f32;
    for (idx, (mode, icon, label)) in items.into_iter().enumerate() {
        let seg_rect = egui::Rect::from_min_size(
            pos2(outer.left() + idx as f32 * seg_w, outer.top()),
            vec2(seg_w, height),
        );
        let id = ui.make_persistent_id(("mode_seg", idx));
        let resp = ui.interact(
            seg_rect,
            id,
            if enabled { Sense::click() } else { Sense::hover() },
        );
        let is_active = current == mode;

        if ui.is_rect_visible(seg_rect) {
            let painter = ui.painter();
            let inner = seg_rect.shrink(3.0);
            if is_active {
                painter.rect_filled(inner, CornerRadius::same(16), accent);
            } else if enabled && resp.hovered() {
                painter.rect_filled(inner, CornerRadius::same(16), CARD_HOVER);
            }

            let fg = if is_active {
                Color32::from_rgb(15, 23, 42)
            } else if enabled {
                TEXT_PRIMARY
            } else {
                TEXT_MUTED
            };

            let center = inner.center();
            draw_icon(painter, pos2(center.x - 22.0, center.y), 7.2, icon, fg);
            painter.text(
                pos2(center.x - 10.0, center.y),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(13.0),
                fg,
            );
        }

        if enabled && resp.clicked() && !is_active {
            selected_new = Some(mode);
        }
    }

    selected_new
}

pub fn fan_speed_bar(
    ui: &mut egui::Ui,
    current: FanSpeed,
    accent: Color32,
    enabled: bool,
) -> Option<FanSpeed> {
    let mut selected_new = None;
    let items: [(FanSpeed, &str); 6] = [
        (FanSpeed::Auto, "Auto"),
        (FanSpeed::Low, "1"),
        (FanSpeed::MediumLow, "2"),
        (FanSpeed::Medium, "3"),
        (FanSpeed::MediumHigh, "4"),
        (FanSpeed::High, "5"),
    ];

    ui.horizontal(|ui| {
        let (icon_rect, _) = ui.allocate_exact_size(vec2(28.0, 34.0), Sense::hover());
        if ui.is_rect_visible(icon_rect) {
            draw_icon(ui.painter(), icon_rect.center(), 10.0, Icon::Fan, TEXT_PRIMARY);
        }
        ui.add_space(4.0);

        let bar_w = ui.available_width();
        let height = 34.0;
        let (outer, _) = ui.allocate_exact_size(vec2(bar_w, height), Sense::hover());

        if ui.is_rect_visible(outer) {
            ui.painter()
                .rect_filled(outer, CornerRadius::same(10), CARD_INNER);
            ui.painter().rect_stroke(
                outer,
                CornerRadius::same(10),
                Stroke::new(1.0_f32, BORDER),
                egui::StrokeKind::Inside,
            );
        }

        let seg_w = bar_w / items.len() as f32;
        for (idx, (speed, label)) in items.into_iter().enumerate() {
            let seg_rect = egui::Rect::from_min_size(
                pos2(outer.left() + idx as f32 * seg_w, outer.top()),
                vec2(seg_w, height),
            );
            let id = ui.make_persistent_id(("fan_seg", idx));
            let resp = ui.interact(
                seg_rect,
                id,
                if enabled { Sense::click() } else { Sense::hover() },
            );
            let is_active = current == speed;

            if ui.is_rect_visible(seg_rect) {
                let painter = ui.painter();
                let inner = seg_rect.shrink(3.0);
                if is_active {
                    painter.rect_filled(inner, CornerRadius::same(7), accent);
                } else if enabled && resp.hovered() {
                    painter.rect_filled(inner, CornerRadius::same(7), CARD_HOVER);
                }

                let fg = if is_active {
                    Color32::from_rgb(15, 23, 42)
                } else {
                    TEXT_PRIMARY
                };
                painter.text(
                    inner.center(),
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(13.0),
                    fg,
                );
            }

            if enabled && resp.clicked() && !is_active {
                selected_new = Some(speed);
            }
        }
    });

    selected_new
}

pub fn feature_tile(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    active: bool,
    accent: Color32,
    width: f32,
    enabled: bool,
) -> bool {
    let size = vec2(width, 40.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);

    if ui.is_rect_visible(rect) {
        let bg = if active {
            with_alpha(accent, 32)
        } else if enabled && resp.hovered() {
            CARD_HOVER
        } else {
            CARD_INNER
        };
        let border = if active {
            accent
        } else if enabled && resp.hovered() {
            Color32::from_rgb(68, 80, 104)
        } else {
            BORDER
        };
        let fg = if active {
            accent
        } else if enabled {
            TEXT_PRIMARY
        } else {
            TEXT_MUTED
        };

        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(11), bg);
        painter.rect_stroke(
            rect,
            CornerRadius::same(11),
            Stroke::new(if active { 1.4 } else { 1.0_f32 }, border),
            egui::StrokeKind::Inside,
        );

        let icon_pos = pos2(rect.left() + 22.0, rect.center().y);
        draw_icon(painter, icon_pos, 8.0, icon, fg);
        painter.text(
            pos2(rect.left() + 38.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(13.0),
            if active { TEXT_PRIMARY } else { TEXT_MUTED },
        );
    }

    enabled && resp.clicked()
}
