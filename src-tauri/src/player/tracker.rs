//! Position bookkeeping that is independent of D-Bus, so it can be unit tested.
//!
//! The player is the source of truth while it reports sane positions. Some
//! Spotify builds always report `Position = 0`; after [`ZERO_STREAK_LIMIT`]
//! consecutive zero reads while playing we switch to counting time ourselves
//! from the last known anchor (track change, seek or resume).

use crate::events::PositionSource;

pub const ZERO_STREAK_LIMIT: u32 = 3;

#[derive(Debug, Clone)]
pub struct PositionTracker {
    anchor_pos_ms: u64,
    anchor_time_ms: u64,
    playing: bool,
    zero_streak: u32,
    source: PositionSource,
}

impl Default for PositionTracker {
    fn default() -> Self {
        Self {
            anchor_pos_ms: 0,
            anchor_time_ms: 0,
            playing: false,
            zero_streak: 0,
            source: PositionSource::Player,
        }
    }
}

impl PositionTracker {
    pub fn source(&self) -> PositionSource {
        self.source
    }

    /// Our own estimate: last anchor plus wall time elapsed while playing.
    pub fn estimate(&self, now_ms: u64) -> u64 {
        if self.playing {
            self.anchor_pos_ms + now_ms.saturating_sub(self.anchor_time_ms)
        } else {
            self.anchor_pos_ms
        }
    }

    fn anchor(&mut self, pos_ms: u64, now_ms: u64) {
        self.anchor_pos_ms = pos_ms;
        self.anchor_time_ms = now_ms;
    }

    /// A new track started; position restarts at `pos_ms` (normally 0).
    /// The zero-streak detection is kept: a broken player stays broken.
    pub fn track_changed(&mut self, pos_ms: u64, now_ms: u64) {
        self.anchor(pos_ms, now_ms);
    }

    pub fn set_playing(&mut self, playing: bool, now_ms: u64) {
        if playing != self.playing {
            let pos = self.estimate(now_ms);
            self.playing = playing;
            self.anchor(pos, now_ms);
        }
    }

    /// The player emitted `Seeked`; trust it unconditionally.
    pub fn seeked(&mut self, pos_ms: u64, now_ms: u64) {
        self.anchor(pos_ms, now_ms);
    }

    /// Feeds a polled `Position` value and returns the position to publish.
    pub fn polled(&mut self, pos_ms: u64, now_ms: u64) -> u64 {
        if pos_ms == 0 {
            if self.playing {
                self.zero_streak = self.zero_streak.saturating_add(1);
                if self.zero_streak >= ZERO_STREAK_LIMIT {
                    self.source = PositionSource::Estimated;
                }
            }
            // Either a real 0 right at track start (estimate is ~0 too) or a
            // broken player; the estimate is correct in both cases.
            if self.playing || self.source == PositionSource::Estimated {
                return self.estimate(now_ms);
            }
        }
        if pos_ms > 0 {
            self.zero_streak = 0;
            self.source = PositionSource::Player;
        }
        self.anchor(pos_ms, now_ms);
        pos_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_player_when_positions_are_sane() {
        let mut t = PositionTracker::default();
        t.set_playing(true, 0);
        assert_eq!(t.polled(1_000, 1_000), 1_000);
        assert_eq!(t.polled(2_050, 2_000), 2_050);
        assert_eq!(t.source(), PositionSource::Player);
        // Between polls the estimate extrapolates from the last read.
        assert_eq!(t.estimate(2_500), 2_550);
    }

    #[test]
    fn switches_to_estimate_after_three_zero_reads() {
        let mut t = PositionTracker::default();
        t.track_changed(0, 10_000);
        t.set_playing(true, 10_000);
        assert_eq!(t.polled(0, 11_000), 1_000);
        assert_eq!(t.polled(0, 12_000), 2_000);
        assert_eq!(t.source(), PositionSource::Player);
        assert_eq!(t.polled(0, 13_000), 3_000);
        assert_eq!(t.source(), PositionSource::Estimated);

        // Pausing freezes the estimate; resuming continues from there.
        t.set_playing(false, 14_000);
        assert_eq!(t.polled(0, 20_000), 4_000);
        t.set_playing(true, 20_000);
        assert_eq!(t.polled(0, 21_000), 5_000);

        // Next track restarts the clock but keeps estimated mode.
        t.track_changed(0, 30_000);
        assert_eq!(t.polled(0, 31_000), 1_000);
        assert_eq!(t.source(), PositionSource::Estimated);

        // Seeked signals still move the estimate.
        t.seeked(60_000, 31_000);
        assert_eq!(t.polled(0, 32_000), 61_000);
    }

    #[test]
    fn recovers_when_player_reports_again() {
        let mut t = PositionTracker::default();
        t.set_playing(true, 0);
        for i in 1..=3 {
            t.polled(0, i * 1000);
        }
        assert_eq!(t.source(), PositionSource::Estimated);
        assert_eq!(t.polled(42_000, 4_000), 42_000);
        assert_eq!(t.source(), PositionSource::Player);
    }

    #[test]
    fn zero_while_paused_is_not_suspicious() {
        let mut t = PositionTracker::default();
        for i in 1..=5 {
            assert_eq!(t.polled(0, i * 1000), 0);
        }
        assert_eq!(t.source(), PositionSource::Player);
    }
}
