//! LRC parser: `[mm:ss.xx]` line stamps (several per line allowed), the
//! `[offset:±ms]` tag, and Enhanced LRC `<mm:ss.xx>` word stamps.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Word {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Line {
    pub start_ms: u64,
    pub end_ms: u64,
    /// Empty for an instrumental break.
    pub text: String,
    pub words: Option<Vec<Word>>,
}

/// How long the last line lasts when the track length is unknown.
const LAST_LINE_MS: u64 = 5_000;

/// LRC comes from a crowd-sourced database, so it is untrusted: stamps past
/// this many minutes are treated as malformed rather than overflowing.
const MAX_MINUTES: i64 = 999;
/// `[offset:]` values beyond an hour are nonsense; clamp them.
const MAX_OFFSET_MS: i64 = 3_600_000;

/// Parses LRC text. `duration_ms` (0 if unknown) closes the last line.
pub fn parse(input: &str, duration_ms: u64) -> Vec<Line> {
    let mut offset_ms: i64 = 0;
    let mut raw: Vec<(i64, String, Option<Vec<RawWord>>)> = Vec::new();

    for line in input.lines() {
        let line = line.trim();
        let mut rest = line;
        let mut stamps = Vec::new();
        // Leading tags: timestamps or metadata.
        while let Some(after) = rest.strip_prefix('[') {
            let Some(end) = after.find(']') else { break };
            let tag = &after[..end];
            if let Some(ms) = parse_timestamp(tag) {
                stamps.push(ms);
            } else if let Some(value) = tag.strip_prefix("offset:") {
                offset_ms = value
                    .trim()
                    .parse::<i64>()
                    .unwrap_or(0)
                    .clamp(-MAX_OFFSET_MS, MAX_OFFSET_MS);
            } else if stamps.is_empty() && is_metadata_tag(tag) {
                // [ar:...], [ti:...] etc. carry no lyrics.
            } else {
                break;
            }
            rest = &after[end + 1..];
        }
        if stamps.is_empty() {
            continue; // metadata, blank, or malformed line
        }
        let (text, words) = parse_words(rest);
        for ms in stamps {
            raw.push((ms, text.clone(), words.clone()));
        }
    }

    // A positive offset makes lyrics appear earlier.
    let shift = |t: i64| u64::try_from(t.saturating_sub(offset_ms)).unwrap_or(0);
    raw.sort_by_key(|(start, ..)| *start);

    let mut lines: Vec<Line> = raw
        .into_iter()
        .map(|(start, text, words)| Line {
            start_ms: shift(start),
            end_ms: 0,
            text,
            words: words.map(|ws| {
                ws.into_iter()
                    .map(|w| Word {
                        start_ms: shift(w.start),
                        // Explicit end stamps are kept; the rest is filled in below.
                        end_ms: w.end.map_or(0, shift),
                        text: w.text,
                    })
                    .collect()
            }),
        })
        .collect();

    // Collapse consecutive blank lines into one break.
    lines.dedup_by(|b, a| a.text.is_empty() && b.text.is_empty());

    for i in 0..lines.len() {
        let next_start = lines.get(i + 1).map(|l| l.start_ms);
        let line = &mut lines[i];
        line.end_ms = match next_start {
            Some(n) => n.max(line.start_ms),
            None if duration_ms > line.start_ms => duration_ms,
            None => line.start_ms.saturating_add(LAST_LINE_MS),
        };
        let line_end = line.end_ms;
        if let Some(words) = &mut line.words {
            for j in 0..words.len() {
                let next = words.get(j + 1).map_or(line_end, |w| w.start_ms);
                let w = &mut words[j];
                let end = if w.end_ms > w.start_ms {
                    w.end_ms.min(next)
                } else {
                    next
                };
                w.end_ms = end.max(w.start_ms);
            }
        }
    }
    lines
}

fn is_metadata_tag(tag: &str) -> bool {
    tag.split_once(':')
        .is_some_and(|(k, _)| !k.is_empty() && k.chars().all(|c| c.is_ascii_alphabetic()))
}

