//! Reusable widgets for TuxLR pages.

use std::fmt::Display;
use std::ops::RangeInclusive;

use eframe::egui::{self, Color32, RichText, Sense, Stroke};
use strum::IntoEnumIterator;
use tuxlr_engine::ipc::GoXLRCommand;

use super::PageCtx;
use super::theme;

/// A titled, padded card that groups related controls.
pub fn card<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(theme::CARD)
        .corner_radius(10)
        .inner_margin(14)
        .outer_margin(egui::Margin::symmetric(0, 6))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(title).size(16.0).strong());
            ui.add_space(6.0);
            add(ui)
        })
        .inner
}

pub fn page_title(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(title).size(24.0).strong());
    if !subtitle.is_empty() {
        ui.label(RichText::new(subtitle).weak());
    }
    ui.add_space(8.0);
}

pub fn banner(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .fill(Color32::from_rgb(0x4A, 0x3A, 0x12))
        .corner_radius(8)
        .inner_margin(10)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(text).color(theme::WARN));
        });
    ui.add_space(6.0);
}

/// Turn `CamelCase` / upstream enum names into human-friendly labels.
pub fn pretty(value: impl Display) -> String {
    let raw = value.to_string();
    let special = match raw.as_str() {
        "BroadcastMix" => Some("Stream"),
        "StreamMix2" => Some("Stream Mix 2"),
        "ChatMic" => Some("Chat Mic"),
        "LineIn" => Some("Line In"),
        "LineOut" => Some("Line Out"),
        "MicMonitor" => Some("Mic Monitor"),
        "Microphone" => Some("Mic"),
        "Samples" | "Sample" => Some("Sample"),
        "MutedToX" => Some("Muted (to target)"),
        "MutedToAll" => Some("Muted (all)"),
        "ToStream" => Some("Mute to Stream"),
        "ToVoiceChat" => Some("Mute to Voice Chat"),
        "ToPhones" => Some("Mute to Headphones"),
        "ToLineOut" => Some("Mute to Line Out"),
        "ToStream2" => Some("Mute to Stream 2"),
        "ToStreams" => Some("Mute to both Streams"),
        "All" => Some("Mute All"),
        "TwoColour" => Some("Two colour"),
        "GradientMeter" => Some("Gradient + meter"),
        "StreamNoMusic" => Some("Stream (no music) for VODs"),
        _ => None,
    };
    if let Some(s) = special {
        return s.to_string();
    }

    let mut s = raw.clone();
    for prefix in ["Equalizer", "Gate", "Comp"] {
        if let Some(rest) = s.strip_prefix(prefix)
            && rest.starts_with(|c: char| c.is_ascii_digit())
        {
            s = rest.to_string();
        }
    }
    if let Some(rest) = s.strip_prefix("Ratio") {
        return format!("{}:1", rest.replace('_', "."));
    }

    // Insert spaces at lower->Upper and letter->digit boundaries.
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if i > 0 {
            let p = chars[i - 1];
            let boundary = (p.is_lowercase() && c.is_uppercase())
                || (p.is_ascii_digit() && c.is_uppercase() && !raw.starts_with("Equalizer"))
                || (p.is_alphabetic() && c.is_ascii_digit() && !s.ends_with("ms"))
                || (p.is_alphabetic() && c.is_ascii_digit() && s.starts_with("Preset"));
            if boundary {
                out.push(' ');
            }
        }
        out.push(*c);
    }
    out
}

/// A combo box for any strum enum. Returns the newly selected value, if changed.
pub fn enum_combo<T>(ui: &mut egui::Ui, id: &str, current: T) -> Option<T>
where
    T: IntoEnumIterator + Display + PartialEq + Copy,
{
    let mut result = None;
    egui::ComboBox::from_id_salt(id)
        .selected_text(pretty(current))
        .show_ui(ui, |ui| {
            for v in T::iter() {
                if ui.selectable_label(v == current, pretty(v)).clicked() && v != current {
                    result = Some(v);
                }
            }
        });
    result
}

