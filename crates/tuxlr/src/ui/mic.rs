use eframe::egui::{self, RichText};
use strum::IntoEnumIterator;
use tuxlr_engine::ipc::GoXLRCommand;
use tuxlr_engine::types::{EqFrequencies, MicrophoneType, MiniEqFrequencies};

use super::widgets::{self, card, pretty};
use super::PageCtx;

fn mini_eq_range(f: MiniEqFrequencies) -> (f32, f32, f64) {
    match f {
        MiniEqFrequencies::Equalizer90Hz => (30.0, 90.0, 0.5),
        MiniEqFrequencies::Equalizer250Hz => (100.0, 300.0, 0.5),
        MiniEqFrequencies::Equalizer500Hz => (310.0, 800.0, 1.0),
        MiniEqFrequencies::Equalizer1KHz => (800.0, 2500.0, 10.0),
        MiniEqFrequencies::Equalizer3KHz => (2600.0, 5000.0, 10.0),
        MiniEqFrequencies::Equalizer8KHz => (5100.0, 18000.0, 100.0),
    }
}

fn eq_range(f: EqFrequencies) -> (f32, f32, f64) {
    use EqFrequencies::*;
    match f {
        Equalizer31Hz | Equalizer63Hz | Equalizer125Hz | Equalizer250Hz => (30.0, 300.0, 0.5),
        Equalizer500Hz | Equalizer1KHz | Equalizer2KHz => (300.0, 2000.0, 100.0),
        Equalizer4KHz | Equalizer8KHz | Equalizer16KHz => (2000.0, 18000.0, 100.0),
    }
}

