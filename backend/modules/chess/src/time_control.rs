use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct TimeControl {
    pub initial_time: Duration,
    pub increment: Duration,
    pub delay: Duration,
}

/// Coarse time-control category, used to tune Elo rating updates.
///
/// Thresholds follow common platform conventions applied to the estimated
/// total thinking time per player: bullet < 3 min, blitz < 8 min,
/// rapid < 25 min, otherwise classical. Sub-bullet (ultra-bullet) controls
/// fold into [`TimeControlCategory::Bullet`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeControlCategory {
    Bullet,
    Blitz,
    Rapid,
    Classical,
}

impl TimeControl {
    /// Classifies this time control using the estimated total time per
    /// player: `initial + 40 * increment` (roughly one increment payout per
    /// move over a 40-move game). `delay` only refunds elapsed thinking
    /// time instead of adding time, so it is ignored here.
    pub fn category(&self) -> TimeControlCategory {
        let estimated_secs = self.initial_time.as_secs() + 40 * self.increment.as_secs();
        TimeControlCategory::from_total_seconds(estimated_secs)
    }
}

impl TimeControlCategory {
    /// Classifies an estimated total time (in seconds) per player.
    pub fn from_total_seconds(total_secs: u64) -> Self {
        if total_secs < 180 {
            Self::Bullet
        } else if total_secs < 480 {
            Self::Blitz
        } else if total_secs < 1500 {
            Self::Rapid
        } else {
            Self::Classical
        }
    }

    /// Classifies a stored base time in whole seconds with no increment
    /// information (e.g. the `duration_sec` column on a game row).
    ///
    /// Non-positive values mean "unknown" and map to [`TimeControlCategory::Classical`],
    /// the stable default tier, rather than the most volatile one.
    pub fn from_base_seconds(base_secs: i32) -> Self {
        if base_secs <= 0 {
            Self::Classical
        } else {
            Self::from_total_seconds(base_secs as u64)
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlayerClock {
    pub remaining_time: Duration,
    pub last_move_time: Option<Instant>,
    pub is_running: bool,
}

impl PlayerClock {
    pub fn new(initial_time: Duration) -> Self {
        Self {
            remaining_time: initial_time,
            last_move_time: None,
            is_running: false,
        }
    }

    pub fn start(&mut self) {
        if self.is_running {
            return;
        }
        self.is_running = true;
        self.last_move_time = Some(Instant::now());
    }

    pub fn stop(&mut self) {
        if let Some(last_move_time) = self.last_move_time {
            let elapsed = last_move_time.elapsed();
            self.remaining_time = self.remaining_time.saturating_sub(elapsed);
        }
        self.is_running = false;
    }

    pub fn apply_increment(&mut self, increment: Duration) {
        self.remaining_time += increment;
    }

    pub fn apply_delay(&mut self, delay: Duration) {
        if let Some(last_move_time) = self.last_move_time {
            let elapsed = last_move_time.elapsed();
            let refund = std::cmp::min(elapsed, delay);
            self.remaining_time += refund;
        }
    }

    pub fn get_real_time_remaining(&self) -> Duration {
        if self.is_running {
            if let Some(last_move_time) = self.last_move_time {
                return self.remaining_time.saturating_sub(last_move_time.elapsed());
            }
        }
        self.remaining_time
    }

    pub fn set_remaining_time(&mut self, time: Duration) {
        self.remaining_time = time;
        self.last_move_time = None;
        self.is_running = false;
    }

    pub fn time_out(&self) -> bool {
        self.get_real_time_remaining().is_zero()
    }
}


// ---------------------------------------------------------------------------
// Time-control presets (#1119)
// ---------------------------------------------------------------------------

/// The time-control systems a preset can describe.
///
/// The three "delay" styles are genuinely different and are easy to conflate:
///
/// * **Bronstein** — the first `delay` seconds of each move are free, so a move
///   costs `max(0, elapsed - delay)`.
/// * **USCF delay** — the clock is granted `delay` seconds per move regardless
///   of how long the move took, so a move costs `min(elapsed, delay)`.
/// * **Fischer** — a flat increment is *added*, not refunded. Added before or
///   after the move changes the result, because adding first can rescue a move
///   that would otherwise flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeControlKind {
    Standard,
    /// Increment added after the move is made.
    FischerAfter,
    /// Increment added before the move is made ("Fischer bezoegen").
    FischerBefore,
    /// First `delay` seconds of each move refunded, up to that cap.
    Bronstein,
    /// Flat `delay` granted per move.
    UscfDelay,
    /// Fixed periods; the clock resets at the start of each new period.
    ByoYomi,
}

/// A fully specified time control, ready to drive a [`PlayerClock`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeControlPreset {
    pub kind: TimeControlKind,
    pub initial_time: Duration,
    pub increment: Duration,
    /// Bronstein's per-move refund cap, or USCF delay's flat grant.
    pub delay: Duration,
    /// Byo-Yomi periods still available.
    pub periods: u32,
    /// Time granted by each Byo-Yomi period.
    pub period_time: Duration,
}

impl TimeControlPreset {
    fn base(kind: TimeControlKind, initial_time: Duration) -> Self {
        Self {
            kind,
            initial_time,
            increment: Duration::ZERO,
            delay: Duration::ZERO,
            periods: 0,
            period_time: Duration::ZERO,
        }
    }

