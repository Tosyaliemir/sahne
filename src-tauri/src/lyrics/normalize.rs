//! Cleans up streaming-service decorations in titles before searching lyrics.

/// A group is decoration when it starts with one of these words...
const NOISE_LEADING: &[&str] = &[
    "feat",
    "ft",
    "featuring",
    "with",
    "explicit",
    "clean",
    "live",
    "from",
    "prod",
    "bonus",
    "deluxe",
    "mono",
    "stereo",
    "remaster",
    "remastered",
    "recorded",
];
/// ...or contains one of these anywhere ("2013 Remaster", "Single Version").
const NOISE_ANYWHERE: &[&str] = &[
    "remaster",
    "remastered",
    "version",
    "mix",
    "edit",
    "anniversary",
];

fn is_noise(s: &str) -> bool {
    let lower = s.to_lowercase();
    let mut words = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty());
    let Some(first) = words.next() else {
        return false;
    };
    NOISE_LEADING.contains(&first)
        || NOISE_ANYWHERE.contains(&first)
        || words.any(|w| NOISE_ANYWHERE.contains(&w))
}

/// Removes decorations such as ` - Remastered 2011`, `(feat. X)`, `[Explicit]`.
/// Returns the input unchanged when cleaning would leave nothing.
pub fn clean_title(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut rest = title;
    // Drop bracketed groups whose content is noise.
    while let Some(open) = rest.find(['(', '[']) {
        let close_char = if rest.as_bytes()[open] == b'(' {
            ')'
        } else {
            ']'
        };
        let Some(close) = rest[open..].find(close_char).map(|i| i + open) else {
            break;
        };
        out.push_str(&rest[..open]);
        let inner = &rest[open + 1..close];
        if !is_noise(inner) {
            out.push_str(&rest[open..=close]);
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);

    // Drop ` - suffix` parts that are noise (possibly several).
    let mut cleaned = out.as_str();
    while let Some(i) = cleaned.rfind(" - ") {
        if is_noise(&cleaned[i + 3..]) {
            cleaned = &cleaned[..i];
        } else {
            break;
        }
    }

    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        title.trim().to_owned()
    } else {
        cleaned
    }
}

/// First artist of a comma-joined MPRIS artist list.
pub fn primary_artist(artist: &str) -> &str {
    artist.split(", ").next().unwrap_or(artist).trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_common_decorations() {
        let cases = [
            ("Bohemian Rhapsody - Remastered 2011", "Bohemian Rhapsody"),
            ("Come Together - 2019 Mix", "Come Together"),
            ("Hotel California - 2013 Remaster", "Hotel California"),
            ("Wonderwall - Live", "Wonderwall"),
            ("Wonderwall - Live at Knebworth", "Wonderwall"),
            ("Bad Guy - Radio Edit", "Bad Guy"),
            ("3500 (feat. Future & 2 Chainz)", "3500"),
            ("Señorita (with Camila Cabello)", "Señorita"),
            ("HUMBLE. [Explicit]", "HUMBLE."),
            ("Song (ft. Someone) - Remastered", "Song"),
            ("Let It Be - Remastered 2009 - Mono", "Let It Be"),
            (
                "Psycho Killer - From \"Stop Making Sense\"",
                "Psycho Killer",
            ),
        ];
        for (input, want) in cases {
            assert_eq!(clean_title(input), want, "input: {input}");
        }
    }

    #[test]
    fn keeps_meaningful_parts() {
        let cases = [
            "Don't Stop Me Now",
            "Smells Like Teen Spirit",
            "Part I (The Beginning)",
            "Señorita",
            "Şımarık",
            "Love - Hate",
            "(I Can't Get No) Satisfaction",
            "Anti-Hero",
            "Liveforever",
        ];
        for input in cases {
            assert_eq!(clean_title(input), input, "input: {input}");
        }
    }

    #[test]
    fn never_returns_empty() {
        assert_eq!(clean_title("(Live)"), "(Live)");
    }

    #[test]
    fn primary_artist_takes_first() {
        assert_eq!(
            primary_artist("Travis Scott, Future, 2 Chainz"),
            "Travis Scott"
        );
        assert_eq!(primary_artist("Tarkan"), "Tarkan");
    }
}
