use chess::{PlayerClock, TimeControl};
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

        clock.apply_delay(time_control.delay);
        assert_eq!(clock.get_real_time_remaining(), Duration::from_secs(300));

        clock.apply_increment(time_control.increment);
        assert_eq!(clock.get_real_time_remaining(), Duration::from_secs(302));

        clock.start();
        std::thread::sleep(Duration::from_secs(2));
        clock.stop();
        assert!(clock.get_real_time_remaining() <= Duration::from_secs(300));

        assert!(!clock.time_out());
        clock.set_remaining_time(Duration::from_secs(0));
        assert!(clock.time_out());
    }
}
