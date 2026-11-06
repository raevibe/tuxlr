use crate::DaemonState;
use crate::events::EventTriggers;
use anyhow::Result;
use std::path::PathBuf;
use tokio::sync::mpsc;
use which::which;

// TuxLR: upstream supports Windows and macOS here; TuxLR is Linux-only.
mod linux;
mod unix;

pub fn perform_preflight() -> Result<()> {
    Ok(())
}

pub async fn spawn_runtime(state: DaemonState, tx: mpsc::Sender<EventTriggers>) -> Result<()> {
    tokio::spawn(linux::sleep::run(tx.clone(), state.shutdown.clone()));
    unix::spawn_platform_runtime(state, tx).await
}

pub fn has_autostart() -> bool {
    linux::autostart::has_autostart()
}

pub fn set_autostart(enabled: bool) -> Result<()> {
    if enabled {
        return linux::autostart::create_startup_link();
    }
    linux::autostart::remove_startup_link()
}

#[allow(dead_code)]
pub fn display_error(message: String) {
    linux::display_error(message);
}

pub fn get_ui_app_path() -> Option<PathBuf> {
    // This simply looks for the GoXLR UI App alongside the daemon binary and returns it..
    let mut path = None;
    let bin_name = get_ui_binary_name();

    // There are three possible places to check for this, the CWD, the binary WD, and $PATH
    let cwd = std::env::current_dir().unwrap().join(bin_name.clone());
    if cwd.exists() {
        path.replace(cwd);
    }

    // IntelliJ complains about duplicate code here, and while yes, it's technically duplicated
    // from goxlr-launcher, the launcher and daemon don't have dependencies on each other.
    if path.is_none()
        && let Some(parent) = std::env::current_exe().unwrap().parent()
    {
        let bin = parent.join(bin_name.clone());
        if bin.exists() {
            path.replace(bin);
        }
    }

    if path.is_none() {
        // Try and locate the binary on $PATH
        if let Ok(which) = which(bin_name) {
            path.replace(which);
        }
    }

    path
}

static UI_NAME: &str = "goxlr-utility-ui";
fn get_ui_binary_name() -> String {
    if cfg!(windows) {
        format!("{UI_NAME}.exe")
    } else {
        String::from(UI_NAME)
    }
}
