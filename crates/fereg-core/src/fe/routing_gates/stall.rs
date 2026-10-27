//! Gate v2 stall rule: a deterministic early handoff for a stalled pilot.
//!
//! At local ages 4, 8 and 12 a packed or outcome projection builds its CMG
//! hierarchy when some active lane's preconditioned energy still exceeds
//! `threshold^(2 age)` times its initial energy, i.e. its average residual
//! contraction per step since the pilot began is at least `threshold`. With
//! the frozen one half, such a lane would need far more than the 16-step
//! pilot to reach a 1e-8 target (0.5^16 is about 1.5e-5), and v1 builds after
//! the pilot anyway. No bank, scan or clock is kept: the initial energy
//! already lives in the recurrence's activity state.
//!
//! The development measurement that froze the threshold (2026-10-27) found
//! at most 0.33 at any age on every routing family that finishes without a
//! hierarchy, and 0.57-0.59 at age 4 on the two AKM panels.

/// Early decision ages; local16 remains gate v1's unconditional boundary.
pub(super) const AGES: [usize; 3] = [4, 8, 12];

/// Frozen per-step residual contraction bound of the selected v2 rule.
pub(crate) const STALL_THRESHOLD: f64 = 0.5;

/// The largest current/initial energy ratio over active lanes, or `None` when
/// a lane has no valid history (an underflowed start or an invalid energy).
pub(super) fn energy_ratio(initial: &[f64], current: &[f64]) -> Option<f64> {
    let mut largest: Option<f64> = None;
    for (&start, &now) in initial.iter().zip(current) {
        if start == 0.0 {
            continue;
        }
        if !(start > 0.0 && start.is_finite() && now.is_finite() && now >= 0.0) {
            return None;
        }
        let ratio = now / start;
        largest = Some(largest.map_or(ratio, |value: f64| value.max(ratio)));
    }
    largest
}

/// `threshold^(2 age)` by repeated multiplication, identical on every host.
pub(super) fn energy_bound(threshold: f64, age: usize) -> f64 {
    let squared = threshold * threshold;
    (0..age).fold(1.0, |bound, _| bound * squared)
}

/// Average per-step residual contraction behind an energy ratio (trace only).
pub(super) fn contraction(ratio: f64, age: usize) -> f64 {
    ratio.powf(0.5 / age as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn energy_ratio_skips_completed_lanes_and_rejects_invalid_history() {
        assert_eq!(energy_ratio(&[4.0, 0.0, 2.0], &[1.0, 9.0, 1.5]), Some(0.75));
        assert_eq!(energy_ratio(&[0.0, 0.0], &[1.0, 2.0]), None);
        assert_eq!(energy_ratio(&[-1.0, 2.0], &[1.0, 1.0]), None);
        assert_eq!(energy_ratio(&[2.0], &[f64::NAN]), None);
        assert_eq!(energy_ratio(&[2.0], &[-1.0]), None);
        assert_eq!(energy_ratio(&[2.0], &[0.0]), Some(0.0));
    }

    #[test]
    fn energy_bound_is_the_squared_threshold_power() {
        assert_eq!(energy_bound(0.5, 1), 0.25);
        assert_eq!(energy_bound(0.5, 4), 0.25f64.powi(4));
        assert_eq!(energy_bound(STALL_THRESHOLD, 0), 1.0);
        for age in AGES {
            let bound = energy_bound(STALL_THRESHOLD, age);
            assert!((contraction(bound, age) - STALL_THRESHOLD).abs() < 1e-15);
        }
    }
}
