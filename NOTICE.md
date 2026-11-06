# Notices and attribution

TuxLR would not exist without the **GoXLR Utility** by the
[GoXLR-on-Linux](https://github.com/GoXLR-on-Linux) team. They reverse-engineered the GoXLR's
USB protocol and wrote the device, profile and audio handling that TuxLR depends on.

- Upstream: <https://github.com/GoXLR-on-Linux/goxlr-utility>
- Vendored from: commit `49ab4e434f1d88c1b6005926a5734070cf8a709b` (v1.2.4, 2026-09-18)
- Upstream authors: Nathan Adams, Craig McLure, Lars Mühlbauer, and
  [all contributors](https://github.com/GoXLR-on-Linux/goxlr-utility/graphs/contributors)
- License: MIT. The full text is in [`vendor/goxlr-utility/LICENSE`](vendor/goxlr-utility/LICENSE)

TuxLR is an independent project. It is not affiliated with or endorsed by the GoXLR-on-Linux
team, TC-Helicon, or Music Tribe. "GoXLR" is a trademark of its respective owner.

## What was taken, and what changed

| Path in TuxLR | Upstream source | Changes |
|---|---|---|
| `vendor/goxlr-utility/usb` | `usb/` (goxlr-usb) | None |
| `vendor/goxlr-utility/types` | `types/` (goxlr-types) | None |
| `vendor/goxlr-utility/ipc` | `ipc/` (goxlr-ipc) | None (TuxLR only uses its data types) |
| `vendor/goxlr-utility/profile` | `profile/` (goxlr-profile-loader) | None |
| `vendor/goxlr-utility/audio` | `audio/` (goxlr-audio) | None |
| `vendor/goxlr-utility/scribbles` | `scribbles/` (goxlr-scribbles) | None |
| `vendor/goxlr-utility/50-goxlr.rules`, `50-goxlr.rules` | `50-goxlr.rules` | None |
| `crates/tuxlr-engine` | `daemon/` (goxlr-daemon) | Changes listed below |
| `crates/tuxlr-engine/defaults` | `defaults/resources` | None (embedded rather than extracted by a helper binary) |

Changes made to the daemon in `crates/tuxlr-engine`:

- `main.rs` became `lib.rs`. It exposes `start()` and an `EngineHandle` so the engine runs
  inside the TuxLR process instead of as a background daemon.
- Removed the HTTP server, web UI, IPC socket, and tray icon. The GUI talks to the engine over
  in-memory channels.
- Removed the Windows and macOS platform code. TuxLR is Linux-only.
- `files.rs`: default resources are embedded with `include_dir` instead of running the
  `goxlr-defaults` binary.
- `settings.rs`: the default log level for new installs is Info instead of Debug, and config and data directories moved to `~/.config/tuxlr` and
  `~/.local/share/tuxlr`, so TuxLR doesn't touch an existing GoXLR Utility setup.
- `events.rs`: "open UI" and "activate" requests now raise the TuxLR window, and a
  shutdown event tells the GUI to exit.
- `platform/linux`: the autostart entry and error dialog titles now say TuxLR.
- `lib.rs` adds a single-instance lock and a check for a running `goxlr-daemon`.

All other logic (device handling, profiles, mic profiles, effects, sampler, sleep/wake
handling) is upstream code, unchanged.

## Third-party content (Music Tribe)

The default profiles, mic profile, effect presets, and icons in `crates/tuxlr-engine/defaults/`
and `crates/tuxlr-engine/profiles/` come from the official GoXLR App. TC-Helicon gave the
GoXLR Utility project permission to include them. They are:

> Copyright 2022 Music Tribe Brands CA Ltd.

They are distributed under the license in
[`vendor/goxlr-utility/LICENSE-3RD-PARTY`](vendor/goxlr-utility/LICENSE-3RD-PARTY).
A copy of that license sits next to the files.

## Other dependencies

TuxLR also uses many crates from crates.io, including
[egui/eframe](https://github.com/emilk/egui) (MIT/Apache-2.0) and
[tokio](https://tokio.rs) (MIT). Each crate's license is listed in its own package metadata.
To list them all, run `cargo install cargo-about && cargo about generate`.
