//! Pure forecasts from caller-owned log-proxy observations.
//!
//! The caller validates positive proxies/target before taking logarithms and
//! supplies samples from one recurrence at exactly eight-step spacing. This
//! module neither stores banks nor changes readiness, budgets or lifecycle.

use super::model::{Unknown, Work};

const CADENCE: usize = 8;
const FIRST_DECISION: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum ProgressClass {
    Decreasing = 1,
    Mixed = 2,
    Stagnant = 4,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct LogWindow {
    pub age: usize,
    /// Log proxies at age minus sixteen, age minus eight, and current age.
    pub previous: Option<f64>,
    pub middle: Option<f64>,
    pub current: Option<f64>,
}

pub(super) fn is_observation_age(age: usize) -> bool {
    age % CADENCE == 0
}

pub(super) fn is_decision_age(age: usize) -> bool {
    age >= FIRST_DECISION && is_observation_age(age)
}

/// A retired lane has no unresolved demand and needs no progress history.
/// Negative activity (including the underflow sentinel) cannot supply credit.
pub(super) fn forecast_lane(
    window: LogWindow,
    log_target: f64,
    activity: f64,
    remaining: usize,
) -> Result<Option<(u128, ProgressClass)>, Unknown> {
    if !activity.is_finite() || activity < 0.0 {
        return Err(Unknown::InvalidInput);
    }
    if activity == 0.0 {
        return Ok(None);
    }
    if remaining == 0 || !is_decision_age(window.age) || !log_target.is_finite() {
        return Err(Unknown::InvalidInput);
    }
    let (Some(previous), Some(middle), Some(current)) =
        (window.previous, window.middle, window.current)
    else {
        return Err(Unknown::InvalidInput);
    };
    if !previous.is_finite() || !middle.is_finite() || !current.is_finite() {
        return Err(Unknown::InvalidInput);
    }
    let first = (previous - middle) / CADENCE as f64;
    let second = (middle - current) / CADENCE as f64;
    let distance = current - log_target;
    if !first.is_finite() || !second.is_finite() || !distance.is_finite() {
        return Err(Unknown::Overflow);
    }
    if distance <= 0.0 {
        return Err(Unknown::InvalidInput);
    }
    let class = match (first > 0.0, second > 0.0) {
        (true, true) => ProgressClass::Decreasing,
        (true, false) | (false, true) => ProgressClass::Mixed,
        (false, false) => ProgressClass::Stagnant,
    };
    if class == ProgressClass::Stagnant {
        return Ok(Some((remaining as u128, class)));
    }
    let forecast = distance / first.max(second);
    if !forecast.is_finite() {
        return Err(Unknown::Overflow);
    }
    let capped = forecast.ceil().max(1.0).min(remaining as f64);
    // u128::MAX rounds to exactly 2^128 in binary64. Reject that endpoint
    // before casting; a float-to-integer saturating cast must never price work.
    if !capped.is_finite() || capped < 1.0 || capped >= u128::MAX as f64 {
        return Err(Unknown::Overflow);
    }
    // usize::MAX can likewise round upward in binary64. Restore the exact
    // integer budget after the checked conversion, without changing it.
    let horizon = (capped as u128).min(remaining as u128);
    Ok(Some((horizon, class)))
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Forecast {
    work: Work,
    classes: u8,
}

impl Forecast {
    pub const fn new() -> Self {
        Self {
            work: Work { h: 0, d: 0 },
            classes: 0,
        }
    }

    /// Add one lane result. Failure leaves the accumulator unchanged.
    pub fn push(&mut self, lane: Option<(u128, ProgressClass)>) -> Result<(), Unknown> {
        let Some((horizon, class)) = lane else {
            return Ok(());
        };
        if horizon == 0 {
            return Err(Unknown::InvalidInput);
        }
        let total = self.work.d.checked_add(horizon).ok_or(Unknown::Overflow)?;
        self.work = Work {
            h: self.work.h.max(horizon),
            d: total,
        };
        self.classes |= class as u8;
        Ok(())
    }

    /// The mask is the union of ProgressClass discriminants. Every present
    /// class needs caller-side support; it is not reduced to a majority class.
    pub fn finish(self) -> Result<(Work, u8), Unknown> {
        if self.work.h == 0 {
            return Err(Unknown::InvalidInput);
        }
        Ok((self.work, self.classes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn logs(previous: f64, middle: f64, current: f64) -> LogWindow {
        LogWindow {
            age: 16,
            previous: Some(previous),
            middle: Some(middle),
            current: Some(current),
        }
    }

    #[test]
    fn cadence_is_recurrence_local_with_an_initial_observation() {
        for (age, observation, decision) in [
            (0, true, false),
            (7, false, false),
            (8, true, false),
            (15, false, false),
            (16, true, true),
            (17, false, false),
            (24, true, true),
        ] {
            assert_eq!(is_observation_age(age), observation);
            assert_eq!(is_decision_age(age), decision);
        }
    }

    #[test]
    fn faster_adjacent_slope_is_used_in_either_order() {
        // Log gap eight, slopes one and two per step: four remaining steps.
        // The sixteen-step average would instead predict ceil(8/1.5)=6.
        for window in [logs(32.0, 24.0, 8.0), logs(32.0, 16.0, 8.0)] {
            assert_eq!(
                forecast_lane(window, 0.0, 1.0, 100),
                Ok(Some((4, ProgressClass::Decreasing)))
            );
        }
    }

    #[test]
    fn mixed_and_stagnant_windows_are_distinct_observable_classes() {
        assert_eq!(
            forecast_lane(logs(0.0, 8.0, 0.0), -5.0, 1.0, 20),
            Ok(Some((5, ProgressClass::Mixed)))
        );
        assert_eq!(
            forecast_lane(logs(8.0, 0.0, 8.0), 3.0, 1.0, 20),
            Ok(Some((5, ProgressClass::Mixed)))
        );
        for window in [logs(0.0, 0.0, 8.0), logs(8.0, 8.0, 8.0)] {
            assert_eq!(
                forecast_lane(window, 0.0, 1.0, 20),
                Ok(Some((20, ProgressClass::Stagnant)))
            );
        }
    }

    #[test]
    fn horizons_round_up_and_respect_the_exact_integer_allowance() {
        // Rate 1/4, gap 9/8 gives ceil(9/2)=5.
        assert_eq!(
            forecast_lane(logs(4.0, 2.0, 0.0), -1.125, 1.0, 100),
            Ok(Some((5, ProgressClass::Decreasing)))
        );
        assert_eq!(
            forecast_lane(logs(4.0, 2.0, 0.0), -1.125, 1.0, 3),
            Ok(Some((3, ProgressClass::Decreasing)))
        );
        assert_eq!(
            forecast_lane(logs(4.0, 2.0, 0.0), -0.01, 1.0, 3),
            Ok(Some((1, ProgressClass::Decreasing)))
        );
        // A finite forecast far outside u128 is capped before conversion.
        assert_eq!(
            forecast_lane(logs(3.0, 2.0, 1.0), -1e300, 1.0, usize::MAX),
            Ok(Some((usize::MAX as u128, ProgressClass::Decreasing)))
        );
    }

    #[test]
    fn missing_invalid_or_negative_sentinel_cannot_supply_credit() {
        let valid = logs(3.0, 2.0, 1.0);
        for window in [
            LogWindow {
                previous: None,
                ..valid
            },
            LogWindow {
                middle: None,
                ..valid
            },
            LogWindow {
                current: None,
                ..valid
            },
            LogWindow { age: 8, ..valid },
            LogWindow { age: 17, ..valid },
            logs(f64::NAN, 2.0, 1.0),
            logs(3.0, f64::INFINITY, 1.0),
            logs(3.0, 2.0, f64::NEG_INFINITY),
        ] {
            assert_eq!(
                forecast_lane(window, 0.0, 1.0, 20),
                Err(Unknown::InvalidInput)
            );
        }
        for activity in [-1.0, -0.5, f64::NAN, f64::INFINITY] {
            assert_eq!(
                forecast_lane(valid, 0.0, activity, 20),
                Err(Unknown::InvalidInput)
            );
        }
        for target in [1.0, 2.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                forecast_lane(valid, target, 1.0, 20),
                Err(Unknown::InvalidInput)
            );
        }
        assert_eq!(
            forecast_lane(valid, 0.0, 1.0, 0),
            Err(Unknown::InvalidInput)
        );
    }

    #[test]
    fn retired_lanes_need_no_history_and_all_live_classes_are_retained() {
        let retired = LogWindow {
            age: 0,
            previous: None,
            middle: None,
            current: None,
        };
        assert_eq!(forecast_lane(retired, f64::NAN, 0.0, 0), Ok(None));
        let mut all = Forecast::new();
        all.push(None).unwrap();
        assert_eq!(all.finish(), Err(Unknown::InvalidInput));
        all.push(forecast_lane(logs(32.0, 24.0, 8.0), 0.0, 1.0, 20).unwrap())
            .unwrap();
        all.push(forecast_lane(logs(0.0, 8.0, 0.0), -5.0, 1.0, 20).unwrap())
            .unwrap();
        all.push(forecast_lane(logs(8.0, 8.0, 8.0), 0.0, 1.0, 20).unwrap())
            .unwrap();
        assert_eq!(all.finish(), Ok((Work { h: 20, d: 29 }, 7)));
    }

    #[test]
    fn arithmetic_overflow_is_unknown_and_accumulator_failure_is_atomic() {
        for (window, target) in [
            (logs(f64::MAX, -f64::MAX, 1.0), 0.0),
            (logs(f64::MAX, f64::MAX, f64::MAX), -f64::MAX),
            (logs(3.0, 2.0, 1.0), -f64::MAX),
        ] {
            assert_eq!(
                forecast_lane(window, target, 1.0, 20),
                Err(Unknown::Overflow)
            );
        }
        let mut all = Forecast::new();
        all.push(Some((u128::MAX, ProgressClass::Stagnant)))
            .unwrap();
        assert_eq!(
            all.push(Some((1, ProgressClass::Mixed))),
            Err(Unknown::Overflow)
        );
        assert_eq!(
            all.finish(),
            Ok((
                Work {
                    h: u128::MAX,
                    d: u128::MAX
                },
                4
            ))
        );
        assert_eq!(
            all.push(Some((0, ProgressClass::Mixed))),
            Err(Unknown::InvalidInput)
        );
    }
}
