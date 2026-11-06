use eframe::egui::{self, Color32, CornerRadius, Stroke};

pub const ACCENT: Color32 = Color32::from_rgb(0x2E, 0xC4, 0xB6);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(0x1B, 0x6F, 0x68);
pub const OK: Color32 = Color32::from_rgb(0x4C, 0xC3, 0x6A);
pub const WARN: Color32 = Color32::from_rgb(0xE8, 0xB3, 0x3A);
pub const ERROR: Color32 = Color32::from_rgb(0xF0, 0x5D, 0x5E);
pub const MUTED: Color32 = Color32::from_rgb(0xD9, 0x48, 0x4A);
pub const PANEL: Color32 = Color32::from_rgb(0x1E, 0x21, 0x26);
pub const CARD: Color32 = Color32::from_rgb(0x26, 0x2A, 0x31);

pub fn apply(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = CARD;
    visuals.extreme_bg_color = Color32::from_rgb(0x15, 0x17, 0x1B);
    visuals.selection.bg_fill = ACCENT_DIM;
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.hyperlink_color = ACCENT;
    visuals.widgets.active.bg_fill = ACCENT_DIM;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    for w in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(6);
    }
    ctx.set_visuals(visuals);

    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
        style.spacing.slider_width = 220.0;
        style.spacing.interact_size.y = 24.0;
    });
}
