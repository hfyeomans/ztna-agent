//! Lightweight Prometheus-compatible metrics for the App Connector.
//!
//! Uses atomic counters for lock-free instrumentation. Renders metrics in
//! Prometheus text exposition format for scraping on the metrics HTTP endpoint.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Lightweight Prometheus-compatible metrics for the App Connector.
pub struct Metrics {
    /// Total packets forwarded to backend services (counter)
    pub forwarded_packets_total: AtomicU64,
    /// Total bytes forwarded to backend services (counter)
    pub forwarded_bytes_total: AtomicU64,
    /// Total TCP proxy sessions created (counter)
    pub tcp_sessions_total: AtomicU64,
    /// Total TCP errors — connect failures, read/write errors (counter)
    pub tcp_errors_total: AtomicU64,
    /// Total reconnections to Intermediate Server (counter)
    pub reconnections_total: AtomicU64,
    /// Server start time (for uptime calculation)
    pub start_time: Instant,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            forwarded_packets_total: AtomicU64::new(0),
            forwarded_bytes_total: AtomicU64::new(0),
            tcp_sessions_total: AtomicU64::new(0),
            tcp_errors_total: AtomicU64::new(0),
            reconnections_total: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }

    /// Render metrics in Prometheus text exposition format.
    pub fn render(&self) -> String {
        let uptime = self.start_time.elapsed().as_secs();
        format!(
            "# HELP ztna_connector_forwarded_packets_total Total packets forwarded to backend\n\
             # TYPE ztna_connector_forwarded_packets_total counter\n\
             ztna_connector_forwarded_packets_total {}\n\
             # HELP ztna_connector_forwarded_bytes_total Total bytes forwarded to backend\n\
             # TYPE ztna_connector_forwarded_bytes_total counter\n\
             ztna_connector_forwarded_bytes_total {}\n\
             # HELP ztna_connector_tcp_sessions_total Total TCP proxy sessions created\n\
             # TYPE ztna_connector_tcp_sessions_total counter\n\
             ztna_connector_tcp_sessions_total {}\n\
             # HELP ztna_connector_tcp_errors_total Total TCP errors\n\
             # TYPE ztna_connector_tcp_errors_total counter\n\
             ztna_connector_tcp_errors_total {}\n\
             # HELP ztna_connector_reconnections_total Total reconnections to Intermediate Server\n\
             # TYPE ztna_connector_reconnections_total counter\n\
             ztna_connector_reconnections_total {}\n\
             # HELP ztna_connector_uptime_seconds Connector uptime in seconds\n\
             # TYPE ztna_connector_uptime_seconds gauge\n\
             ztna_connector_uptime_seconds {}\n",
            self.forwarded_packets_total.load(Ordering::Relaxed),
            self.forwarded_bytes_total.load(Ordering::Relaxed),
            self.tcp_sessions_total.load(Ordering::Relaxed),
            self.tcp_errors_total.load(Ordering::Relaxed),
            self.reconnections_total.load(Ordering::Relaxed),
            uptime,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_render_format() {
        let m = Metrics::new();
        m.forwarded_packets_total.fetch_add(42, Ordering::Relaxed);
        m.forwarded_bytes_total.fetch_add(8192, Ordering::Relaxed);
        m.tcp_sessions_total.fetch_add(3, Ordering::Relaxed);
        let output = m.render();
        assert!(output.contains("ztna_connector_forwarded_packets_total 42"));
        assert!(output.contains("ztna_connector_forwarded_bytes_total 8192"));
        assert!(output.contains("ztna_connector_tcp_sessions_total 3"));
        assert!(output.contains("ztna_connector_tcp_errors_total 0"));
        assert!(output.contains("ztna_connector_reconnections_total 0"));
    }

    #[test]
    fn test_metrics_default_zero() {
        let m = Metrics::new();
        assert_eq!(m.forwarded_packets_total.load(Ordering::Relaxed), 0);
        assert_eq!(m.forwarded_bytes_total.load(Ordering::Relaxed), 0);
        assert_eq!(m.tcp_sessions_total.load(Ordering::Relaxed), 0);
        assert_eq!(m.tcp_errors_total.load(Ordering::Relaxed), 0);
        assert_eq!(m.reconnections_total.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_metrics_counter_increments() {
        let m = Metrics::new();
        m.tcp_errors_total.fetch_add(5, Ordering::Relaxed);
        m.reconnections_total.fetch_add(2, Ordering::Relaxed);
        let output = m.render();
        assert!(output.contains("ztna_connector_tcp_errors_total 5"));
        assert!(output.contains("ztna_connector_reconnections_total 2"));
    }

    #[test]
    fn test_metrics_uptime_present() {
        let m = Metrics::new();
        let output = m.render();
        assert!(output
            .lines()
            .any(|l| l.starts_with("ztna_connector_uptime_seconds ")));
    }

    #[test]
    fn test_metrics_render_prometheus_format() {
        let m = Metrics::new();
        let output = m.render();
        // Verify all HELP/TYPE lines are present
        assert!(output.contains("# HELP ztna_connector_forwarded_packets_total"));
        assert!(output.contains("# TYPE ztna_connector_forwarded_packets_total counter"));
        assert!(output.contains("# HELP ztna_connector_forwarded_bytes_total"));
        assert!(output.contains("# TYPE ztna_connector_forwarded_bytes_total counter"));
        assert!(output.contains("# HELP ztna_connector_tcp_sessions_total"));
        assert!(output.contains("# TYPE ztna_connector_tcp_sessions_total counter"));
        assert!(output.contains("# HELP ztna_connector_tcp_errors_total"));
        assert!(output.contains("# TYPE ztna_connector_tcp_errors_total counter"));
        assert!(output.contains("# HELP ztna_connector_reconnections_total"));
        assert!(output.contains("# TYPE ztna_connector_reconnections_total counter"));
        assert!(output.contains("# HELP ztna_connector_uptime_seconds"));
        assert!(output.contains("# TYPE ztna_connector_uptime_seconds gauge"));
    }
}
