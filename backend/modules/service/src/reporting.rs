use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Rate limit: maximum reports per user per hour
const MAX_REPORTS_PER_HOUR: u32 = 3;

/// Shadow ban threshold: number of distinct reports within 24 hours
const SHADOW_BAN_THRESHOLD: u32 = 10;

/// Time window for shadow ban detection (24 hours)
const SHADOW_BAN_WINDOW: Duration = Duration::from_secs(24 * 60 * 60);

/// Time window for rate limiting (1 hour)
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60 * 60);

/// Report reasons
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ReportReason {
    Cheating,
    Harassment,
    Stall,
    Bot,
}

/// Represents a player report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerReport {
    /// Unique report ID
    pub id: u64,
    /// Reporter's address
    pub reporter: String,
    /// Reported player's address
    pub reported: String,
    /// Reason for the report
    pub reason: ReportReason,
    /// Optional game ID as evidence
    pub evidence_game_id: Option<String>,
    /// Timestamp when the report was filed
    pub timestamp: Instant,
}

/// Represents a player's report history
#[derive(Debug, Clone, Default)]
struct PlayerReportHistory {
    /// Reports filed by this player (for rate limiting)
    reports_filed: Vec<Instant>,
    /// Reports received by this player (for shadow ban detection)
    reports_received: Vec<PlayerReport>,
    /// Whether the player is shadow banned
    shadow_banned: bool,
}

/// In-memory report storage (for demonstration; use a database in production)
pub struct ReportStorage {
    /// Map of player address -> report history
    histories: Mutex<HashMap<String, PlayerReportHistory>>,
    /// Report counter for unique IDs
    report_counter: Mutex<u64>,
}

impl ReportStorage {
    pub fn new() -> Self {
        Self {
            histories: Mutex::new(HashMap::new()),
            report_counter: Mutex::new(0),
        }
    }

    /// File a report against a player
    pub fn file_report(
        &self,
        reporter: &str,
        reported: &str,
        reason: ReportReason,
        evidence_game_id: Option<String>,
    ) -> Result<PlayerReport, String> {
        let mut histories = self.histories.lock().map_err(|e| e.to_string())?;

        // Check if reporter is shadow banned
        if let Some(history) = histories.get(reporter) {
            if history.shadow_banned {
                return Err("You are not allowed to file reports".to_string());
            }
        }

        // Rate limit check: max 3 reports per hour
        let now = Instant::now();
        let reporter_history = histories
            .entry(reporter.to_string())
            .or_insert_with(PlayerReportHistory::default);

        // Remove old reports outside the rate limit window
        reporter_history
            .reports_filed
            .retain(|t| now.duration_since(*t) < RATE_LIMIT_WINDOW);

        if reporter_history.reports_filed.len() >= MAX_REPORTS_PER_HOUR as usize {
            return Err(format!(
                "Rate limit exceeded: maximum {} reports per hour",
                MAX_REPORTS_PER_HOUR
            ));
        }

        // Check for duplicate reports (same reporter, same reported, same game)
        if let Some(history) = histories.get(reported) {
            for existing in &history.reports_received {
                if existing.reporter == reporter
                    && existing.evidence_game_id == evidence_game_id
                {
                    return Err("You have already reported this player for this game".to_string());
                }
            }
        }

        // Generate report ID
        let mut counter = self.report_counter.lock().map_err(|e| e.to_string())?;
        *counter += 1;
        let report_id = *counter;

        let report = PlayerReport {
            id: report_id,
            reporter: reporter.to_string(),
            reported: reported.to_string(),
            reason,
            evidence_game_id,
            timestamp: now,
        };

        // Record the report
        reporter_history.reports_filed.push(now);

        let reported_history = histories
            .entry(reported.to_string())
            .or_insert_with(PlayerReportHistory::default);
        reported_history.reports_received.push(report.clone());

        // Check for shadow ban threshold
        let recent_reports = reported_history
            .reports_received
            .iter()
            .filter(|r| now.duration_since(r.timestamp) < SHADOW_BAN_WINDOW)
            .count() as u32;

        if recent_reports >= SHADOW_BAN_THRESHOLD && !reported_history.shadow_banned {
            reported_history.shadow_banned = true;
            // In production, emit an event here
        }

        Ok(report)
    }

