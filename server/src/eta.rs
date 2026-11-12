//! The time estimate shown while a file converts.
//!
//! A conversion takes about `BASE_S` plus a rate times the file size (MiB) times the number of
//! one-version steps. The rate starts at `DEFAULT_RATE` and follows the finished conversions of
//! this run (an exponential moving average, kept in memory only), so the estimate keeps up when
//! the engine or the machine gets faster or slower. The shown estimate adds `MARGIN` on top, so it
//! is a little long rather than short.

use std::sync::Mutex;
use std::time::Duration;

const MIB: f64 = (1u64 << 20) as f64;
/// Fixed cost of one job: starting the engine, the first check, reading the target references.
const BASE_S: f64 = 1.0;
/// Seconds per MiB per step, as measured on the server with the current engine (large models
/// take 0.18 to 0.20; small files spend relatively more on fixed costs, which `BASE_S` covers).
pub const DEFAULT_RATE: f64 = 0.19;
/// How much longer than the expected time the estimate is.
pub const MARGIN: f64 = 1.15;
/// Weight of the newest conversion in the moving average.
const WEIGHT: f64 = 0.2;
/// Jobs smaller than this many MiB x steps are not learned from: their time is mostly fixed costs.
const MIN_WORK: f64 = 20.0;
/// The learned rate stays within this factor of `DEFAULT_RATE` either way, so that one odd job
/// (a machine busy with something else) cannot make the estimate absurd.
const BOUND: f64 = 3.0;

/// The current rate, shared by every request and worker.
#[derive(Debug)]
pub struct Eta {
    rate: Mutex<f64>,
}

impl Default for Eta {
    fn default() -> Eta {
        Eta::new()
    }
}

fn work(bytes: u64, steps: u32) -> f64 {
    bytes as f64 / MIB * f64::from(steps)
}

impl Eta {
    pub fn new() -> Eta {
        Eta {
            rate: Mutex::new(DEFAULT_RATE),
        }
    }

    /// Seconds per MiB per step the estimate uses now (before the margin).
    pub fn rate(&self) -> f64 {
        *self.rate.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The estimate shown to the user, rounded up to whole seconds.
    pub fn estimate(&self, bytes: u64, steps: u32) -> Duration {
        let s = (BASE_S + self.rate() * work(bytes, steps)) * MARGIN;
        Duration::from_secs(s.ceil() as u64)
    }

    /// Learns from a conversion that finished in `took`.
    pub fn observe(&self, bytes: u64, steps: u32, took: Duration) {
        let w = work(bytes, steps);
        if w < MIN_WORK {
            return;
        }
        let seen = ((took.as_secs_f64() - BASE_S).max(0.0) / w)
            .clamp(DEFAULT_RATE / BOUND, DEFAULT_RATE * BOUND);
        let mut rate = self.rate.lock().unwrap_or_else(|e| e.into_inner());
        *rate += WEIGHT * (seen - *rate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIB_U: u64 = 1 << 20;

    fn secs(eta: &Eta, mib_x100: u64, steps: u32) -> u64 {
        eta.estimate(mib_x100 * MIB_U / 100, steps).as_secs()
    }

    /// Times measured with the release engine on the server, probe + downgrade:
    /// (MiB x 100, steps, seconds).
    const MEASURED: [(u64, u32, f64); 14] = [
        (43, 1, 0.25),
        (43, 3, 0.50),
        (43, 5, 0.77),
        (793, 1, 1.76),
        (1743, 1, 3.35),
        (846, 1, 4.00),
        (846, 3, 8.92),
        (846, 5, 12.09),
        (3676, 1, 7.41),
        (3676, 3, 22.25),
        (3676, 5, 35.48),
        (14858, 1, 27.61),
        (14858, 3, 80.79),
        (14858, 4, 106.79),
    ];

    #[test]
    fn estimate_is_a_little_above_measured_times() {
        let eta = Eta::new();
        for (mib_x100, steps, real) in MEASURED {
            let est = secs(&eta, mib_x100, steps) as f64;
            // Never more than 3 s or 60 % over (small files round up to whole seconds)...
            assert!(
                est <= (real * 1.6).max(real + 3.0),
                "{mib_x100} {steps}: {est} for {real}"
            );
            // ...and for large models 10 to 35 % over.
            if mib_x100 >= 3000 {
                let over = est / real;
                assert!((1.10..=1.35).contains(&over), "{mib_x100} {steps}: {over}");
            }
            // At worst a quarter short (one heavy 2027 model).
            assert!(est >= real * 0.75, "{mib_x100} {steps}: {est} for {real}");
        }
    }

    #[test]
    fn the_old_example_is_now_a_few_seconds() {
        // 2023 -> 2022, 17 MiB: the old formula said 18 s, it takes 3.4 s.
        assert_eq!(secs(&Eta::new(), 1743, 1), 5);
        assert_eq!(secs(&Eta::new(), 0, 1), 2);
    }

    #[test]
    fn learns_from_large_jobs_only_and_stays_bounded() {
        let eta = Eta::new();
        // Small jobs teach nothing.
        eta.observe(MIB_U, 1, Duration::from_secs(60));
        assert_eq!(eta.rate(), DEFAULT_RATE);
        // A faster engine: 100 MiB x 1 step in 11 s is 0.1 s per MiB per step.
        for _ in 0..40 {
            eta.observe(100 * MIB_U, 1, Duration::from_secs(11));
        }
        assert!((eta.rate() - 0.1).abs() < 0.001, "{}", eta.rate());
        assert_eq!(eta.estimate(100 * MIB_U, 1).as_secs(), 13);
        // One job on a busy machine moves it, within bounds.
        let r0 = eta.rate();
        eta.observe(100 * MIB_U, 1, Duration::from_secs(10_000));
        let moved = eta.rate();
        assert!(moved > r0);
        assert!((moved - (r0 + WEIGHT * (DEFAULT_RATE * BOUND - r0))).abs() < 1e-9);
        for _ in 0..100 {
            eta.observe(100 * MIB_U, 1, Duration::ZERO);
        }
        assert!((eta.rate() - DEFAULT_RATE / BOUND).abs() < 0.001);
    }
}