    /// No increment, no delay.
    pub fn standard(initial_time: Duration) -> Self {
        Self::base(TimeControlKind::Standard, initial_time)
    }

    /// Fischer, increment added after the move.
    pub fn fischer(initial_time: Duration, increment: Duration) -> Self {
        Self {
            increment,
            ..Self::base(TimeControlKind::FischerAfter, initial_time)
        }
    }

    /// Fischer with the increment added before the move is made.
    pub fn fischer_before(initial_time: Duration, increment: Duration) -> Self {
        Self {
            increment,
            ..Self::base(TimeControlKind::FischerBefore, initial_time)
        }
    }

    /// Bronstein: the first `max_delay` of each move is refunded.
    pub fn bronstein(initial_time: Duration, max_delay: Duration) -> Self {
        Self {
            delay: max_delay,
            ..Self::base(TimeControlKind::Bronstein, initial_time)
        }
    }

    /// USCF delay: `delay` is granted per move.
    pub fn uscf_delay(initial_time: Duration, delay: Duration) -> Self {
        Self {
            delay,
            ..Self::base(TimeControlKind::UscfDelay, initial_time)
        }
    }

    /// Byo-Yomi: `periods` periods of `period_time` each, on top of
    /// `initial_time`.
    pub fn byo_yomi(initial_time: Duration, periods: u32, period_time: Duration) -> Self {
        Self {
            periods,
            period_time,
            ..Self::base(TimeControlKind::ByoYomi, initial_time)
        }
    }

    /// Short human-readable label, e.g. `3+2` or `3x5 Byo-Yomi`.
    pub fn describe(&self) -> String {
        let base = self.initial_time.as_secs();
        match self.kind {
            TimeControlKind::Standard => format!("{base}"),
            TimeControlKind::FischerAfter => {
                format!("{base}+{}", self.increment.as_secs())
            }
            TimeControlKind::FischerBefore => {
                format!("{base}+{} before", self.increment.as_secs())
            }
            TimeControlKind::Bronstein => format!("{base}|{} Bronstein", self.delay.as_secs()),
            TimeControlKind::UscfDelay => format!("{base}|{} delay", self.delay.as_secs()),
            TimeControlKind::ByoYomi => {
                format!("{base}+{}x{} Byo-Yomi", self.periods, self.period_time.as_secs())
            }
        }
    }
}

/// What completing a move did to a clock, so a caller can log or display it
/// rather than re-deriving the rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClockAdjustment {
    /// Time actually removed from the mover's clock.
    pub deducted: Duration,
    /// Time returned to the mover (Bronstein or USCF delay).
    pub refunded: Duration,
    /// Increment added (Fischer).
    pub increment_added: Duration,
    /// Byo-Yomi periods consumed because the clock hit zero.
    pub periods_consumed: u32,
    /// The mover ran out of time with no period left to save them.
    pub flagged: bool,
}

impl PlayerClock {
    /// Builds a clock positioned at the start of `preset.initial_time`.
    pub fn from_preset(preset: &TimeControlPreset) -> Self {
        Self::new(preset.initial_time)
    }

