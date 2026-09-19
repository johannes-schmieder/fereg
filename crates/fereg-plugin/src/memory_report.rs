//! Versioned whole-action estimates. Prediction never consumes an observed peak.
//!
//! Admission bounds and empirical estimates have distinct coverage: a practical
//! estimate may eventually be calibrated without claiming a strict runtime proof.
use fereg_core::error::{ErrorCode, FeregError, Result};
use std::collections::BTreeMap;

pub(crate) const ROWS: usize = 11;
pub(crate) const CELLS: usize = ROWS * 2;
const MAX_EXACT: u128 = 1_u128 << 53;
pub(crate) const BASELINE_GAP: u32 = 1 << 16;
pub(crate) const EXPECTED_USE_GAP: u32 = 1 << 17;
pub(crate) const STATA_GROWTH_GAP: u32 = 1 << 18;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Baseline {
    pub(crate) entry_used: Option<u128>,
    pub(crate) entry_allocated: Option<u128>,
    pub(crate) before_used: Option<u128>,
    pub(crate) before_allocated: Option<u128>,
    pub(crate) calibrated_stata_layout: bool,
}
impl Baseline {
    pub(crate) fn parse(values: &BTreeMap<String, String>) -> Result<Self> {
        fn bytes(values: &BTreeMap<String, String>, name: &str) -> Result<Option<u128>> {
            let raw = values
                .get(name)
                .ok_or_else(|| invalid("missing Stata memory baseline field"))?;
            if raw == "." {
                return Ok(None);
            }
            let n = raw
                .parse::<u128>()
                .map_err(|_| invalid("invalid Stata memory baseline"))?;
            if n > MAX_EXACT {
                return Err(invalid("inexact Stata memory baseline"));
            }
            Ok(Some(n))
        }
        let result = Self {
            entry_used: bytes(values, "stata_used_entry")?,
            entry_allocated: bytes(values, "stata_alloc_entry")?,
            before_used: bytes(values, "stata_used_before")?,
            before_allocated: bytes(values, "stata_alloc_before")?,
            calibrated_stata_layout: values
                .get("stata")
                .and_then(|value| value.parse::<f64>().ok())
                .is_some_and(|version| version.floor() == 19.0),
        };
        for (used, allocated) in [
            (result.entry_used, result.entry_allocated),
            (result.before_used, result.before_allocated),
        ] {
            if matches!((used, allocated), (Some(u), Some(a)) if u > a) {
                return Err(invalid("Stata used memory exceeds allocated memory"));
            }
        }
        Ok(result)
    }
}

/// Fixed-size publication payload. Practical calibration and strict allocation
/// proof coverage are independent; unknown practical scopes remain partial.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Report(pub(crate) [f64; CELLS]);
impl Report {
    #[cfg(test)]
    pub(crate) fn partial(
        baseline: Baseline,
        native_live_before: Option<u64>,
        model: crate::action_memory::Summary,
        safety_headroom: u128,
    ) -> Result<Self> {
        Self::estimated(baseline, native_live_before, model, safety_headroom, None)
    }

