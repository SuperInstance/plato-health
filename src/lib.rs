//! # plato-health
//!
//! Health check system for PLATO rooms — tracks uptime, response times, error rates.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ── Health status ────────────────────────────────────────────────────

/// Overall health of a room or service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Dead,
}

impl HealthStatus {
    /// Numeric severity: lower is better (0 = Healthy, 3 = Dead).
    pub fn severity(&self) -> u8 {
        match self {
            HealthStatus::Healthy => 0,
            HealthStatus::Degraded => 1,
            HealthStatus::Unhealthy => 2,
            HealthStatus::Dead => 3,
        }
    }

    /// True if the status is Healthy or Degraded (still operational).
    pub fn is_operational(&self) -> bool {
        matches!(self, HealthStatus::Healthy | HealthStatus::Degraded)
    }
}

// ── Health check ─────────────────────────────────────────────────────

/// A single health check result for a room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub id: Uuid,
    pub room_id: String,
    pub status: HealthStatus,
    pub response_time_ms: u64,
    pub error_count: u64,
    pub check_count: u64,
    pub timestamp: u64,
    pub message: String,
    pub metadata: HashMap<String, String>,
}

impl HealthCheck {
    /// Create a new health check.
    pub fn new(room_id: &str, status: HealthStatus) -> Self {
        Self {
            id: Uuid::new_v4(),
            room_id: room_id.to_string(),
            status,
            response_time_ms: 0,
            error_count: 0,
            check_count: 1,
            timestamp: now_millis(),
            message: String::new(),
            metadata: HashMap::new(),
        }
    }

    /// Builder-style: set response time.
    pub fn with_response_time(mut self, ms: u64) -> Self {
        self.response_time_ms = ms;
        self
    }

    /// Builder-style: set error count.
    pub fn with_errors(mut self, count: u64) -> Self {
        self.error_count = count;
        self
    }

    /// Builder-style: set message.
    pub fn with_message(mut self, msg: &str) -> Self {
        self.message = msg.to_string();
        self
    }

    /// Builder-style: add metadata.
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Error rate as a fraction (0.0..1.0).
    pub fn error_rate(&self) -> f64 {
        if self.check_count == 0 {
            return 0.0;
        }
        self.error_count as f64 / self.check_count as f64
    }
}

// ── Health report ────────────────────────────────────────────────────

/// Aggregated health report across multiple checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub room_id: String,
    pub checks: Vec<HealthCheck>,
    pub total_checks: u64,
    pub total_errors: u64,
    pub uptime_start: u64,
    pub uptime_end: u64,
}

impl HealthReport {
    /// Create a new report for a room.
    pub fn new(room_id: &str) -> Self {
        Self {
            room_id: room_id.to_string(),
            checks: Vec::new(),
            total_checks: 0,
            total_errors: 0,
            uptime_start: now_millis(),
            uptime_end: now_millis(),
        }
    }

    /// Add a check to the report.
    pub fn add_check(&mut self, check: HealthCheck) {
        self.total_checks += check.check_count;
        self.total_errors += check.error_count;
        if check.timestamp < self.uptime_start {
            self.uptime_start = check.timestamp;
        }
        if check.timestamp > self.uptime_end {
            self.uptime_end = check.timestamp;
        }
        self.checks.push(check);
    }

    /// Overall error rate across all checks.
    pub fn overall_error_rate(&self) -> f64 {
        if self.total_checks == 0 {
            return 0.0;
        }
        self.total_errors as f64 / self.total_checks as f64
    }

    /// Overall status derived from all checks (worst wins).
    pub fn overall_status(&self) -> HealthStatus {
        self.checks
            .iter()
            .map(|c| c.status)
            .max_by_key(|s| s.severity())
            .unwrap_or(HealthStatus::Healthy)
    }

    /// Mean response time across all checks.
    pub fn mean_response_time_ms(&self) -> f64 {
        if self.checks.is_empty() {
            return 0.0;
        }
        self.checks.iter().map(|c| c.response_time_ms as f64).sum::<f64>() / self.checks.len() as f64
    }
}

// ── Core functions ───────────────────────────────────────────────────

