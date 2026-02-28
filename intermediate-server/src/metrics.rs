//! Lightweight Prometheus-compatible metrics for the Intermediate Server.
//!
//! Uses atomic counters for lock-free instrumentation. Renders metrics in
//! Prometheus text exposition format for scraping on the metrics HTTP endpoint.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Lightweight Prometheus-compatible metrics for the Intermediate Server.
pub struct Metrics {
    /// Total active QUIC connections (gauge)
    pub active_connections: AtomicU64,
    /// Total bytes relayed via DATAGRAMs (counter)
    pub relay_bytes_total: AtomicU64,
    /// Total successful registrations — agents + connectors (counter)
    pub registrations_total: AtomicU64,
    /// Total registration rejections — NACK (counter)
    pub registration_rejections_total: AtomicU64,
    /// Total DATAGRAMs relayed (counter)
    pub datagrams_relayed_total: AtomicU64,
    /// Total P2P signaling sessions created (counter)
    pub signaling_sessions_total: AtomicU64,
    /// Total retry tokens validated successfully (counter)
    pub retry_tokens_validated: AtomicU64,
    /// Total retry token validation failures (counter)
    pub retry_token_failures: AtomicU64,
    /// Server start time (for uptime calculation)
    pub start_time: Instant,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            active_connections: AtomicU64::new(0),
            relay_bytes_total: AtomicU64::new(0),
            registrations_total: AtomicU64::new(0),
            registration_rejections_total: AtomicU64::new(0),
            datagrams_relayed_total: AtomicU64::new(0),
            signaling_sessions_total: AtomicU64::new(0),
            retry_tokens_validated: AtomicU64::new(0),
            retry_token_failures: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }

    /// Render metrics in Prometheus text exposition format.
    pub fn render(&self) -> String {
        let uptime = self.start_time.elapsed().as_secs();
        format!(
            "# HELP ztna_active_connections Current number of active QUIC connections\n\
             # TYPE ztna_active_connections gauge\n\
             ztna_active_connections {}\n\
             # HELP ztna_relay_bytes_total Total bytes relayed via DATAGRAMs\n\
             # TYPE ztna_relay_bytes_total counter\n\
             ztna_relay_bytes_total {}\n\
             # HELP ztna_registrations_total Total successful service registrations\n\
             # TYPE ztna_registrations_total counter\n\
             ztna_registrations_total {}\n\
             # HELP ztna_registration_rejections_total Total registration rejections (NACK)\n\
             # TYPE ztna_registration_rejections_total counter\n\
             ztna_registration_rejections_total {}\n\
             # HELP ztna_datagrams_relayed_total Total DATAGRAMs relayed\n\
             # TYPE ztna_datagrams_relayed_total counter\n\
             ztna_datagrams_relayed_total {}\n\
             # HELP ztna_signaling_sessions_total Total P2P signaling sessions created\n\
             # TYPE ztna_signaling_sessions_total counter\n\
             ztna_signaling_sessions_total {}\n\
             # HELP ztna_retry_tokens_validated Total retry tokens successfully validated\n\
             # TYPE ztna_retry_tokens_validated counter\n\
             ztna_retry_tokens_validated {}\n\
             # HELP ztna_retry_token_failures Total retry token validation failures\n\
             # TYPE ztna_retry_token_failures counter\n\
             ztna_retry_token_failures {}\n\
             # HELP ztna_uptime_seconds Server uptime in seconds\n\
             # TYPE ztna_uptime_seconds gauge\n\
             ztna_uptime_seconds {}\n",
            self.active_connections.load(Ordering::Relaxed),
            self.relay_bytes_total.load(Ordering::Relaxed),
            self.registrations_total.load(Ordering::Relaxed),
            self.registration_rejections_total.load(Ordering::Relaxed),
            self.datagrams_relayed_total.load(Ordering::Relaxed),
            self.signaling_sessions_total.load(Ordering::Relaxed),
            self.retry_tokens_validated.load(Ordering::Relaxed),
            self.retry_token_failures.load(Ordering::Relaxed),
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
        m.registrations_total.fetch_add(5, Ordering::Relaxed);
        m.relay_bytes_total.fetch_add(1024, Ordering::Relaxed);
        let output = m.render();
        assert!(output.contains("ztna_registrations_total 5"));
        assert!(output.contains("ztna_relay_bytes_total 1024"));
        assert!(output.contains("ztna_active_connections 0"));
        assert!(output.contains("# TYPE ztna_uptime_seconds gauge"));
    }

    #[test]
    fn test_metrics_default_zero() {
        let m = Metrics::new();
        assert_eq!(m.active_connections.load(Ordering::Relaxed), 0);
        assert_eq!(m.relay_bytes_total.load(Ordering::Relaxed), 0);
        assert_eq!(m.registrations_total.load(Ordering::Relaxed), 0);
        assert_eq!(m.registration_rejections_total.load(Ordering::Relaxed), 0);
        assert_eq!(m.datagrams_relayed_total.load(Ordering::Relaxed), 0);
        assert_eq!(m.signaling_sessions_total.load(Ordering::Relaxed), 0);
        assert_eq!(m.retry_tokens_validated.load(Ordering::Relaxed), 0);
        assert_eq!(m.retry_token_failures.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_metrics_counter_increments() {
        let m = Metrics::new();
        m.active_connections.fetch_add(3, Ordering::Relaxed);
        m.datagrams_relayed_total.fetch_add(100, Ordering::Relaxed);
        m.retry_tokens_validated.fetch_add(10, Ordering::Relaxed);
        m.retry_token_failures.fetch_add(2, Ordering::Relaxed);
        let output = m.render();
        assert!(output.contains("ztna_active_connections 3"));
        assert!(output.contains("ztna_datagrams_relayed_total 100"));
        assert!(output.contains("ztna_retry_tokens_validated 10"));
        assert!(output.contains("ztna_retry_token_failures 2"));
    }

    #[test]
    fn test_metrics_uptime_present() {
        let m = Metrics::new();
        let output = m.render();
        // Uptime should be 0 or very small (just created)
        assert!(output.contains("ztna_uptime_seconds 0"));
    }

    #[test]
    fn test_metrics_render_prometheus_format() {
        let m = Metrics::new();
        let output = m.render();
        // Verify all HELP/TYPE lines are present
        assert!(output.contains("# HELP ztna_active_connections"));
        assert!(output.contains("# TYPE ztna_active_connections gauge"));
        assert!(output.contains("# HELP ztna_relay_bytes_total"));
        assert!(output.contains("# TYPE ztna_relay_bytes_total counter"));
        assert!(output.contains("# HELP ztna_registrations_total"));
        assert!(output.contains("# TYPE ztna_registrations_total counter"));
        assert!(output.contains("# HELP ztna_registration_rejections_total"));
        assert!(output.contains("# TYPE ztna_registration_rejections_total counter"));
        assert!(output.contains("# HELP ztna_datagrams_relayed_total"));
        assert!(output.contains("# TYPE ztna_datagrams_relayed_total counter"));
        assert!(output.contains("# HELP ztna_signaling_sessions_total"));
        assert!(output.contains("# TYPE ztna_signaling_sessions_total counter"));
        assert!(output.contains("# HELP ztna_retry_tokens_validated"));
        assert!(output.contains("# TYPE ztna_retry_tokens_validated counter"));
        assert!(output.contains("# HELP ztna_retry_token_failures"));
        assert!(output.contains("# TYPE ztna_retry_token_failures counter"));
        assert!(output.contains("# HELP ztna_uptime_seconds"));
        assert!(output.contains("# TYPE ztna_uptime_seconds gauge"));
    }
}
