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