/// Perform a health check on a single room.
pub fn check_room(room_id: &str, errors: u64, total: u64, response_ms: u64) -> HealthCheck {
    let status = if total == 0 {
        HealthStatus::Dead
    } else {
        let err_rate = errors as f64 / total as f64;
        if err_rate == 0.0 && response_ms < 500 {
            HealthStatus::Healthy
        } else if err_rate < 0.1 && response_ms < 2000 {
            HealthStatus::Degraded
        } else if err_rate < 0.5 {
            HealthStatus::Unhealthy
        } else {
            HealthStatus::Dead
        }
    };

    HealthCheck::new(room_id, status)
        .with_response_time(response_ms)
        .with_errors(errors)
        .with_check_count(total)
}

/// Check an entire fleet of rooms, returning one check per room.
pub fn check_fleet(rooms: &[(&str, u64, u64, u64)]) -> Vec<HealthCheck> {
    rooms
        .iter()
        .map(|(id, errors, total, resp)| check_room(id, *errors, *total, *resp))
        .collect()
}

/// Compute uptime percentage from a series of health checks.
/// A check counts as "up" if it's Healthy or Degraded.
pub fn uptime_percentage(checks: &[HealthCheck]) -> f64 {
    if checks.is_empty() {
        return 100.0;
    }
    let up = checks.iter().filter(|c| c.status.is_operational()).count();
    (up as f64 / checks.len() as f64) * 100.0
}

/// Compute mean time between failures (in seconds) from a sorted list of checks.
/// A "failure" is any Unhealthy or Dead check.
pub fn mean_time_between_failures(checks: &[HealthCheck]) -> f64 {
    let failures: Vec<&HealthCheck> = checks
        .iter()
        .filter(|c| !c.status.is_operational())
        .collect();

    if failures.len() < 2 {
        return 0.0;
    }

    let mut gaps: Vec<f64> = Vec::new();
    for w in failures.windows(2) {
        let gap_ms = w[1].timestamp.saturating_sub(w[0].timestamp);
        gaps.push(gap_ms as f64 / 1000.0);
    }

    gaps.iter().sum::<f64>() / gaps.len() as f64
}

