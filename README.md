# TuxLR

TuxLR is a standalone Linux desktop app for the **TC-Helicon GoXLR** and **GoXLR Mini**.
It's one native Rust program: no background daemon, no web UI, no browser. Open it and your
GoXLR is ready to control.

TuxLR is built on the excellent [GoXLR Utility](https://github.com/GoXLR-on-Linux/goxlr-utility)
by the GoXLR-on-Linux team. All the USB protocol, profile, and audio work comes from that
project. TuxLR embeds it in-process and adds a native [egui](https://github.com/emilk/egui)
interface. See [NOTICE.md](NOTICE.md) for full attribution and the list of changes.

## Features

| Page | What you can do |
|---|---|
| **Mixer** | Set volumes for every input and output, mute faders, watch a live mic level meter, and use submixes (Mix A/B) with a monitor mix |
| **Microphone** | Pick the mic type, set gain, and adjust the noise gate, compressor, 6-band (Mini) or 10-band (full) EQ, de-esser, and bleep volume |
| **Routing** | Click cells in the input → output routing matrix, and set VOD mode |
| **Faders & Mutes** | Assign channels to faders, set mute behaviour, cough button mode, mute-hold time, and scribble-strip text (full GoXLR) |
| **Lighting** | Set the global colour, fader styles and colours, button on/off colours, and animations (on supported firmware) |
| **Effects** | Full GoXLR only: effect presets, reverb, echo, pitch, gender, megaphone, robot, and hard tune |
| **Profiles** | Load, save, save-as, and create device profiles and mic profiles from the top bar |
| **Settings** | Start on login, log level, data folders, restore default profiles, and device/firmware info |

TuxLR also applies the GoXLR Utility's sleep/wake and shutdown handling, so the device is set
correctly when your PC suspends or TuxLR quits.

## Requirements

- Linux (X11 or Wayland)
- A Rust toolchain, 1.88 or newer (edition 2024)
- Build dependencies (Debian/Ubuntu names):

```bash
sudo apt install build-essential pkg-config libusb-1.0-0-dev libpulse-dev
```

## Build and install

```bash
cargo build --release
```

The binary is written to `target/release/tuxlr`. To install it for your user (binary, icon,
and desktop entry) and the system udev rule, run:

```bash
./install.sh
```

### USB permissions (required once)

TuxLR runs as your normal user. Your user needs permission to open the GoXLR, which the
included udev rule grants:

```bash
sudo install -m 644 50-goxlr.rules /etc/udev/rules.d/ && sudo udevadm control --reload-rules && sudo udevadm trigger
```

Unplug and re-plug the GoXLR afterwards. If TuxLR sees a GoXLR it can't open, it shows these
steps on screen.

> **Don't run TuxLR alongside `goxlr-daemon`.** Both would try to control the same device.
> TuxLR warns you if it finds the daemon running.

## Usage

```
tuxlr [--minimised] [--log-level off|error|warn|info|debug|trace]
```

| Path | Contents |
|---|---|
| `~/.config/tuxlr/settings.json` | Settings |
| `~/.local/share/tuxlr/` | Profiles, mic profiles, presets, samples, icons, logs, backups |

If you already use the GoXLR Utility, copy your `.goxlr` and `.goxlrMicProfile` files from
`~/.local/share/goxlr-utility/` into the matching TuxLR folders.

## Project layout

```
crates/tuxlr/          The desktop app (egui UI + bridge to the engine)
crates/tuxlr-engine/   The GoXLR Utility daemon core, adapted to run in-process
vendor/goxlr-utility/  Unmodified GoXLR Utility library crates (usb, types, ipc, profile, audio, scribbles)
```

## Not yet implemented

- A sampler page (the engine supports it; there's no UI for it yet)
- Firmware updates (use the official tools for now)
- A tray icon, and per-parameter advanced effect controls (reverb decay, robot EQ, …)

## License

TuxLR is MIT licensed (see [LICENSE](LICENSE)). The vendored GoXLR Utility code is MIT
licensed by its authors. The bundled default profiles and presets are © Music Tribe Brands CA
Ltd. under the license in `vendor/goxlr-utility/LICENSE-3RD-PARTY`. See [NOTICE.md](NOTICE.md).

TuxLR is not affiliated with TC-Helicon, Music Tribe, or the GoXLR-on-Linux project.
