use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

const MAX_I32_FLOAT: f32 = 2_147_483_648.0;
#[allow(dead_code)]
const DB_FLOOR: f32 = -90.0;
const DB_RANGE: f32 = 90.0;
const PEAK_SCALE_MAX: u8 = 255;

pub struct ChannelMeter {
    peaks: Vec<AtomicU8>,
}

impl ChannelMeter {
    pub fn new(channels: usize) -> Arc<Self> {
        Arc::new(Self {
            peaks: (0..channels).map(|_| AtomicU8::new(0)).collect(),
        })
    }

    /// Update peak for a channel from i32 PCM samples (normalized to 0..2^31-1 range)
    pub fn update_i32(&self, channel: usize, samples: &[i32]) {
        if channel >= self.peaks.len() {
            return;
        }
        let peak = samples
            .iter()
            .map(|&s| s.unsigned_abs())
            .max()
            .unwrap_or(0);
        let db_val = if peak == 0 {
            0u8
        } else {
            let db = 20.0 * (peak as f32 / MAX_I32_FLOAT).log10();
            // Map -90dB..0dB to 0..255 (Dante-like logarithmic scale)
            ((db + DB_RANGE) * PEAK_SCALE_MAX as f32 / DB_RANGE).clamp(0.0, PEAK_SCALE_MAX as f32) as u8
        };
        // Peak hold: only update if new peak is higher
        let current = self.peaks[channel].load(Ordering::Relaxed);
        if db_val > current {
            self.peaks[channel].store(db_val, Ordering::Relaxed);
        }
    }

    /// Decay all peaks by 1 unit (call periodically, e.g., every 5 seconds)
    pub fn decay(&self) {
        for peak in &self.peaks {
            let v = peak.load(Ordering::Relaxed);
            if v > 0 {
                peak.store(v - 1, Ordering::Relaxed);
            }
        }
    }

    /// Get current peak values for all channels
    pub fn get_peaks(&self) -> Vec<u8> {
        self.peaks
            .iter()
            .map(|p| p.load(Ordering::Relaxed))
            .collect()
    }
}