// ── Helpers ──────────────────────────────────────────────────────────

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// Extend HealthCheck builder with check_count
impl HealthCheck {
    /// Builder-style: set check count.
    pub fn with_check_count(mut self, count: u64) -> Self {
        self.check_count = count;
        self
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_status_severity_ordering() {
        assert!(HealthStatus::Healthy.severity() < HealthStatus::Degraded.severity());
        assert!(HealthStatus::Degraded.severity() < HealthStatus::Unhealthy.severity());
        assert!(HealthStatus::Unhealthy.severity() < HealthStatus::Dead.severity());
    }

    #[test]
    fn health_status_is_operational() {
        assert!(HealthStatus::Healthy.is_operational());
        assert!(HealthStatus::Degraded.is_operational());
        assert!(!HealthStatus::Unhealthy.is_operational());
        assert!(!HealthStatus::Dead.is_operational());
    }

    #[test]
    fn health_check_new() {
        let hc = HealthCheck::new("room-1", HealthStatus::Healthy);
        assert_eq!(hc.room_id, "room-1");
        assert_eq!(hc.status, HealthStatus::Healthy);
        assert_eq!(hc.check_count, 1);
        assert!(!hc.id.is_nil());
    }

    #[test]
    fn health_check_builder() {
        let hc = HealthCheck::new("room-2", HealthStatus::Degraded)
            .with_response_time(120)
            .with_errors(3)
            .with_message("slow response")
            .with_metadata("region", "us-east");

        assert_eq!(hc.response_time_ms, 120);
        assert_eq!(hc.error_count, 3);
        assert_eq!(hc.message, "slow response");
        assert_eq!(hc.metadata.get("region").unwrap(), "us-east");
    }

    #[test]
    fn error_rate_calculation() {
        let hc = HealthCheck::new("r", HealthStatus::Healthy)
            .with_errors(2)
            .with_check_count(10);
        assert!((hc.error_rate() - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn error_rate_zero_checks() {
        let hc = HealthCheck::new("r", HealthStatus::Dead).with_check_count(0);
        assert_eq!(hc.error_rate(), 0.0);
    }

    #[test]
    fn check_room_healthy() {
        let hc = check_room("room-a", 0, 100, 50);
        assert_eq!(hc.status, HealthStatus::Healthy);
        assert_eq!(hc.response_time_ms, 50);
    }

    #[test]
    fn check_room_degraded() {
        let hc = check_room("room-b", 5, 100, 800);
        assert_eq!(hc.status, HealthStatus::Degraded);
    }

    #[test]
    fn check_room_unhealthy() {
        let hc = check_room("room-c", 20, 100, 3000);
        assert_eq!(hc.status, HealthStatus::Unhealthy);
    }

    #[test]
    fn check_room_dead() {
        let hc = check_room("room-d", 60, 100, 5000);
        assert_eq!(hc.status, HealthStatus::Dead);
    }

    #[test]
    fn check_room_no_data() {
        let hc = check_room("room-e", 0, 0, 0);
        assert_eq!(hc.status, HealthStatus::Dead);
    }

    #[test]
    fn check_fleet_multiple() {
        let rooms = vec![
            ("room-1", 0u64, 100u64, 50u64),
            ("room-2", 5u64, 100u64, 800u64),
            ("room-3", 60u64, 100u64, 5000u64),
        ];
        let results = check_fleet(&rooms);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].status, HealthStatus::Healthy);
        assert_eq!(results[1].status, HealthStatus::Degraded);
        assert_eq!(results[2].status, HealthStatus::Dead);
    }

    #[test]
    fn uptime_percentage_all_healthy() {
        let checks = vec![
            HealthCheck::new("r", HealthStatus::Healthy),
            HealthCheck::new("r", HealthStatus::Healthy),
            HealthCheck::new("r", HealthStatus::Healthy),
        ];
        assert!((uptime_percentage(&checks) - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn uptime_percentage_mixed() {
        let checks = vec![
            HealthCheck::new("r", HealthStatus::Healthy),
            HealthCheck::new("r", HealthStatus::Degraded),
            HealthCheck::new("r", HealthStatus::Unhealthy),
            HealthCheck::new("r", HealthStatus::Dead),
        ];
        assert!((uptime_percentage(&checks) - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn uptime_percentage_empty() {
        let checks: Vec<HealthCheck> = vec![];
        assert!((uptime_percentage(&checks) - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn mean_time_between_failures_basic() {
        let base = 1_700_000_000_000u64;
        let checks = vec![
            HealthCheck::new("r", HealthStatus::Healthy).with_check_count(1),
            {
                let mut c = HealthCheck::new("r", HealthStatus::Dead);
                c.timestamp = base + 10_000;
                c
            },
            {
                let mut c = HealthCheck::new("r", HealthStatus::Healthy);
                c.timestamp = base + 20_000;
                c
            },
            {
                let mut c = HealthCheck::new("r", HealthStatus::Dead);
                c.timestamp = base + 40_000;
                c
            },
        ];
        // Two failures at t=base+10s and t=base+40s → gap = 30s
        let mtbf = mean_time_between_failures(&checks);
        assert!((mtbf - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn mean_time_between_failures_none() {
        let checks = vec![HealthCheck::new("r", HealthStatus::Healthy)];
        assert_eq!(mean_time_between_failures(&checks), 0.0);
    }

    #[test]
    fn health_report_overall_status() {
        let mut report = HealthReport::new("room-x");
        report.add_check(HealthCheck::new("room-x", HealthStatus::Healthy));
        report.add_check(HealthCheck::new("room-x", HealthStatus::Degraded));
        assert_eq!(report.overall_status(), HealthStatus::Degraded);
    }

    #[test]
    fn health_report_mean_response_time() {
        let mut report = HealthReport::new("room-y");
        report.add_check(HealthCheck::new("room-y", HealthStatus::Healthy).with_response_time(100));
        report.add_check(HealthCheck::new("room-y", HealthStatus::Healthy).with_response_time(200));
        assert!((report.mean_response_time_ms() - 150.0).abs() < f64::EPSILON);
    }

    #[test]
    fn health_report_error_rate() {
        let mut report = HealthReport::new("room-z");
        report.add_check(HealthCheck::new("room-z", HealthStatus::Healthy).with_errors(1).with_check_count(10));
        report.add_check(HealthCheck::new("room-z", HealthStatus::Healthy).with_errors(3).with_check_count(10));
        assert!((report.overall_error_rate() - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn health_check_serializes() {
        let hc = HealthCheck::new("room-s", HealthStatus::Healthy)
            .with_response_time(42)
            .with_errors(0);
        let json = serde_json::to_string(&hc).unwrap();
        assert!(json.contains("room-s"));
        assert!(json.contains("Healthy"));
    }

    #[test]
    fn health_report_empty_status() {
        let report = HealthReport::new("empty");
        assert_eq!(report.overall_status(), HealthStatus::Healthy);
    }
}
