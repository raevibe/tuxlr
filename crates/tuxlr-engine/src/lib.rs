//! TuxLR Engine
//!
//! This crate is the device-management core of the GoXLR Utility daemon
//! (https://github.com/GoXLR-on-Linux/goxlr-utility, MIT licensed, Copyright (c) 2022 - 2024
//! Craig McLure, Nathan Adams and contributors), adapted by TuxLR to run *in-process* inside a
//! native desktop application rather than as a separate background daemon.
//!
//! Changes from upstream (see NOTICE.md at the repository root for the full list):
//!  * `main.rs` became this library entry point (`start`), returning an [`EngineHandle`].
//!  * The HTTP server, IPC socket, web UI and tray icon were removed – the GUI talks to the
//!    engine directly over in-memory channels.
//!  * Windows / macOS platform code was removed (TuxLR is Linux-only).
//!  * Default profiles / presets / icons are embedded instead of calling `goxlr-defaults`.
//!  * Settings and data live under `~/.config/tuxlr` and `~/.local/share/tuxlr`.

extern crate core;

use std::fs::{File, create_dir_all};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow, bail};
use directories::ProjectDirs;
use enum_map::EnumMap;
use file_rotate::compression::Compression;
use file_rotate::suffix::AppendCount;
use file_rotate::{ContentLimit, FileRotate};
use json_patch::Patch;
use lazy_static::lazy_static;
use log::{debug, info, warn};
use simplelog::{
    ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, TerminalMode, WriteLogger,
};
use sys_locale::get_locale;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinHandle;

pub use goxlr_ipc as ipc;
pub use goxlr_types as types;

use goxlr_ipc::{
    DaemonCommand, DaemonStatus, FirmwareSource, GoXLRCommand, HttpSettings, LogLevel,
};

use crate::events::{DaemonState, EventTriggers, spawn_event_handler};
use crate::files::{FileManager, spawn_file_notification_service};
use crate::platform::{perform_preflight, spawn_runtime};
use crate::primary_worker::{DeviceCommand, DeviceSender, spawn_usb_handler};
use crate::settings::SettingsHandle;
use crate::shutdown::Shutdown;
use crate::tts::spawn_tts_service;

mod audio;
mod device;
mod events;
mod files;
mod firmware;
mod mic_profile;
mod platform;
mod primary_worker;
mod profile;
mod settings;
mod shutdown;
mod tts;

/// Version of the upstream GoXLR Utility this engine is derived from.
pub const UPSTREAM_VERSION: &str = "1.2.4";
/// Upstream commit the vendored sources were taken from.
pub const UPSTREAM_COMMIT: &str = "49ab4e434f1d88c1b6005926a5734070cf8a709b";

const VERSION: &str = env!("CARGO_PKG_VERSION");

const FIRMWARE_PATHS: EnumMap<FirmwareSource, &str> = EnumMap::from_array([
    "https://utility.frostycoolslug.com/update-site/stable/",
    "https://utility.frostycoolslug.com/update-site/beta/",
]);

static OVERRIDE_SAMPLER_INPUT: Mutex<Option<String>> = Mutex::new(None);
static OVERRIDE_SAMPLER_OUTPUT: Mutex<Option<String>> = Mutex::new(None);

lazy_static! {
    static ref SYSTEM_LOCALE: String = get_locale()
        .unwrap_or_else(|| String::from("en_GB"))
        .replace('-', "_");
}

/// A JSON patch describing a change to the [`DaemonStatus`].
#[derive(Debug, Clone)]
pub struct PatchEvent {
    pub data: Patch,
}

/// Events the engine wants the GUI to react to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEvent {
    /// Something asked for the main window to be shown.
    Raise,
    /// The engine has finished shutting down; the GUI should exit.
    Shutdown,
}

#[derive(Debug, Clone, Default)]
pub struct EngineOptions {
    /// Override the settings file location (defaults to `~/.config/tuxlr/settings.json`).
    pub config: Option<PathBuf>,
    /// Override the configured log level.
    pub log_level: Option<log::LevelFilter>,
    /// Regex override for the sampler's input device.
    pub sampler_input: Option<String>,
    /// Regex override for the sampler's output device.
    pub sampler_output: Option<String>,
}

pub fn default_config_location() -> PathBuf {
    ProjectDirs::from("", "", "tuxlr")
        .map(|d| d.config_dir().join("settings.json"))
        .unwrap_or_else(|| PathBuf::from("tuxlr-settings.json"))
}

/// A cheap, cloneable handle used by the GUI to talk to the engine.
#[derive(Clone)]
pub struct EngineHandle {
    usb_tx: DeviceSender,
    global_tx: mpsc::Sender<EventTriggers>,
    patch_tx: broadcast::Sender<PatchEvent>,
    ui_tx: broadcast::Sender<UiEvent>,
}

