//! TuxLR – a standalone Linux desktop app for the TC-Helicon GoXLR and GoXLR Mini.
//!
//! Device communication is provided by the GoXLR Utility project
//! (https://github.com/GoXLR-on-Linux/goxlr-utility, MIT licensed). See NOTICE.md.

mod bridge;
mod ui;

use std::process::ExitCode;

use anyhow::Result;
use eframe::egui;
use tuxlr_engine::EngineOptions;

const APP_ID: &str = "tuxlr";
const ICON_PNG: &[u8] = include_bytes!("../../../assets/tuxlr.png");

struct Args {
    minimised: bool,
    log_level: Option<log::LevelFilter>,
}

fn parse_args() -> Result<Option<Args>> {
    let mut args = Args {
        minimised: false,
        log_level: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--minimised" | "--minimized" => args.minimised = true,
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
                     USAGE: tuxlr [--minimised] [--log-level LEVEL]\n",
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

    let _lock = tuxlr_engine::acquire_instance_lock()?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("tuxlr-engine")
        .build()?;

    let (engine, engine_join) = runtime.block_on(tuxlr_engine::start(EngineOptions {
        log_level: args.log_level,
        ..Default::default()
    }))?;

    let daemon_conflict = tuxlr_engine::goxlr_daemon_running();

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
    let gui_engine = engine.clone();
    let result = eframe::run_native(
        "TuxLR",
        options,
        Box::new(move |cc| {
            Ok(Box::new(ui::TuxApp::new(
                cc,
                handle,
                gui_engine,
                daemon_conflict,
                args.minimised,
            )))
        }),
    );

    // Whatever happened to the window, make sure the device gets its shutdown commands applied
    // and the engine finishes cleanly.
    runtime.block_on(async {
        engine.stop().await;
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), engine_join).await;
    });
    runtime.shutdown_timeout(std::time::Duration::from_secs(1));

    result.map_err(|e| anyhow::anyhow!("GUI error: {e}"))
}
