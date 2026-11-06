use eframe::egui::{self, RichText};

use super::widgets::{self, card};

pub fn show(ui: &mut egui::Ui) {
    widgets::page_title(ui, "About TuxLR", "");
    card(ui, "TuxLR", |ui| {
        ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
        ui.label("A standalone Linux desktop app for the TC-Helicon GoXLR and GoXLR Mini.");
        ui.label(RichText::new("TuxLR is not affiliated with or endorsed by TC-Helicon or Music Tribe.").weak());
    });

    card(ui, "Built on the GoXLR Utility", |ui| {
        ui.label(
            "All device communication, profile handling and the audio engine in TuxLR come from the \
             GoXLR Utility project by the GoXLR-on-Linux team. TuxLR embeds that work directly so no \
             separate daemon or web UI is needed.",
        );
        ui.add_space(4.0);
        ui.hyperlink_to("github.com/GoXLR-on-Linux/goxlr-utility", "https://github.com/GoXLR-on-Linux/goxlr-utility");
        ui.label(format!(
            "Engine based on GoXLR Utility v{} (commit {})",
            tuxlr_engine::UPSTREAM_VERSION,
            &tuxlr_engine::UPSTREAM_COMMIT[..12]
        ));
        ui.label("Copyright (c) 2022 – 2024 Craig McLure, Nathan Adams, Lars Mühlbauer and contributors. MIT License.");
    });

    card(ui, "Third-party content", |ui| {
        ui.label(
            "The bundled default profiles, mic profile, effect presets and icons come from the official \
             GoXLR App and are included with the permission of TC-Helicon under the Music Tribe license.",
        );
        ui.label("Copyright 2022 Music Tribe Brands CA Ltd.");
        ui.add_space(4.0);
        ui.label(RichText::new("Full license texts: LICENSE, NOTICE.md and vendor/goxlr-utility/ in the source tree.").weak());
    });
}
