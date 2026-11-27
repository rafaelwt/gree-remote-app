use eframe::egui::{self, Color32, CornerRadius, Margin, Stroke};

use crate::gree::protocol::AcMode;

pub const BG_DEEP: Color32 = Color32::from_rgb(18, 21, 28);
pub const CARD_BG: Color32 = Color32::from_rgb(27, 32, 43);
pub const CARD_INNER: Color32 = Color32::from_rgb(22, 26, 36);
pub const CARD_HOVER: Color32 = Color32::from_rgb(36, 43, 58);
pub const BORDER: Color32 = Color32::from_rgb(44, 52, 70);
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(241, 245, 249);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(148, 163, 184);
pub const POWER_ON: Color32 = Color32::from_rgb(34, 197, 94);
pub const POWER_OFF: Color32 = Color32::from_rgb(55, 65, 81);
pub const ERROR: Color32 = Color32::from_rgb(248, 113, 113);

pub fn mode_accent(mode: AcMode) -> Color32 {
    match mode {
        AcMode::Cool => Color32::from_rgb(56, 189, 248), // Cyan #38BDF8
        AcMode::Heat => Color32::from_rgb(249, 115, 22), // Amber #F97316
        AcMode::Dry => Color32::from_rgb(167, 139, 250), // Violet #A78BFA
        AcMode::Fan => Color32::from_rgb(45, 212, 191),  // Teal #2DD4BF
        AcMode::Auto => Color32::from_rgb(96, 165, 250), // Sky #60A5FA
    }
}

pub fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD_BG)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(14))
}

pub fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG_DEEP;
    visuals.window_fill = CARD_BG;
    visuals.extreme_bg_color = CARD_INNER;
    visuals.faint_bg_color = CARD_BG;
    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.window_corner_radius = CornerRadius::same(12);

    visuals.widgets.noninteractive.bg_fill = CARD_BG;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_MUTED);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(8);

    visuals.widgets.inactive.bg_fill = CARD_INNER;
    visuals.widgets.inactive.weak_bg_fill = CARD_INNER;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(8);

    visuals.widgets.hovered.bg_fill = CARD_HOVER;
    visuals.widgets.hovered.weak_bg_fill = CARD_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(71, 85, 105));
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(8);

    visuals.widgets.active.bg_fill = Color32::from_rgb(45, 55, 72);
    visuals.widgets.active.weak_bg_fill = Color32::from_rgb(45, 55, 72);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(96, 165, 250));
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.active.corner_radius = CornerRadius::same(8);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    ctx.set_style(style);
}
