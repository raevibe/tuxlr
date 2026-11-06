mod about;
mod effects;
mod faders;
mod lighting;
mod mic;
mod mixer;
mod routing;
mod settings;
mod theme;
pub mod widgets;

use std::time::Duration;

use eframe::egui::{self, RichText};
use tokio::runtime::Handle;
use tuxlr_engine::EngineHandle;
use tuxlr_engine::ipc::{DaemonStatus, GoXLRCommand, MixerStatus};
use tuxlr_engine::types::DeviceType;

use crate::bridge::{Bridge, Overrides};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Mixer,
    Microphone,
    Routing,
    Faders,
    Lighting,
    Effects,
    Settings,
    About,
}

impl Page {
    fn from_name(name: &str) -> Option<Page> {
        Some(match name {
            "mixer" => Page::Mixer,
            "mic" => Page::Microphone,
            "routing" => Page::Routing,
            "faders" => Page::Faders,
            "lighting" => Page::Lighting,
            "effects" => Page::Effects,
            "settings" => Page::Settings,
            "about" => Page::About,
            _ => return None,
        })
    }

    fn label(self) -> &'static str {
        match self {
            Page::Mixer => "Mixer",
            Page::Microphone => "Microphone",
            Page::Routing => "Routing",
            Page::Faders => "Faders & Mutes",
            Page::Lighting => "Lighting",
            Page::Effects => "Effects",
            Page::Settings => "Settings",
            Page::About => "About",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Page::Mixer => "🔊",
            Page::Microphone => "🎤",
            Page::Routing => "🔀",
            Page::Faders => "🔇",
            Page::Lighting => "💡",
            Page::Effects => "✨",
            Page::Settings => "⚙",
            Page::About => "ℹ",
        }
    }
}

/// Everything a page needs to render and issue commands.
pub struct PageCtx<'a> {
    pub bridge: &'a Bridge,
    pub ov: &'a mut Overrides,
    pub serial: &'a str,
    pub mixer: &'a MixerStatus,
    pub status: &'a DaemonStatus,
    pub mic_level: f64,
}

impl PageCtx<'_> {
    pub fn send(&self, cmd: GoXLRCommand) {
        self.bridge.send(self.serial, cmd);
    }
    pub fn send_keyed(&self, key: &str, cmd: GoXLRCommand) {
        self.bridge.send_keyed(self.serial, key, cmd);
    }
    pub fn is_mini(&self) -> bool {
        self.mixer.hardware.device_type == DeviceType::Mini
    }
}

#[derive(Default)]
struct Dialogs {
    new_profile: Option<String>,
    save_profile_as: Option<String>,
    new_mic_profile: Option<String>,
    save_mic_profile_as: Option<String>,
}

pub struct TuxApp {
    bridge: Bridge,
    overrides: Overrides,
    page: Page,
    selected_serial: Option<String>,
    daemon_conflict: bool,
    dialogs: Dialogs,
    inaccessible: Vec<String>,
    inaccessible_checked: Option<std::time::Instant>,
    frame: u64,
    screenshot_start: Option<std::time::Instant>,
    screenshot: Option<std::path::PathBuf>,
}

impl TuxApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        rt: Handle,
        engine: EngineHandle,
        daemon_conflict: bool,
        start_minimised: bool,
    ) -> Self {
        if start_minimised {
            cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        theme::apply(&cc.egui_ctx);
        Self {
            bridge: Bridge::new(rt, engine, cc.egui_ctx.clone()),
            overrides: Overrides::default(),
            page: std::env::var("TUXLR_PAGE").ok().and_then(|p| Page::from_name(&p)).unwrap_or(Page::Mixer),
            selected_serial: None,
            daemon_conflict,
            dialogs: Dialogs::default(),
            inaccessible: Vec::new(),
            inaccessible_checked: None,
            frame: 0,
            screenshot_start: None,
            // Debug aid: TUXLR_SCREENSHOT=out.png [TUXLR_PAGE=mic] saves a frame then exits.
            screenshot: std::env::var_os("TUXLR_SCREENSHOT").map(Into::into),
        }
    }
}

