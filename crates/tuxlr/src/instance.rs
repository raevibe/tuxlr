//! Single-instance control socket.
//!
//! The running TuxLR listens on `$XDG_RUNTIME_DIR/tuxlr.sock`. Launching `tuxlr` again doesn't
//! start a second copy: it asks the running one to show its window (or, with `--quit`, to exit).
//! This is what makes the window-less background mode usable without a tray icon.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, ensure};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    /// Open (or raise) the main window.
    Show,
    /// Shut TuxLR down cleanly.
    Quit,
}

impl Request {
    fn as_str(self) -> &'static str {
        match self {
            Request::Show => "show",
            Request::Quit => "quit",
        }
    }

    fn parse(line: &str) -> Option<Self> {
        match line.trim() {
            "show" => Some(Request::Show),
            "quit" => Some(Request::Quit),
            _ => None,
        }
    }
}

fn socket_path() -> PathBuf {
    tuxlr_engine::runtime_dir().join("tuxlr.sock")
}

/// Start answering requests from later launches. Only call this while holding the instance lock.
pub fn listen(on_request: impl Fn(Request) + Send + 'static) -> Result<()> {
    let path = socket_path();
    // Holding the lock means any existing socket file is stale.
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).with_context(|| format!("Unable to bind {path:?}"))?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;

    std::thread::Builder::new()
        .name("tuxlr-instance".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let mut line = String::new();
                if BufReader::new(&stream).read_line(&mut line).is_ok()
                    && let Some(request) = Request::parse(&line)
                {
                    on_request(request);
                    let _ = (&stream).write_all(b"ok\n");
                }
            }
        })?;
    Ok(())
}

/// Send a request to the TuxLR that's already running.
pub fn send(request: Request) -> Result<()> {
    let mut stream = UnixStream::connect(socket_path())
        .context("TuxLR is already running, but it isn't accepting requests")?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.write_all(format!("{}\n", request.as_str()).as_bytes())?;
    let mut reply = String::new();
    BufReader::new(&stream).read_line(&mut reply)?;
    ensure!(reply.trim() == "ok", "The running TuxLR didn't confirm the request");
    Ok(())
}

/// Remove the socket file on exit. Only call this while still holding the instance lock.
pub fn cleanup() {
    let _ = std::fs::remove_file(socket_path());
}

#[cfg(test)]
mod tests {
    use super::Request;

    #[test]
    fn requests_round_trip() {
        for r in [Request::Show, Request::Quit] {
            assert_eq!(Request::parse(&format!("{}\n", r.as_str())), Some(r));
        }
        assert_eq!(Request::parse("reboot"), None);
    }
}
