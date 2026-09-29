//! A necessary support screen, not a certificate of independent sampling or bias reduction.
use super::{error, filled, ErrorCode, PpmlInterruptPoint, Result};
use crate::fe::{BorrowedMapWeights, FeDesign};

/// Per-effect summaries over final retained levels. Quartiles use nearest ranks.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PpmlBiasSupport {
    pub levels: usize,
    pub minimum_units: usize,
    pub p25_units: usize,
    pub median_units: usize,
    pub p75_units: usize,
    pub maximum_units: usize,
    pub single_unit_levels: usize,
    pub minimum_information_units: f64,
    pub median_information_units: f64,
    pub maximum_unit_information_share: f64,
}

impl PpmlBiasSupport {
    /// Public support matrix columns, in documented order.
    pub fn values(&self) -> [f64; 11] {
        [
            self.levels as f64,
            self.minimum_units as f64,
            self.p25_units as f64,
            self.median_units as f64,
            self.p75_units as f64,
            self.maximum_units as f64,
            self.single_unit_levels as f64,
            self.single_unit_levels as f64 / self.levels as f64,
            self.minimum_information_units,
            self.median_information_units,
            self.maximum_unit_information_share,
        ]
    }

    pub(super) fn validate(&self, levels: usize, units: usize) -> Result<()> {
        let counts = [
            self.minimum_units,
            self.p25_units,
            self.median_units,
            self.p75_units,
            self.maximum_units,
        ];
        if self.levels != levels
            || levels == 0
            || self.single_unit_levels != 0
            || counts[0] < 2
            || counts[4] > units
            || counts.windows(2).any(|w| w[0] > w[1])
            || self.values().iter().any(|v| !v.is_finite())
            || self.minimum_information_units < 1.
            || self.median_information_units < self.minimum_information_units
            || self.minimum_information_units > self.minimum_units as f64
            || self.median_information_units > self.maximum_units as f64
            || self.maximum_unit_information_share <= 0.
            || self.maximum_unit_information_share > 1.
        {
            return Err(error(
                ErrorCode::Internal,
                "bias support metadata is inconsistent",
            ));
        }
        Ok(())
    }
}

