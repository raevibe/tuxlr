//! TuxLR – a standalone Linux desktop app for the TC-Helicon GoXLR and GoXLR Mini.
//!
//! Device communication is provided by the GoXLR Utility project
//! (https://github.com/GoXLR-on-Linux/goxlr-utility, MIT licensed). See NOTICE.md.

mod bridge;
mod instance;
mod ui;

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use eframe::egui;
use tokio::sync::broadcast::error::RecvError;
use tuxlr_engine::{EngineHandle, EngineOptions, UiEvent};

const APP_ID: &str = "tuxlr";
const ICON_PNG: &[u8] = include_bytes!("../../../assets/tuxlr.png");

struct Args {
    /// Start with no window (no taskbar entry); launching `tuxlr` again opens it.
    background: bool,
    quit: bool,
    log_level: Option<log::LevelFilter>,
}

fn parse_args() -> Result<Option<Args>> {
    let mut args = Args {
        background: false,
        quit: false,
        log_level: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--minimised" | "--minimized" | "--background" => args.background = true,
            "--quit" => args.quit = true,
            "--log-level" => {
                let value = it.next().unwrap_or_default();
                args.log_level = Some(value.parse().map_err(|_| {
                    anyhow::anyhow!("Invalid log level '{value}' (off/error/warn/info/debug/trace)")
                })?);
            }
            "--version" | "-V" => {
                println!(
                    "TuxLR {} (GoXLR Utility engine v{})",
                    env!("CARGO_PKG_VERSION"),
                    tuxlr_engine::UPSTREAM_VERSION
                );
                return Ok(None);
            }
            "--help" | "-h" => {
                println!(
                    "TuxLR {}\nA standalone Linux app for the TC-Helicon GoXLR / GoXLR Mini.\n\n\
                     USAGE: tuxlr [--minimised] [--quit] [--log-level LEVEL]\n\n\
                     --minimised   Run in the background with no window or taskbar entry.\n\
                     \x20             Run `tuxlr` again to open the window; closing it\n\
                     \x20             returns TuxLR to the background.\n\
                     --quit        Stop a TuxLR that is running in the background.\n\
                     --log-level   off, error, warn, info, debug or trace.\n",
                    env!("CARGO_PKG_VERSION")
                );
                return Ok(None);
            }
            other => anyhow::bail!("Unknown argument: {other} (try --help)"),
        }
    }
    Ok(Some(args))
}

fn load_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(ICON_PNG).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("TuxLR: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let Some(args) = parse_args()? else {
        return Ok(());
    };

    // If TuxLR is already running, hand the request to it instead of starting a second copy.
    let Some(_lock) = tuxlr_engine::try_acquire_instance_lock()? else {
        if args.quit {
            instance::send(instance::Request::Quit)?;
            println!("Asked TuxLR to quit.");
        } else {
            instance::send(instance::Request::Show)?;
        }
        return Ok(());
    };
    if args.quit {
        println!("TuxLR is not running.");
        return Ok(());
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("tuxlr-engine")
        .build()?;

    let (engine, engine_join) = runtime.block_on(tuxlr_engine::start(EngineOptions {
        log_level: args.log_level,
        ..Default::default()
    }))?;

    {
        let engine = engine.clone();
        let rt = runtime.handle().clone();
        let listening = instance::listen(move |request| match request {
            instance::Request::Show => engine.raise_ui(),
            instance::Request::Quit => {
                let engine = engine.clone();
                rt.spawn(async move { engine.stop().await });
            }
        });
        if let Err(e) = listening {
            log::warn!("Launching TuxLR again won't be able to open this window: {e:#}");
        }
    }

    let daemon_conflict = tuxlr_engine::goxlr_daemon_running();
    if args.background {
        log::info!("Running in the background. Run `tuxlr` again to open the window.");
    }

    let mut ui_events = engine.subscribe_ui();
    let mut show_window = !args.background;
    let mut result = Ok(());
    loop {
        if show_window {
            let quit = Arc::new(AtomicBool::new(false));
            result = open_window(&runtime, &engine, daemon_conflict, args.background, quit.clone());
            // Without --minimised, closing the window quits as before. In background mode it
            // only closes the window, unless the user chose Quit or the engine shut down.
            if result.is_err() || quit.load(Ordering::SeqCst) || !args.background {
                break;
            }
            show_window = false;
            // Forget raise requests that arrived while the window was already open.
            ui_events = ui_events.resubscribe();
            log::info!("Window closed; TuxLR keeps running in the background.");
        }
        match runtime.block_on(ui_events.recv()) {
            Ok(UiEvent::Raise) => show_window = true,
            Ok(UiEvent::Shutdown) | Err(RecvError::Closed) => break,
            Err(RecvError::Lagged(_)) => {}
        }
    }

    // Whatever happened to the window, make sure the device gets its shutdown commands applied
    // and the engine finishes cleanly.
    runtime.block_on(async {
        engine.stop().await;
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), engine_join).await;
    });
    runtime.shutdown_timeout(std::time::Duration::from_secs(1));
    instance::cleanup();

    result
}

/// Show the main window until it is closed. `quit` is set if TuxLR should exit afterwards.
fn open_window(
    runtime: &tokio::runtime::Runtime,
    engine: &EngineHandle,
    daemon_conflict: bool,
    background: bool,
    quit: Arc<AtomicBool>,
) -> Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("TuxLR")
        .with_app_id(APP_ID)
        .with_inner_size([1100.0, 720.0])
        .with_min_inner_size([760.0, 520.0])
        .with_visible(true);
    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let handle = runtime.handle().clone();
    let engine = engine.clone();
    eframe::run_native(
        "TuxLR",
        options,
        Box::new(move |cc| {
            Ok(Box::new(ui::TuxApp::new(
                cc,
                handle,
                engine,
                daemon_conflict,
                background,
                quit,
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("GUI error: {e}"))
}
