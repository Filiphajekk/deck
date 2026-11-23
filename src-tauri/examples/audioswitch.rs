//! Acceptance check for `audioDevice`: switches the default output to the next device,
//! verifies it, then switches back to the original device.
use deckless_lib::audio;
use deckless_lib::profiles::model::AudioFlow;

fn default_name() -> String {
    audio::list_devices(AudioFlow::Output)
        .unwrap()
        .into_iter()
        .find(|d| d.is_default)
        .map(|d| d.name)
        .unwrap_or_default()
}

fn main() -> anyhow::Result<()> {
    audio::com_init();
    let before = audio::list_devices(AudioFlow::Output)?;
    let original = before
        .iter()
        .find(|d| d.is_default)
        .cloned()
        .expect("default device");
    println!("default before: {}", original.name);
    let next = audio::cycle_device(AudioFlow::Output)?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    println!(
        "switched to:    {} (default now: {})",
        next.name,
        default_name()
    );
    assert_eq!(default_name(), next.name, "default output did not change");
    audio::set_default_device(&original.id)?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    println!("restored:       {}", default_name());
    assert_eq!(default_name(), original.name);
    println!("OK");
    Ok(())
}
