//! Lock-free network metrics, atomic bandwidth counters, and latency distribution histograms.

use std::sync::atomic::{AtomicU64, Ordering};

/// Lock-free atomic traffic counters for real-time dashboard updates
pub struct TrafficCounters {
    tx_bytes: AtomicU64,
    rx_bytes: AtomicU64,
    tx_packets: AtomicU64,
    rx_packets: AtomicU64,
}

impl Default for TrafficCounters {
    fn default() -> Self {
        Self::new()
    }
}

impl TrafficCounters {
    pub const fn new() -> Self {
        Self {
            tx_bytes: AtomicU64::new(0),
            rx_bytes: AtomicU64::new(0),
            tx_packets: AtomicU64::new(0),
            rx_packets: AtomicU64::new(0),
        }
    }

    #[inline]
    pub fn add_tx(&self, bytes: u64) {
        self.tx_bytes.fetch_add(bytes, Ordering::Relaxed);
        self.tx_packets.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn add_rx(&self, bytes: u64) {
        self.rx_bytes.fetch_add(bytes, Ordering::Relaxed);
        self.rx_packets.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn tx_bytes(&self) -> u64 {
        self.tx_bytes.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn rx_bytes(&self) -> u64 {
        self.rx_bytes.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn tx_packets(&self) -> u64 {
        self.tx_packets.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn rx_packets(&self) -> u64 {
        self.rx_packets.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.tx_bytes.store(0, Ordering::Relaxed);
        self.rx_bytes.store(0, Ordering::Relaxed);
        self.tx_packets.store(0, Ordering::Relaxed);
        self.rx_packets.store(0, Ordering::Relaxed);
    }
}

/// Exponentially Weighted Moving Average (EWMA) for smooth transfer speed display
#[derive(Debug, Clone, Copy)]
pub struct EwmaRate {
    rate: f64,
    alpha: f64,
    initialized: bool,
}

impl EwmaRate {
    pub fn new(alpha: f64) -> Self {
        assert!((0.0..=1.0).contains(&alpha), "Alpha must be between 0.0 and 1.0");
        Self {
            rate: 0.0,
            alpha,
            initialized: false,
        }
    }

    pub fn update(&mut self, sample: f64) -> f64 {
        if !self.initialized {
            self.rate = sample;
            self.initialized = true;
        } else {
            self.rate = self.alpha * sample + (1.0 - self.alpha) * self.rate;
        }
        self.rate
    }

    #[inline]
    pub fn value(&self) -> f64 {
        self.rate
    }
}

/// Latency histogram with fixed bucket boundaries (microsecond resolution)
pub struct LatencyHistogram {
    // Buckets: <1ms, 1-5ms, 5-10ms, 10-25ms, 25-50ms, 50-100ms, 100-250ms, 250-500ms, >500ms
    buckets: [AtomicU64; 9],
    total_samples: AtomicU64,
    sum_micros: AtomicU64,
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        Self::new()
    }
}

impl LatencyHistogram {
    pub const fn new() -> Self {
        Self {
            buckets: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            total_samples: AtomicU64::new(0),
            sum_micros: AtomicU64::new(0),
        }
    }

    pub fn record_micros(&self, micros: u64) {
        let idx = match micros {
            0..=999 => 0,
            1000..=4999 => 1,
            5000..=9999 => 2,
            10000..=24999 => 3,
            25000..=49999 => 4,
            50000..=99999 => 5,
            100000..=249999 => 6,
            250000..=499999 => 7,
            _ => 8,
        };
        self.buckets[idx].fetch_add(1, Ordering::Relaxed);
        self.total_samples.fetch_add(1, Ordering::Relaxed);
        self.sum_micros.fetch_add(micros, Ordering::Relaxed);
    }

    pub fn mean_micros(&self) -> f64 {
        let count = self.total_samples.load(Ordering::Relaxed);
        if count == 0 {
            0.0
        } else {
            (self.sum_micros.load(Ordering::Relaxed) as f64) / (count as f64)
        }
    }

    pub fn total(&self) -> u64 {
        self.total_samples.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_traffic_counters() {
        let counters = TrafficCounters::new();
        counters.add_tx(1024);
        counters.add_rx(2048);
        assert_eq!(counters.tx_bytes(), 1024);
        assert_eq!(counters.rx_bytes(), 2048);
        assert_eq!(counters.tx_packets(), 1);
        assert_eq!(counters.rx_packets(), 1);
    }

    #[test]
    fn test_ewma_smoothing() {
        let mut ewma = EwmaRate::new(0.5);
        assert_eq!(ewma.update(100.0), 100.0);
        assert_eq!(ewma.update(200.0), 150.0);
    }
}
