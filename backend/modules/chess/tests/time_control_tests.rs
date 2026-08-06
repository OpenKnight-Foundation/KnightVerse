use chess::{TimeControl, PlayerClock};
use std::time::Duration;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_control() {
        let time_control = TimeControl {
            initial_time: Duration::from_secs(300),
            increment: Duration::from_secs(2),
            delay: Duration::from_secs(1),
        };

        let mut clock = PlayerClock::new(time_control.initial_time);
        clock.start();
        std::thread::sleep(Duration::from_secs(1));
        clock.stop();

        assert!(clock.get_real_time_remaining() <= Duration::from_secs(299));

        let before_delay = clock.get_real_time_remaining();
        clock.apply_delay(time_control.delay);
        let after_delay = clock.get_real_time_remaining();
        assert!(after_delay > before_delay);
        assert!(after_delay <= Duration::from_secs(300));
        assert!(after_delay > Duration::from_secs(298));

        let before_inc = clock.get_real_time_remaining();
        clock.apply_increment(time_control.increment);
        let after_inc = clock.get_real_time_remaining();
        assert_eq!(after_inc, before_inc + time_control.increment);

        clock.start();
        std::thread::sleep(Duration::from_secs(2));
        clock.stop();
        assert!(clock.get_real_time_remaining() <= Duration::from_secs(300));

        assert!(!clock.time_out());
        clock.set_remaining_time(Duration::from_secs(0));
        assert!(clock.time_out());
    }
}