impl EngineHandle {
    /// Fetch a full snapshot of the current state.
    pub async fn status(&self) -> Result<DaemonStatus> {
        let (tx, rx) = oneshot::channel();
        self.usb_tx
            .send(DeviceCommand::SendDaemonStatus(tx))
            .await
            .map_err(|_| anyhow!("Engine is not running"))?;
        Ok(rx.await?)
    }

    /// Run a command against a specific device (identified by serial number).
    pub async fn command(&self, serial: &str, command: GoXLRCommand) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.usb_tx
            .send(DeviceCommand::RunDeviceCommand(serial.to_string(), command, tx))
            .await
            .map_err(|_| anyhow!("Engine is not running"))?;
        rx.await?
    }

    /// Run a non-device ('daemon level') command.
    pub async fn daemon_command(&self, command: DaemonCommand) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.usb_tx
            .send(DeviceCommand::RunDaemonCommand(command, tx))
            .await
            .map_err(|_| anyhow!("Engine is not running"))?;
        rx.await?
    }

    /// Current microphone input level (0.0 – 1.0) of a device.
    pub async fn mic_level(&self, serial: &str) -> Result<f64> {
        let (tx, rx) = oneshot::channel();
        self.usb_tx
            .send(DeviceCommand::GetDeviceMicLevel(serial.to_string(), tx))
            .await
            .map_err(|_| anyhow!("Engine is not running"))?;
        rx.await?
    }

    /// Ask the engine to shut down cleanly (applies the device's shutdown commands first).
    pub async fn stop(&self) {
        let _ = self.global_tx.send(EventTriggers::Stop(false)).await;
    }

    pub fn subscribe_patches(&self) -> broadcast::Receiver<PatchEvent> {
        self.patch_tx.subscribe()
    }

    pub fn subscribe_ui(&self) -> broadcast::Receiver<UiEvent> {
        self.ui_tx.subscribe()
    }
}

/// Guards against two copies of TuxLR (or TuxLR + goxlr-daemon) fighting over the USB device.
pub struct InstanceLock {
    _file: File,
}

pub fn acquire_instance_lock() -> Result<InstanceLock> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let path = dir.join("tuxlr.lock");
    let file = File::create(&path).with_context(|| format!("Unable to create {path:?}"))?;
    // SAFETY: flock is called with a valid, owned file descriptor.
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result != 0 {
        bail!("TuxLR is already running.");
    }
    Ok(InstanceLock { _file: file })
}

/// Returns true if the upstream GoXLR Utility daemon appears to be running.
pub fn goxlr_daemon_running() -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    entries.flatten().any(|entry| {
        std::fs::read_to_string(entry.path().join("comm"))
            .map(|c| c.trim() == "goxlr-daemon")
            .unwrap_or(false)
    })
}

fn is_root() -> bool {
    nix::unistd::Uid::effective().is_root()
}

async fn init_logging(settings: &SettingsHandle, override_level: Option<log::LevelFilter>) {
    let log_path = settings.get_log_directory().await;
    if !log_path.exists() {
        let _ = create_dir_all(&log_path);
    }
    let log_file = log_path.join("tuxlr.log");

    let mut config = ConfigBuilder::new();
    for noisy in [
        "tracing",
        "zbus",
        "hyper_util",
        "reqwest",
        "symphonia",
        "eframe",
        "egui_glow",
        "winit",
        "sctk",
        "calloop",
        "naga",
        "wgpu",
    ] {
        config.add_filter_ignore_str(noisy);
    }
    let _ = config.set_time_offset_to_local();

    let level = override_level.unwrap_or(match settings.get_log_level().await {
        LogLevel::Off => log::LevelFilter::Off,
        LogLevel::Error => log::LevelFilter::Error,
        LogLevel::Warn => log::LevelFilter::Warn,
        LogLevel::Info => log::LevelFilter::Info,
        LogLevel::Debug => log::LevelFilter::Debug,
        LogLevel::Trace => log::LevelFilter::Trace,
    });

    let rotator = FileRotate::new(
        log_file,
        AppendCount::new(5),
        ContentLimit::Bytes(1024 * 1024 * 2),
        Compression::OnRotate(1),
        None,
    );

    let _ = CombinedLogger::init(vec![
        TermLogger::new(level, config.build(), TerminalMode::Mixed, ColorChoice::Auto),
        WriteLogger::new(level, config.build(), rotator),
    ]);
    log_panics::init();
}