    /// Check if a player is shadow banned
    pub fn is_shadow_banned(&self, player: &str) -> bool {
        let histories = self.histories.lock().unwrap_or_else(|e| e.into_inner());
        histories
            .get(player)
            .map(|h| h.shadow_banned)
            .unwrap_or(false)
    }

    /// Get reports received by a player
    pub fn get_reports_against(&self, player: &str) -> Vec<PlayerReport> {
        let histories = self.histories.lock().unwrap_or_else(|e| e.into_inner());
        histories
            .get(player)
            .map(|h| h.reports_received.clone())
            .unwrap_or_default()
    }

    /// Get admin dashboard data: all reports with filtering
    pub fn get_admin_reports(&self, limit: Option<usize>) -> Vec<PlayerReport> {
        let histories = self.histories.lock().unwrap_or_else(|e| e.into_inner());
        let mut all_reports: Vec<PlayerReport> = histories
            .values()
            .flat_map(|h| h.reports_received.clone())
            .collect();

        // Sort by timestamp (newest first)
        all_reports.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        if let Some(limit) = limit {
            all_reports.truncate(limit);
        }

        all_reports
    }
}

/// API request body for filing a report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportRequest {
    /// Reason for the report
    pub reason: ReportReason,
    /// Optional game ID as evidence
    pub evidence_game_id: Option<String>,
}

/// API response for a filed report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportResponse {
    /// Report ID
    pub id: u64,
    /// Status message
    pub message: String,
}

/// API response for the admin reports endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminReportsResponse {
    /// Total number of reports
    pub total: usize,
    /// List of reports
    pub reports: Vec<PlayerReport>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_report_success() {
        let storage = ReportStorage::new();
        let result = storage.file_report(
            "reporter1",
            "player1",
            ReportReason::Cheating,
            Some("game1".to_string()),
        );
        assert!(result.is_ok());
        let report = result.unwrap();
        assert_eq!(report.id, 1);
        assert_eq!(report.reporter, "reporter1");
        assert_eq!(report.reported, "player1");
    }

    #[test]
    fn test_rate_limit() {
        let storage = ReportStorage::new();

        // File 3 reports (at the limit)
        for i in 0..3 {
            let result = storage.file_report(
                "reporter1",
                &format!("player{}", i),
                ReportReason::Cheating,
                None,
            );
            assert!(result.is_ok());
        }

        // 4th report should fail
        let result = storage.file_report("reporter1", "player3", ReportReason::Cheating, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Rate limit exceeded"));
    }

    #[test]
    fn test_shadow_ban_detection() {
        let storage = ReportStorage::new();

        // File 10 reports against the same player
        for i in 0..10 {
            let result = storage.file_report(
                &format!("reporter{}", i),
                "toxic_player",
                ReportReason::Harassment,
                None,
            );
            assert!(result.is_ok());
        }

        // Player should be shadow banned
        assert!(storage.is_shadow_banned("toxic_player"));
    }

    #[test]
    fn test_shadow_banned_player_cannot_report() {
        let storage = ReportStorage::new();

        // Shadow ban the reporter
        for i in 0..10 {
            let _ = storage.file_report(
                &format!("reporter{}", i),
                "bad_reporter",
                ReportReason::Harassment,
                None,
            );
        }

        // Shadow banned player tries to file a report
        let result = storage.file_report("bad_reporter", "innocent", ReportReason::Cheating, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not allowed"));
    }

    #[test]
    fn test_get_admin_reports() {
        let storage = ReportStorage::new();

        let _ = storage.file_report("r1", "p1", ReportReason::Cheating, None);
        let _ = storage.file_report("r2", "p1", ReportReason::Harassment, None);
        let _ = storage.file_report("r3", "p2", ReportReason::Bot, None);

        let reports = storage.get_admin_reports(None);
        assert_eq!(reports.len(), 3);

        let reports_limited = storage.get_admin_reports(Some(2));
        assert_eq!(reports_limited.len(), 2);
    }
}