    /// True once the clock is at or below `threshold`, which is the cue for the
    /// audible low-time warning.
    pub fn should_warn(&self, threshold: Duration) -> bool {
        self.get_real_time_remaining() <= threshold
    }

    /// Settles a move under `preset` and reports exactly what changed.
    ///
    /// The clock must be running; elapsed time is measured once, here, rather
    /// than by the caller calling [`PlayerClock::stop`] first, so the deduction
    /// and the refund cannot be computed from two different readings.
    pub fn complete_move(&mut self, preset: &TimeControlPreset) -> ClockAdjustment {
        let elapsed = match self.last_move_time {
            Some(started) if self.is_running => started.elapsed(),
            _ => Duration::ZERO,
        };
        self.is_running = false;
        self.last_move_time = None;

        let mut adjustment = ClockAdjustment::default();

        // The two delay systems differ in exactly one respect: Bronstein caps
        // the refund at the time actually spent, so a fast move gains nothing,
        // whereas USCF delay grants the full delay every move, so a move faster
        // than the delay actually adds time to the clock.
        let refund = match preset.kind {
            TimeControlKind::Bronstein => std::cmp::min(elapsed, preset.delay),
            TimeControlKind::UscfDelay => preset.delay,
            _ => Duration::ZERO,
        };
        adjustment.refunded = refund;

        // Fischer-before grants the increment up front, so it can offset the
        // move that is about to be deducted.
        if preset.kind == TimeControlKind::FischerBefore {
            self.remaining_time += preset.increment;
            adjustment.increment_added = preset.increment;
        }

        self.remaining_time = self
            .remaining_time
            .saturating_add(refund)
            .saturating_sub(elapsed);
        adjustment.deducted = elapsed;

        // The flag decision is taken here, *before* a Fischer-after increment.
        // That increment is only paid once the move is completed, so it must not
        // rescue a mover who was already out of time.
        if self.remaining_time.is_zero() {
            if preset.kind == TimeControlKind::ByoYomi && preset.periods > 0 {
                self.remaining_time = preset.period_time;
                adjustment.periods_consumed = 1;
            } else {
                adjustment.flagged = true;
            }
        }

        if preset.kind == TimeControlKind::FischerAfter {
            self.remaining_time += preset.increment;
            adjustment.increment_added = preset.increment;
        }

        adjustment
    }

    /// Byo-Yomi periods still available, i.e. the clock has not been used up.
    pub fn periods_left(&self, preset: &TimeControlPreset) -> u32 {
        if preset.kind == TimeControlKind::ByoYomi {
            preset.periods
        } else {
            0
        }
    }
}

// ---------------------------------------------------------------------------
// Endgame drill timing (#1124)
// ---------------------------------------------------------------------------

/// Audible warning threshold shared by rated play and drills.
pub const LOW_TIME_WARNING: Duration = Duration::from_secs(10);

/// How a single drill move is judged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrillVerdict {
    /// Solved within the target time.
    Pass,
    /// Legal, but slower than the target the drill was set to.
    TooSlow,
    /// Ran out of time before finishing the move.
    Flagged,
}

/// Accumulates per-move timings for one endgame drill attempt.
///
/// Only the timing and flagging half of a drill lives here: it reuses the
/// preset engine so a drill and a rated game are timed by identical rules. The
/// tablebase evaluation, the position catalogue and the pass/fail record
/// written to a player profile are not part of this module.
#[derive(Debug, Clone)]
pub struct DrillSession {
    preset: TimeControlPreset,
    clock: PlayerClock,
    moves: Vec<Duration>,
    last_move_started: Option<Instant>,
    finished: bool,
}

impl DrillSession {
    pub fn start(preset: TimeControlPreset) -> Self {
        let mut clock = PlayerClock::from_preset(&preset);
        clock.start();
        Self {
            preset,
            clock,
            moves: Vec::new(),
            last_move_started: Some(Instant::now()),
            finished: false,
        }
    }

