use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct TimeControl {
    pub initial_time: Duration,
    pub increment: Duration,
    pub delay: Duration,
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
            if elapsed < delay {
                self.remaining_time += delay - elapsed;
            }
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
}
