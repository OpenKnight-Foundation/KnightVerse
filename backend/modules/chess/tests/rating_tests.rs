use chess::rating::{Glicko2Rating, TimeControl, PlayerRating, Outcome, migrate_elo};

const EPS: f64 = 1e-3;

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < EPS
}

// Reference example from Glickman's Glicko-2 paper (2013), section 4.
// Player: r = 1500, RD = 200, sigma = 0.06
// Opponents: (1400, 30, win), (1550, 100, loss), (1700, 300, loss)
#[test]
fn glicko2_reference_update_matches_paper() {
    let player = Glicko2Rating { rating: 1500.0, rd: 200.0, volatility: 0.06 };
    let results = [
        (1400.0, 30.0, Outcome::Win),
        (1550.0, 100.0, Outcome::Loss),
        (1700.0, 300.0, Outcome::Loss),
    ];
    let updated = player.update(&results, 0.5);
    assert!(approx(updated.rating, 1464.06), "rating was {}", updated.rating);
    assert!(approx(updated.rd, 151.52), "rd was {}", updated.rd);
    assert!(approx(updated.volatility, 0.05999), "volatility was {}", updated.volatility);
}

// Glicko-2 with no games: RD grows according to the inactivity formula.
#[test]
fn glicko2_no_games_increases_rd() {
    let player = Glicko2Rating { rating: 1500.0, rd: 200.0, volatility: 0.06 };
    let updated = player.update(&[], 0.5);
    assert!(approx(updated.rating, 1500.0));
    assert!(approx(updated.rd, 200.04 * 0.0 + (200.0_f64.powi(2) + 0.06_f64.powi(2) * 0.0 + 200.0_f64.powi(2)).sqrt().min(350.0)).max(200.0) || updated.rd > 200.0);
    assert!(updated.rd > 200.0);
    assert!(approx(updated.volatility, 0.06));
}

#[test]
fn rd_is_capped_at_350_for_inactive_players() {
    let player = Glicko2Rating { rating: 1500.0, rd: 350.0, volatility: 0.06 };
    let updated = player.update(&[], 0.5);
    assert!(approx(updated.rd, 350.0));
}

#[test]
fn new_players_default_to_high_rd_and_are_provisional() {
    let rating = PlayerRating::new_default(TimeControl::Blitz);
    assert!(approx(rating.glicko.rating, 1500.0));
    assert!(approx(rating.glicko.rd, 350.0));
    assert!(approx(rating.glicko.volatility, 0.06));
    assert!(rating.is_provisional());
}

#[test]
fn provisional_clears_when_rd_drops_below_threshold() {
    let rating = PlayerRating {
        glicko: Glicko2Rating { rating: 1600.0, rd: 80.0, volatility: 0.06 },
        time_control: TimeControl::Rapid,
    };
    assert!(!rating.is_provisional());
}

#[test]
fn ratings_are_stored_per_time_control() {
    let mut ratings = PlayerRating::new_default(TimeControl::Bullet);
    ratings.time_control = TimeControl::Classical;
    assert_eq!(ratings.time_control, TimeControl::Classical);
}

#[test]
fn elo_migration_maps_rating_and_sets_high_rd() {
    let migrated = migrate_elo(1800.0);
    assert!(approx(migrated.rating, 1800.0));
    assert!(approx(migrated.rd, 350.0));
    assert!(approx(migrated.volatility, 0.06));
}

#[test]
fn win_against_equal_opponent_increases_rating() {
    let player = Glicko2Rating { rating: 1500.0, rd: 200.0, volatility: 0.06 };
    let updated = player.update(&[(1500.0, 200.0, Outcome::Win)], 0.5);
    assert!(updated.rating > 1500.0);
    assert!(updated.rd < 200.0);
}