/// Accepts `mm:ss`, `mm:ss.x`, `mm:ss.xx`, `mm:ss.xxx` and `mm:ss:xx`.
fn parse_timestamp(tag: &str) -> Option<i64> {
    let (min, rest) = tag.split_once(':')?;
    let min: i64 = parse_digits(min)?;
    if min > MAX_MINUTES {
        return None;
    }
    let (sec, frac) = match rest.find(['.', ':']) {
        Some(i) => (&rest[..i], Some(&rest[i + 1..])),
        None => (rest, None),
    };
    let sec: i64 = parse_digits(sec)?;
    if sec >= 60 {
        return None;
    }
    let frac_ms = match frac {
        None => 0,
        Some(f) if !f.is_empty() && f.len() <= 3 => {
            let n: i64 = parse_digits(f)?;
            n * 10_i64.pow(3 - f.len() as u32)
        }
        Some(_) => return None,
    };
    Some(min * 60_000 + sec * 1000 + frac_ms)
}

fn parse_digits(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

#[derive(Debug, Clone)]
struct RawWord {
    start: i64,
    /// Set when a stamp directly follows the word (e.g. a closing stamp at line end).
    end: Option<i64>,
    text: String,
}

/// Splits Enhanced LRC word stamps out of a line's text. Returns the plain
/// text and, if the line had any `<mm:ss.xx>` stamps, the timed words.
fn parse_words(text: &str) -> (String, Option<Vec<RawWord>>) {
    if !text.contains('<') {
        return (text.trim().to_owned(), None);
    }
    let mut words: Vec<RawWord> = Vec::new();
    let mut plain = String::new();
    let mut rest = text;
    let mut current: Option<i64> = None;
    loop {
        let next_tag = rest.find('<').and_then(|i| {
            let close = rest[i..].find('>')? + i;
            parse_timestamp(&rest[i + 1..close]).map(|ms| (i, close, ms))
        });
        let (segment, after) = match next_tag {
            Some((open, _, _)) => (&rest[..open], Some(next_tag)),
            None => (rest, None),
        };
        plain.push_str(segment);
        let word = segment.trim();
        if let Some(start) = current {
            if !word.is_empty() {
                words.push(RawWord {
                    start,
                    end: None,
                    text: word.to_owned(),
                });
            }
        }
        match after.flatten() {
            Some((_, close, ms)) => {
                if let Some(last) = words.last_mut() {
                    if last.end.is_none() && !word.is_empty() {
                        last.end = Some(ms);
                    }
                }
                current = Some(ms);
                rest = &rest[close + 1..];
            }
            None => break,
        }
    }
    let plain = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    if words.is_empty() {
        (plain, None)
    } else {
        (plain, Some(words))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn starts(lines: &[Line]) -> Vec<(u64, &str)> {
        lines
            .iter()
            .map(|l| (l.start_ms, l.text.as_str()))
            .collect()
    }

    #[test]
    fn parses_basic_lines() {
        let lrc = "[ar:Rick Astley]\n[ti:Never Gonna Give You Up]\n\
                   [00:18.68] We're no strangers to love\n[00:22.84]You know the rules and so do I\n";
        let lines = parse(lrc, 30_000);
        assert_eq!(
            starts(&lines),
            vec![
                (18_680, "We're no strangers to love"),
                (22_840, "You know the rules and so do I")
            ]
        );
        assert_eq!(lines[0].end_ms, 22_840);
        assert_eq!(lines[1].end_ms, 30_000);
        assert!(lines[0].words.is_none());
    }

    #[test]
    fn supports_precision_variants() {
        let lines = parse(
            "[01:02]a\n[01:03.5]b\n[01:04.25]c\n[01:05.125]d\n[01:06:50]e",
            0,
        );
        assert_eq!(
            lines.iter().map(|l| l.start_ms).collect::<Vec<_>>(),
            vec![62_000, 63_500, 64_250, 65_125, 66_500]
        );
        // Unknown duration: last line gets a default length.
        assert_eq!(lines[4].end_ms, 66_500 + LAST_LINE_MS);
    }

    #[test]
    fn expands_multiple_stamps_and_sorts() {
        let lines = parse("[00:10.00][00:30.00]Chorus\n[00:20.00]Verse", 40_000);
        assert_eq!(
            starts(&lines),
            vec![(10_000, "Chorus"), (20_000, "Verse"), (30_000, "Chorus")]
        );
    }

    #[test]
    fn applies_offset_tag() {
        let lines = parse("[offset:+500]\n[00:10.00]a\n[00:00.20]b", 0);
        // Positive offset = earlier; never below zero.
        assert_eq!(starts(&lines), vec![(0, "b"), (9_500, "a")]);
        let lines = parse("[offset:-250]\n[00:10.00]a", 0);
        assert_eq!(lines[0].start_ms, 10_250);
    }

    #[test]
    fn marks_and_collapses_breaks() {
        let lines = parse(
            "[00:01.00]a\n[00:05.00]\n[00:06.00]   \n[00:20.00]b",
            25_000,
        );
        assert_eq!(
            starts(&lines),
            vec![(1_000, "a"), (5_000, ""), (20_000, "b")]
        );
        assert_eq!(lines[1].end_ms, 20_000);
    }

    #[test]
    fn parses_enhanced_word_stamps() {
        let lines = parse(
            "[00:10.00]<00:10.00> Hello <00:10.50> big <00:11.20> world\n[00:13.00]next",
            0,
        );
        assert_eq!(lines[0].text, "Hello big world");
        let words = lines[0].words.as_ref().unwrap();
        let w: Vec<_> = words
            .iter()
            .map(|w| (w.start_ms, w.end_ms, w.text.as_str()))
            .collect();
        assert_eq!(
            w,
            vec![
                (10_000, 10_500, "Hello"),
                (10_500, 11_200, "big"),
                (11_200, 13_000, "world")
            ]
        );
    }

    #[test]
    fn enhanced_with_trailing_end_stamp() {
        // Some files close the last word with a stamp; it must not become a word.
        let lines = parse(
            "[00:01.00]<00:01.00>a <00:01.50>b <00:02.00>\n[00:05.00]c",
            0,
        );
        let words = lines[0].words.as_ref().unwrap();
        assert_eq!(words.len(), 2);
        assert_eq!((words[1].start_ms, words[1].end_ms), (1_500, 2_000));
        assert_eq!(lines[0].text, "a b");
    }

    #[test]
    fn skips_malformed_lines() {
        let lrc = "garbage\n[xx:yy]bad\n[00:61.00]bad seconds\n[00:01.00]good\n[00:02.0000]too precise\n[]empty\n[00:03.00]also good";
        let lines = parse(lrc, 0);
        assert_eq!(starts(&lines), vec![(1_000, "good"), (3_000, "also good")]);
    }

    #[test]
    fn keeps_brackets_inside_lyrics() {
        let lines = parse("[00:01.00][Chorus] la la <3", 0);
        assert_eq!(lines[0].text, "[Chorus] la la <3");
        assert!(lines[0].words.is_none());
    }

    #[test]
    fn turkish_text_survives() {
        let lines = parse("[00:01.00]Şımarık, çığlık; ölürüm İstanbul'a", 0);
        assert_eq!(lines[0].text, "Şımarık, çığlık; ölürüm İstanbul'a");
    }

    // S8: crowd-sourced LRC is untrusted; arithmetic must not overflow.
    #[test]
    fn huge_minutes_do_not_overflow() {
        let lines = parse("[99999999999999999:00.00]x\n[00:01.00]y", 0);
        assert_eq!(starts(&lines), vec![(1000, "y")]);
    }

    #[test]
    fn extreme_offset_does_not_overflow() {
        let lines = parse("[offset:-9223372036854775808]\n[00:01.00]y", 0);
        assert_eq!(lines.len(), 1);
        let lines = parse("[offset:9223372036854775807]\n[00:01.00]y", 0);
        assert_eq!(lines[0].start_ms, 0);
    }

    proptest::proptest! {
        #[test]
        fn never_panics_and_keeps_invariants(input in "\\PC{0,400}", duration in 0u64..20_000_000) {
            let lines = parse(&input, duration);
            for pair in lines.windows(2) {
                proptest::prop_assert!(pair[0].start_ms <= pair[1].start_ms);
            }
            for l in &lines {
                proptest::prop_assert!(l.end_ms >= l.start_ms);
                for w in l.words.iter().flatten() {
                    proptest::prop_assert!(w.end_ms >= w.start_ms);
                }
            }
        }

        #[test]
        fn never_panics_on_tag_soup(input in r"(\[[0-9:.+-]{0,24}\]|<[0-9:.]{0,24}>|\[offset:-?[0-9]{0,22}\]|[a-z ]{0,4}|\n){0,40}") {
            let _ = parse(&input, 0);
        }
    }
}
