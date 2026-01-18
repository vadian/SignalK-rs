//! Server statistics collection.
//!
//! This module collects and tracks server performance metrics:
//! - Delta throughput (deltas per second)
//! - Active path count
//! - WebSocket client count
//! - Per-provider statistics
//! - Server uptime
//!
//! Statistics are collected continuously and broadcast to Admin UI
//! clients via the server events WebSocket.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use crate::server_events::{ProviderStatistics, ServerStatistics};

/// Collects and tracks server statistics.
pub struct StatisticsCollector {
    /// Server start time.
    start_time: Instant,

    /// Total deltas processed.
    total_deltas: AtomicU64,

    /// Deltas in current measurement window.
    window_deltas: AtomicU64,

    /// Last calculated delta rate.
    delta_rate: AtomicU64, // Stored as f64 bits

    /// Number of active paths.
    active_paths: AtomicUsize,

    /// Connected WebSocket clients.
    ws_clients: AtomicUsize,
}

impl StatisticsCollector {
    /// Create a new statistics collector.
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            total_deltas: AtomicU64::new(0),
            window_deltas: AtomicU64::new(0),
            delta_rate: AtomicU64::new(0),
            active_paths: AtomicUsize::new(0),
            ws_clients: AtomicUsize::new(0),
        }
    }

    /// Record a delta being processed.
    pub fn record_delta(&self) {
        self.total_deltas.fetch_add(1, Ordering::Relaxed);
        self.window_deltas.fetch_add(1, Ordering::Relaxed);
    }

    /// Update the delta rate calculation (call once per second).
    pub fn update_rate(&self) {
        let window = self.window_deltas.swap(0, Ordering::Relaxed);
        self.delta_rate
            .store((window as f64).to_bits(), Ordering::Relaxed);
    }

    /// Set the number of active paths.
    pub fn set_active_paths(&self, count: usize) {
        self.active_paths.store(count, Ordering::Relaxed);
    }

    /// Increment WebSocket client count.
    pub fn client_connected(&self) {
        self.ws_clients.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement WebSocket client count.
    pub fn client_disconnected(&self) {
        self.ws_clients.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get current statistics snapshot.
    pub fn snapshot(&self) -> ServerStatistics {
        ServerStatistics {
            delta_rate: f64::from_bits(self.delta_rate.load(Ordering::Relaxed)),
            number_of_available_paths: self.active_paths.load(Ordering::Relaxed),
            ws_clients: self.ws_clients.load(Ordering::Relaxed),
            uptime: self.start_time.elapsed().as_secs(),
            provider_statistics: Vec::new(), // TODO: Collect per-provider stats
        }
    }
}

impl Default for StatisticsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statistics_collection() {
        let stats = StatisticsCollector::new();

        // Record some deltas
        stats.record_delta();
        stats.record_delta();
        stats.record_delta();

        // Update rate
        stats.update_rate();

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.delta_rate, 3.0);
        assert_eq!(snapshot.ws_clients, 0);
    }

    #[test]
    fn test_client_tracking() {
        let stats = StatisticsCollector::new();

        stats.client_connected();
        stats.client_connected();
        assert_eq!(stats.snapshot().ws_clients, 2);

        stats.client_disconnected();
        assert_eq!(stats.snapshot().ws_clients, 1);
    }
}