impl eframe::App for TuxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Pull the latest shared state out of the bridge.
        let (status, mic_level, error, shutdown, raise) = {
            let mut s = self.bridge.shared.lock().unwrap();
            let error = s
                .last_error
                .as_ref()
                .filter(|(_, t)| t.elapsed() < Duration::from_secs(6))
                .map(|(e, _)| e.clone());
            let raise = std::mem::take(&mut s.raise_requested);
            (s.status.clone(), s.mic_level, error, s.shutdown_complete, raise)
        };

        if shutdown {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if raise {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        // Keep the selected device valid.
        if let Some(status) = &status {
            let valid = self
                .selected_serial
                .as_ref()
                .is_some_and(|s| status.mixers.contains_key(s));
            if !valid {
                let mut serials: Vec<_> = status.mixers.keys().cloned().collect();
                serials.sort();
                self.selected_serial = serials.into_iter().next();
            }
        }
        self.bridge.set_mic_meter_device(self.selected_serial.clone());

        // Re-check USB permissions every couple of seconds while no device is usable.
        let device_missing = status.as_ref().is_none_or(|s| s.mixers.is_empty());
        if device_missing && self.inaccessible_checked.is_none_or(|t| t.elapsed() > Duration::from_secs(2)) {
            self.inaccessible = tuxlr_engine::inaccessible_goxlrs();
            self.inaccessible_checked = Some(std::time::Instant::now());
        }
        if device_missing {
            ctx.request_repaint_after(Duration::from_secs(2));
        }

        self.handle_screenshot(ctx);

        let mixer = status
            .as_ref()
            .zip(self.selected_serial.as_ref())
            .and_then(|(st, s)| st.mixers.get(s));

        self.top_bar(ctx, status.as_ref(), mixer);
        self.status_bar(ctx, mixer, error);
        self.nav(ctx, mixer);

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.daemon_conflict {
                widgets::banner(
                    ui,
                    "⚠ The GoXLR Utility daemon (goxlr-daemon) is running. It will fight TuxLR \
                     for control of the device – please stop it (and disable its autostart).",
                );
            }

            match (status.as_ref(), mixer, self.selected_serial.clone()) {
                (Some(st), Some(mixer), Some(serial)) => {
                    let mut pc = PageCtx {
                        bridge: &self.bridge,
                        ov: &mut self.overrides,
                        serial: &serial,
                        mixer,
                        status: st,
                        mic_level,
                    };
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| match self.page {
                            Page::Mixer => mixer::show(ui, &mut pc),
                            Page::Microphone => mic::show(ui, &mut pc),
                            Page::Routing => routing::show(ui, &mut pc),
                            Page::Faders => faders::show(ui, &mut pc),
                            Page::Lighting => lighting::show(ui, &mut pc),
                            Page::Effects => effects::show(ui, &mut pc),
                            Page::Settings => settings::show(ui, &self.bridge, Some(&pc)),
                            Page::About => about::show(ui),
                        });
                }
                _ => match self.page {
                    Page::About => about::show(ui),
                    Page::Settings => settings::show(ui, &self.bridge, None),
                    _ => no_device(ui, status.is_some(), &self.inaccessible),
                },
            }
        });

        self.dialogs(ctx);
    }
}

fn no_device(ui: &mut egui::Ui, engine_ready: bool, inaccessible: &[String]) {
    ui.add_space(80.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("🐧").size(64.0));
        ui.add_space(8.0);
        if engine_ready && !inaccessible.is_empty() {
            ui.heading(format!("{} found, but TuxLR can't access it", inaccessible.join(", ")));
            ui.add_space(6.0);
            ui.label("Your user doesn't have permission to talk to the device over USB.");
            ui.label("Install the udev rule once, then unplug and re-plug the GoXLR:");
            ui.add_space(6.0);
            let cmd = "sudo install -m 644 50-goxlr.rules /etc/udev/rules.d/ && \\\n  sudo udevadm control --reload-rules && sudo udevadm trigger";
            ui.code(cmd);
            ui.label(RichText::new("(50-goxlr.rules ships with TuxLR – see the README)").weak());
        } else if engine_ready {
            ui.heading("No GoXLR detected");
            ui.add_space(6.0);
            ui.label("Plug in a GoXLR or GoXLR Mini. It will appear here automatically.");
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "If it is plugged in, make sure the udev rule is installed \
                     (see the README) and re-plug the device.",
                )
                .weak(),
            );
        } else {
            ui.heading("Starting…");
            ui.spinner();
        }
    });
}

