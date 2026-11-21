use serde::{Deserialize, Deserializer, Serialize, Serializer};
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

/// Lifecycle status of a report in the moderation queue
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReportStatus {
    Open,
    Assigned,
    Resolved,
}

/// Represents a player report
#[derive(Debug, Clone)]
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
    /// Current queue status
    pub status: ReportStatus,
    /// Moderator currently assigned to the report
    pub assigned_to: Option<String>,
    /// Linked anti-cheat result identifier (BE-172)
    pub anticheat_result_id: Option<String>,
    /// Resolution note recorded by the moderator
    pub resolution: Option<String>,
}

impl Serialize for PlayerReport {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("PlayerReport", 10)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("reporter", &self.reporter)?;
        state.serialize_field("reported", &self.reported)?;
        state.serialize_field("reason", &self.reason)?;
        state.serialize_field("evidence_game_id", &self.evidence_game_id)?;
        state.serialize_field("timestamp_nanos", &self.timestamp.elapsed().as_nanos())?;
        state.serialize_field("status", &self.status)?;
        state.serialize_field("assigned_to", &self.assigned_to)?;
        state.serialize_field("anticheat_result_id", &self.anticheat_result_id)?;
        state.serialize_field("resolution", &self.resolution)?;
        state.end()
    }
}

/// An entry in the admin audit log
#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    /// Sequential audit entry ID
    pub id: u64,
    /// Moderator/admin who performed the action
    pub actor: String,
    /// Action performed (e.g. "assign", "resolve")
    pub action: String,
    /// Report the action applied to
    pub report_id: u64,
    /// Free-form details
    pub details: String,
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
    /// Admin audit log
    audit_log: Mutex<Vec<AuditEntry>>,
}

impl ReportStorage {
    pub fn new() -> Self {
        Self {
            histories: Mutex::new(HashMap::new()),
            report_counter: Mutex::new(0),
            audit_log: Mutex::new(Vec::new()),
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
        {
            if let Some(history) = histories.get(reporter) {
                if history.shadow_banned {
                    return Err("You are not allowed to file reports".to_string());
                }
            }
        }

        // Rate limit check: max 3 reports per hour
        let now = Instant::now();

        // Duplicate checks must happen before the mutable report-history entry is created
        // so we do not hold overlapping mutable and immutable borrows on the same map.
        {
            if let Some(history) = histories.get(reported) {
                for existing in &history.reports_received {
                    if existing.reporter == reporter
                        && existing.evidence_game_id == evidence_game_id
                    {
                        return Err(
                            "You have already reported this player for this game".to_string()
                        );
                    }
                }
            }
        }

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
            status: ReportStatus::Open,
            assigned_to: None,
            anticheat_result_id: None,
            resolution: None,
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

    /// List reports in the moderation queue, optionally filtered by status
    pub fn list_queue(&self, status: Option<ReportStatus>) -> Vec<PlayerReport> {
        let mut reports = self.get_admin_reports(None);
        if let Some(status) = status {
            reports.retain(|r| r.status == status);
        }
        // Oldest first for queue processing
        reports.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        reports
    }

    /// Find a report by ID across all histories
    fn find_report_mut<'a>(
        histories: &'a mut HashMap<String, PlayerReportHistory>,
        report_id: u64,
    ) -> Option<&'a mut PlayerReport> {
        histories
            .values_mut()
            .flat_map(|h| h.reports_received.iter_mut())
            .find(|r| r.id == report_id)
    }

    /// Assign a report to a moderator
    pub fn assign_report(
        &self,
        actor: &str,
        report_id: u64,
        moderator: &str,
    ) -> Result<PlayerReport, String> {
        let mut histories = self.histories.lock().map_err(|e| e.to_string())?;
        let report = Self::find_report_mut(&mut histories, report_id)
            .ok_or_else(|| format!("Report {} not found", report_id))?;
        if report.status == ReportStatus::Resolved {
            return Err(format!("Report {} is already resolved", report_id));
        }
        report.status = ReportStatus::Assigned;
        report.assigned_to = Some(moderator.to_string());
        let snapshot = report.clone();
        drop(histories);
        self.record_audit(actor, "assign", report_id, &format!("assigned to {}", moderator))?;
        Ok(snapshot)
    }

