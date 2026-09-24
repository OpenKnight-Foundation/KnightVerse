use serde::{Deserialize, Serialize};

/// Minimum variance threshold below which move timing is considered artificial
const VAR_MIN_THRESHOLD: f64 = 0.01;

/// Engine correlation threshold above which the game is flagged
const ENGINE_CORRELATION_THRESHOLD: f64 = 0.95;

/// Represents a single move's timing data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveTiming {
    /// Move number (1-indexed)
    pub move_number: u32,
    /// Time taken for this move in milliseconds
    pub time_ms: u64,
    /// Engine evaluation score for this position (optional)
    pub engine_eval: Option<f64>,
}

/// Represents the result of anti-cheat analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiCheatReport {
    /// Game ID
    pub game_id: String,
    /// Player address being analyzed
    pub player_address: String,
    /// Whether the game was flagged as suspicious
    pub flagged: bool,
    /// Reason for flagging (if any)
    pub flag_reason: Option<String>,
    /// Statistical metrics
    pub metrics: LatencyMetrics,
}

/// Statistical metrics for move timing analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyMetrics {
    /// Mean move time in milliseconds
    pub mean_ms: f64,
    /// Variance of move times
    pub variance: f64,
    /// Standard deviation of move times
    pub std_dev: f64,
    /// Skewness of move time distribution
    pub skewness: f64,
    /// Kurtosis of move time distribution
    pub kurtosis: f64,
    /// Pearson correlation with engine evaluation (if available)
    pub engine_correlation: Option<f64>,
    /// Number of moves analyzed
    pub move_count: u32,
}

/// Calculate the mean of a slice of values
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let sum: f64 = values.iter().sum();
    sum / values.len() as f64
}

/// Calculate the variance of a slice of values
pub fn variance(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    let sum_sq_diff: f64 = values.iter().map(|&x| (x - m).powi(2)).sum();
    sum_sq_diff / (values.len() - 1) as f64
}

/// Calculate the standard deviation of a slice of values
pub fn std_dev(values: &[f64]) -> f64 {
    variance(values).sqrt()
}

/// Calculate the skewness of a slice of values
pub fn skewness(values: &[f64]) -> f64 {
    if values.len() < 3 {
        return 0.0;
    }
    let n = values.len() as f64;
    let m = mean(values);
    let s = std_dev(values);

    if s == 0.0 {
        return 0.0;
    }

    let sum_cubed_diff: f64 = values.iter().map(|&x| ((x - m) / s).powi(3)).sum();
    (n / ((n - 1.0) * (n - 2.0))) * sum_cubed_diff
}

/// Calculate the kurtosis of a slice of values
pub fn kurtosis(values: &[f64]) -> f64 {
    if values.len() < 4 {
        return 0.0;
    }
    let n = values.len() as f64;
    let m = mean(values);
    let s = std_dev(values);

    if s == 0.0 {
        return 0.0;
    }

    let sum_fourth_diff: f64 = values.iter().map(|&x| ((x - m) / s).powi(4)).sum();
    let k = (n * (n + 1.0) / ((n - 1.0) * (n - 2.0) * (n - 3.0))) * sum_fourth_diff;
    k - (3.0 * (n - 1.0).powi(2) / ((n - 2.0) * (n - 3.0)))
}

/// Calculate Pearson correlation coefficient between two slices
pub fn pearson_correlation(x: &[f64], y: &[f64]) -> f64 {
    if x.len() != y.len() || x.len() < 2 {
        return 0.0;
    }

    let n = x.len() as f64;
    let mean_x = mean(x);
    let mean_y = mean(y);

    let mut sum_xy = 0.0;
    let mut sum_x_sq = 0.0;
    let mut sum_y_sq = 0.0;

    for i in 0..x.len() {
        let dx = x[i] - mean_x;
        let dy = y[i] - mean_y;
        sum_xy += dx * dy;
        sum_x_sq += dx * dx;
        sum_y_sq += dy * dy;
    }

    let denominator = (sum_x_sq * sum_y_sq).sqrt();
    if denominator == 0.0 {
        return 0.0;
    }

    sum_xy / denominator
}