/// Start the engine. Must be called from within a Tokio runtime.
///
/// Returns a handle for the GUI and a join handle that completes once the engine (and all the
/// devices it manages) has fully shut down.
pub async fn start(options: EngineOptions) -> Result<(EngineHandle, JoinHandle<()>)> {
    if is_root() {
        bail!(
            "TuxLR should not be run as root. Install the udev rule (see README) so your \
             user can access the GoXLR instead."
        );
    }

    let config = options.config.unwrap_or_else(default_config_location);
    let settings = SettingsHandle::load(config).await?;
    init_logging(&settings, options.log_level).await;

    if let Some(device) = options.sampler_input {
        OVERRIDE_SAMPLER_INPUT.lock().unwrap().replace(device);
    }
    if let Some(device) = options.sampler_output {
        OVERRIDE_SAMPLER_OUTPUT.lock().unwrap().replace(device);
    }

    info!(
        "Starting TuxLR Engine v{} (based on GoXLR Utility v{})",
        VERSION, UPSTREAM_VERSION
    );
    info!("System Locale: {}", *SYSTEM_LOCALE);
    perform_preflight()?;

    // TuxLR has no HTTP server; this only exists to satisfy the shared status structure.
    let http_settings = HttpSettings {
        enabled: false,
        bind_address: String::from("localhost"),
        cors_enabled: false,
        port: 0,
    };

    let (global_tx, global_rx) = mpsc::channel(32);
    let (patch_tx, _) = broadcast::channel(16);
    let (usb_tx, usb_rx) = mpsc::channel(32);
    let (tts_sender, tts_rx) = mpsc::channel(32);
    let (device_state_tx, device_state_rx) = mpsc::channel(1);
    let (ui_tx, _) = broadcast::channel(8);

    let shutdown = Shutdown::new();
    let shutdown_blocking = Arc::new(AtomicBool::new(false));

    let file_manager = FileManager::new(&settings).await;
    let file_paths = file_manager.paths().clone();

    let (file_tx, file_rx) = mpsc::channel(20);
    let file_handle = tokio::spawn(spawn_file_notification_service(
        file_paths,
        file_tx,
        shutdown.clone(),
    ));

    let (ready_tx, ready_rx) = oneshot::channel();
    let usb_handle = tokio::spawn(spawn_usb_handler(
        usb_rx,
        file_rx,
        device_state_rx,
        patch_tx.clone(),
        global_tx.clone(),
        ready_tx,
        shutdown.clone(),
        settings.clone(),
        http_settings.clone(),
        file_manager,
    ));
    let _ = ready_rx.await;

    let tts_handle = tokio::spawn(spawn_tts_service(
        settings.clone(),
        tts_rx,
        shutdown.clone(),
    ));

    let state = DaemonState {
        tts_sender,
        show_tray: Arc::new(AtomicBool::new(false)),
        shutdown: shutdown.clone(),
        shutdown_blocking,
        settings_handle: settings.clone(),
        http_settings,
        ui_tx: ui_tx.clone(),
    };

    let event_handle = tokio::spawn(spawn_event_handler(
        state.clone(),
        global_rx,
        device_state_tx,
    ));
    let platform_handle = tokio::spawn(spawn_runtime(state, global_tx.clone()));

    let mut local_shutdown = shutdown;
    let join = tokio::spawn(async move {
        local_shutdown.recv().await;
        info!("Shutting down TuxLR engine");
        let _ = tokio::join!(
            usb_handle,
            file_handle,
            tts_handle,
            event_handle,
            platform_handle
        );
        debug!("Engine stopped");
    });

    if goxlr_daemon_running() {
        warn!("goxlr-daemon appears to be running; it will conflict with TuxLR.");
    }

    Ok((
        EngineHandle {
            usb_tx,
            global_tx,
            patch_tx,
            ui_tx,
        },
        join,
    ))
}

/// Finds GoXLR devices that are plugged in but which the current user can't open
/// (normally because the udev rule isn't installed). Returns their product names.
pub fn inaccessible_goxlrs() -> Vec<String> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir("/sys/bus/usb/devices") else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let read = |name: &str| {
            std::fs::read_to_string(path.join(name))
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };
        if read("idVendor") != "1220" {
            continue;
        }
        let name = match read("idProduct").as_str() {
            "8fe0" => "GoXLR",
            "8fe4" => "GoXLR Mini",
            _ => continue,
        };
        let (Ok(bus), Ok(dev)) = (read("busnum").parse::<u32>(), read("devnum").parse::<u32>()) else {
            continue;
        };
        let node = format!("/dev/bus/usb/{bus:03}/{dev:03}");
        let Ok(c_node) = std::ffi::CString::new(node) else {
            continue;
        };
        // SAFETY: c_node is a valid NUL-terminated string.
        if unsafe { libc::access(c_node.as_ptr(), libc::R_OK | libc::W_OK) } != 0 {
            found.push(name.to_string());
        }
    }
    found
}