    /// Link a report to an anti-cheat result (BE-172)
    pub fn link_anticheat_result(
        &self,
        actor: &str,
        report_id: u64,
        anticheat_result_id: &str,
    ) -> Result<PlayerReport, String> {
        let mut histories = self.histories.lock().map_err(|e| e.to_string())?;
        let report = Self::find_report_mut(&mut histories, report_id)
            .ok_or_else(|| format!("Report {} not found", report_id))?;
        report.anticheat_result_id = Some(anticheat_result_id.to_string());
        let snapshot = report.clone();
        drop(histories);
        self.record_audit(
            actor,
            "link_anticheat",
            report_id,
            &format!("anticheat result {}", anticheat_result_id),
        )?;
        Ok(snapshot)
    }

    /// Resolve a report with a note
    pub fn resolve_report(
        &self,
        actor: &str,
        report_id: u64,
        resolution: &str,
    ) -> Result<PlayerReport, String> {
        let mut histories = self.histories.lock().map_err(|e| e.to_string())?;
        let report = Self::find_report_mut(&mut histories, report_id)
            .ok_or_else(|| format!("Report {} not found", report_id))?;
        if report.status == ReportStatus::Resolved {
            return Err(format!("Report {} is already resolved", report_id));
        }
        report.status = ReportStatus::Resolved;
        report.resolution = Some(resolution.to_string());
        let snapshot = report.clone();
        drop(histories);
        self.record_audit(actor, "resolve", report_id, resolution)?;
        Ok(snapshot)
    }

    /// Append an entry to the admin audit log
    fn record_audit(
        &self,
        actor: &str,
        action: &str,
        report_id: u64,
        details: &str,
    ) -> Result<(), String> {
        let mut log = self.audit_log.lock().map_err(|e| e.to_string())?;
        let id = log.len() as u64 + 1;
        log.push(AuditEntry {
            id,
            actor: actor.to_string(),
            action: action.to_string(),
            report_id,
            details: details.to_string(),
        });
        Ok(())
    }

    /// Read the admin audit log
    pub fn audit_entries(&self) -> Vec<AuditEntry> {
        self.audit_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
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
#[derive(Debug, Clone)]
pub struct AdminReportsResponse {
    /// Total number of reports
    pub total: usize,
    /// List of reports
    pub reports: Vec<PlayerReport>,
}

impl Serialize for AdminReportsResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("AdminReportsResponse", 2)?;
        state.serialize_field("total", &self.total)?;
        state.serialize_field("reports", &self.reports)?;
        state.end()
    }
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
        assert_eq!(report.status, ReportStatus::Open);
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

    #[test]
    fn test_queue_assign_resolve_and_audit() {
        let storage = ReportStorage::new();
        let report = storage
            .file_report("r1", "p1", ReportReason::Cheating, Some("g1".to_string()))
            .unwrap();

        let open = storage.list_queue(Some(ReportStatus::Open));
        assert_eq!(open.len(), 1);

        let assigned = storage.assign_report("admin1", report.id, "mod1").unwrap();
        assert_eq!(assigned.status, ReportStatus::Assigned);
        assert_eq!(assigned.assigned_to.as_deref(), Some("mod1"));

        let linked = storage
            .link_anticheat_result("mod1", report.id, "ac-42")
            .unwrap();
        assert_eq!(linked.anticheat_result_id.as_deref(), Some("ac-42"));

        let resolved = storage
            .resolve_report("mod1", report.id, "confirmed cheating")
            .unwrap();
        assert_eq!(resolved.status, ReportStatus::Resolved);
        assert!(storage.resolve_report("mod1", report.id, "again").is_err());

        let log = storage.audit_entries();
        assert_eq!(log.len(), 3);
        assert_eq!(log[0].action, "assign");
        assert_eq!(log[2].action, "resolve");
    }

    #[test]
    fn test_assign_unknown_report_fails() {
        let storage = ReportStorage::new();
        assert!(storage.assign_report("admin", 99, "mod").is_err());
    }
}