pub fn show(ui: &mut egui::Ui, pc: &mut PageCtx) {
    widgets::page_title(ui, "Microphone", "Mic type, gain and processing. Changes go to the active mic profile.");
    let mic = pc.mixer.mic_status.clone();

    card(ui, "Input", |ui| {
        widgets::row(ui, "Microphone type", |ui| {
            for t in MicrophoneType::iter() {
                let label = match t {
                    MicrophoneType::Dynamic => "Dynamic (XLR)",
                    MicrophoneType::Condenser => "Condenser (XLR, +48V)",
                    MicrophoneType::Jack => "3.5mm jack",
                };
                if ui.selectable_label(mic.mic_type == t, label).clicked() && mic.mic_type != t {
                    pc.send(GoXLRCommand::SetMicrophoneType(t));
                }
            }
        });
        let mic_type = mic.mic_type;
        widgets::slider(ui, pc, "mic-gain", "Gain", mic.mic_gains[mic_type] as i32, 0..=72, " dB", move |v| {
            GoXLRCommand::SetMicrophoneGain(mic_type, v as u16)
        });
        widgets::row(ui, "Level", |ui| widgets::meter(ui, pc.mic_level, 300.0));
    });

    ui.columns(2, |cols| {
        card(&mut cols[0], "Noise gate", |ui| {
            let mut enabled = mic.noise_gate.enabled;
            if ui.checkbox(&mut enabled, "Enabled").changed() {
                pc.send(GoXLRCommand::SetGateActive(enabled));
            }
            widgets::slider(ui, pc, "gate-thr", "Threshold", mic.noise_gate.threshold as i32, -59..=0, " dB", |v| {
                GoXLRCommand::SetGateThreshold(v as i8)
            });
            widgets::slider(ui, pc, "gate-att", "Attenuation", mic.noise_gate.attenuation as i32, 0..=100, "%", |v| {
                GoXLRCommand::SetGateAttenuation(v as u8)
            });
            widgets::row(ui, "Attack", |ui| {
                if let Some(v) = widgets::enum_combo(ui, "gate-attack", mic.noise_gate.attack) {
                    pc.send(GoXLRCommand::SetGateAttack(v));
                }
            });
            widgets::row(ui, "Release", |ui| {
                if let Some(v) = widgets::enum_combo(ui, "gate-release", mic.noise_gate.release) {
                    pc.send(GoXLRCommand::SetGateRelease(v));
                }
            });
        });

        card(&mut cols[1], "Compressor", |ui| {
            widgets::slider(ui, pc, "comp-thr", "Threshold", mic.compressor.threshold as i32, -40..=0, " dB", |v| {
                GoXLRCommand::SetCompressorThreshold(v as i8)
            });
            widgets::row(ui, "Ratio", |ui| {
                if let Some(v) = widgets::enum_combo(ui, "comp-ratio", mic.compressor.ratio) {
                    pc.send(GoXLRCommand::SetCompressorRatio(v));
                }
            });
            widgets::row(ui, "Attack", |ui| {
                if let Some(v) = widgets::enum_combo(ui, "comp-attack", mic.compressor.attack) {
                    pc.send(GoXLRCommand::SetCompressorAttack(v));
                }
            });
            widgets::row(ui, "Release", |ui| {
                if let Some(v) = widgets::enum_combo(ui, "comp-release", mic.compressor.release) {
                    pc.send(GoXLRCommand::SetCompressorReleaseTime(v));
                }
            });
            widgets::slider(ui, pc, "comp-makeup", "Make-up gain", mic.compressor.makeup_gain as i32, -6..=24, " dB", |v| {
                GoXLRCommand::SetCompressorMakeupGain(v as i8)
            });
        });
    });

    card(ui, "Equaliser", |ui| {
        ui.horizontal(|ui| {
            if pc.is_mini() {
                for f in MiniEqFrequencies::iter() {
                    let gain = *mic.equaliser_mini.gain.get(&f).unwrap_or(&0);
                    let freq = *mic.equaliser_mini.frequency.get(&f).unwrap_or(&0.0);
                    eq_band(ui, pc, &pretty(f), &format!("mini-{f}"), gain, freq, mini_eq_range(f),
                        move |g| GoXLRCommand::SetEqMiniGain(f, g),
                        move |hz| GoXLRCommand::SetEqMiniFreq(f, hz));
                }
            } else {
                for f in EqFrequencies::iter() {
                    let gain = *mic.equaliser.gain.get(&f).unwrap_or(&0);
                    let freq = *mic.equaliser.frequency.get(&f).unwrap_or(&0.0);
                    eq_band(ui, pc, &pretty(f), &format!("eq-{f}"), gain, freq, eq_range(f),
                        move |g| GoXLRCommand::SetEqGain(f, g),
                        move |hz| GoXLRCommand::SetEqFreq(f, hz));
                }
            }
        });
        ui.label(RichText::new("Frequencies must stay in order (each band between its neighbours).").weak().small());
    });

    card(ui, "Other", |ui| {
        widgets::slider(ui, pc, "deess", "De-esser", pc.mixer.levels.deess as i32, 0..=100, "%", |v| {
            GoXLRCommand::SetDeeser(v as u8)
        });
        widgets::slider(ui, pc, "bleep", "Bleep volume", pc.mixer.levels.bleep as i32, -36..=0, " dB", |v| {
            GoXLRCommand::SetSwearButtonVolume(v as i8)
        });
        if !pc.is_mini() {
            let mut fx = pc.mixer.settings.enable_monitor_with_fx;
            if ui.checkbox(&mut fx, "Monitor mic in headphones when effects are enabled").changed() {
                pc.send(GoXLRCommand::SetMonitorWithFx(fx));
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn eq_band(
    ui: &mut egui::Ui,
    pc: &mut PageCtx,
    label: &str,
    key: &str,
    gain: i8,
    freq: f32,
    (min, max, step): (f32, f32, f64),
    gain_cmd: impl Fn(i8) -> GoXLRCommand,
    freq_cmd: impl Fn(f32) -> GoXLRCommand,
) {
    ui.vertical(|ui| {
        ui.set_width(70.0);
        ui.vertical_centered(|ui| {
            let gkey = format!("{key}-gain");
            let mut g = pc.ov.get(&gkey, gain as f64) as i8;
            ui.label(RichText::new(format!("{g:+} dB")).monospace().small());
            ui.spacing_mut().slider_width = 150.0;
            if ui.add(egui::Slider::new(&mut g, -9..=9).vertical().show_value(false)).changed() {
                pc.ov.set(&gkey, g as f64);
                pc.send_keyed(&gkey, gain_cmd(g));
            }
            ui.label(RichText::new(label).strong());
            let fkey = format!("{key}-freq");
            let mut hz = pc.ov.get(&fkey, freq as f64) as f32;
            let r = ui.add(
                egui::DragValue::new(&mut hz)
                    .range(min..=max)
                    .speed(step.max(1.0))
                    .suffix(" Hz")
                    .max_decimals(1),
            );
            if r.changed() {
                let snapped = ((hz as f64 / step).round() * step) as f32;
                pc.ov.set(&fkey, snapped as f64);
                pc.send_keyed(&fkey, freq_cmd(snapped));
            }
        });
    });
}
