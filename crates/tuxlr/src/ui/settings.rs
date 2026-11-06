use eframe::egui::{self, RichText};
use tuxlr_engine::ipc::{DaemonCommand, LogLevel, PathTypes};

use super::widgets::{self, card};
use super::PageCtx;
use crate::bridge::Bridge;

pub fn show(ui: &mut egui::Ui, bridge: &Bridge, pc: Option<&PageCtx>) {
    widgets::page_title(ui, "Settings", "");

    let status = bridge.shared.lock().unwrap().status.clone();
    let Some(status) = status else {
        ui.spinner();
        return;
    };
    let config = &status.config;

    card(ui, "Application", |ui| {
        let mut autostart = config.autostart_enabled;
        if ui.checkbox(&mut autostart, "Start TuxLR (minimised) when I log in").changed() {
            bridge.send_daemon(DaemonCommand::SetAutoStartEnabled(autostart));
        }
        widgets::row(ui, "Log level", |ui| {
            let levels = [
                LogLevel::Off,
                LogLevel::Error,
                LogLevel::Warn,
                LogLevel::Info,
                LogLevel::Debug,
                LogLevel::Trace,
            ];
            egui::ComboBox::from_id_salt("log-level")
                .selected_text(format!("{:?}", config.log_level))
                .show_ui(ui, |ui| {
                    for l in levels {
                        if ui.selectable_label(l == config.log_level, format!("{l:?}")).clicked() {
                            bridge.send_daemon(DaemonCommand::SetLogLevel(l));
                        }
                    }
                });
            ui.label(RichText::new("(applies on next start)").weak());
        });
    });

    card(ui, "Folders", |ui| {
        let paths = &status.paths;
        egui::Grid::new("paths").num_columns(3).spacing([12.0, 6.0]).show(ui, |ui| {
            for (label, path, kind) in [
                ("Profiles", &paths.profile_directory, PathTypes::Profiles),
                ("Mic profiles", &paths.mic_profile_directory, PathTypes::MicProfiles),
                ("Effect presets", &paths.presets_directory, PathTypes::Presets),
                ("Samples", &paths.samples_directory, PathTypes::Samples),
                ("Icons", &paths.icons_directory, PathTypes::Icons),
                ("Logs", &paths.logs_directory, PathTypes::Logs),
            ] {
                ui.label(label);
                ui.label(RichText::new(path.display().to_string()).monospace().weak());
                if ui.button("Open").clicked() {
                    bridge.send_daemon(DaemonCommand::OpenPath(kind));
                }
                ui.end_row();
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Restore bundled defaults:");
            for (label, kind) in [
                ("Profiles", PathTypes::Profiles),
                ("Mic profiles", PathTypes::MicProfiles),
                ("Presets", PathTypes::Presets),
                ("Icons", PathTypes::Icons),
            ] {
                if ui.button(label).on_hover_text("Re-creates any missing default files").clicked() {
                    bridge.send_daemon(DaemonCommand::RecoverDefaults(kind));
                }
            }
        });
    });

    if let Some(pc) = pc {
        let hw = &pc.mixer.hardware;
        card(ui, "Device", |ui| {
            egui::Grid::new("hw").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
                ui.label("Model");
                ui.label(format!("{} ({:?})", hw.usb_device.product_name, hw.colour_way));
                ui.end_row();
                ui.label("Serial");
                ui.label(&hw.serial_number);
                ui.end_row();
                ui.label("Manufactured");
                ui.label(&hw.manufactured_date);
                ui.end_row();
                ui.label("Firmware");
                ui.label(hw.versions.firmware.to_string());
                ui.end_row();
                ui.label("DICE");
                ui.label(hw.versions.dice.to_string());
                ui.end_row();
                ui.label("USB");
                ui.label(format!("bus {} · address {}", hw.usb_device.bus_number, hw.usb_device.address));
                ui.end_row();
                if let Some(latest) = config
                    .latest_firmware
                    .as_ref()
                    .and_then(|l| l[hw.device_type].as_ref())
                {
                    ui.label("Latest firmware");
                    ui.label(latest.version.to_string());
                    ui.end_row();
                }
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new("Firmware updates are not offered in TuxLR yet – use the official tools.")
                    .weak()
                    .small(),
            );
        });
    }
}
