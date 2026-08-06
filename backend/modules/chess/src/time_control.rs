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
        self.remaining_time.is_zero()
    }
}
