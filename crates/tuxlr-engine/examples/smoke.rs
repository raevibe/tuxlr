//! Hardware smoke test: connects to the first GoXLR, changes a few settings, checks the
//! device state reflects them, then restores the originals.
//! Run with: cargo run -p tuxlr-engine --example smoke
use std::time::Duration;

use anyhow::{Result, bail};
use tuxlr_engine::ipc::GoXLRCommand;
use tuxlr_engine::types::{ChannelName, FaderName, InputDevice, OutputDevice};

#[tokio::main]
async fn main() -> Result<()> {
    let _lock = tuxlr_engine::acquire_instance_lock()?;
    let (engine, join) = tuxlr_engine::start(Default::default()).await?;

    let mut serial = None;
    for _ in 0..50 {
        if let Some(s) = engine.status().await?.mixers.keys().next() {
            serial = Some(s.clone());
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let Some(serial) = serial else { bail!("No GoXLR found") };
    let before = engine.status().await?.mixers[&serial].clone();
    println!("Device {serial}: profile {:?}", before.profile_name);

    // Volume
    let vol = before.levels.volumes[ChannelName::Music];
    let test = if vol > 10 { vol - 10 } else { vol + 10 };
    engine.command(&serial, GoXLRCommand::SetVolume(ChannelName::Music, test)).await?;
    let now = engine.status().await?.mixers[&serial].levels.volumes[ChannelName::Music];
    println!("Music volume {vol} -> {now} (wanted {test})");
    assert_eq!(now, test);
    engine.command(&serial, GoXLRCommand::SetVolume(ChannelName::Music, vol)).await?;

    // Routing
    let routed = before.router[InputDevice::Music][OutputDevice::ChatMic];
    engine.command(&serial, GoXLRCommand::SetRouter(InputDevice::Music, OutputDevice::ChatMic, !routed)).await?;
    let now = engine.status().await?.mixers[&serial].router[InputDevice::Music][OutputDevice::ChatMic];
    println!("Music -> Chat Mic route {routed} -> {now}");
    assert_eq!(now, !routed);
    engine.command(&serial, GoXLRCommand::SetRouter(InputDevice::Music, OutputDevice::ChatMic, routed)).await?;

    // Lighting (visible on the device for ~2 seconds)
    let fl = before.lighting.faders[&FaderName::A].colours.clone();
    engine.command(&serial, GoXLRCommand::SetFaderColours(FaderName::A, "FF0000".into(), "FF00FF".into())).await?;
    println!("Fader A lit red/magenta for 2s…");
    tokio::time::sleep(Duration::from_secs(2)).await;
    engine.command(&serial, GoXLRCommand::SetFaderColours(FaderName::A, fl.colour_one, fl.colour_two)).await?;

    // Mic gain
    let mt = before.mic_status.mic_type;
    let gain = before.mic_status.mic_gains[mt];
    engine.command(&serial, GoXLRCommand::SetMicrophoneGain(mt, gain + 1)).await?;
    let now = engine.status().await?.mixers[&serial].mic_status.mic_gains[mt];
    println!("Mic gain {gain} -> {now}");
    assert_eq!(now, gain + 1);
    engine.command(&serial, GoXLRCommand::SetMicrophoneGain(mt, gain)).await?;

    // Invalid value must be rejected, not crash
    let err = engine.command(&serial, GoXLRCommand::SetGateThreshold(10)).await;
    println!("Out-of-range gate threshold rejected: {}", err.is_err());
    assert!(err.is_err());

    println!("Mic level: {:.3}", engine.mic_level(&serial).await?);
    println!("ALL OK");
    engine.stop().await;
    let _ = tokio::time::timeout(Duration::from_secs(5), join).await;
    Ok(())
}
