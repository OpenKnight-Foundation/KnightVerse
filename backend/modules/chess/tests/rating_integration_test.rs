//! Glicko-2 rating (BE-187) through the crate's public API.

use chess::rating::{
    glicko2_update, Glicko2Match, Glicko2Rating, GLICKO_DEFAULT_RD, GLICKO_DEFAULT_VOLATILITY,
};
use chess::{RatingConfig, K_FACTOR_CLASSICAL};

fn rating(rating: f64, rd: f64) -> Glicko2Rating {
    Glicko2Rating {
        rating,
        rd,
        volatility: 0.06,
    }
}

fn approx(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() < tol
}

#[test]
fn rating_config_defaults() {
    let config = RatingConfig::default();
    assert_eq!(config.k_factor, K_FACTOR_CLASSICAL);
    assert_eq!(config.min_rating, 100);
    assert_eq!(config.max_rating, 3000);
}

// Reference example from Glickman's Glicko-2 paper: r=1500, RD=200, sigma=0.06
// against (1400, 30, win), (1550, 100, loss), (1700, 300, loss).
#[test]
fn glicko2_reference_values() {
    let matches = [
        Glicko2Match {
            opponent: rating(1400.0, 30.0),
            score: 1.0,
        },
        Glicko2Match {
            opponent: rating(1550.0, 100.0),
            score: 0.0,
        },
        Glicko2Match {
            opponent: rating(1700.0, 300.0),
            score: 0.0,
        },
    ];
    let updated = glicko2_update(rating(1500.0, 200.0), &matches);
    assert!(
        approx(updated.rating, 1464.06, 0.1),
        "rating {}",
        updated.rating
    );
    assert!(approx(updated.rd, 151.52, 0.1), "rd {}", updated.rd);
    assert!(
        approx(updated.volatility, 0.05999, 0.0001),
        "vol {}",
        updated.volatility
    );
}

#[test]
fn win_against_equal_opponent_raises_rating_and_lowers_rd() {
    let player = rating(1500.0, 200.0);
    let updated = glicko2_update(
        player,
        &[Glicko2Match {
            opponent: rating(1500.0, 200.0),
            score: 1.0,
        }],
    );
    assert!(updated.rating > player.rating);
    assert!(updated.rd < player.rd);
}

#[test]
fn provisional_status_follows_rd() {
    assert!(Glicko2Rating::default().is_provisional());
    assert!(!rating(1500.0, 60.0).is_provisional());
}

#[test]
fn elo_migration_keeps_rating_and_starts_provisional() {
    let migrated = Glicko2Rating::from_elo(1800);
    assert_eq!(migrated.rating, 1800.0);
    assert_eq!(migrated.rd, GLICKO_DEFAULT_RD);
    assert_eq!(migrated.volatility, GLICKO_DEFAULT_VOLATILITY);
    assert!(migrated.is_provisional());
}
