use eframe::egui::{self, RichText};
use strum::IntoEnumIterator;
use tuxlr_engine::ipc::GoXLRCommand;
use tuxlr_engine::types::{ChannelName, FaderName, MuteFunction, MuteState};

use super::widgets::{self, card, pretty};
use super::{PageCtx, theme};

fn mute_state_selector(ui: &mut egui::Ui, current: MuteState, function: MuteFunction) -> Option<MuteState> {
    let mut result = None;
    ui.horizontal(|ui| {
        let options: &[(MuteState, &str)] = if function == MuteFunction::All {
            &[(MuteState::Unmuted, "Unmuted"), (MuteState::MutedToAll, "Muted")]
        } else {
            &[
                (MuteState::Unmuted, "Unmuted"),
                (MuteState::MutedToX, "Muted to target"),
                (MuteState::MutedToAll, "Muted to all"),
            ]
        };
        for (state, label) in options {
            let selected = current == *state
                || (function == MuteFunction::All && *state == MuteState::MutedToAll && current == MuteState::MutedToX);
            let mut button = egui::Button::new(*label).selected(selected);
            if selected && *state != MuteState::Unmuted {
                button = button.fill(theme::MUTED);
            }
            if ui.add(button).clicked() && !selected {
                result = Some(*state);
            }
        }
    });
    result
}

pub fn show(ui: &mut egui::Ui, pc: &mut PageCtx) {
    widgets::page_title(ui, "Faders & Mutes", "Assign channels to hardware faders and set mute behaviour.");

    ui.columns(2, |cols| {
        for (i, fader) in FaderName::iter().enumerate() {
            let status = pc.mixer.fader_status[fader].clone();
            let col = &mut cols[i % 2];
            card(col, &format!("Fader {fader}"), |ui| {
                widgets::row(ui, "Channel", |ui| {
                    egui::ComboBox::from_id_salt(format!("fader-ch-{fader}"))
                        .selected_text(pretty(status.channel))
                        .show_ui(ui, |ui| {
                            for ch in ChannelName::iter() {
                                if ch == ChannelName::Sample && pc.is_mini() {
                                    continue;
                                }
                                if ui.selectable_label(ch == status.channel, pretty(ch)).clicked() && ch != status.channel {
                                    pc.send(GoXLRCommand::SetFader(fader, ch));
                                }
                            }
                        });
                });
                widgets::row(ui, "Mute button", |ui| {
                    if let Some(f) = widgets::enum_combo(ui, &format!("fader-mf-{fader}"), status.mute_type) {
                        pc.send(GoXLRCommand::SetFaderMuteFunction(fader, f));
                    }
                });
                widgets::row(ui, "State", |ui| {
                    if let Some(s) = mute_state_selector(ui, status.mute_state, status.mute_type) {
                        pc.send(GoXLRCommand::SetFaderMuteState(fader, s));
                    }
                });
                if !pc.is_mini() {
                    let scribble = status.scribble.clone().unwrap_or_default();
                    widgets::row(ui, "Scribble text", |ui| {
                        let id = egui::Id::new(("scribble", fader));
                        let mut text = ui.data_mut(|d| {
                            d.get_temp::<String>(id).unwrap_or_else(|| scribble.bottom_text.clone().unwrap_or_default())
                        });
                        let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(140.0).char_limit(12));
                        ui.data_mut(|d| d.insert_temp(id, text.clone()));
                        if r.lost_focus() {
                            pc.send(GoXLRCommand::SetScribbleText(fader, text));
                            ui.data_mut(|d| d.remove::<String>(id));
                        }
                    });
                    widgets::row(ui, "Invert scribble", |ui| {
                        let mut inv = scribble.inverted;
                        if ui.checkbox(&mut inv, "").changed() {
                            pc.send(GoXLRCommand::SetScribbleInvert(fader, inv));
                        }
                    });
                }
            });
        }
    });

    let cough = pc.mixer.cough_button;
    card(ui, "Cough button", |ui| {
        widgets::row(ui, "Behaviour", |ui| {
            if ui.selectable_label(!cough.is_toggle, "Hold to mute").clicked() && cough.is_toggle {
                pc.send(GoXLRCommand::SetCoughIsHold(true));
            }
            if ui.selectable_label(cough.is_toggle, "Toggle").clicked() && !cough.is_toggle {
                pc.send(GoXLRCommand::SetCoughIsHold(false));
            }
        });
        widgets::row(ui, "Mutes", |ui| {
            if let Some(f) = widgets::enum_combo(ui, "cough-mf", cough.mute_type) {
                pc.send(GoXLRCommand::SetCoughMuteFunction(f));
            }
        });
        widgets::row(ui, "State", |ui| {
            if let Some(s) = mute_state_selector(ui, cough.state, cough.mute_type) {
                pc.send(GoXLRCommand::SetCoughMuteState(s));
            }
        });
    });

    let settings = pc.mixer.settings.clone();
    card(ui, "Mute options", |ui| {
        widgets::slider(ui, pc, "mute-hold", "Hold-to-mute-all", settings.mute_hold_duration as i32, 0..=5000, " ms", |v| {
            GoXLRCommand::SetMuteHoldDuration(v as u16)
        });
        let mut vc = settings.vc_mute_also_mute_cm;
        if ui.checkbox(&mut vc, "Muting to Voice Chat also mutes the Chat Mic output").changed() {
            pc.send(GoXLRCommand::SetVCMuteAlsoMuteCM(vc));
        }
        if !pc.is_mini() {
            let mut lock = settings.lock_faders;
            if ui.checkbox(&mut lock, "Lock faders when muted").changed() {
                pc.send(GoXLRCommand::SetLockFaders(lock));
            }
        }
        ui.label(RichText::new("Hold a fader's mute button this long to mute it to everything.").weak().small());
    });
}
