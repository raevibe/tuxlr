//! Prints the raw mic level (dBFS) of the first GoXLR ten times a second for 3 seconds.
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _lock = tuxlr_engine::acquire_instance_lock()?;
    let (engine, _) = tuxlr_engine::start(tuxlr_engine::EngineOptions {
        log_level: Some(log::LevelFilter::Warn),
        ..Default::default()
    })
    .await?;
    let mut serial = None;
    for _ in 0..50 {
        if let Some(s) = engine.status().await?.mixers.keys().next() {
            serial = Some(s.clone());
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let serial = serial.ok_or_else(|| anyhow::anyhow!("No GoXLR found"))?;
    let mut out = Vec::new();
    for _ in 0..30 {
        out.push(format!("{:.1}", engine.mic_level(&serial).await?));
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    println!("{}", out.join(" "));
    engine.stop().await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    std::process::exit(0)
}
