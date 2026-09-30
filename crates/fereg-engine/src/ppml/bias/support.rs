//! Harvested support semantics from frozen ppml/bias_support.rs; final qmu only.
use super::{filled, invalid, numerical, units::Units};
use crate::{
    data::{DropReason, NumericColumnView},
    error::{ErrorCode, FeregError, Result},
    ppml::irls,
    topology::Partition,
};
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Summary {
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
impl Summary {
    /// SPEC's eleven support-matrix columns in declared order.
    #[must_use]
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
}
#[derive(Debug, PartialEq)]
pub struct Support {
    pub effects: [Summary; 2],
    units: Units,
}
impl Support {
    #[must_use]
    pub fn identities(&self) -> &[f64] {
        &self.units.identities
    }
    /// One unit code per retained physical row; frequency copies do not add IDs.
    #[must_use]
    pub fn codes(&self) -> &[usize] {
        &self.units.codes
    }
    #[must_use]
    pub fn minimum_unit_rows(&self) -> usize {
        self.units.minimum_rows
    }
    #[must_use]
    pub fn maximum_unit_rows(&self) -> usize {
        self.units.maximum_rows
    }
}
/// Necessary FE/unit screen only. Finite-model grammar, request controls and
/// numerical correction certification remain separate requirements.
pub fn screen(
    fit: &irls::Fit<'_>,
    input: NumericColumnView<'_>,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Support> {
    interrupt()?;
    let sample = fit.problem().sample;
    let topology = sample
        .topology
        .as_ref()
        .ok_or_else(|| invalid("bias topology is absent"))?;
    if topology.effects().len() != 2
        || topology.effects().iter().any(|effect| {
            !effect.intercept
                || !effect.slopes.is_empty()
                || effect.term_id.is_none()
                || effect.categorical_sources.is_empty()
        })
    {
        return Err(FeregError::new(
            ErrorCode::Unsupported,
            "bias correction requires exactly two categorical intercept FE sets without slopes",
        ));
    }
    if sample.drop_reasons.iter().any(|reason| {
        matches!(
            reason,
            DropReason::FixedEffectSeparation
                | DropReason::SimplexSeparation
                | DropReason::IteratedRectifierSeparation
                | DropReason::MuSeparation
        )
    }) {
        return Err(invalid(
            "bias correction refuses outcome-dependent separation deletions",
        ));
    }
    let units = Units::new(sample, input, interrupt)?;
    let mut scratch = filled(sample.n_active(), (0usize, 0.))?;
    let mut effects = [Summary::default(); 2];
    for (index, summary) in effects.iter_mut().enumerate() {
        *summary = summarize(
            topology.effect_partition(index)?,
            &fit.hessian_weights,
            &units,
            &mut scratch,
            interrupt,
        )?;
    }
    if effects.iter().any(|s| s.single_unit_levels > 0) {
        return Err(invalid(format!(
            "PPML bias correction requires at least two distinct bias units per retained FE level; FE1: {}/{} and FE2: {}/{} levels have only one unit. More rows or probes do not repair this support",
            effects[0].single_unit_levels, effects[0].levels,
            effects[1].single_unit_levels, effects[1].levels,
        )));
    }
    Ok(Support { effects, units })
}
fn summarize(
    partition: &Partition,
    h: &[f64],
    units: &Units,
    scratch: &mut [(usize, f64)],
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Summary> {
    let mut counts = filled(partition.n_levels(), 0usize)?;
    let mut equivalents = filled(partition.n_levels(), 0.)?;
    let mut maximum_share = 0_f64;
    for level in 0..partition.n_levels() {
        interrupt()?;
        let members = partition.membership().members(level)?;
        let entries = &mut scratch[..members.len()];
        let mut scale = 0_f64;
        for (entry, &row) in entries.iter_mut().zip(members) {
            if !h[row].is_finite() || h[row] <= 0. {
                return Err(numerical(
                    "bias support information is not positive and finite",
                ));
            }
            *entry = (units.codes[row], h[row]);
            scale = scale.max(h[row]);
        }
        // Sorting information within units fixes accumulation under row permutations.
        // Scale before grouping/squaring; no dense FE-by-unit table is retained.
        entries.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let (mut total, mut squares, mut largest) = (0., 0., 0_f64);
        let mut start = 0;
        while start < entries.len() {
            let mut end = start + 1;
            while end < entries.len() && entries[end].0 == entries[start].0 {
                end += 1;
            }
            let information: f64 = entries[start..end].iter().map(|e| e.1 / scale).sum();
            total += information;
            squares += information * information;
            largest = largest.max(information);
            counts[level] += 1;
            start = end;
        }
        if counts[level] == 0 || !total.is_finite() || !squares.is_finite() || squares <= 0. {
            return Err(numerical("bias support information cannot be summarized"));
        }
        // These bounds only remove rounding outside the defining [1, unit-count] range.
        equivalents[level] = (total * (total / squares)).clamp(1., counts[level] as f64);
        maximum_share = maximum_share.max((largest / total).min(1.));
    }
    counts.sort_unstable();
    equivalents.sort_unstable_by(f64::total_cmp);
    let levels = counts.len();
    let rank = |q: usize| (levels / 4 * q + (levels % 4 * q).div_ceil(4)).saturating_sub(1);
    Ok(Summary {
        levels,
        minimum_units: counts[0],
        p25_units: counts[rank(1)],
        median_units: counts[rank(2)],
        p75_units: counts[rank(3)],
        maximum_units: counts[levels - 1],
        single_unit_levels: counts.iter().filter(|&&n| n == 1).count(),
        minimum_information_units: equivalents[0],
        median_information_units: equivalents[rank(2)],
        maximum_unit_information_share: maximum_share,
    })
}
