use eframe::egui::{self, RichText};
use strum::IntoEnumIterator;
use tuxlr_engine::ipc::GoXLRCommand;
use tuxlr_engine::types::{InputDevice, OutputDevice, VodMode};

use super::widgets::{self, card, pretty};
use super::theme;
use super::PageCtx;

pub fn show(ui: &mut egui::Ui, pc: &mut PageCtx) {
    widgets::page_title(ui, "Routing", "Choose which inputs are sent to which outputs.");

    let outputs: Vec<OutputDevice> = OutputDevice::iter()
        .filter(|o| !(pc.is_mini() && *o == OutputDevice::Sampler))
        .filter(|o| *o != OutputDevice::StreamMix2 || pc.mixer.settings.vod_mode == VodMode::StreamNoMusic)
        .collect();
    let inputs: Vec<InputDevice> = InputDevice::iter()
        .filter(|i| !(pc.is_mini() && *i == InputDevice::Samples))
        .collect();

    card(ui, "Routing matrix", |ui| {
        egui::Grid::new("routing")
            .num_columns(outputs.len() + 1)
            .spacing([18.0, 10.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("");
                for o in &outputs {
                    ui.label(RichText::new(pretty(o)).strong());
                }
                ui.end_row();
                for input in &inputs {
                    ui.label(RichText::new(pretty(input)).strong());
                    for output in &outputs {
                        let on = pc.mixer.router[*input][*output];
                        let (text, fill) = if on {
                            ("✔", theme::ACCENT_DIM)
                        } else {
                            ("", egui::Color32::from_rgb(0x30, 0x34, 0x3B))
                        };
                        let button = egui::Button::new(RichText::new(text).size(16.0))
                            .fill(fill)
                            .min_size(egui::vec2(46.0, 26.0));
                        if ui.add(button).clicked() {
                            pc.send(GoXLRCommand::SetRouter(*input, *output, !on));
                        }
                    }
                    ui.end_row();
                }
            });
    });

    card(ui, "VOD mode", |ui| {
        ui.label(
            RichText::new(
                "'Stream (no music)' adds a second stream mix without music, \
                 handy for VOD tracks that avoid copyright claims.",
            )
            .weak(),
        );
        if let Some(mode) = widgets::enum_combo(ui, "vod_mode", pc.mixer.settings.vod_mode) {
            pc.send(GoXLRCommand::SetVodMode(mode));
        }
    });
}