    /// Schema 2 exposes the calibrated Stata-19, single-frame engineering model.
    /// Strict gaps remain visible. No current observed peak enters prediction.
    pub(crate) fn estimated(
        baseline: Baseline,
        native_live_before: Option<u64>,
        model: crate::action_memory::Summary,
        safety_headroom: u128,
        forecast: Option<crate::memory_forecast::Forecast>,
    ) -> Result<Self> {
        let native = forecast.map_or(model.accounted_peak, |f| f.native_peak);
        let known_baseline = baseline.entry_allocated.is_some()
            && baseline.before_used.is_some()
            && baseline.before_allocated.is_some()
            && native_live_before.is_some();
        let calibrated = forecast.is_some()
            && known_baseline
            && baseline.calibrated_stata_layout
            && model.missing_stages == 0;
        let mut gaps = u32::from(model.gaps);
        if forecast.is_none() || model.missing_stages != 0 {
            gaps |= EXPECTED_USE_GAP;
        }
        if !baseline.calibrated_stata_layout || !known_baseline {
            gaps |= STATA_GROWTH_GAP;
        }
        if !known_baseline {
            gaps |= BASELINE_GAP;
        }
        // Missing stages are deliberately retained independently of proof gaps.
        gaps |= u32::from(model.missing_stages) << 20;
        let (combined, reserve) = if calibrated {
            let f = forecast.expect("calibrated forecast");
            let native_before = native_live_before.expect("calibrated native baseline");
            let whole = f.combined(
                baseline
                    .before_allocated
                    .expect("calibrated Stata baseline"),
                native_before,
            )?;
            let entry = crate::memory_forecast::sum(&[
                baseline.entry_allocated.expect("calibrated entry baseline"),
                u128::from(native_before),
            ])?;
            (Some(whole.max(entry)), Some(f.additional_reserve()))
        } else {
            (None, None)
        };
        let rows = [
            Some(2),
            Some(if calibrated { 2 } else { 1 }),
            baseline.before_used,
            baseline.before_allocated,
            native_live_before.map(u128::from),
            Some(native),
            forecast.map(|f| f.stata_growth),
            combined,
            reserve,
            Some(safety_headroom),
            Some(u128::from(gaps)),
        ];
        let mut values = [0.0; CELLS];
        for (i, value) in rows.into_iter().enumerate() {
            if let Some(value) = value {
                if value > MAX_EXACT {
                    return Err(invalid("memory report exceeds exact integer range"));
                }
                values[2 * i] = 1.0;
                values[2 * i + 1] = value as f64;
            }
        }
        Ok(Self(values))
    }
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_baseline_is_not_zero_and_partial_has_no_combined_prediction() {
        let m = crate::action_memory::Ledger::new(crate::action_memory::Estimator::Ols).summary();
        let r = Report::partial(Baseline::default(), None, m, 512 << 20)
            .unwrap()
            .0;
        assert_eq!(&r[0..4], &[1.0, 2.0, 1.0, 1.0]);
        for row in [2, 3, 4, 6, 7, 8] {
            assert_eq!(&r[2 * row..2 * row + 2], &[0.0, 0.0]);
        }
        assert_ne!(r[21] as u32 & BASELINE_GAP, 0);
    }
    #[test]
    fn baseline_parser_rejects_missing_negative_inexact_and_contradictory_values() {
        let mut v = BTreeMap::from([
            ("stata_used_entry".into(), ".".into()),
            ("stata_alloc_entry".into(), ".".into()),
            ("stata_used_before".into(), "100".into()),
            ("stata_alloc_before".into(), "200".into()),
        ]);
        assert_eq!(Baseline::parse(&v).unwrap().before_allocated, Some(200));
        for bad in ["-1", "NaN", "9007199254740993", "201", "1.5"] {
            v.insert("stata_used_before".into(), bad.into());
            assert!(Baseline::parse(&v).is_err(), "{bad}");
        }
        v.remove("stata_used_before");
        assert!(Baseline::parse(&v).is_err());
    }
    #[test]
    fn uncalibrated_forecast_never_becomes_a_complete_public_prediction() {
        let m = crate::action_memory::Ledger::new(crate::action_memory::Estimator::Ols).summary();
        let f = crate::memory_forecast::Forecast {
            native_peak: 200,
            stata_growth: 30,
        };
        let r = Report::estimated(
            Baseline {
                before_allocated: Some(1000),
                ..Baseline::default()
            },
            Some(40),
            m,
            512,
            Some(f),
        )
        .unwrap()
        .0;
        assert_eq!(r[3], 1.0);
        assert_eq!(&r[10..14], &[1.0, 200.0, 1.0, 30.0]);
        assert_eq!(&r[14..18], &[0.0, 0.0, 0.0, 0.0]);
        assert_ne!(r[21] as u32 & EXPECTED_USE_GAP, 0);
    }

    #[test]
    fn report_rejects_unrepresentable_forecast() {
        let mut m =
            crate::action_memory::Ledger::new(crate::action_memory::Estimator::Ols).summary();
        m.accounted_peak = MAX_EXACT + 1;
        assert!(Report::partial(Baseline::default(), None, m, 0).is_err());
    }

    #[test]
    fn practical_completeness_keeps_strict_gaps_and_resident_memory_out_of_reserve() {
        let model = crate::action_memory::Summary {
            accounted_peak: 9999,
            complete_peak: None,
            missing_stages: 0,
            gaps: 125,
        };
        let baseline = Baseline {
            entry_used: Some(800),
            entry_allocated: Some(1400),
            before_used: Some(800),
            before_allocated: Some(1000),
            calibrated_stata_layout: true,
        };
        let forecast = crate::memory_forecast::Forecast {
            native_peak: 200,
            stata_growth: 30,
        };
        let report = Report::estimated(baseline, Some(40), model, 512, Some(forecast))
            .unwrap()
            .0;
        assert_eq!(&report[0..4], &[1.0, 2.0, 1.0, 2.0]);
        assert_eq!(&report[14..18], &[1.0, 1440.0, 1.0, 200.0]);
        assert_eq!(report[21], 125.0);
        assert_eq!(model.complete_peak, None);
        for (b, m, f) in [
            (
                Baseline {
                    before_used: None,
                    ..baseline
                },
                model,
                Some(forecast),
            ),
            (
                Baseline {
                    calibrated_stata_layout: false,
                    ..baseline
                },
                model,
                Some(forecast),
            ),
            (
                baseline,
                crate::action_memory::Summary {
                    missing_stages: 1,
                    ..model
                },
                Some(forecast),
            ),
            (baseline, model, None),
        ] {
            let partial = Report::estimated(b, Some(40), m, 512, f).unwrap().0;
            assert_eq!(partial[3], 1.0);
            assert_eq!(&partial[14..18], &[0.0; 4]);
        }
    }
}
