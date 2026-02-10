//! Clock and timing utilities for stream synchronization.
//!
//! All GrabMe streams are anchored to a monotonic clock epoch recorded
//! at capture start. This module provides utilities for:
//! - Capturing the epoch
//! - Converting between monotonic and wall-clock time
//! - Calculating stream drift

use std::time::Instant;

/// A recording clock that provides monotonic timestamps relative to
/// a fixed epoch (the moment recording started).
#[derive(Debug, Clone)]
pub struct RecordingClock {
    /// The instant recording started.
    epoch: Instant,

    /// Wall-clock time at epoch (ISO 8601 string).
    epoch_wall: String,
}

impl RecordingClock {
    /// Create a new recording clock anchored to now.
    pub fn start() -> Self {
        Self {
            epoch: Instant::now(),
            epoch_wall: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Create a clock from a known epoch (for loading saved projects).
    pub fn from_epoch(epoch: Instant, wall: String) -> Self {
        Self {
            epoch,
            epoch_wall: wall,
        }
    }

    /// Get nanoseconds elapsed since recording start.
    pub fn elapsed_ns(&self) -> u64 {
        self.epoch.elapsed().as_nanos() as u64
    }

    /// Get seconds elapsed since recording start.
    pub fn elapsed_secs(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }

    /// Wall-clock time at recording start.
    pub fn epoch_wall(&self) -> &str {
        &self.epoch_wall
    }

    /// The underlying epoch instant.
    pub fn epoch(&self) -> Instant {
        self.epoch
    }

    /// Convert an elapsed nanosecond value to seconds.
    pub fn ns_to_secs(ns: u64) -> f64 {
        ns as f64 / 1_000_000_000.0
    }

    /// Convert seconds to nanoseconds.
    pub fn secs_to_ns(secs: f64) -> u64 {
        (secs * 1_000_000_000.0) as u64
    }
}

/// Drift measurement between two streams.
#[derive(Debug, Clone, Copy)]
pub struct DriftMeasurement {
    /// Timestamp in the reference stream (ns).
    pub reference_ns: u64,
    /// Timestamp in the measured stream (ns).
    pub measured_ns: u64,
}

impl DriftMeasurement {
    /// Drift in nanoseconds (positive = measured is ahead).
    pub fn drift_ns(&self) -> i64 {
        self.measured_ns as i64 - self.reference_ns as i64
    }

    /// Drift in milliseconds.
    pub fn drift_ms(&self) -> f64 {
        self.drift_ns() as f64 / 1_000_000.0
    }

    /// Whether drift exceeds an acceptable threshold.
    pub fn exceeds_threshold_ms(&self, threshold_ms: f64) -> bool {
        self.drift_ms().abs() > threshold_ms
    }
}

/// Frame rate controller for event sampling.
#[derive(Debug)]
pub struct RateController {
    target_interval_ns: u64,
    last_tick_ns: Option<u64>,
}

impl RateController {
    /// Create a controller targeting the given Hz rate.
    pub fn new(target_hz: u32) -> Self {
        Self {
            target_interval_ns: 1_000_000_000 / target_hz as u64,
            last_tick_ns: None,
        }
    }

    /// Check if enough time has passed for the next tick.
    /// Returns true and updates internal state if ready.
    /// The first call always returns true.
    pub fn should_tick(&mut self, current_ns: u64) -> bool {
        match self.last_tick_ns {
            None => {
                self.last_tick_ns = Some(current_ns);
                true
            }
            Some(last) if current_ns >= last + self.target_interval_ns => {
                self.last_tick_ns = Some(current_ns);
                true
            }
            _ => false,
        }
    }

    /// Target interval in nanoseconds.
    pub fn interval_ns(&self) -> u64 {
        self.target_interval_ns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_elapsed() {
        let clock = RecordingClock::start();
        // Should be very small but non-negative
        assert!(clock.elapsed_ns() < 1_000_000_000); // less than 1 second
    }

    #[test]
    fn test_ns_to_secs_conversion() {
        assert!((RecordingClock::ns_to_secs(1_500_000_000) - 1.5).abs() < 1e-9);
        assert_eq!(RecordingClock::secs_to_ns(2.0), 2_000_000_000);
    }

    #[test]
    fn test_drift_measurement() {
        let drift = DriftMeasurement {
            reference_ns: 1_000_000_000,
            measured_ns: 1_050_000_000,
        };
        assert_eq!(drift.drift_ns(), 50_000_000);
        assert!((drift.drift_ms() - 50.0).abs() < 1e-9);
        assert!(drift.exceeds_threshold_ms(10.0));
        assert!(!drift.exceeds_threshold_ms(100.0));
    }

    #[test]
    fn test_rate_controller() {
        let mut ctrl = RateController::new(60);
        assert!(ctrl.should_tick(0)); // first tick always fires
        assert!(!ctrl.should_tick(1_000_000)); // 1ms later, too soon
        assert!(ctrl.should_tick(17_000_000)); // ~17ms later, should fire (60Hz ~ 16.67ms)
    }
}
