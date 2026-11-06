//! Glue between the (async, Tokio-based) engine and the (immediate-mode, egui) GUI.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;
use tokio::runtime::Handle;
use tokio::sync::{broadcast, mpsc};
use tuxlr_engine::ipc::{DaemonCommand, DaemonStatus, GoXLRCommand};
use tuxlr_engine::{EngineHandle, UiEvent};

/// State shared between the background tasks and the GUI.
#[derive(Default)]
pub struct Shared {
    pub status: Option<DaemonStatus>,
    pub mic_level: f64,
    pub last_error: Option<(String, Instant)>,
    pub shutdown_complete: bool,
    pub raise_requested: bool,
}

enum Request {
    Device(String, GoXLRCommand),
    Daemon(DaemonCommand),
}

pub struct Bridge {
    pub shared: Arc<Mutex<Shared>>,
    pending: Arc<Mutex<Vec<(String, Request)>>>,
    wake: mpsc::UnboundedSender<()>,
    mic_serial: Arc<Mutex<Option<String>>>,
}

impl Bridge {
    pub fn new(rt: Handle, engine: EngineHandle, ctx: egui::Context) -> Self {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let pending: Arc<Mutex<Vec<(String, Request)>>> = Arc::new(Mutex::new(Vec::new()));
        let mic_serial: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let (wake, mut wake_rx) = mpsc::unbounded_channel::<()>();

        // Status poller: refresh on every patch the engine broadcasts, plus a slow heartbeat.
        {
            let engine = engine.clone();
            let shared = shared.clone();
            let ctx = ctx.clone();
            let mut patches = engine.subscribe_patches();
            rt.spawn(async move {
                let mut heartbeat = tokio::time::interval(Duration::from_millis(1000));
                loop {
                    tokio::select! {
                        r = patches.recv() => {
                            if matches!(r, Err(broadcast::error::RecvError::Closed)) { break; }
                        }
                        _ = heartbeat.tick() => {}
                    }
                    if let Ok(status) = engine.status().await {
                        shared.lock().unwrap().status = Some(status);
                        ctx.request_repaint();
                    }
                }
            });
        }

        // Mic level meter (~20 fps).
        {
            let engine = engine.clone();
            let shared = shared.clone();
            let mic_serial = mic_serial.clone();
            let ctx = ctx.clone();
            rt.spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_millis(50));
                loop {
                    tick.tick().await;
                    let serial = mic_serial.lock().unwrap().clone();
                    if let Some(serial) = serial
                        && let Ok(db) = engine.mic_level(&serial).await
                    {
                        // The engine reports dBFS (-72.2 ..= 0); show -60 dB .. 0 dB on the meter.
                        let level = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
                        let mut s = shared.lock().unwrap();
                        if (s.mic_level - level).abs() > 0.001 {
                            s.mic_level = level;
                            ctx.request_repaint();
                        }
                    }
                }
            });
        }

        // UI events from the engine (window raise / shutdown complete).
        {
            let mut events = engine.subscribe_ui();
            let shared = shared.clone();
            let ctx = ctx.clone();
            rt.spawn(async move {
                while let Ok(event) = events.recv().await {
                    let mut s = shared.lock().unwrap();
                    match event {
                        UiEvent::Raise => s.raise_requested = true,
                        UiEvent::Shutdown => s.shutdown_complete = true,
                    }
                    ctx.request_repaint();
                }
            });
        }

        // Command sender. Commands are coalesced by key so dragging a slider sends only the
        // most recent value rather than queueing hundreds of USB writes.
        {
            let engine = engine.clone();
            let shared = shared.clone();
            let pending = pending.clone();
            let ctx = ctx.clone();
            rt.spawn(async move {
                while wake_rx.recv().await.is_some() {
                    while wake_rx.try_recv().is_ok() {}
                    let batch: Vec<(String, Request)> = std::mem::take(&mut *pending.lock().unwrap());
                    for (_, request) in batch {
                        let result = match request {
                            Request::Device(serial, cmd) => engine.command(&serial, cmd).await,
                            Request::Daemon(cmd) => engine.daemon_command(cmd).await,
                        };
                        if let Err(e) = result {
                            log::warn!("Command failed: {e}");
                            shared.lock().unwrap().last_error = Some((e.to_string(), Instant::now()));
                        }
                    }
                    if let Ok(status) = engine.status().await {
                        shared.lock().unwrap().status = Some(status);
                    }
                    ctx.request_repaint();
                    // Small pause so rapid slider drags coalesce into fewer writes.
                    tokio::time::sleep(Duration::from_millis(15)).await;
                }
            });
        }

        Self {
            shared,
            pending,
            wake,
            mic_serial,
        }
    }

    fn push(&self, key: String, request: Request) {
        let mut pending = self.pending.lock().unwrap();
        if let Some(existing) = pending.iter_mut().find(|(k, _)| *k == key) {
            existing.1 = request;
        } else {
            pending.push((key, request));
        }
        drop(pending);
        let _ = self.wake.send(());
    }

    /// Send a device command. Commands sharing a `key` replace each other while queued.
    pub fn send_keyed(&self, serial: &str, key: impl Into<String>, cmd: GoXLRCommand) {
        self.push(key.into(), Request::Device(serial.to_string(), cmd));
    }

    /// Send a device command that must never be coalesced (profile loads, toggles, ...).
    pub fn send(&self, serial: &str, cmd: GoXLRCommand) {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.push(format!("#unique-{n}"), Request::Device(serial.to_string(), cmd));
    }

    pub fn send_daemon(&self, cmd: DaemonCommand) {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.push(format!("#daemon-{n}"), Request::Daemon(cmd));
    }

    pub fn set_mic_meter_device(&self, serial: Option<String>) {
        *self.mic_serial.lock().unwrap() = serial;
    }
}

/// Keeps the GUI showing the value the user is dragging, instead of snapping back to the
/// (slightly stale) device value while commands are in flight.
#[derive(Default)]
pub struct Overrides {
    values: HashMap<String, (f64, Instant)>,
}

const OVERRIDE_HOLD: Duration = Duration::from_millis(600);

impl Overrides {
    pub fn get(&self, key: &str, device_value: f64) -> f64 {
        match self.values.get(key) {
            Some((v, t)) if t.elapsed() < OVERRIDE_HOLD => *v,
            _ => device_value,
        }
    }

    pub fn set(&mut self, key: &str, value: f64) {
        self.values.insert(key.to_string(), (value, Instant::now()));
    }
}