/// Analyze move timing data and generate an anti-cheat report
pub fn analyze_move_timing(
    game_id: &str,
    player_address: &str,
    move_timings: &[MoveTiming],
) -> AntiCheatReport {
    if move_timings.len() < 10 {
        return AntiCheatReport {
            game_id: game_id.to_string(),
            player_address: player_address.to_string(),
            flagged: false,
            flag_reason: None,
            metrics: LatencyMetrics {
                mean_ms: 0.0,
                variance: 0.0,
                std_dev: 0.0,
                skewness: 0.0,
                kurtosis: 0.0,
                engine_correlation: None,
                move_count: move_timings.len() as u32,
            },
        };
    }

    let times: Vec<f64> = move_timings.iter().map(|m| m.time_ms as f64).collect();

    let metrics = LatencyMetrics {
        mean_ms: mean(&times),
        variance: variance(&times),
        std_dev: std_dev(&times),
        skewness: skewness(&times),
        kurtosis: kurtosis(&times),
        engine_correlation: None,
        move_count: move_timings.len() as u32,
    };

    // Check for engine correlation if engine evals are available
    let mut engine_correlation = None;
    let evals: Vec<f64> = move_timings
        .iter()
        .filter_map(|m| m.engine_eval)
        .collect();

    if evals.len() >= 10 {
        // Match evals with corresponding times (skip entries without evals)
        let paired_times: Vec<f64> = move_timings
            .iter()
            .filter(|m| m.engine_eval.is_some())
            .map(|m| m.time_ms as f64)
            .collect();

        if paired_times.len() == evals.len() && paired_times.len() >= 10 {
            let corr = pearson_correlation(&paired_times, &evals);
            engine_correlation = Some(corr);
        }
    }

    // Flag if variance is suspiciously low and engine correlation is high
    let mut flagged = false;
    let mut flag_reason = None;

    if metrics.variance < VAR_MIN_THRESHOLD {
        if let Some(corr) = engine_correlation {
            if corr > ENGINE_CORRELATION_THRESHOLD {
                flagged = true;
                flag_reason = Some(format!(
                    "Suspicious latency: variance {:.4} < {:.4} and engine correlation {:.4} > {:.4}",
                    metrics.variance, VAR_MIN_THRESHOLD, corr, ENGINE_CORRELATION_THRESHOLD
                ));
            }
        }
    }

    AntiCheatReport {
        game_id: game_id.to_string(),
        player_address: player_address.to_string(),
        flagged,
        flag_reason,
        metrics: LatencyMetrics {
            engine_correlation,
            ..metrics
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mean() {
        assert_eq!(mean(&[1.0, 2.0, 3.0, 4.0, 5.0]), 3.0);
        assert_eq!(mean(&[10.0, 10.0, 10.0]), 10.0);
        assert_eq!(mean(&[]), 0.0);
    }

    #[test]
    fn test_variance() {
        let v = variance(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert!((v - 2.5).abs() < 0.001);
        assert_eq!(variance(&[5.0]), 0.0);
    }

    #[test]
    fn test_std_dev() {
        let s = std_dev(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert!((s - 1.5811).abs() < 0.001);
    }

    #[test]
    fn test_pearson_correlation() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let corr = pearson_correlation(&x, &y);
        assert!((corr - 1.0).abs() < 0.001);

        let y_neg = vec![10.0, 8.0, 6.0, 4.0, 2.0];
        let corr_neg = pearson_correlation(&x, &y_neg);
        assert!((corr_neg - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_analyze_timing_not_flagged() {
        let timings: Vec<MoveTiming> = (0..20)
            .map(|i| MoveTiming {
                move_number: i,
                time_ms: 1000 + (i as u64 * 100), // Varying times
                engine_eval: None,
            })
            .collect();

        let report = analyze_move_timing("game1", "player1", &timings);
        assert!(!report.flagged);
        assert!(report.flag_reason.is_none());
    }

    #[test]
    fn test_analyze_timing_suspicious() {
        // Fixed timing (bot-like) with high engine correlation
        let timings: Vec<MoveTiming> = (0..20)
            .map(|i| MoveTiming {
                move_number: i,
                time_ms: 1200, // Exactly same time every move
                engine_eval: Some(i as f64 * 0.5),
            })
            .collect();

        let report = analyze_move_timing("game1", "player1", &timings);
        // Low variance should be detected
        assert!(report.metrics.variance < 0.001);
    }

    #[test]
    fn test_analyze_timing_insufficient_data() {
        let timings: Vec<MoveTiming> = (0..5)
            .map(|i| MoveTiming {
                move_number: i,
                time_ms: 1000,
                engine_eval: None,
            })
            .collect();

        let report = analyze_move_timing("game1", "player1", &timings);
        assert!(!report.flagged);
        assert_eq!(report.metrics.move_count, 5);
    }
}
