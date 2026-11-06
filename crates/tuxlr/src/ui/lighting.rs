use eframe::egui::{self, RichText};
use strum::IntoEnumIterator;
use tuxlr_engine::ipc::GoXLRCommand;
use tuxlr_engine::types::{Button, FaderName, SimpleColourTargets};

use super::widgets::{self, card, pretty};
use super::PageCtx;

const MINI_BUTTONS: [Button; 6] = [
    Button::Fader1Mute,
    Button::Fader2Mute,
    Button::Fader3Mute,
    Button::Fader4Mute,
    Button::Bleep,
    Button::Cough,
];

pub fn show(ui: &mut egui::Ui, pc: &mut PageCtx) {
    widgets::page_title(ui, "Lighting", "Colours for faders and buttons. Saved with the profile.");
    let lighting = pc.mixer.lighting.clone();

    card(ui, "Global", |ui| {
        let current = lighting
            .simple
            .get(&SimpleColourTargets::Global)
            .map(|c| c.colour_one.clone())
            .unwrap_or_else(|| "00FFFF".into());
        widgets::row(ui, "Global colour", |ui| {
            if let Some(hex) = widgets::colour_button(ui, pc, "global", &current) {
                pc.send_keyed("global", GoXLRCommand::SetGlobalColour(hex));
            }
            ui.label(RichText::new("Sets every light at once").weak());
        });

        if lighting.animation.supported {
            ui.separator();
            let anim = lighting.animation.clone();
            widgets::row(ui, "Animation", |ui| {
                if let Some(m) = widgets::enum_combo(ui, "anim-mode", anim.mode) {
                    pc.send(GoXLRCommand::SetAnimationMode(m));
                }
            });
            widgets::slider(ui, pc, "anim-mod1", "Modifier 1", anim.mod1 as i32, 0..=100, "", |v| {
                GoXLRCommand::SetAnimationMod1(v as u8)
            });
            widgets::slider(ui, pc, "anim-mod2", "Modifier 2", anim.mod2 as i32, 0..=100, "", |v| {
                GoXLRCommand::SetAnimationMod2(v as u8)
            });
            widgets::row(ui, "Waterfall", |ui| {
                if let Some(w) = widgets::enum_combo(ui, "anim-wf", anim.waterfall_direction) {
                    pc.send(GoXLRCommand::SetAnimationWaterfall(w));
                }
            });
        }
    });

    card(ui, "Faders", |ui| {
        egui::Grid::new("fader-lights").num_columns(4).spacing([18.0, 8.0]).show(ui, |ui| {
            ui.label("");
            ui.label(RichText::new("Style").strong());
            ui.label(RichText::new("Top").strong());
            ui.label(RichText::new("Bottom").strong());
            ui.end_row();
            for fader in FaderName::iter() {
                let Some(fl) = lighting.faders.get(&fader).cloned() else { continue };
                ui.label(format!("Fader {fader}"));
                if let Some(s) = widgets::enum_combo(ui, &format!("fstyle-{fader}"), fl.style) {
                    pc.send(GoXLRCommand::SetFaderDisplayStyle(fader, s));
                }
                let k1 = format!("fcol1-{fader}");
                let k2 = format!("fcol2-{fader}");
                if let Some(hex) = widgets::colour_button(ui, pc, &k1, &fl.colours.colour_one) {
                    pc.send_keyed(&k1, GoXLRCommand::SetFaderColours(fader, hex, fl.colours.colour_two.clone()));
                }
                if let Some(hex) = widgets::colour_button(ui, pc, &k2, &fl.colours.colour_two) {
                    pc.send_keyed(&k2, GoXLRCommand::SetFaderColours(fader, fl.colours.colour_one.clone(), hex));
                }
                ui.end_row();
            }
        });
    });

    card(ui, "Buttons", |ui| {
        let buttons: Vec<Button> = if pc.is_mini() { MINI_BUTTONS.to_vec() } else { Button::iter().collect() };
        egui::Grid::new("button-lights").num_columns(4).spacing([18.0, 8.0]).striped(true).show(ui, |ui| {
            ui.label("");
            ui.label(RichText::new("On").strong());
            ui.label(RichText::new("Off").strong());
            ui.label(RichText::new("When off").strong());
            ui.end_row();
            for button in buttons {
                let Some(bl) = lighting.buttons.get(&button).cloned() else { continue };
                ui.label(pretty(button));
                let k1 = format!("bcol1-{button}");
                let k2 = format!("bcol2-{button}");
                if let Some(hex) = widgets::colour_button(ui, pc, &k1, &bl.colours.colour_one) {
                    pc.send_keyed(&k1, GoXLRCommand::SetButtonColours(button, hex, Some(bl.colours.colour_two.clone())));
                }
                if let Some(hex) = widgets::colour_button(ui, pc, &k2, &bl.colours.colour_two) {
                    pc.send_keyed(&k2, GoXLRCommand::SetButtonColours(button, bl.colours.colour_one.clone(), Some(hex)));
                }
                if let Some(s) = widgets::enum_combo(ui, &format!("boff-{button}"), bl.off_style) {
                    pc.send(GoXLRCommand::SetButtonOffStyle(button, s));
                }
                ui.end_row();
            }
        });
    });

    if !lighting.simple.is_empty() {
        card(ui, "Other lights", |ui| {
            egui::Grid::new("simple-lights").num_columns(2).spacing([18.0, 8.0]).show(ui, |ui| {
                for target in SimpleColourTargets::iter() {
                    if target == SimpleColourTargets::Global {
                        continue;
                    }
                    let Some(c) = lighting.simple.get(&target) else { continue };
                    ui.label(pretty(target));
                    let key = format!("simple-{target}");
                    if let Some(hex) = widgets::colour_button(ui, pc, &key, &c.colour_one) {
                        pc.send_keyed(&key, GoXLRCommand::SetSimpleColour(target, hex));
                    }
                    ui.end_row();
                }
            });
        });
    }
}
