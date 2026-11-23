//! Diagnostic: prints the current media session (read only).
fn main() {
    deckless_lib::audio::com_init();
    match deckless_lib::media::query(None) {
        Some(np) => println!(
            "title={:?} artist={:?} playing={} cover={}",
            np.title,
            np.artist,
            np.playing,
            np.cover
                .map(|c| format!("{} bytes", c.len()))
                .unwrap_or("none".into())
        ),
        None => println!("no media session"),
    }
}
