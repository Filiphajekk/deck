//! Diagnostic: prints audio devices, master volume state and microphone mute (read only).
use deckless_lib::audio;
use deckless_lib::profiles::model::AudioFlow;

fn main() -> anyhow::Result<()> {
    audio::com_init();
    for flow in [AudioFlow::Output, AudioFlow::Input] {
        println!("{flow:?}:");
        for d in audio::list_devices(flow)? {
            println!("  {} {}", if d.is_default { "*" } else { " " }, d.name);
        }
    }
    println!("output muted: {}", audio::output_muted()?);
    println!("mic muted: {}", audio::mic_muted()?);
    println!(
        "apps with audio: {:?}",
        audio::sessions::apps_with_sessions()
    );
    Ok(())
}