pub(super) fn check(
    design: &FeDesign,
    weights: BorrowedMapWeights<'_>,
    units: &[f64],
    interrupt: &mut dyn FnMut(PpmlInterruptPoint) -> Result<()>,
) -> Result<[PpmlBiasSupport; 2]> {
    if design.effects().len() != 2
        || units.is_empty()
        || matches!(weights, BorrowedMapWeights::Provided(w) if w.len() != units.len())
        || units.iter().any(|v| !v.is_finite())
        || design
            .effects()
            .iter()
            .any(|e| e.n_levels() == 0 || e.codes().len() != units.len())
    {
        return Err(error(
            ErrorCode::InvalidArgument,
            "bias support inputs are missing or misaligned",
        ));
    }
    let mut entries = filled(units.len(), (0., 0.))?;
    let mut result = [PpmlBiasSupport::default(); 2];
    for (effect, summary) in design.effects().iter().zip(&mut result) {
        let mut counts = filled(effect.n_levels(), 0usize)?;
        let mut effective = filled(effect.n_levels(), 0.)?;
        let mut maximum_share: f64 = 0.;
        for level in 0..effect.n_levels() {
            interrupt(PpmlInterruptPoint::BiasCorrection)?;
            let members = effect.level_index().checked_members(level)?;
            let entries = &mut entries[..members.len()];
            let mut scale: f64 = 0.;
            for (entry, &row) in entries.iter_mut().zip(members) {
                let h = match weights {
                    BorrowedMapWeights::Unit => 1.,
                    BorrowedMapWeights::Provided(w) => w[row],
                };
                if !h.is_finite() || h <= 0. {
                    return Err(error(
                        ErrorCode::NonConvergence,
                        "bias support information is not positive and finite",
                    ));
                }
                *entry = (if units[row] == 0. { 0. } else { units[row] }, h);
                scale = scale.max(h);
            }
            // Sort information within each unit too, so row permutations do not
            // change the summation order. No FE-by-unit dense table is needed.
            entries.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
            let mut total = 0.;
            let mut squares = 0.;
            let mut largest: f64 = 0.;
            let mut start = 0;
            while start < entries.len() {
                let mut end = start + 1;
                while end < entries.len() && entries[end].0 == entries[start].0 {
                    end += 1;
                }
                // Rescaling before summing/squaring avoids overflow and is
                // invariant to the fit's common probability-weight multiplier.
                let information: f64 = entries[start..end].iter().map(|e| e.1 / scale).sum();
                total += information;
                squares += information * information;
                largest = largest.max(information);
                counts[level] += 1;
                start = end;
            }
            if counts[level] == 0 || !total.is_finite() || !squares.is_finite() || squares <= 0. {
                return Err(error(
                    ErrorCode::NonConvergence,
                    "bias support information cannot be summarized",
                ));
            }
            effective[level] = (total * (total / squares)).clamp(1., counts[level] as f64);
            maximum_share = maximum_share.max((largest / total).min(1.));
        }
        counts.sort_unstable();
        effective.sort_unstable_by(f64::total_cmp);
        let levels = counts.len();
        // ceil(q*L/4)-1, without floating-point ranks or usize multiplication.
        let rank = |q: usize| (levels / 4 * q + (levels % 4 * q).div_ceil(4)).saturating_sub(1);
        *summary = PpmlBiasSupport {
            levels,
            minimum_units: counts[0],
            p25_units: counts[rank(1)],
            median_units: counts[rank(2)],
            p75_units: counts[rank(3)],
            maximum_units: counts[levels - 1],
            single_unit_levels: counts.iter().filter(|&&n| n == 1).count(),
            minimum_information_units: effective[0],
            median_information_units: effective[rank(2)],
            maximum_unit_information_share: maximum_share,
        };
    }
    if result.iter().any(|s| s.single_unit_levels > 0) {
        return Err(error(ErrorCode::InvalidArgument, &format!(
            "PPML bias correction requires at least two distinct bias units per retained FE level; FE1: {}/{} and FE2: {}/{} levels have only one unit. More rows or probes do not repair this support; choose units from the sampling assumptions, or omit biascorrection()",
            result[0].single_unit_levels, result[0].levels,
            result[1].single_unit_levels, result[1].levels,
        )));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    fn design() -> FeDesign {
        FeDesign::new(
            8,
            vec![
                EffectTerm::new(vec![0, 0, 0, 0, 1, 1, 1, 1], 2, true, vec![]).unwrap(),
                EffectTerm::new(vec![0, 1, 0, 1, 0, 1, 0, 1], 2, true, vec![]).unwrap(),
            ],
        )
        .unwrap()
    }

    #[test]
    fn bias_support_crossing_units_and_information_concentration() {
        // Neither FE nests the units. Equal counts hide a 90% information share.
        let units = [1., 2., 1., 2., 2., 1., 2., 1.];
        let weights = [9., 1., 9., 1., 1., 9., 1., 9.];
        let got = check(
            &design(),
            BorrowedMapWeights::Provided(&weights),
            &units,
            &mut |_| Ok(()),
        )
        .unwrap();
        for s in got {
            assert_eq!(s.levels, 2);
            assert_eq!(s.minimum_units, 2);
            assert_eq!(s.maximum_units, 2);
            assert_eq!(s.single_unit_levels, 0);
            assert!((s.minimum_information_units - 100. / 82.).abs() < 1e-14);
            assert!((s.maximum_unit_information_share - 0.9).abs() < 1e-14);
            s.validate(2, 2).unwrap();
        }
        let balanced = check(&design(), BorrowedMapWeights::Unit, &units, &mut |_| Ok(())).unwrap();
        assert_eq!(balanced[0].minimum_information_units, 2.);
        assert_eq!(balanced[1].maximum_unit_information_share, 0.5);
        let extreme = weights.map(|w| w * 1e300);
        let scaled = check(
            &design(),
            BorrowedMapWeights::Provided(&extreme),
            &units,
            &mut |_| Ok(()),
        )
        .unwrap();
        assert!(
            (scaled[0].minimum_information_units - got[0].minimum_information_units).abs() < 1e-14
        );
    }

    #[test]
    fn bias_support_rejects_partial_support_and_signed_zero_aliases() {
        let e = check(
            &design(),
            BorrowedMapWeights::Unit,
            &[-0., 0., -0., 0., 1., 2., 3., 4.],
            &mut |_| Ok(()),
        )
        .unwrap_err();
        assert!(e.message.contains("FE1: 1/2 and FE2: 0/2"), "{e}");
    }

    #[test]
    fn bias_support_nearest_rank_distribution_and_invalid_metadata() {
        let first = vec![0, 0, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 3];
        let n = first.len();
        let design = FeDesign::new(
            n,
            vec![
                EffectTerm::new(first, 4, true, vec![]).unwrap(),
                EffectTerm::new(vec![0; n], 1, true, vec![]).unwrap(),
            ],
        )
        .unwrap();
        let units: Vec<_> = (0..n).map(|r| r as f64).collect();
        let got = check(&design, BorrowedMapWeights::Unit, &units, &mut |_| Ok(())).unwrap();
        assert_eq!(&got[0].values()[..8], &[4., 2., 2., 3., 4., 5., 0., 0.]);
        assert_eq!(got[0].minimum_information_units, 2.);
        assert_eq!(got[0].median_information_units, 3.);
        let mut invalid = got[0];
        invalid.minimum_units = 1;
        assert!(invalid.validate(4, n).is_err());
        invalid = got[0];
        invalid.maximum_unit_information_share = f64::NAN;
        assert!(invalid.validate(4, n).is_err());
    }
}

#[cfg(test)]
#[test]
#[ignore = "focused support-screen timing, run explicitly with --ignored --nocapture"]
fn bias_support_timing_probe() {
    use crate::fe::EffectTerm;
    for n in [120_000, 1_000_000] {
        let design = FeDesign::new(
            n,
            vec![
                EffectTerm::new(
                    (0..n).map(|r| (r % 128) as u32).collect(),
                    128,
                    true,
                    vec![],
                )
                .unwrap(),
                EffectTerm::new(
                    (0..n).map(|r| ((r / 128) % 128) as u32).collect(),
                    128,
                    true,
                    vec![],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let units: Vec<_> = (0..n).map(|r| (r / 4) as f64).collect();
        let information: Vec<_> = (0..n).map(|r| 0.1 + (r % 113) as f64).collect();
        for repetition in 0..4 {
            let started = std::time::Instant::now();
            let support = check(
                &design,
                BorrowedMapWeights::Provided(&information),
                &units,
                &mut |_| Ok(()),
            )
            .unwrap();
            assert!(support.iter().all(|s| s.minimum_units >= 2));
            println!("BIAS_SUPPORT_TIMING rows={n} repetition={repetition} seconds={:.9} scratch_bound_bytes={}",
                started.elapsed().as_secs_f64(), 16 * n + 16 * 128);
        }
    }
}
