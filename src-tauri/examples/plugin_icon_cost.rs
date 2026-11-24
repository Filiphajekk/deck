//! Diagnostic: how long it takes to turn all icons of the installed plugins into data URLs
//! (what one editor plugin-list request cost before the image cache).
use deckless_lib::sdk_host::manifest::{resolve_image, Manifest};

fn main() {
    let dir = deckless_lib::paths::plugins_dir();
    let start = std::time::Instant::now();
    let mut n = 0;
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let Ok(m) = Manifest::load(&e.path()) else {
            continue;
        };
        let mut rels = vec![m.icon.clone()];
        for a in &m.actions {
            rels.push(a.icon.clone());
            rels.extend(a.states.iter().map(|s| s.image.clone()));
        }
        for rel in rels {
            if let Some(p) = resolve_image(&e.path(), &rel) {
                let _ = deckless_lib::icons::load_image_data_url(&p);
                n += 1;
            }
        }
    }
    println!("{n} images in {:?}", start.elapsed());
}