    /// Clocks out the current move, judges it against `target`, and starts the
    /// next one.
    pub fn record_move(&mut self, target: Duration) -> DrillVerdict {
        if self.finished {
            return DrillVerdict::Flagged;
        }

        let adjustment = self.clock.complete_move(&self.preset);
        let elapsed = self
            .last_move_started
            .map(|started| started.elapsed())
            .unwrap_or_default();
        self.moves.push(elapsed);

        let verdict = if adjustment.flagged {
            self.finished = true;
            DrillVerdict::Flagged
        } else if elapsed > target {
            DrillVerdict::TooSlow
        } else {
            DrillVerdict::Pass
        };

        if !self.finished {
            self.clock.start();
            self.last_move_started = Some(Instant::now());
        }

        verdict
    }

    /// True once the clock has flagged and no further moves are accepted.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn move_count(&self) -> usize {
        self.moves.len()
    }

    pub fn total_time(&self) -> Duration {
        self.moves.iter().copied().sum()
    }

    /// Longest single move, used to spot where a solver lost the drill.
    pub fn slowest_move(&self) -> Option<Duration> {
        self.moves.iter().copied().max()
    }

    pub fn remaining_time(&self) -> Duration {
        self.clock.get_real_time_remaining()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopped_clock_reports_timeout_when_remaining_is_zero() {
        let mut clock = PlayerClock::new(Duration::from_secs(0));
        assert!(clock.time_out());

        clock.set_remaining_time(Duration::from_secs(10));
        assert!(!clock.time_out());
    }

    #[test]
    fn running_clock_flags_when_it_runs_past_zero_on_the_mover() {
        // Player on the move with almost no time left, clock running.
        let mut clock = PlayerClock::new(Duration::from_millis(1));
        clock.start();
        // The mover sits on the running clock past zero without moving.
        std::thread::sleep(Duration::from_millis(5));

        // remaining_time is untouched until stop(), so the raw field is still
        // non-zero; flag-fall must be detected via the real remaining time.
        assert!(!clock.remaining_time.is_zero());
        assert!(clock.time_out());
    }

    #[test]
    fn running_clock_with_time_left_is_not_flagged() {
        let mut clock = PlayerClock::new(Duration::from_secs(60));
        clock.start();
        assert!(!clock.time_out());
    }

    #[test]
    fn category_follows_total_time_thresholds() {
        use TimeControlCategory::*;
        assert_eq!(TimeControlCategory::from_base_seconds(60), Bullet);
        assert_eq!(TimeControlCategory::from_base_seconds(179), Bullet);
        assert_eq!(TimeControlCategory::from_base_seconds(180), Blitz);
        assert_eq!(TimeControlCategory::from_base_seconds(300), Blitz);
        assert_eq!(TimeControlCategory::from_base_seconds(479), Blitz);
        assert_eq!(TimeControlCategory::from_base_seconds(480), Rapid);
        assert_eq!(TimeControlCategory::from_base_seconds(600), Rapid);
        assert_eq!(TimeControlCategory::from_base_seconds(1499), Rapid);
        assert_eq!(TimeControlCategory::from_base_seconds(1500), Classical);
        assert_eq!(TimeControlCategory::from_base_seconds(3600), Classical);
    }

    #[test]
    fn unknown_base_time_falls_back_to_classical() {
        assert_eq!(
            TimeControlCategory::from_base_seconds(0),
            TimeControlCategory::Classical
        );
        assert_eq!(
            TimeControlCategory::from_base_seconds(-5),
            TimeControlCategory::Classical
        );
    }

    #[test]
    fn category_accounts_for_increment() {
        // 2|2 estimates 120 + 40*2 = 200s -> Blitz, although the 120s base
        // alone would classify as Bullet.
        let tc = TimeControl {
            initial_time: Duration::from_secs(120),
            increment: Duration::from_secs(2),
            delay: Duration::from_secs(0),
        };
        assert_eq!(tc.category(), TimeControlCategory::Blitz);

        // 5|0 stays Blitz; 10|0 is Rapid; 90|0 is Bullet.
        for (base, expected) in [
            (300, TimeControlCategory::Blitz),
            (600, TimeControlCategory::Rapid),
            (90, TimeControlCategory::Bullet),
        ] {
            let tc = TimeControl {
                initial_time: Duration::from_secs(base),
                increment: Duration::from_secs(0),
                delay: Duration::from_secs(0),
            };
            assert_eq!(tc.category(), expected);
        }
    }
}
