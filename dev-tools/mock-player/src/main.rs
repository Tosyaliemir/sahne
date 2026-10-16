//! A fake MPRIS player for testing Sahne without Spotify.
//!
//! Publishes `org.mpris.MediaPlayer2.sahnemock` on the session bus with real
//! song metadata and a position that advances in real time. Unlike Spotify it
//! uses the spec's variant types (`o` track ids, `x` lengths), so both
//! encodings get exercised.
//!
//! Usage:
//!   cargo run -- [--paused] [--zero-position] [--track "Artist|Title|Album|seconds[|artUrl]"]...
//!
//! While running, type on stdin: `p` play/pause, `n` next, `b` previous,
//! `s <seconds>` seek, `q` quit.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, BufReader};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};
use zbus::{connection, interface};

const BUS_NAME: &str = "org.mpris.MediaPlayer2.sahnemock";
const PATH: &str = "/org/mpris/MediaPlayer2";

#[derive(Clone)]
struct Track {
    artist: String,
    title: String,
    album: String,
    length_us: i64,
    art_url: Option<String>,
}

fn default_tracks() -> Vec<Track> {
    let t = |artist: &str, title: &str, album: &str, secs: f64| Track {
        artist: artist.into(),
        title: title.into(),
        album: album.into(),
        length_us: (secs * 1_000_000.0) as i64,
        art_url: None,
    };
    vec![
        t(
            "Rick Astley",
            "Never Gonna Give You Up",
            "Whenever You Need Somebody",
            213.6,
        ),
        t("Queen", "Bohemian Rhapsody", "A Night At The Opera", 354.3),
        t("Tarkan", "Şımarık", "Ölürüm Sana", 234.0),
    ]
}

fn parse_track(spec: &str) -> Result<Track, String> {
    let parts: Vec<&str> = spec.split('|').collect();
    if parts.len() < 4 {
        return Err(format!(
            "expected Artist|Title|Album|seconds[|artUrl], got {spec:?}"
        ));
    }
    let secs: f64 = parts[3]
        .parse()
        .map_err(|_| format!("bad seconds in {spec:?}"))?;
    Ok(Track {
        artist: parts[0].into(),
        title: parts[1].into(),
        album: parts[2].into(),
        length_us: (secs * 1_000_000.0) as i64,
        art_url: parts.get(4).map(|s| s.to_string()),
    })
}

struct Player {
    tracks: Vec<Track>,
    index: usize,
    playing: bool,
    anchor_pos_us: i64,
    anchor: Instant,
    zero_position: bool,
}

impl Player {
    fn position_us(&self) -> i64 {
        let elapsed = if self.playing {
            self.anchor.elapsed().as_micros() as i64
        } else {
            0
        };
        (self.anchor_pos_us + elapsed).min(self.track().length_us)
    }

    fn track(&self) -> &Track {
        &self.tracks[self.index]
    }

    fn set_pos(&mut self, us: i64) {
        self.anchor_pos_us = us.clamp(0, self.track().length_us);
        self.anchor = Instant::now();
    }

    fn set_playing(&mut self, playing: bool) {
        let pos = self.position_us();
        self.playing = playing;
        self.set_pos(pos);
    }

    fn go(&mut self, delta: isize) {
        let n = self.tracks.len() as isize;
        self.index = ((self.index as isize + delta).rem_euclid(n)) as usize;
        self.set_pos(0);
        let t = self.track();
        println!("▶ {} – {}", t.artist, t.title);
    }

    fn track_path(&self) -> String {
        format!("/dev/sahne/mock/track/{}", self.index)
    }

    async fn announce_track(&self, e: &SignalEmitter<'_>) -> zbus::Result<()> {
        self.metadata_changed(e).await?;
        Self::seeked(e, 0).await
    }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    async fn play_pause(
        &mut self,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        self.set_playing(!self.playing);
        self.playback_status_changed(&e).await?;
        Ok(())
    }

    async fn play(
        &mut self,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        self.set_playing(true);
        self.playback_status_changed(&e).await?;
        Ok(())
    }

    async fn pause(
        &mut self,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        self.set_playing(false);
        self.playback_status_changed(&e).await?;
        Ok(())
    }

    async fn next(
        &mut self,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        self.go(1);
        self.announce_track(&e).await?;
        Ok(())
    }

    async fn previous(
        &mut self,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        // Like real players: restart the track unless we are near its beginning.
        if self.position_us() > 3_000_000 {
            self.set_pos(0);
            Self::seeked(&e, 0).await?;
        } else {
            self.go(-1);
            self.announce_track(&e).await?;
        }
        Ok(())
    }

    async fn seek(
        &mut self,
        offset_us: i64,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        self.set_pos(self.position_us() + offset_us);
        Self::seeked(&e, self.anchor_pos_us).await?;
        Ok(())
    }

    async fn set_position(
        &mut self,
        track_id: ObjectPath<'_>,
        position_us: i64,
        #[zbus(signal_emitter)] e: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        // The spec says to ignore requests for a stale track id.
        if track_id.as_str() == self.track_path() {
            self.set_pos(position_us);
            Self::seeked(&e, self.anchor_pos_us).await?;
        }
        Ok(())
    }

