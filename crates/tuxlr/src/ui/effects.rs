//! Voice effects – only present on the full-size GoXLR.

use eframe::egui::{self, RichText};
use strum::IntoEnumIterator;
use tuxlr_engine::ipc::GoXLRCommand;
use tuxlr_engine::types::{EffectBankPresets, GenderStyle, PitchStyle};

use super::widgets::{self, card, pretty};
use super::PageCtx;

pub fn show(ui: &mut egui::Ui, pc: &mut PageCtx) {
    widgets::page_title(ui, "Effects", "Voice effects for the full-size GoXLR.");
    let Some(fx) = pc.mixer.effects.clone() else {
        ui.label("This device has no effects.");
        return;
    };

    card(ui, "Effects bank", |ui| {
        let mut enabled = fx.is_enabled;
        if ui.checkbox(&mut enabled, "Effects enabled (FX button)").changed() {
            pc.send(GoXLRCommand::SetFXEnabled(enabled));
        }
        ui.horizontal_wrapped(|ui| {
            for preset in EffectBankPresets::iter() {
                let name = fx.preset_names.get(&preset).cloned().unwrap_or_else(|| pretty(preset));
                if ui.selectable_label(fx.active_preset == preset, name).clicked() && fx.active_preset != preset {
                    pc.send(GoXLRCommand::SetActiveEffectPreset(preset));
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Load preset file into this slot:");
            let mut presets = pc.status.files.presets.clone();
            presets.sort();
            egui::ComboBox::from_id_salt("preset-file").selected_text("Choose…").show_ui(ui, |ui| {
                for p in presets {
                    if ui.selectable_label(false, &p).clicked() {
                        pc.send(GoXLRCommand::LoadEffectPreset(p));
                    }
                }
            });
            if ui.button("💾 Save slot").clicked() {
                pc.send(GoXLRCommand::SaveActivePreset());
            }
        });
    });

    let cur = fx.current.clone();
    ui.columns(2, |cols| {
        card(&mut cols[0], "Reverb", |ui| {
            widgets::row(ui, "Style", |ui| {
                if let Some(s) = widgets::enum_combo(ui, "rev-style", cur.reverb.style) {
                    pc.send(GoXLRCommand::SetReverbStyle(s));
                }
            });
            widgets::slider(ui, pc, "rev-amt", "Amount", cur.reverb.amount as i32, 0..=100, "%", |v| {
                GoXLRCommand::SetReverbAmount(v as u8)
            });
        });
        card(&mut cols[1], "Echo", |ui| {
            widgets::row(ui, "Style", |ui| {
                if let Some(s) = widgets::enum_combo(ui, "echo-style", cur.echo.style) {
                    pc.send(GoXLRCommand::SetEchoStyle(s));
                }
            });
            widgets::slider(ui, pc, "echo-amt", "Amount", cur.echo.amount as i32, 0..=100, "%", |v| {
                GoXLRCommand::SetEchoAmount(v as u8)
            });
        });
        card(&mut cols[0], "Pitch", |ui| {
            widgets::row(ui, "Style", |ui| {
                if let Some(s) = widgets::enum_combo(ui, "pitch-style", cur.pitch.style) {
                    pc.send(GoXLRCommand::SetPitchStyle(s));
                }
            });
            let range = match (cur.hard_tune.is_enabled, cur.pitch.style) {
                (true, PitchStyle::Narrow) => -1..=1,
                (true, PitchStyle::Wide) => -2..=2,
                (false, PitchStyle::Narrow) => -12..=12,
                (false, PitchStyle::Wide) => -24..=24,
            };
            widgets::slider(ui, pc, "pitch-amt", "Amount", cur.pitch.amount as i32, range, "", |v| {
                GoXLRCommand::SetPitchAmount(v as i8)
            });
        });
        card(&mut cols[1], "Gender", |ui| {
            widgets::row(ui, "Style", |ui| {
                if let Some(s) = widgets::enum_combo(ui, "gender-style", cur.gender.style) {
                    pc.send(GoXLRCommand::SetGenderStyle(s));
                }
            });
            let range = match cur.gender.style {
                GenderStyle::Narrow => -12..=12,
                GenderStyle::Medium => -25..=25,
                GenderStyle::Wide => -50..=50,
            };
            widgets::slider(ui, pc, "gender-amt", "Amount", cur.gender.amount as i32, range, "", |v| {
                GoXLRCommand::SetGenderAmount(v as i8)
            });
        });
    });

    card(ui, "Megaphone · Robot · Hard Tune", |ui| {
        egui::Grid::new("fx-toggles").num_columns(4).spacing([18.0, 8.0]).show(ui, |ui| {
            let mut m = cur.megaphone.is_enabled;
            if ui.checkbox(&mut m, RichText::new("Megaphone").strong()).changed() {
                pc.send(GoXLRCommand::SetMegaphoneEnabled(m));
            }
            if let Some(s) = widgets::enum_combo(ui, "mega-style", cur.megaphone.style) {
                pc.send(GoXLRCommand::SetMegaphoneStyle(s));
            }
            let mut amt = pc.ov.get("mega-amt", cur.megaphone.amount as f64) as u8;
            if ui.add(egui::Slider::new(&mut amt, 0..=100).suffix("%")).changed() {
                pc.ov.set("mega-amt", amt as f64);
                pc.send_keyed("mega-amt", GoXLRCommand::SetMegaphoneAmount(amt));
            }
            ui.end_row();

            let mut r = cur.robot.is_enabled;
            if ui.checkbox(&mut r, RichText::new("Robot").strong()).changed() {
                pc.send(GoXLRCommand::SetRobotEnabled(r));
            }
            if let Some(s) = widgets::enum_combo(ui, "robot-style", cur.robot.style) {
                pc.send(GoXLRCommand::SetRobotStyle(s));
            }
            ui.label("");
            ui.end_row();

            let mut h = cur.hard_tune.is_enabled;
            if ui.checkbox(&mut h, RichText::new("Hard Tune").strong()).changed() {
                pc.send(GoXLRCommand::SetHardTuneEnabled(h));
            }
            if let Some(s) = widgets::enum_combo(ui, "ht-style", cur.hard_tune.style) {
                pc.send(GoXLRCommand::SetHardTuneStyle(s));
            }
            let mut amt = pc.ov.get("ht-amt", cur.hard_tune.amount as f64) as u8;
            if ui.add(egui::Slider::new(&mut amt, 0..=100).suffix("%")).changed() {
                pc.ov.set("ht-amt", amt as f64);
                pc.send_keyed("ht-amt", GoXLRCommand::SetHardTuneAmount(amt));
            }
            ui.end_row();
        });
    });
}