impl TuxApp {
    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        let started = *self.screenshot_start.get_or_insert_with(std::time::Instant::now);
        if self.frame < u64::MAX && started.elapsed() > Duration::from_secs(4) {
            self.frame = u64::MAX;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        ctx.request_repaint();
        let image = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let [w, h] = image.size;
            let rgba: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
            if let Some(buf) = ::image::RgbaImage::from_raw(w as u32, h as u32, rgba) {
                let _ = buf.save(&path);
            }
            self.screenshot = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn top_bar(&mut self, ctx: &egui::Context, status: Option<&DaemonStatus>, mixer: Option<&MixerStatus>) {
        egui::TopBottomPanel::top("top").exact_height(52.0).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(RichText::new("TuxLR").size(22.0).strong().color(theme::ACCENT));
                ui.add_space(12.0);

                // Device picker (only shown when more than one device is attached).
                if let Some(status) = status
                    && status.mixers.len() > 1
                {
                    let mut serials: Vec<_> = status.mixers.keys().cloned().collect();
                    serials.sort();
                    let current = self.selected_serial.clone().unwrap_or_default();
                    egui::ComboBox::from_id_salt("device")
                        .selected_text(&current)
                        .show_ui(ui, |ui| {
                            for s in serials {
                                ui.selectable_value(&mut self.selected_serial, Some(s.clone()), s);
                            }
                        });
                    ui.separator();
                }

                let (Some(status), Some(mixer), Some(serial)) =
                    (status, mixer, self.selected_serial.clone())
                else {
                    return;
                };

                ui.label(RichText::new("Profile").weak());
                let mut profiles = status.files.profiles.clone();
                profiles.sort_by_key(|p| p.to_lowercase());
                egui::ComboBox::from_id_salt("profile")
                    .selected_text(&mixer.profile_name)
                    .width(170.0)
                    .show_ui(ui, |ui| {
                        for p in profiles {
                            if ui.selectable_label(p == mixer.profile_name, &p).clicked() {
                                self.bridge.send(&serial, GoXLRCommand::LoadProfile(p, true));
                            }
                        }
                    });
                ui.menu_button("⏷", |ui| {
                    if ui.button("💾 Save").clicked() {
                        self.bridge.send(&serial, GoXLRCommand::SaveProfile());
                        ui.close_menu();
                    }
                    if ui.button("Save as…").clicked() {
                        self.dialogs.save_profile_as = Some(String::new());
                        ui.close_menu();
                    }
                    if ui.button("New…").clicked() {
                        self.dialogs.new_profile = Some(String::new());
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Open profiles folder").clicked() {
                        self.bridge.send_daemon(tuxlr_engine::ipc::DaemonCommand::OpenPath(
                            tuxlr_engine::ipc::PathTypes::Profiles,
                        ));
                        ui.close_menu();
                    }
                });

                ui.add_space(12.0);
                ui.label(RichText::new("Mic profile").weak());
                let mut mic_profiles = status.files.mic_profiles.clone();
                mic_profiles.sort_by_key(|p| p.to_lowercase());
                egui::ComboBox::from_id_salt("mic_profile")
                    .selected_text(&mixer.mic_profile_name)
                    .width(150.0)
                    .show_ui(ui, |ui| {
                        for p in mic_profiles {
                            if ui.selectable_label(p == mixer.mic_profile_name, &p).clicked() {
                                self.bridge.send(&serial, GoXLRCommand::LoadMicProfile(p, true));
                            }
                        }
                    });
                ui.menu_button("⏷", |ui| {
                    if ui.button("💾 Save").clicked() {
                        self.bridge.send(&serial, GoXLRCommand::SaveMicProfile());
                        ui.close_menu();
                    }
                    if ui.button("Save as…").clicked() {
                        self.dialogs.save_mic_profile_as = Some(String::new());
                        ui.close_menu();
                    }
                    if ui.button("New…").clicked() {
                        self.dialogs.new_mic_profile = Some(String::new());
                        ui.close_menu();
                    }
                });
            });
        });
    }

    fn status_bar(&mut self, ctx: &egui::Context, mixer: Option<&MixerStatus>, error: Option<String>) {
        egui::TopBottomPanel::bottom("status").exact_height(26.0).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                match mixer {
                    Some(m) => {
                        let kind = match m.hardware.device_type {
                            DeviceType::Mini => "GoXLR Mini",
                            DeviceType::Full => "GoXLR",
                            DeviceType::Unknown => "Unknown device",
                        };
                        status_dot(ui, theme::OK);
                        ui.label(format!(
                            "{kind}  ·  Serial {}  ·  Firmware {}",
                            m.hardware.serial_number, m.hardware.versions.firmware
                        ));
                    }
                    None => {
                        status_dot(ui, theme::WARN);
                        ui.label("No device");
                    }
                }
                if let Some(e) = error {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("⚠ {e}")).color(theme::ERROR));
                    });
                }
            });
        });
    }

    fn nav(&mut self, ctx: &egui::Context, mixer: Option<&MixerStatus>) {
        let is_full = mixer.is_some_and(|m| m.hardware.device_type == DeviceType::Full);
        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(180.0)
            .show(ctx, |ui| {
                ui.add_space(10.0);
                let pages = [
                    Page::Mixer,
                    Page::Microphone,
                    Page::Routing,
                    Page::Faders,
                    Page::Lighting,
                    Page::Effects,
                ];
                for page in pages {
                    if page == Page::Effects && !is_full {
                        continue;
                    }
                    nav_button(ui, &mut self.page, page);
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.add_space(10.0);
                    nav_button(ui, &mut self.page, Page::About);
                    nav_button(ui, &mut self.page, Page::Settings);
                });
            });
        if self.page == Page::Effects && !is_full {
            self.page = Page::Mixer;
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(serial) = self.selected_serial.clone() else {
            return;
        };
        let bridge = &self.bridge;
        name_dialog(ctx, "New profile", &mut self.dialogs.new_profile, |n| {
            bridge.send(&serial, GoXLRCommand::NewProfile(n))
        });
        name_dialog(ctx, "Save profile as", &mut self.dialogs.save_profile_as, |n| {
            bridge.send(&serial, GoXLRCommand::SaveProfileAs(n))
        });
        name_dialog(ctx, "New mic profile", &mut self.dialogs.new_mic_profile, |n| {
            bridge.send(&serial, GoXLRCommand::NewMicProfile(n))
        });
        name_dialog(ctx, "Save mic profile as", &mut self.dialogs.save_mic_profile_as, |n| {
            bridge.send(&serial, GoXLRCommand::SaveMicProfileAs(n))
        });
    }
}

fn status_dot(ui: &mut egui::Ui, colour: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.5, colour);
}

fn nav_button(ui: &mut egui::Ui, current: &mut Page, page: Page) {
    let selected = *current == page;
    let text = RichText::new(format!("{}  {}", page.icon(), page.label())).size(15.0);
    let button = egui::Button::new(text)
        .selected(selected)
        .min_size(egui::vec2(ui.available_width(), 34.0))
        .frame(selected);
    if ui.add(button).clicked() {
        *current = page;
    }
}

fn name_dialog(
    ctx: &egui::Context,
    title: &str,
    state: &mut Option<String>,
    on_ok: impl FnOnce(String),
) {
    let Some(name) = state.as_mut() else {
        return;
    };
    let mut open = true;
    let mut submit = false;
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label("Name:");
            let r = ui.text_edit_singleline(name);
            r.request_focus();
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submit = true;
            }
            ui.add_space(6.0);
            if ui.button("OK").clicked() {
                submit = true;
            }
        });
    let trimmed = name.trim().to_string();
    if submit && !trimmed.is_empty() {
        on_ok(trimmed);
        *state = None;
    } else if !open {
        *state = None;
    }
}