    #[zbus(signal)]
    async fn seeked(e: &SignalEmitter<'_>, position_us: i64) -> zbus::Result<()>;

    #[zbus(property)]
    fn playback_status(&self) -> &str {
        if self.playing {
            "Playing"
        } else {
            "Paused"
        }
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        let t = self.track();
        let mut m: HashMap<String, OwnedValue> = HashMap::new();
        let mut put = |k: &str, v: Value<'_>| {
            if let Ok(owned) = v.try_to_owned() {
                m.insert(k.into(), owned);
            }
        };
        if let Ok(path) = ObjectPath::try_from(self.track_path()) {
            put("mpris:trackid", Value::from(path));
        }
        put("mpris:length", Value::from(t.length_us));
        put("xesam:title", Value::from(t.title.as_str()));
        put("xesam:album", Value::from(t.album.as_str()));
        put("xesam:artist", Value::from(vec![t.artist.as_str()]));
        if let Some(art) = &t.art_url {
            put("mpris:artUrl", Value::from(art.as_str()));
        }
        m
    }

    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> i64 {
        if self.zero_position {
            0
        } else {
            self.position_us()
        }
    }

    #[zbus(property)]
    fn rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }
}

struct Root;

#[interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    fn raise(&self) {}
    fn quit(&self) {}

    #[zbus(property)]
    fn identity(&self) -> &str {
        "Sahne Mock Player"
    }

    #[zbus(property)]
    fn can_quit(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn can_raise(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> {
        Vec::new()
    }

    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> {
        Vec::new()
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tracks = Vec::new();
    let mut playing = true;
    let mut zero_position = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--paused" => playing = false,
            "--zero-position" => zero_position = true,
            "--track" => tracks.push(parse_track(&args.next().ok_or("--track needs a value")?)?),
            other => return Err(format!("unknown argument {other:?}").into()),
        }
    }
    if tracks.is_empty() {
        tracks = default_tracks();
    }

    let player = Player {
        tracks,
        index: 0,
        playing,
        anchor_pos_us: 0,
        anchor: Instant::now(),
        zero_position,
    };
    let first = player.track().clone();
    let conn = connection::Builder::session()?
        .name(BUS_NAME)?
        .serve_at(PATH, Root)?
        .serve_at(PATH, player)?
        .build()
        .await?;
    println!(
        "{BUS_NAME} ready{}",
        if zero_position {
            " (Position always 0)"
        } else {
            ""
        }
    );
    println!("▶ {} – {}", first.artist, first.title);
    println!("commands: p play/pause, n next, b previous, s <seconds> seek, q quit");

    let iface = conn.object_server().interface::<_, Player>(PATH).await?;
    let mut stdin = BufReader::new(tokio::io::stdin()).lines();
    let mut tick = tokio::time::interval(Duration::from_millis(250));
    loop {
        tokio::select! {
            _ = tick.tick() => {
                // Auto-advance when a track ends, like a real player.
                let mut p = iface.get_mut().await;
                if p.playing && p.position_us() >= p.track().length_us {
                    p.go(1);
                    p.announce_track(iface.signal_emitter()).await?;
                }
            }
            line = stdin.next_line() => {
                let Some(line) = line? else { break };
                let mut p = iface.get_mut().await;
                let e = iface.signal_emitter();
                let mut words = line.split_whitespace();
                match words.next() {
                    Some("p") => {
                        let playing = !p.playing;
                        p.set_playing(playing);
                        p.playback_status_changed(e).await?;
                        println!("{}", if playing { "playing" } else { "paused" });
                    }
                    Some("n") => {
                        p.go(1);
                        p.announce_track(e).await?;
                    }
                    Some("b") => {
                        p.go(-1);
                        p.announce_track(e).await?;
                    }
                    Some("s") => match words.next().and_then(|s| s.parse::<f64>().ok()) {
                        Some(secs) => {
                            p.set_pos((secs * 1_000_000.0) as i64);
                            Player::seeked(e, p.anchor_pos_us).await?;
                            println!("seeked to {secs}s");
                        }
                        None => println!("usage: s <seconds>"),
                    },
                    Some("q") => break,
                    _ => println!("commands: p, n, b, s <seconds>, q"),
                }
            }
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(playing: bool) -> Player {
        Player {
            tracks: default_tracks(),
            index: 0,
            playing,
            anchor_pos_us: 0,
            anchor: Instant::now(),
            zero_position: false,
        }
    }

    #[test]
    fn parses_track_specs() {
        let t = parse_track("Örnek Sanatçı|Işıklı Günler|Deneme|213.5|file:///tmp/c.png").unwrap();
        assert_eq!(
            (t.artist.as_str(), t.title.as_str()),
            ("Örnek Sanatçı", "Işıklı Günler")
        );
        assert_eq!(t.length_us, 213_500_000);
        assert_eq!(t.art_url.as_deref(), Some("file:///tmp/c.png"));
        assert!(parse_track("a|b|c").is_err());
        assert!(parse_track("a|b|c|long").is_err());
    }

    #[test]
    fn seeks_are_clamped_and_tracks_wrap() {
        let mut p = player(false);
        p.set_pos(-5);
        assert_eq!(p.position_us(), 0);
        p.set_pos(i64::MAX);
        assert_eq!(p.position_us(), p.track().length_us);
        p.go(-1);
        assert_eq!(p.index, p.tracks.len() - 1);
        assert_eq!(p.position_us(), 0);
        p.go(1);
        assert_eq!(p.index, 0);
    }

    #[test]
    fn paused_position_stands_still() {
        let mut p = player(true);
        p.set_pos(1_000_000);
        p.set_playing(false);
        let before = p.position_us();
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(p.position_us(), before);
    }
}
