use eframe::egui::{self, RichText};
use strum::IntoEnumIterator;
use tuxlr_engine::ipc::GoXLRCommand;
use tuxlr_engine::types::{ChannelName, FaderName, Mix, MuteFunction, MuteState, OutputDevice, SubMixChannelName};

use super::widgets::{self, card, pretty};
use super::{PageCtx, theme};

const INPUTS: [ChannelName; 8] = [
    ChannelName::Mic,
    ChannelName::LineIn,
    ChannelName::Console,
    ChannelName::System,
    ChannelName::Game,
    ChannelName::Chat,
    ChannelName::Sample,
    ChannelName::Music,
];
const OUTPUTS: [ChannelName; 3] = [ChannelName::Headphones, ChannelName::MicMonitor, ChannelName::LineOut];

fn fader_for(pc: &PageCtx, channel: ChannelName) -> Option<FaderName> {
    FaderName::iter().find(|f| pc.mixer.fader_status[*f].channel == channel)
}

fn mute_button(ui: &mut egui::Ui, pc: &PageCtx, channel: ChannelName) {
    let Some(fader) = fader_for(pc, channel) else {
        ui.label(RichText::new(" ").small());
        return;
    };
    let status = &pc.mixer.fader_status[fader];
    let muted = status.mute_state != MuteState::Unmuted;
    let text = if muted { "🔇 Muted" } else { "🔊" };
    let mut button = egui::Button::new(RichText::new(text).small());
    if muted {
        button = button.fill(theme::MUTED);
    }
    if ui
        .add(button)
        .on_hover_text(format!("Fader {fader} · {}", pretty(status.mute_type)))
        .clicked()
    {
        let new_state = if muted {
            MuteState::Unmuted
        } else if status.mute_type == MuteFunction::All {
            MuteState::MutedToAll
        } else {
            MuteState::MutedToX
        };
        pc.send(GoXLRCommand::SetFaderMuteState(fader, new_state));
    }
}

pub fn show(ui: &mut egui::Ui, pc: &mut PageCtx) {
    widgets::page_title(ui, "Mixer", "Channel volumes. Channels on a hardware fader can be muted here too.");

    card(ui, "Inputs", |ui| {
        ui.horizontal(|ui| {
            for channel in INPUTS {
                if channel == ChannelName::Sample && pc.is_mini() {
                    continue;
                }
                let key = format!("vol-{channel}");
                let value = pc.mixer.levels.volumes[channel];
                widgets::channel_strip(
                    ui,
                    pc,
                    &key,
                    &pretty(channel),
                    value,
                    |v| GoXLRCommand::SetVolume(channel, v),
                    |ui, pc| mute_button(ui, pc, channel),
                );
            }
        });
    });

    card(ui, "Outputs", |ui| {
        ui.horizontal(|ui| {
            for channel in OUTPUTS {
                let key = format!("vol-{channel}");
                let value = pc.mixer.levels.volumes[channel];
                widgets::channel_strip(
                    ui,
                    pc,
                    &key,
                    &pretty(channel),
                    value,
                    |v| GoXLRCommand::SetVolume(channel, v),
                    |_, _| {},
                );
            }
            ui.add_space(24.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("Mic input level").strong());
                widgets::meter(ui, pc.mic_level, 260.0);
                ui.add_space(12.0);
                if pc.mixer.levels.submix_supported {
                    ui.label(RichText::new("Headphone monitor").strong());
                    if let Some(out) = widgets::enum_combo(ui, "monitor_mix", pc.mixer.levels.output_monitor) {
                        pc.send(GoXLRCommand::SetMonitorMix(out));
                    }
                }
            });
        });
    });

    submixes(ui, pc);
}

fn submixes(ui: &mut egui::Ui, pc: &mut PageCtx) {
    if !pc.mixer.levels.submix_supported {
        return;
    }
    let submix = pc.mixer.levels.submix.clone();
    card(ui, "Submixes (Mix B)", |ui| {
        let mut enabled = submix.is_some();
        if ui
            .checkbox(&mut enabled, "Enable submixes – give outputs an independent second mix")
            .changed()
        {
            pc.send(GoXLRCommand::SetSubMixEnabled(enabled));
        }
        let Some(submix) = submix else {
            return;
        };
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            for channel in INPUTS {
                let Some(sub) = SubMixChannelName::iter().find(|s| s.to_string() == channel.to_string()) else {
                    continue;
                };
                if channel == ChannelName::Sample && pc.is_mini() {
                    continue;
                }
                let state = &submix.inputs[sub];
                let key = format!("sub-{channel}");
                let linked = state.linked;
                widgets::channel_strip(
                    ui,
                    pc,
                    &key,
                    &pretty(channel),
                    state.volume,
                    |v| GoXLRCommand::SetSubMixVolume(channel, v),
                    |ui, pc| {
                        let mut l = linked;
                        if ui.checkbox(&mut l, "🔗").on_hover_text("Link to Mix A").changed() {
                            pc.send(GoXLRCommand::SetSubMixLinked(channel, l));
                        }
                    },
                );
            }
        });
        ui.add_space(10.0);
        ui.label(RichText::new("Which mix each output hears").strong());
        egui::Grid::new("submix_outputs").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
            for output in OutputDevice::iter() {
                if output == OutputDevice::Sampler && pc.is_mini() {
                    continue;
                }
                ui.label(pretty(output));
                let current = submix.outputs[output];
                ui.horizontal(|ui| {
                    for mix in Mix::iter() {
                        if ui.selectable_label(current == mix, format!("Mix {mix}")).clicked() && current != mix {
                            pc.send(GoXLRCommand::SetSubMixOutputMix(output, mix));
                        }
                    }
                });
                ui.end_row();
            }
        });
    });
}