/// Horizontal labelled integer slider which sends a (coalesced) command as it moves.
#[allow(clippy::too_many_arguments)]
pub fn slider(
    ui: &mut egui::Ui,
    pc: &mut PageCtx,
    key: &str,
    label: &str,
    device_value: i32,
    range: RangeInclusive<i32>,
    suffix: &str,
    make: impl Fn(i32) -> GoXLRCommand,
) {
    ui.horizontal(|ui| {
        ui.add_sized([150.0, 20.0], egui::Label::new(label));
        // Leave room for the value box so sliders fit inside narrow (two-column) cards.
        ui.spacing_mut().slider_width = (ui.available_width() - 90.0).clamp(80.0, 260.0);
        let mut v = pc.ov.get(key, device_value as f64) as i32;
        let r = ui.add(egui::Slider::new(&mut v, range).suffix(suffix));
        if r.changed() {
            pc.ov.set(key, v as f64);
            pc.send_keyed(key, make(v));
        }
    });
}

/// Labelled row with an arbitrary control.
pub fn row(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized([150.0, 20.0], egui::Label::new(label));
        add(ui);
    });
}

/// A vertical channel strip slider (0–255 device value shown as a percentage).
pub fn channel_strip(
    ui: &mut egui::Ui,
    pc: &mut PageCtx,
    key: &str,
    label: &str,
    device_value: u8,
    make: impl Fn(u8) -> GoXLRCommand,
    extra: impl FnOnce(&mut egui::Ui, &mut PageCtx),
) {
    ui.vertical(|ui| {
        ui.set_width(78.0);
        ui.vertical_centered(|ui| {
            let mut v = pc.ov.get(key, device_value as f64) as u8;
            let pct = (v as f32 / 255.0 * 100.0).round() as u32;
            ui.label(RichText::new(format!("{pct}%")).monospace());
            ui.spacing_mut().slider_width = 200.0;
            let r = ui.add(
                egui::Slider::new(&mut v, 0..=255)
                    .vertical()
                    .show_value(false)
                    .trailing_fill(true),
            );
            if r.changed() {
                pc.ov.set(key, v as f64);
                pc.send_keyed(key, make(v));
            }
            ui.add_space(2.0);
            ui.label(RichText::new(label).strong());
            extra(ui, pc);
        });
    });
}

/// A horizontal level meter (0.0 – 1.0).
pub fn meter(ui: &mut egui::Ui, level: f64, width: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 12.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, Color32::from_rgb(0x12, 0x14, 0x17));
    let level = level.clamp(0.0, 1.0) as f32;
    let mut fill = rect;
    fill.set_width(rect.width() * level);
    let colour = if level > 0.9 {
        theme::ERROR
    } else if level > 0.7 {
        theme::WARN
    } else {
        theme::OK
    };
    painter.rect_filled(fill, 4.0, colour);
    painter.rect_stroke(rect, 4.0, Stroke::new(1.0_f32, Color32::from_gray(60)), egui::StrokeKind::Inside);
}

/// Parse an `RRGGBB` hex string.
pub fn parse_hex(hex: &str) -> [u8; 3] {
    let hex = hex.trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0);
    [byte(0), byte(2), byte(4)]
}

pub fn to_hex(rgb: [u8; 3]) -> String {
    format!("{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

/// A colour swatch button. Returns the new hex value while it changes.
pub fn colour_button(ui: &mut egui::Ui, pc: &mut PageCtx, key: &str, hex: &str) -> Option<String> {
    let device = parse_hex(hex);
    let packed = u32::from_be_bytes([0, device[0], device[1], device[2]]) as f64;
    let current = pc.ov.get(key, packed) as u32;
    let b = current.to_be_bytes();
    let mut rgb = [b[1], b[2], b[3]];
    if ui.color_edit_button_srgb(&mut rgb).changed() {
        let packed = u32::from_be_bytes([0, rgb[0], rgb[1], rgb[2]]) as f64;
        pc.ov.set(key, packed);
        return Some(to_hex(rgb));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_names() {
        assert_eq!(pretty("LineIn"), "Line In");
        assert_eq!(pretty("Gate10ms"), "10ms");
        assert_eq!(pretty("Ratio3_2"), "3.2:1");
        assert_eq!(pretty("Equalizer1KHz"), "1KHz");
        assert_eq!(pretty("Preset1"), "Preset 1");
        assert_eq!(pretty("BroadcastMix"), "Stream");
        assert_eq!(pretty("Fader1Mute"), "Fader 1 Mute");
        assert_eq!(pretty("Equalizer8KHz"), "8KHz");
    }

    #[test]
    fn hex_roundtrip() {
        assert_eq!(to_hex(parse_hex("00FFAA")), "00FFAA");
        assert_eq!(parse_hex("garbage"), [0, 0, 0]);
    }
}
