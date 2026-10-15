//! Which media player to follow when several are running. Shared by the
//! platform backends; names are short ids such as `spotify` or `vlc`.

/// Picks which player to follow.
///
/// Order: an explicit override (env/settings), then Spotify, then the first
/// player that is currently playing, then any player. `players` holds
/// `(short_name, is_playing)` pairs in bus order.
pub fn choose_player<'a>(
    players: &'a [(String, bool)],
    preferred: Option<&str>,
) -> Option<&'a str> {
    let find = |want: &str| {
        players
            .iter()
            .find(|(name, _)| name == want || is_instance_of(name, want))
            .map(|(name, _)| name.as_str())
    };
    preferred
        .and_then(find)
        .or_else(|| find("spotify"))
        .or_else(|| {
            players
                .iter()
                .find(|(_, playing)| *playing)
                .map(|(n, _)| n.as_str())
        })
        .or_else(|| players.first().map(|(n, _)| n.as_str()))
}

/// Matches multi-instance names such as `chromium.instance1234` against `chromium`.
fn is_instance_of(name: &str, base: &str) -> bool {
    name.strip_prefix(base)
        .is_some_and(|rest| rest.starts_with(".instance"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn players(list: &[(&str, bool)]) -> Vec<(String, bool)> {
        list.iter().map(|(n, p)| ((*n).to_owned(), *p)).collect()
    }

    #[test]
    fn player_selection_order() {
        let p = players(&[
            ("chromium.instance1", true),
            ("spotifast", false),
            ("spotify", false),
        ]);
        assert_eq!(choose_player(&p, None), Some("spotify"));
        assert_eq!(choose_player(&p, Some("spotifast")), Some("spotifast"));
        assert_eq!(
            choose_player(&p, Some("chromium")),
            Some("chromium.instance1")
        );
        // Unknown preference falls back to Spotify.
        assert_eq!(choose_player(&p, Some("vlc")), Some("spotify"));

        let p = players(&[("vlc", false), ("firefox.instance9", true)]);
        assert_eq!(choose_player(&p, None), Some("firefox.instance9"));
        let p = players(&[("vlc", false), ("mpv", false)]);
        assert_eq!(choose_player(&p, None), Some("vlc"));
        assert_eq!(choose_player(&[], None), None);
    }
}
