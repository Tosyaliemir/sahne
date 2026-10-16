//! Writes a preview fixture for the screenshot harness. Run through
//! `pnpm preview:fixture`, which reads the track from the running player.
//!
//! Usage: preview_fixture <out-dir> <name> <id> <title> <artist> <album> <length-ms> <art-url>
//!
//! Writes `<name>.json`, `<name>-cover.img` and `<name>-bg.png` to `<out-dir>`.

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [out, name, id, title, artist, album, length, art] = args.as_slice() else {
        eprintln!("usage: preview_fixture <out-dir> <name> <id> <title> <artist> <album> <length-ms> <art-url>");
        std::process::exit(2);
    };
    let out = PathBuf::from(out);
    let length_ms: u64 = length.parse().expect("length-ms must be a number");
    let track = sahne_lib::devtools::track(id, title, artist, album, length_ms, art);
    let fixture = sahne_lib::devtools::build(&track, &out.join(format!("{name}-bg.png")))
        .unwrap_or_else(|e| {
            eprintln!("error: {e}");
            std::process::exit(1);
        });
    std::fs::write(out.join(format!("{name}-cover.img")), &fixture.cover).expect("write cover");
    let json = serde_json::json!({
        "track": {
            "id": id, "title": title, "artist": artist, "album": album,
            "length_ms": length_ms, "art_url": null, "player": "spotify",
        },
        "lyrics": fixture.lyrics,
        "palette": fixture.palette,
        "cover": format!("{name}-cover.img"),
        "background": format!("{name}-bg.png"),
    });
    let file = out.join(format!("{name}.json"));
    std::fs::write(&file, serde_json::to_vec_pretty(&json).unwrap()).expect("write fixture");
    eprintln!("wrote {}", file.display());
}
