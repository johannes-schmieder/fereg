use std::time::Instant;

use crate::data::DenseMatrix;
use crate::error::{allocation, invalid_argument, invalid_data, ErrorCode, FeregError, Result};
use crate::fe::{
    BorrowedInterceptFeDesign, FeCertificate, FeSolverKind, LsmrConfig, LsmrPreconditioner,
    MapConfig, SolveReport, SolveStopReason,
};
use crate::linalg::neumaier_add;
use crate::memory::{estimate_fe_solver_memory, FeSolverMemoryBudget, SolverWorkspacePeak};
use rayon::prelude::*;
use within::{Effect, LsmrOptions, PreconditionerConfig};

use super::map::ReusableMapCapacities;

mod lsmr_norms;
pub(crate) mod three_fe_map;
pub(crate) mod three_fe_schur;

pub(crate) const PARALLEL_PROJECTION_MIN_ROWS: usize = 16_384;
pub(crate) const FUSED_PARALLEL_MAX_RHS: usize = 64;
// Fixed row partitions make the compensated dot reduction independent of the
// installed team width. The 512-row crossover is measured on the wide PPML
// checkpoint shape; smaller chunks add scheduling overhead and larger chunks
// leave the deterministic reduction under-parallelized.
pub(crate) const PARALLEL_DOT_CHUNK_ROWS: usize = 512;
const PARALLEL_GROUP_MAX_RHS: usize = FUSED_PARALLEL_MAX_RHS;
/// Objective weights for the strict borrowed-intercept MAP path.
///
/// Unit weights remain structural: the solver never materializes an
/// observation-scale vector of ones.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BorrowedMapWeights<'a> {
    Unit,
    Provided(&'a [f64]),
}

/// A borrowed-intercept MAP result whose construction certifies the returned
/// residualized values.
#[derive(Debug, PartialEq)]
pub struct CertifiedBorrowedMap {
    residualized: DenseMatrix,
    report: SolveReport,
    coefficients: Option<FeCoefficientBatch>,
    residual_norms: Vec<f64>,
}

/// Optional coefficient-space product of one certified absorption.
///
/// Values are RHS-major. Within each RHS, effects are term-major and channels
/// are channel-major within an effect: `effect_offset + channel * levels + level`.
/// This matches `within::BatchSolveResult::x` and lets prepared OLS combine the
/// already-computed FE solutions after the finite coefficients are known.
#[derive(Debug, PartialEq)]
pub struct FeCoefficientBatch {
    values: Vec<f64>,
    effect_channels: Vec<usize>,
    level_counts: Vec<usize>,
    effect_offsets: Vec<usize>,
    coefficients_per_rhs: usize,
    rhs_count: usize,
}

/// MAP-native coefficient accumulator.
///
/// Parallel group means are produced effect-major, level-major, then RHS-major.
/// Keeping the running coefficients in that same order makes every projection
/// a contiguous add. The public solver product is transposed to its canonical
/// RHS-major layout only once, after MAP has certified the residuals.
struct MapCoefficientAccumulator {
    values: Vec<f64>,
    level_counts: Vec<usize>,
    effect_offsets: Vec<usize>,
    rhs_count: usize,
}

trait InterceptCoefficientRecorder {
    fn rhs_count(&self) -> usize;

    fn add_intercept_level(&mut self, effect: usize, level: usize, means: &[f64]) -> Result<()>;

    fn add_intercept_level_major(
        &mut self,
        effect: usize,
        means: &[f64],
        levels: usize,
    ) -> Result<()>;

    fn level_major_values_mut(
        &mut self,
        effect: usize,
        levels: usize,
    ) -> Result<Option<&mut [f64]>>;
}

impl FeCoefficientBatch {
    pub(crate) fn retained_bytes(&self) -> u128 {
        self.values.capacity() as u128 * std::mem::size_of::<f64>() as u128
            + (self.effect_channels.capacity() as u128
                + self.level_counts.capacity() as u128
                + self.effect_offsets.capacity() as u128)
                * std::mem::size_of::<usize>() as u128
    }

    pub(crate) fn zeros(
        effect_channels: Vec<usize>,
        level_counts: Vec<usize>,
        rhs_count: usize,
    ) -> Result<Self> {
        let coefficients_per_rhs = coefficient_count(&effect_channels, &level_counts)?;
        let elements = coefficients_per_rhs
            .checked_mul(rhs_count)
            .ok_or_else(|| allocation("FE coefficient batch dimensions overflow"))?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(elements)
            .map_err(|_| allocation("unable to reserve FE coefficient batch"))?;
        values.resize(elements, 0.0);
        Self::from_values(effect_channels, level_counts, rhs_count, values)
    }

    pub(crate) fn from_values(
        effect_channels: Vec<usize>,
        level_counts: Vec<usize>,
        rhs_count: usize,
        values: Vec<f64>,
    ) -> Result<Self> {
        if rhs_count == 0 || effect_channels.len() != level_counts.len() {
            return Err(invalid_argument(
                "FE coefficient batch layout dimensions are inconsistent",
            ));
        }
        let coefficients_per_rhs = coefficient_count(&effect_channels, &level_counts)?;
        if values.len() != coefficients_per_rhs.saturating_mul(rhs_count)
            || values.iter().any(|value| !value.is_finite())
        {
            return Err(invalid_data(
                "FE coefficient batch values are not finite and aligned",
            ));
        }
        let mut effect_offsets = Vec::new();
        effect_offsets
            .try_reserve_exact(effect_channels.len() + 1)
            .map_err(|_| allocation("unable to reserve FE coefficient effect offsets"))?;
        effect_offsets.push(0);
        for (&channels, &levels) in effect_channels.iter().zip(&level_counts) {
            let width = channels
                .checked_mul(levels)
                .ok_or_else(|| allocation("FE coefficient effect width overflows"))?;
            effect_offsets.push(
                effect_offsets
                    .last()
                    .copied()
                    .unwrap_or(0_usize)
                    .checked_add(width)
                    .ok_or_else(|| allocation("FE coefficient offset overflows"))?,
            );
        }
        Ok(Self {
            values,
            effect_channels,
            level_counts,
            effect_offsets,
            coefficients_per_rhs,
            rhs_count,
        })
    }

    pub(crate) fn intercept_zeros(
        design: &BorrowedInterceptFeDesign<'_>,
        rhs_count: usize,
    ) -> Result<Self> {
        let mut effect_channels = Vec::new();
        effect_channels
            .try_reserve_exact(design.effects().len())
            .map_err(|_| allocation("unable to reserve intercept FE channel layout"))?;
        effect_channels.resize(design.effects().len(), 1);
        let mut level_counts = Vec::new();
        level_counts
            .try_reserve_exact(design.effects().len())
            .map_err(|_| allocation("unable to reserve intercept FE level layout"))?;
        for effect in design.effects() {
            level_counts.push(effect.n_levels());
        }
        Self::zeros(effect_channels, level_counts, rhs_count)
    }

    pub(crate) fn rhs(&self, rhs: usize) -> Result<&[f64]> {
        if rhs >= self.rhs_count {
            return Err(invalid_argument("FE coefficient RHS is out of range"));
        }
        let start = rhs * self.coefficients_per_rhs;
        Ok(&self.values[start..start + self.coefficients_per_rhs])
    }

    pub(crate) fn add_intercept(
        &mut self,
        effect: usize,
        level: usize,
        rhs: usize,
        value: f64,
    ) -> Result<()> {
        if !value.is_finite()
            || self.effect_channels.get(effect) != Some(&1)
            || level >= self.level_counts.get(effect).copied().unwrap_or(0)
            || rhs >= self.rhs_count
        {
            return Err(invalid_argument(
                "intercept FE coefficient update is not finite and aligned",
            ));
        }
        let index = rhs * self.coefficients_per_rhs + self.effect_offsets[effect] + level;
        self.values[index] += value;
        if !self.values[index].is_finite() {
            return Err(invalid_data("intercept FE coefficient update is nonfinite"));
        }
        Ok(())
    }

    fn add_intercept_level(&mut self, effect: usize, level: usize, means: &[f64]) -> Result<()> {
        if self.effect_channels.get(effect) != Some(&1)
            || level >= self.level_counts.get(effect).copied().unwrap_or(0)
            || means.len() != self.rhs_count
        {
            return Err(invalid_argument(
                "intercept FE level update is not finite and aligned",
            ));
        }
        let effect_offset = self.effect_offsets[effect] + level;
        for (rhs, &mean) in means.iter().enumerate() {
            if !mean.is_finite() {
                return Err(invalid_argument(
                    "intercept FE level update is not finite and aligned",
                ));
            }
            let index = rhs * self.coefficients_per_rhs + effect_offset;
            self.values[index] += mean;
            if !self.values[index].is_finite() {
                return Err(invalid_data("intercept FE coefficient update is nonfinite"));
            }
        }
        Ok(())
    }

    fn add_intercept_level_major(
        &mut self,
        effect: usize,
        means: &[f64],
        levels: usize,
    ) -> Result<()> {
        if self.effect_channels.get(effect) != Some(&1)
            || self.level_counts.get(effect).copied() != Some(levels)
            || means.len() != levels.saturating_mul(self.rhs_count)
        {
            return Err(invalid_argument(
                "intercept FE group means are not finite and aligned",
            ));
        }
        let effect_offset = self.effect_offsets[effect];
        if levels.saturating_mul(self.rhs_count) >= PARALLEL_PROJECTION_MIN_ROWS
            && self.rhs_count > 1
            && rayon::current_num_threads() > 1
        {
            let coefficients_per_rhs = self.coefficients_per_rhs;
            let rhs_count = self.rhs_count;
            self.values
                .par_chunks_mut(coefficients_per_rhs)
                .take(rhs_count)
                .enumerate()
                .try_for_each(|(rhs, rhs_values)| -> Result<()> {
                    let destination = &mut rhs_values[effect_offset..effect_offset + levels];
                    for (level, value) in destination.iter_mut().enumerate() {
                        let mean = means[level * rhs_count + rhs];
                        if !mean.is_finite() {
                            return Err(invalid_argument(
                                "intercept FE group means are not finite and aligned",
                            ));
                        }
                        *value += mean;
                        if !value.is_finite() {
                            return Err(invalid_data(
                                "intercept FE coefficient update is nonfinite",
                            ));
                        }
                    }
                    Ok(())
                })?;
            return Ok(());
        }
        for rhs in 0..self.rhs_count {
            let start = rhs * self.coefficients_per_rhs + effect_offset;
            let destination = &mut self.values[start..start + levels];
            for (level, value) in destination.iter_mut().enumerate() {
                let mean = means[level * self.rhs_count + rhs];
                if !mean.is_finite() {
                    return Err(invalid_argument(
                        "intercept FE group means are not finite and aligned",
                    ));
                }
                *value += mean;
                if !value.is_finite() {
                    return Err(invalid_data("intercept FE coefficient update is nonfinite"));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn add_assign(&mut self, other: &Self, scale: f64) -> Result<()> {
        if !scale.is_finite()
            || self.effect_channels != other.effect_channels
            || self.level_counts != other.level_counts
            || self.rhs_count != other.rhs_count
        {
            return Err(invalid_argument(
                "FE coefficient batches cannot be combined",
            ));
        }
        for (destination, source) in self.values.iter_mut().zip(&other.values) {
            *destination = scale.mul_add(*source, *destination);
            if !destination.is_finite() {
                return Err(invalid_data("combined FE coefficient is nonfinite"));
            }
        }
        Ok(())
    }

    fn try_clone(&self) -> Result<Self> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(self.values.len())
            .map_err(|_| allocation("unable to clone FE coefficient batch"))?;
        values.extend_from_slice(&self.values);
        let mut effect_channels = Vec::new();
        effect_channels
            .try_reserve_exact(self.effect_channels.len())
            .map_err(|_| allocation("unable to clone FE coefficient channel layout"))?;
        effect_channels.extend_from_slice(&self.effect_channels);
        let mut level_counts = Vec::new();
        level_counts
            .try_reserve_exact(self.level_counts.len())
            .map_err(|_| allocation("unable to clone FE coefficient level layout"))?;
        level_counts.extend_from_slice(&self.level_counts);
        Self::from_values(effect_channels, level_counts, self.rhs_count, values)
    }

    fn fill_zero(&mut self) {
        self.values.fill(0.0);
    }

    fn zero_rhs(&mut self, rhs: usize) -> Result<()> {
        let start = rhs
            .checked_mul(self.coefficients_per_rhs)
            .ok_or_else(|| invalid_argument("FE coefficient RHS offset overflows"))?;
        let values = self
            .values
            .get_mut(start..start + self.coefficients_per_rhs)
            .ok_or_else(|| invalid_argument("FE coefficient RHS is out of range"))?;
        values.fill(0.0);
        Ok(())
    }

    fn add_scaled_rhs(&mut self, other: &Self, scales: &[f64], global_scale: f64) -> Result<()> {
        self.validate_binary_layout(other, scales)?;
        if !global_scale.is_finite() {
            return Err(invalid_data("FE coefficient global scale is nonfinite"));
        }
        for (rhs, &scale) in scales.iter().enumerate() {
            if !scale.is_finite() {
                return Err(invalid_data("FE coefficient scale is nonfinite"));
            }
            let scale = global_scale * scale;
            let start = rhs * self.coefficients_per_rhs;
            for coefficient in 0..self.coefficients_per_rhs {
                let index = start + coefficient;
                self.values[index] = scale.mul_add(other.values[index], self.values[index]);
                if !self.values[index].is_finite() {
                    return Err(invalid_data("scaled FE coefficient update is nonfinite"));
                }
            }
        }
        Ok(())
    }

    fn assign_sum_scaled_rhs(&mut self, residual: &Self, scales: &[f64]) -> Result<()> {
        self.validate_binary_layout(residual, scales)?;
        for (rhs, &scale) in scales.iter().enumerate() {
            if !scale.is_finite() {
                return Err(invalid_data("FE coefficient scale is nonfinite"));
            }
            let start = rhs * self.coefficients_per_rhs;
            for coefficient in 0..self.coefficients_per_rhs {
                let index = start + coefficient;
                self.values[index] = scale.mul_add(self.values[index], residual.values[index]);
                if !self.values[index].is_finite() {
                    return Err(invalid_data("CG FE direction update is nonfinite"));
                }
            }
        }
        Ok(())
    }

    fn validate_binary_layout(&self, other: &Self, scales: &[f64]) -> Result<()> {
        if !self.same_layout(other) || scales.len() != self.rhs_count {
            return Err(invalid_argument(
                "FE coefficient recurrence layouts are inconsistent",
            ));
        }
        Ok(())
    }

    fn same_layout(&self, other: &Self) -> bool {
        self.effect_channels == other.effect_channels
            && self.level_counts == other.level_counts
            && self.rhs_count == other.rhs_count
    }

    fn extrapolate_irons_tuck(x0: &Self, x1: &Self, x2: &Self, alpha: f64) -> Result<Self> {
        if !x0.same_layout(x1) || !x0.same_layout(x2) {
            return Err(invalid_argument(
                "Irons-Tuck FE coefficient layouts are inconsistent",
            ));
        }
        if !alpha.is_finite() {
            return Err(invalid_data("Irons-Tuck FE coefficient scale is nonfinite"));
        }
        let mut candidate = x0.try_clone()?;
        for index in 0..candidate.values.len() {
            let a = x0.values[index];
            let b = x1.values[index];
            let c = x2.values[index];
            let d1 = b - a;
            let d2 = c - 2.0 * b + a;
            candidate.values[index] = alpha.mul_add(2.0 * d1, a).mul_add(1.0, alpha * alpha * d2);
            if !candidate.values[index].is_finite() {
                return Err(invalid_data(
                    "Irons-Tuck FE coefficient extrapolation is nonfinite",
                ));
            }
        }
        Ok(candidate)
    }

    pub(crate) fn effect_channels(&self) -> &[usize] {
        &self.effect_channels
    }

    pub(crate) fn level_counts(&self) -> &[usize] {
        &self.level_counts
    }

    #[cfg(test)]
    pub(crate) fn effect_offsets(&self) -> &[usize] {
        &self.effect_offsets
    }

    pub(crate) const fn coefficients_per_rhs(&self) -> usize {
        self.coefficients_per_rhs
    }

    pub(crate) const fn rhs_count(&self) -> usize {
        self.rhs_count
    }
}

impl InterceptCoefficientRecorder for FeCoefficientBatch {
    fn rhs_count(&self) -> usize {
        FeCoefficientBatch::rhs_count(self)
    }

    fn add_intercept_level(&mut self, effect: usize, level: usize, means: &[f64]) -> Result<()> {
        FeCoefficientBatch::add_intercept_level(self, effect, level, means)
    }

    fn add_intercept_level_major(
        &mut self,
        effect: usize,
        means: &[f64],
        levels: usize,
    ) -> Result<()> {
        FeCoefficientBatch::add_intercept_level_major(self, effect, means, levels)
    }

    fn level_major_values_mut(
        &mut self,
        _effect: usize,
        _levels: usize,
    ) -> Result<Option<&mut [f64]>> {
        Ok(None)
    }
}

impl MapCoefficientAccumulator {
    fn zeros(design: &BorrowedInterceptFeDesign<'_>, rhs_count: usize) -> Result<Self> {
        if rhs_count == 0 {
            return Err(invalid_argument(
                "MAP coefficient accumulator requires at least one RHS",
            ));
        }
        let mut level_counts = Vec::new();
        level_counts
            .try_reserve_exact(design.effects().len())
            .map_err(|_| allocation("unable to reserve MAP coefficient level layout"))?;
        let mut effect_offsets = Vec::new();
        effect_offsets
            .try_reserve_exact(design.effects().len() + 1)
            .map_err(|_| allocation("unable to reserve MAP coefficient effect offsets"))?;
        effect_offsets.push(0);
        for effect in design.effects() {
            let levels = effect.n_levels();
            level_counts.push(levels);
            let width = levels
                .checked_mul(rhs_count)
                .ok_or_else(|| allocation("MAP coefficient effect width overflows"))?;
            effect_offsets.push(
                effect_offsets
                    .last()
                    .copied()
                    .unwrap_or(0_usize)
                    .checked_add(width)
                    .ok_or_else(|| allocation("MAP coefficient offset overflows"))?,
            );
        }
        let elements = effect_offsets.last().copied().unwrap_or(0);
        let mut values = Vec::new();
        values
            .try_reserve_exact(elements)
            .map_err(|_| allocation("unable to reserve MAP coefficient accumulator"))?;
        values.resize(elements, 0.0);
        Ok(Self {
            values,
            level_counts,
            effect_offsets,
            rhs_count,
        })
    }

    fn into_batch(self) -> Result<(FeCoefficientBatch, u128)> {
        let Self {
            values,
            level_counts,
            effect_offsets,
            rhs_count,
        } = self;
        let coefficients_per_rhs = level_counts.iter().try_fold(0_usize, |total, &levels| {
            total
                .checked_add(levels)
                .ok_or_else(|| allocation("MAP coefficient count overflows"))
        })?;
        if coefficients_per_rhs == 0 {
            return Err(invalid_data(
                "MAP coefficient accumulator has no effect coefficients",
            ));
        }
        let elements = coefficients_per_rhs
            .checked_mul(rhs_count)
            .ok_or_else(|| allocation("MAP coefficient batch dimensions overflow"))?;
        if values.len() != elements {
            return Err(invalid_argument(
                "MAP coefficient accumulator dimensions are inconsistent",
            ));
        }
        let mut rhs_major = Vec::new();
        rhs_major
            .try_reserve_exact(elements)
            .map_err(|_| allocation("unable to reserve canonical MAP coefficient batch"))?;
        rhs_major.resize(elements, 0.0);
        rhs_major
            .par_chunks_mut(coefficients_per_rhs)
            .enumerate()
            .try_for_each(|(rhs, destination)| -> Result<()> {
                let mut canonical_effect_offset = 0_usize;
                for (effect, &levels) in level_counts.iter().enumerate() {
                    let source_offset = effect_offsets[effect];
                    for level in 0..levels {
                        let value = values[source_offset + level * rhs_count + rhs];
                        if !value.is_finite() {
                            return Err(invalid_data(
                                "MAP coefficient accumulator produced a nonfinite value",
                            ));
                        }
                        destination[canonical_effect_offset + level] = value;
                    }
                    canonical_effect_offset += levels;
                }
                Ok(())
            })?;
        let mut effect_channels = Vec::new();
        effect_channels
            .try_reserve_exact(level_counts.len())
            .map_err(|_| allocation("unable to reserve canonical MAP coefficient channels"))?;
        effect_channels.resize(level_counts.len(), 1);
        let batch =
            FeCoefficientBatch::from_values(effect_channels, level_counts, rhs_count, rhs_major)?;
        // The level layout moves into the result, while the original value and
        // offset banks coexist with the canonical RHS-major batch.
        let conversion_bytes = values.capacity() as u128 * std::mem::size_of::<f64>() as u128
            + effect_offsets.capacity() as u128 * std::mem::size_of::<usize>() as u128
            + batch.retained_bytes();
        Ok((batch, conversion_bytes))
    }
}

impl InterceptCoefficientRecorder for MapCoefficientAccumulator {
    fn rhs_count(&self) -> usize {
        self.rhs_count
    }

    fn add_intercept_level(&mut self, effect: usize, level: usize, means: &[f64]) -> Result<()> {
        if means.len() != self.rhs_count
            || self
                .level_counts
                .get(effect)
                .is_none_or(|&levels| level >= levels)
        {
            return Err(invalid_argument(
                "MAP coefficient level update is not aligned",
            ));
        }
        let start = self.effect_offsets[effect] + level * self.rhs_count;
        for (destination, &mean) in self.values[start..start + self.rhs_count]
            .iter_mut()
            .zip(means)
        {
            if !mean.is_finite() {
                return Err(invalid_argument(
                    "MAP coefficient level update is not finite",
                ));
            }
            *destination += mean;
            if !destination.is_finite() {
                return Err(invalid_data(
                    "MAP coefficient level accumulation is nonfinite",
                ));
            }
        }
        Ok(())
    }

    fn add_intercept_level_major(
        &mut self,
        effect: usize,
        means: &[f64],
        levels: usize,
    ) -> Result<()> {
        if self.level_counts.get(effect).copied() != Some(levels)
            || means.len() != levels.saturating_mul(self.rhs_count)
        {
            return Err(invalid_argument(
                "MAP coefficient group means are not aligned",
            ));
        }
        let start = self.effect_offsets[effect];
        let destination = &mut self.values[start..start + means.len()];
        // This is a contiguous, cache-local update whose work is much smaller
        // than the observation projection. Keeping it on the caller thread
        // avoids launching a second Rayon job after every group-mean pass.
        for (destination, &mean) in destination.iter_mut().zip(means) {
            if !mean.is_finite() {
                return Err(invalid_argument("MAP coefficient group mean is not finite"));
            }
            *destination += mean;
            if !destination.is_finite() {
                return Err(invalid_data(
                    "MAP coefficient group accumulation is nonfinite",
                ));
            }
        }
        Ok(())
    }

    fn level_major_values_mut(
        &mut self,
        effect: usize,
        levels: usize,
    ) -> Result<Option<&mut [f64]>> {
        if self.level_counts.get(effect).copied() != Some(levels) {
            return Err(invalid_argument(
                "MAP coefficient level-major block is not aligned",
            ));
        }
        let elements = levels
            .checked_mul(self.rhs_count)
            .ok_or_else(|| allocation("MAP coefficient level-major block overflows"))?;
        let start = self.effect_offsets[effect];
        Ok(Some(&mut self.values[start..start + elements]))
    }
}

fn coefficient_count(effect_channels: &[usize], level_counts: &[usize]) -> Result<usize> {
    effect_channels
        .iter()
        .zip(level_counts)
        .try_fold(0_usize, |total, (&channels, &levels)| {
            if channels == 0 || levels == 0 {
                return Err(invalid_argument(
                    "FE coefficient effects require positive channel and level counts",
                ));
            }
            total
                .checked_add(
                    channels
                        .checked_mul(levels)
                        .ok_or_else(|| allocation("FE coefficient effect width overflows"))?,
                )
                .ok_or_else(|| allocation("FE coefficient count overflows"))
        })
}

impl CertifiedBorrowedMap {
    // These norms belong to this exact residualized matrix and normalized weights.
    // Moving them to OLS avoids a second O(NK) pass after fresh certification.
    pub(crate) fn take_residual_norms(&mut self) -> Vec<f64> {
        std::mem::take(&mut self.residual_norms)
    }
    pub(crate) fn with_residual_norms(mut self, norms: Vec<f64>) -> Result<Self> {
        if norms.len() != self.residualized.ncols()
            || norms.iter().any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err(invalid_argument(
                "certified residual norms are not finite and aligned",
            ));
        }
        self.residual_norms = norms;
        Ok(self)
    }

    #[must_use]
    pub const fn residualized(&self) -> &DenseMatrix {
        &self.residualized
    }

    #[must_use]
    pub const fn report(&self) -> &SolveReport {
        &self.report
    }

    pub(crate) fn set_solver_workspace_peak(&mut self, peak: crate::memory::SolverWorkspacePeak) {
        self.report.solver_workspace_peak = peak;
    }

    #[must_use]
    pub fn into_parts(self) -> (DenseMatrix, SolveReport) {
        (self.residualized, self.report)
    }

    pub(crate) fn into_full_parts(self) -> (DenseMatrix, SolveReport, Option<FeCoefficientBatch>) {
        (self.residualized, self.report, self.coefficients)
    }

    pub(crate) fn from_certified_parts(
        residualized: DenseMatrix,
        report: SolveReport,
    ) -> Result<Self> {
        if !report.certified() {
            return Err(invalid_data(
                "borrowed MAP attempted to construct an uncertified result",
            ));
        }
        Ok(Self {
            residualized,
            report,
            coefficients: None,
            residual_norms: Vec::new(),
        })
    }

    pub(crate) fn from_certified_parts_with_coefficients(
        residualized: DenseMatrix,
        report: SolveReport,
        coefficients: FeCoefficientBatch,
    ) -> Result<Self> {
        if !report.certified()
            || coefficients.rhs_count() != residualized.ncols()
            || coefficients.coefficients_per_rhs() == 0
        {
            return Err(invalid_data(
                "borrowed absorption coefficient product is not certified and aligned",
            ));
        }
        Ok(Self {
            residualized,
            report,
            coefficients: Some(coefficients),
            residual_norms: Vec::new(),
        })
    }
}

trait WorkspaceReservation {
    fn reserve_f64(
        &mut self,
        target: &mut Vec<f64>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemWorkspaceReservation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CgRefinementPolicy {
    minimum_iterations: usize,
}

impl CgRefinementPolicy {
    const ORDINARY: Self = Self {
        minimum_iterations: 0,
    };

    #[cfg(test)]
    const fn forced(minimum_iterations: usize) -> Self {
        Self { minimum_iterations }
    }
}

#[derive(Debug, Clone, Copy)]
struct CgContinuation<'a> {
    reference_norms: Option<&'a [f64]>,
    refinement: CgRefinementPolicy,
    retain_coefficients: bool,
    stable_projection: bool,
    reconstruction_tolerance: Option<f64>,
}

impl CgContinuation<'_> {
    const ORDINARY: Self = Self {
        reference_norms: None,
        refinement: CgRefinementPolicy::ORDINARY,
        retain_coefficients: false,
        stable_projection: false,
        reconstruction_tolerance: None,
    };
}

impl WorkspaceReservation for SystemWorkspaceReservation {
    fn reserve_f64(
        &mut self,
        target: &mut Vec<f64>,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(additional)
            .map_err(|_| allocation(label))
    }
}

/// Complete owned one-RHS CG recovery frame: its input, three recurrence
/// matrices, four coefficient banks, MAP banks and eight recurrence scalars.
/// The prepared topology, target and enclosing recovery views belong to the
/// caller. This shares the allocation planner used by the actual MAP banks.
pub(crate) fn borrowed_cg_recovery_workspace_bytes(
    observations: usize,
    effects: usize,
    total_levels: usize,
    maximum_levels: usize,
    threads: usize,
) -> Result<u128> {
    let counts = BorrowedMapWorkspace::planned_counts(maximum_levels, observations, 1, threads)?;
    let numeric = (15 + counts.into_iter().map(|count| count as u128).sum::<u128>()) * 8;
    let coefficient =
        total_levels as u128 * 8 + (effects as u128 * 3 + 1) * std::mem::size_of::<usize>() as u128;
    // These are linear expressions in usize axes with fixed coefficients;
    // their sum is representable in u128 on every supported 64-bit target.
    Ok(observations as u128 * 32 + coefficient * 4 + numeric)
}

struct BorrowedMapWorkspace {
    reference_norms: Vec<f64>,
    residual_norms: Vec<f64>,
    rhs_scales: Vec<f64>,
    rhs_relative_infinity_norms: Vec<f64>,
    value_scales: Vec<f64>,
    group_sums: Vec<f64>,
    group_corrections: Vec<f64>,
    parallel_group_means: Vec<f64>,
    parallel_group_weights: Vec<f64>,
    parallel_dot_sums: Vec<f64>,
}

impl BorrowedMapWorkspace {
    // Every MAP numeric bank is reserved at construction and never grows
    // during projection/certification. Count capacities rather than lengths
    // so moved norms and subsequent narrow RHS views do not shrink the peak.
    fn payload_peak(&self) -> Result<SolverWorkspacePeak> {
        let elements = [
            &self.reference_norms,
            &self.residual_norms,
            &self.rhs_scales,
            &self.rhs_relative_infinity_norms,
            &self.value_scales,
            &self.group_sums,
            &self.group_corrections,
            &self.parallel_group_means,
            &self.parallel_group_weights,
            &self.parallel_dot_sums,
        ]
        .iter()
        .map(|bank| bank.capacity() as u128)
        .sum::<u128>();
        SolverWorkspacePeak::from_bytes(elements * std::mem::size_of::<f64>() as u128)
    }

    fn planned_counts(
        maximum_levels: usize,
        observation_count: usize,
        rhs_count: usize,
        parallel_threads: usize,
    ) -> Result<[usize; 3]> {
        let counts = super::categorical_parallel_counts(
            maximum_levels as u128,
            observation_count as u128,
            rhs_count as u128,
            parallel_threads as u128,
        )?;
        let mut concrete = [0; 3];
        for (target, count) in concrete.iter_mut().zip(counts) {
            *target = usize::try_from(count)
                .map_err(|_| allocation("borrowed MAP parallel dimensions overflow"))?;
        }
        Ok(concrete)
    }

    fn planned_payload_bytes(
        design: &BorrowedInterceptFeDesign<'_>,
        observations: usize,
        rhs: usize,
        threads: usize,
    ) -> Result<u128> {
        let maximum_levels = design
            .effects()
            .iter()
            .map(crate::fe::BorrowedInterceptEffect::n_levels)
            .max()
            .unwrap_or(0);
        let counts = Self::planned_counts(maximum_levels, observations, rhs, threads)?;
        let cells = counts
            .into_iter()
            .try_fold((rhs as u128) * 7, |sum, count| {
                sum.checked_add(count as u128)
                    .ok_or_else(|| allocation("MAP planned capacities overflow"))
            })?;
        cells
            .checked_mul(8)
            .ok_or_else(|| allocation("MAP planned byte count overflows"))
    }

    fn new<R: WorkspaceReservation>(
        design: &BorrowedInterceptFeDesign<'_>,
        observation_count: usize,
        rhs_count: usize,
        reservation: &mut R,
    ) -> Result<Self> {
        Self::new_with_parallelism(
            design,
            observation_count,
            rhs_count,
            rayon::current_num_threads(),
            reservation,
        )
    }

    fn new_with_parallelism<R: WorkspaceReservation>(
        design: &BorrowedInterceptFeDesign<'_>,
        observation_count: usize,
        rhs_count: usize,
        parallel_threads: usize,
        reservation: &mut R,
    ) -> Result<Self> {
        let maximum_levels = design
            .effects()
            .iter()
            .map(crate::fe::BorrowedInterceptEffect::n_levels)
            .max()
            .unwrap_or(0);
        let [parallel_group_levels, parallel_group_mean_count, parallel_dot_sum_count] =
            Self::planned_counts(
                maximum_levels,
                observation_count,
                rhs_count,
                parallel_threads,
            )?;
        Ok(Self {
            reference_norms: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP reference norms",
            )?,
            residual_norms: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP residual norms",
            )?,
            rhs_scales: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP RHS scales",
            )?,
            rhs_relative_infinity_norms: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP per-RHS certificate norms",
            )?,
            value_scales: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP value scales",
            )?,
            group_sums: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP group sums",
            )?,
            group_corrections: reserved_zeros(
                rhs_count,
                reservation,
                "unable to reserve borrowed MAP group corrections",
            )?,
            parallel_group_means: if parallel_group_mean_count == 0 {
                Vec::new()
            } else {
                reserved_zeros(
                    parallel_group_mean_count,
                    reservation,
                    "unable to reserve borrowed MAP parallel group means",
                )?
            },
            parallel_group_weights: if parallel_group_levels == 0 {
                Vec::new()
            } else {
                reserved_zeros(
                    parallel_group_levels,
                    reservation,
                    "unable to reserve borrowed MAP parallel group weights",
                )?
            },
            parallel_dot_sums: if parallel_dot_sum_count == 0 {
                Vec::new()
            } else {
                reserved_zeros(
                    parallel_dot_sum_count,
                    reservation,
                    "unable to reserve borrowed CG parallel dot sums",
                )?
            },
        })
    }

    fn set_rhs_count(&mut self, rhs_count: usize) -> Result<()> {
        for values in [
            &mut self.reference_norms,
            &mut self.residual_norms,
            &mut self.rhs_scales,
            &mut self.rhs_relative_infinity_norms,
            &mut self.value_scales,
            &mut self.group_sums,
            &mut self.group_corrections,
        ] {
            if rhs_count > values.capacity() {
                return Err(invalid_argument(
                    "reusable categorical MAP RHS workspaces exceed their fixed capacity",
                ));
            }
            values.resize(rhs_count, 0.0);
        }
        Ok(())
    }
}

/// PPML-owned reusable categorical MAP state.
///
/// Unlike the generic reusable projector, this session borrows finalized
/// intercept-only codes and membership and therefore reduces every local
/// projection to weighted group means.  Its RHS-sized and parallel buffers
/// are allocated once at construction and resized only within those fixed
/// capacities on later changing-weight solves.
pub(crate) struct ReusableCategoricalMapAbsorber<'a> {
    design: BorrowedInterceptFeDesign<'a>,
    workspace: BorrowedMapWorkspace,
    two_fe_pcg: Option<TwoFePcgWorkspace>,
    cg_residual_square: Vec<f64>,
    cg_initial_residual_square: Vec<f64>,
    cg_next_residual_square: Vec<f64>,
    cg_direction_projection: Vec<f64>,
    cg_ratio: Vec<f64>,
    max_rhs: usize,
    max_level_members: usize,
    pub(crate) retain_coefficients: bool,
    checkpoint_lsmr_enabled: bool,
    checkpoint_deterministic: bool,
    checkpoint_component_scaling: bool,
    // Developer fault plan. Shipped code runs the same checks with every
    // entry unset; only the test setters below write them. Each entry is
    // consumed once.
    checkpoint_lsmr_certificate_failure: bool,
    checkpoint_lsmr_fatal_failure: u8,
    checkpoint_lsmr_retry_error: Option<ErrorCode>,
    checkpoint_schur_iteration_cap: Option<usize>,
    checkpoint_schur_certificate_failure: bool,
}

// Multiplying every weight in one disconnected FE component by the same
// positive scalar leaves its projection unchanged. This keeps low-mass
// components visible to the Krylov recurrence; final acceptance uses raw weights.
fn component_scaled_checkpoint_weights(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
) -> Result<Option<Vec<f64>>> {
    let mut codes = [&[][..]; 3];
    let mut levels = [0; 3];
    for (i, effect) in design.effects().iter().enumerate() {
        codes[i] = effect.codes();
        levels[i] = effect.n_levels();
    }
    let components = super::connected_components_all(
        &codes[..design.effects().len()],
        &levels[..design.effects().len()],
    )?;
    if components.n_components <= 1 {
        return Ok(None);
    }
    let mut reservation = SystemWorkspaceReservation;
    let mut maxima = reserved_zeros(
        components.n_components,
        &mut reservation,
        "unable to reserve component weight scales",
    )?;
    let component = |row: usize| components.component_by_effect_level[0][codes[0][row] as usize];
    for row in 0..design.nobs() {
        maxima[component(row)] = maxima[component(row)].max(weights.values.map_or(1., |w| w[row]));
    }
    let mut scaled = reserved_zeros(
        design.nobs(),
        &mut reservation,
        "unable to reserve component-scaled checkpoint weights",
    )?;
    for (row, value) in scaled.iter_mut().enumerate() {
        *value = weights.values.map_or(1., |w| w[row]) / maxima[component(row)];
    }
    Ok(Some(scaled))
}

/// Reusable level-space PCG state for two-way categorical graphs.
///
/// Eliminating the larger effect turns two-way demeaning into a weighted
/// Schur-complement system on the smaller effect. This is still an
/// acceleration of the requested MAP projection: the published result is
/// reconstructed in observation space and must pass the ordinary MAP FE
/// certificate before it can leave the solver.
struct TwoFePcgWorkspace {
    cmg_schur: Option<Box<super::cmg_schur::CmgSchurPreconditioner>>,
    left_effect: usize,
    right_effect: usize,
    component_roots: Vec<usize>,
    component_scales: Vec<f64>,
    ppml_operator_rhs_count: u128,
    left_weights: Vec<f64>,
    right_weights: Vec<f64>,
    left_rhs: Vec<f64>,
    left_work: Vec<f64>,
    solution: Vec<f64>,
    residual: Vec<f64>,
    direction: Vec<f64>,
    operator: Vec<f64>,
}

impl TwoFePcgWorkspace {
    fn new(
        design: &BorrowedInterceptFeDesign<'_>,
        max_rhs: usize,
        reservation: &mut impl WorkspaceReservation,
    ) -> Result<Self> {
        let left_effect =
            usize::from(design.effects()[1].n_levels() > design.effects()[0].n_levels());
        let right_effect = 1 - left_effect;
        let left_levels = design.effects()[left_effect].n_levels();
        let right_levels = design.effects()[right_effect].n_levels();
        let plan = super::workspace_capacity::TwoFeCapacity::new(
            left_levels as u128,
            right_levels as u128,
            max_rhs as u128,
        )?;
        let left_cells = usize::try_from(plan.left)
            .map_err(|_| allocation("two-FE PCG left workspace dimensions overflow"))?;
        let right_cells = usize::try_from(plan.right)
            .map_err(|_| allocation("two-FE PCG right workspace dimensions overflow"))?;
        Ok(Self {
            cmg_schur: None,
            left_effect,
            right_effect,
            component_roots: Vec::new(),
            component_scales: Vec::new(),
            ppml_operator_rhs_count: 0,
            left_weights: reserved_zeros(
                left_levels,
                reservation,
                "unable to reserve two-FE PCG left weights",
            )?,
            right_weights: reserved_zeros(
                right_levels,
                reservation,
                "unable to reserve two-FE PCG right weights",
            )?,
            left_rhs: reserved_zeros(
                left_cells,
                reservation,
                "unable to reserve two-FE PCG left RHS",
            )?,
            left_work: reserved_zeros(
                left_cells,
                reservation,
                "unable to reserve two-FE PCG left work",
            )?,
            solution: reserved_zeros(
                right_cells,
                reservation,
                "unable to reserve two-FE PCG solution",
            )?,
            residual: reserved_zeros(
                right_cells,
                reservation,
                "unable to reserve two-FE PCG residual",
            )?,
            direction: reserved_zeros(
                right_cells,
                reservation,
                "unable to reserve two-FE PCG direction",
            )?,
            operator: reserved_zeros(
                right_cells,
                reservation,
                "unable to reserve two-FE PCG operator product",
            )?,
        })
    }

    fn prepare_component_topology(&mut self, design: &BorrowedInterceptFeDesign<'_>) -> Result<()> {
        fn root(parents: &mut [usize], mut node: usize) -> usize {
            while parents[node] != node {
                parents[node] = parents[parents[node]];
                node = parents[node];
            }
            node
        }
        let count = self.right_weights.len();
        let mut parents = Vec::new();
        parents
            .try_reserve_exact(count)
            .map_err(|_| allocation("unable to reserve two-FE component topology"))?;
        parents.extend(0..count);
        // Operator storage has no numerical authority during construction.
        // Union ranks need at most log2(levels), exactly representable in f64.
        let ranks = &mut self.operator[..count];
        ranks.fill(0.0);
        let left = &design.effects()[self.left_effect];
        let codes = design.effects()[self.right_effect].codes();
        for level in 0..left.n_levels() {
            let members = left.level_index().checked_members(level)?;
            if let Some((&first, rest)) = members.split_first() {
                for &row in rest {
                    let mut a = root(&mut parents, codes[first] as usize);
                    let mut b = root(&mut parents, codes[row] as usize);
                    if a == b {
                        continue;
                    }
                    if ranks[a] < ranks[b] {
                        std::mem::swap(&mut a, &mut b);
                    }
                    parents[b] = a;
                    if ranks[a] == ranks[b] {
                        ranks[a] += 1.0;
                    }
                }
            }
        }
        let mut components = 0;
        for level in 0..count {
            parents[level] = root(&mut parents, level);
            components += usize::from(parents[level] == level);
        }
        if components > 1 {
            self.component_scales = reserved_zeros(
                count,
                &mut SystemWorkspaceReservation,
                "unable to reserve two-FE component scales",
            )?;
            self.component_roots = parents;
        }
        Ok(())
    }

    fn refresh_component_scales(&mut self) {
        let maxima = &mut self.operator[..self.right_weights.len()];
        maxima.fill(0.0);
        for (&root, &weight) in self.component_roots.iter().zip(&self.right_weights) {
            maxima[root] = maxima[root].max(weight);
        }
        for (scale, &root) in self.component_scales.iter_mut().zip(&self.component_roots) {
            *scale = maxima[root];
        }
    }

    fn capacity_elements(&self) -> usize {
        self.left_weights
            .capacity()
            .saturating_add(self.right_weights.capacity())
            .saturating_add(self.left_rhs.capacity())
            .saturating_add(self.left_work.capacity())
            .saturating_add(self.solution.capacity())
            .saturating_add(self.residual.capacity())
            .saturating_add(self.direction.capacity())
            .saturating_add(self.operator.capacity())
            .saturating_add(self.component_roots.capacity())
            .saturating_add(self.component_scales.capacity())
    }
}

/// The PCG group banks have no numerical authority before its current-weight
/// initialization. Lend them to initial certification and MAP pilot work, then
/// restore ownership before any recurrence, error return or unwinding.
struct PcgPilotGroupScratch<'a> {
    workspace: &'a mut BorrowedMapWorkspace,
    graph: &'a mut TwoFePcgWorkspace,
}

impl<'a> PcgPilotGroupScratch<'a> {
    fn new(workspace: &'a mut BorrowedMapWorkspace, graph: &'a mut TwoFePcgWorkspace) -> Self {
        std::mem::swap(&mut workspace.parallel_group_means, &mut graph.left_work);
        std::mem::swap(
            &mut workspace.parallel_group_weights,
            &mut graph.left_weights,
        );
        Self { workspace, graph }
    }
}

impl Drop for PcgPilotGroupScratch<'_> {
    fn drop(&mut self) {
        std::mem::swap(
            &mut self.workspace.parallel_group_means,
            &mut self.graph.left_work,
        );
        std::mem::swap(
            &mut self.workspace.parallel_group_weights,
            &mut self.graph.left_weights,
        );
    }
}

impl<'a> ReusableCategoricalMapAbsorber<'a> {
    /// Attach a fixed-weight OLS preconditioner after admission of the base owner.
    pub(crate) fn enable_cmg_schur(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        degree: usize,
        threads: usize,
        budget: Option<FeSolverMemoryBudget>,
    ) -> Result<crate::memory::SolverWorkspacePeak> {
        let graph = self.two_fe_pcg.as_mut().ok_or_else(|| {
            FeregError::new(
                ErrorCode::Unsupported,
                "cmgschur requires exactly two intercept effects",
            )
        })?;
        let preconditioner = super::cmg_schur::CmgSchurPreconditioner::build(
            &self.design,
            weights,
            graph.left_effect,
            degree,
            threads,
            budget,
        )?;
        let peak = preconditioner.construction_peak();
        graph.cmg_schur = Some(Box::new(preconditioner));
        Ok(peak)
    }

    pub(crate) fn cmg_schur_bytes(&self) -> Result<u128> {
        self.two_fe_pcg
            .as_ref()
            .and_then(|g| g.cmg_schur.as_ref())
            .map_or(Ok(0), |p| p.retained_bytes())
    }

    /// Completed two-pass Schur products, in observation/RHS multiply-adds.
    /// This is a routing observation, never weighted numerical authority.
    pub(crate) fn ppml_operator_work(&self) -> Option<u128> {
        let graph = self.two_fe_pcg.as_ref()?;
        graph
            .ppml_operator_rhs_count
            .checked_mul(self.design.nobs() as u128)?
            .checked_mul(2)
    }

    pub(crate) fn owner_bytes(
        design: &BorrowedInterceptFeDesign<'_>,
        rhs: usize,
        threads: usize,
    ) -> Result<u128> {
        let levels = design.effects().iter().map(|e| e.n_levels() as u128).sum();
        let maximum = design
            .effects()
            .iter()
            .map(|e| e.n_levels() as u128)
            .max()
            .unwrap_or(0);
        let accelerator = super::CategoricalAcceleratorCapacity::new(
            design.effects().len(),
            true,
            design.nobs() as u128,
            rhs as u128,
            levels,
            maximum,
        )?
        .cells(rhs as u128)?;
        BorrowedMapWorkspace::planned_payload_bytes(design, design.nobs(), rhs, threads)?
            .checked_add(
                accelerator
                    .checked_mul(8)
                    .ok_or_else(|| allocation("FE graph capacity overflows"))?,
            )
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>() as u128))
            .and_then(|n| n.checked_add(design.descriptor_bytes()))
            .ok_or_else(|| allocation("FE owner overlap overflows"))
    }

    pub(crate) fn new(
        design: BorrowedInterceptFeDesign<'a>,
        max_rhs: usize,
        parallel_threads: usize,
    ) -> Result<Self> {
        if max_rhs == 0 || parallel_threads == 0 {
            return Err(invalid_argument(
                "reusable categorical MAP dimensions must be positive",
            ));
        }
        if !matches!(design.effects().len(), 1..=3) {
            return Err(invalid_argument(
                "reusable categorical MAP requires one to three effects",
            ));
        }
        let max_level_members = design
            .effects()
            .iter()
            .flat_map(|effect| {
                (0..effect.n_levels()).map(|level| effect.level_index().members(level).len())
            })
            .max()
            .unwrap_or(1)
            .max(1);
        let mut reservation = SystemWorkspaceReservation;
        let workspace = BorrowedMapWorkspace::new_with_parallelism(
            &design,
            design.nobs(),
            max_rhs,
            parallel_threads,
            &mut reservation,
        )?;
        let accelerator = super::CategoricalAcceleratorCapacity::new(
            design.effects().len(),
            true,
            design.nobs() as u128,
            max_rhs as u128,
            design.effects().iter().map(|e| e.n_levels() as u128).sum(),
            design
                .effects()
                .iter()
                .map(|e| e.n_levels() as u128)
                .max()
                .unwrap_or(0),
        )?;
        let mut two_fe_pcg =
            if matches!(accelerator, super::CategoricalAcceleratorCapacity::TwoFe(_)) {
                Some(TwoFePcgWorkspace::new(&design, max_rhs, &mut reservation)?)
            } else {
                None
            };
        if let Some(graph) = two_fe_pcg.as_mut() {
            graph.prepare_component_topology(&design)?;
        }
        let cg_scalar_rhs = usize::from(!matches!(
            accelerator,
            super::CategoricalAcceleratorCapacity::None
        )) * max_rhs;
        let owner = Self {
            design,
            workspace,
            two_fe_pcg,
            cg_residual_square: reserved_zeros(
                cg_scalar_rhs,
                &mut reservation,
                "unable to reserve reusable categorical CG residual norms",
            )?,
            cg_initial_residual_square: reserved_zeros(
                cg_scalar_rhs,
                &mut reservation,
                "unable to reserve reusable categorical CG initial norms",
            )?,
            cg_next_residual_square: reserved_zeros(
                cg_scalar_rhs,
                &mut reservation,
                "unable to reserve reusable categorical CG updated norms",
            )?,
            cg_direction_projection: reserved_zeros(
                cg_scalar_rhs,
                &mut reservation,
                "unable to reserve reusable categorical CG direction products",
            )?,
            cg_ratio: reserved_zeros(
                cg_scalar_rhs,
                &mut reservation,
                "unable to reserve reusable categorical CG recurrence ratios",
            )?,
            max_rhs,
            max_level_members,
            retain_coefficients: false,
            checkpoint_lsmr_enabled: false,
            checkpoint_deterministic: false,
            checkpoint_component_scaling: false,
            checkpoint_lsmr_certificate_failure: false,
            checkpoint_lsmr_fatal_failure: 0,
            checkpoint_lsmr_retry_error: None,
            checkpoint_schur_iteration_cap: None,
            checkpoint_schur_certificate_failure: false,
        };
        let admitted = Self::owner_bytes(&owner.design, max_rhs, parallel_threads)?;
        crate::memory::require_admitted_capacity(owner.payload_bytes()?, admitted)?;
        Ok(owner)
    }

    pub(crate) fn enable_checkpoint_lsmr(&mut self, deterministic: bool) {
        self.checkpoint_lsmr_enabled = true;
        self.checkpoint_deterministic = deterministic;
    }

    pub(crate) fn enable_component_checkpoint_scaling(&mut self) {
        self.checkpoint_component_scaling = true;
    }

    #[cfg(test)]
    pub(crate) fn inject_checkpoint_lsmr_certificate_failure(&mut self) {
        self.checkpoint_lsmr_certificate_failure = true;
    }

    #[cfg(test)]
    pub(crate) fn inject_checkpoint_lsmr_fatal_failure(&mut self, code: ErrorCode) {
        self.checkpoint_lsmr_fatal_failure = match code {
            ErrorCode::Allocation => 1,
            ErrorCode::Interrupted => 2,
            _ => panic!("checkpoint LSMR test injection supports allocation or interruption"),
        };
    }

    fn take_checkpoint_lsmr_fatal_failure(&mut self) -> Option<FeregError> {
        let state = std::mem::take(&mut self.checkpoint_lsmr_fatal_failure);
        match state {
            0 => None,
            1 => Some(FeregError::new(
                ErrorCode::Allocation,
                "injected checkpoint LSMR allocation failure",
            )),
            2 => Some(FeregError::new(
                ErrorCode::Interrupted,
                "injected checkpoint LSMR interruption",
            )),
            _ => unreachable!("checkpoint LSMR failure injection is validated"),
        }
    }

    pub(crate) const fn max_rhs(&self) -> usize {
        self.max_rhs
    }

    // The last successful projection certified this exact RHS in raw/max weights.
    pub(crate) fn certified_rhs_norms(&self, rhs: usize) -> &[f64] {
        &self.workspace.residual_norms[..rhs]
    }

    fn payload_bytes(&self) -> Result<u128> {
        let c = self.capacities();
        let banks = [
            c.reference_norms,
            c.residual_norms,
            c.rhs_scales,
            c.norm_scales,
            c.norm_sums,
            c.group_sums,
            c.group_corrections,
            c.projection_gram,
            c.projection_rhs,
            c.certificate_columns,
            c.accelerator_state,
        ]
        .into_iter()
        .try_fold(0_u128, |n, capacity| {
            n.checked_add(capacity as u128)
                .ok_or_else(|| allocation("FE actual capacities overflow"))
        })?;
        let cmg_bytes = self.cmg_schur_bytes()?;
        banks
            .checked_mul(8)
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>() as u128))
            .and_then(|n| n.checked_add(self.design.descriptor_bytes()))
            .and_then(|n| n.checked_add(cmg_bytes))
            .ok_or_else(|| allocation("FE actual owner footprint overflows"))
    }
    pub(crate) fn capacities(&self) -> ReusableMapCapacities {
        ReusableMapCapacities {
            rhs: self.max_rhs,
            max_channels: 1,
            max_level_members: self.max_level_members,
            reference_norms: self.workspace.reference_norms.capacity(),
            residual_norms: self.workspace.residual_norms.capacity(),
            rhs_scales: self.workspace.rhs_scales.capacity(),
            norm_scales: self.workspace.value_scales.capacity(),
            norm_sums: self.workspace.rhs_relative_infinity_norms.capacity(),
            group_sums: self.workspace.group_sums.capacity(),
            group_corrections: self.workspace.group_corrections.capacity(),
            projection_gram: self.workspace.parallel_group_weights.capacity(),
            projection_rhs: self.workspace.parallel_group_means.capacity(),
            projection_workers: 0,
            certificate_columns: self.workspace.parallel_dot_sums.capacity(),
            certificate_basis: 0,
            certificate_geometry: 0,
            certificate_ranks: 0,
            accelerator_state: self
                .two_fe_pcg
                .as_ref()
                .map_or(0, TwoFePcgWorkspace::capacity_elements)
                .saturating_add(self.cg_residual_square.capacity())
                .saturating_add(self.cg_initial_residual_square.capacity())
                .saturating_add(self.cg_next_residual_square.capacity())
                .saturating_add(self.cg_direction_projection.capacity())
                .saturating_add(self.cg_ratio.capacity()),
            topology_effects: 0,
            topology_codes: 0,
            topology_slope_vectors: 0,
            topology_slope_values: 0,
            topology_level_offsets: 0,
            topology_level_observations: 0,
            topology_resident_bytes: 0,
        }
    }

    /// Select norms in caller column order without allocating an index buffer.
    /// Validate the selection before projection or any write to caller norms.
    pub(crate) fn absorb_with_selected_norms(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: MapConfig,
        norm_weight_scale: f64,
        rhs_positions: impl ExactSizeIterator<Item = usize> + Clone,
        output: &mut [f64],
    ) -> Result<(CertifiedBorrowedMap, bool)> {
        if rhs_positions.len() != output.len()
            || rhs_positions
                .clone()
                .any(|position| position >= values.ncols())
        {
            return Err(invalid_argument(
                "reusable categorical MAP norm selection is not aligned",
            ));
        }
        let absorbed = self.absorb_ppml(weights, values, config)?;
        if self.design.effects().len() != 1 {
            return Ok((absorbed, false));
        }
        // The linear frame may deliberately use a divisor other than MAP's
        // maximum raw weight. Leave its stable norm calculation authoritative.
        let raw_scale = match weights {
            BorrowedMapWeights::Unit => 1.0,
            BorrowedMapWeights::Provided(values) => values.iter().copied().fold(0.0_f64, f64::max),
        };
        if raw_scale != norm_weight_scale {
            return Ok((absorbed, false));
        }
        let norms = &self.workspace.residual_norms;
        if norms.len() != absorbed.residualized().ncols() {
            return Err(invalid_data(
                "reusable categorical MAP certified norms are not aligned",
            ));
        }
        for (destination, position) in output.iter_mut().zip(rhs_positions) {
            *destination = norms[position];
        }
        Ok((absorbed, true))
    }

    fn with_pilot_group_scratch<T>(
        &mut self,
        rhs: usize,
        work: impl FnOnce(&BorrowedInterceptFeDesign<'a>, &mut BorrowedMapWorkspace) -> Result<T>,
    ) -> Result<T> {
        // Group reductions scale with rows times RHSs. Below the ordinary
        // row-only reservation boundary, a wide pilot can use the existing
        // largest-partition banks without increasing its admitted memory.
        let lend = rayon::current_num_threads() > 1
            && self.design.nobs().saturating_mul(rhs) >= PARALLEL_PROJECTION_MIN_ROWS
            && self.workspace.parallel_group_means.is_empty()
            && self.workspace.parallel_group_weights.is_empty()
            && self.two_fe_pcg.is_some();
        if lend {
            let loan =
                PcgPilotGroupScratch::new(&mut self.workspace, self.two_fe_pcg.as_mut().unwrap());
            work(&self.design, loan.workspace)
        } else {
            work(&self.design, &mut self.workspace)
        }
    }

    pub(crate) fn absorb_ppml(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: MapConfig,
    ) -> Result<CertifiedBorrowedMap> {
        self.absorb(weights, values.into(), config, false)
    }

    /// Ordinary OLS certifies a half-request proposal before spending work on
    /// one tighter correction. Final residuals retain their existing solve.
    pub(crate) fn absorb_ols(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        values: crate::ols::OlsProjectionInput,
        config: MapConfig,
        final_residual: bool,
    ) -> Result<CertifiedBorrowedMap> {
        self.absorb(weights, values, config, !final_residual)
    }

    fn absorb(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        values: crate::ols::OlsProjectionInput,
        config: MapConfig,
        refine_ols: bool,
    ) -> Result<CertifiedBorrowedMap> {
        validate_config(config)?;
        let rhs_count = values.ncols();
        if values.nrows() != self.design.nobs()
            || values.nrows() == 0
            || rhs_count == 0
            || rhs_count > self.max_rhs
        {
            return Err(invalid_argument(
                "reusable categorical MAP inputs exceed their fixed session dimensions",
            ));
        }
        values.validate_finite("reusable categorical MAP values must be finite")?;
        let mut values = values.into_values();
        let weights = validate_weights(weights, values.nrows())?;
        self.workspace.set_rhs_count(rhs_count)?;

        let setup_started = Instant::now();
        update_workspace_norms(&values, weights, &mut self.workspace, true)?;
        let setup_seconds = setup_started.elapsed().as_secs_f64();
        let solve_started = Instant::now();
        let initial_certification_started = Instant::now();
        let initial = self.with_pilot_group_scratch(rhs_count, |design, workspace| {
            if initial_group_rejects(design, weights, &values, config.tolerance, workspace)? {
                Ok(None)
            } else {
                certify_initial(design, weights, &values, config.tolerance, workspace).map(Some)
            }
        })?;
        // A global norm can hide projection drift in a small-mass component.
        // The same component scaling used by the checkpoint recurrence must
        // also govern its zero-iteration shortcut. This only tightens the
        // shortcut; the original-weight certificate remains authoritative.
        let balanced_weights = if self.checkpoint_lsmr_enabled
            && self.checkpoint_component_scaling
            && matches!(self.design.effects().len(), 2 | 3)
        {
            component_scaled_checkpoint_weights(&self.design, weights)?
        } else {
            None
        };
        let balanced_initial_passed = if initial.as_ref().is_some_and(|initial| initial.passed) {
            if let Some(scaled) = balanced_weights.as_deref() {
                let scaled =
                    validate_weights(BorrowedMapWeights::Provided(scaled), values.nrows())?;
                update_workspace_norms(&values, scaled, &mut self.workspace, true)?;
                let passed = self
                    .with_pilot_group_scratch(rhs_count, |design, workspace| {
                        certify_initial(design, scaled, &values, config.tolerance, workspace)
                    })?
                    .passed;
                // Retained norm handoff and every later certificate consume
                // the original frame, even if this early return is admitted.
                update_workspace_norms(&values, weights, &mut self.workspace, true)?;
                self.workspace
                    .residual_norms
                    .copy_from_slice(&self.workspace.reference_norms);
                passed
            } else {
                true
            }
        } else {
            false
        };
        let initial_certification_seconds = initial_certification_started.elapsed().as_secs_f64();
        if let Some(initial) = initial.filter(|initial| initial.passed && balanced_initial_passed) {
            return certified_result(
                values,
                SolveStopReason::Certified,
                0,
                initial,
                setup_seconds,
                solve_started.elapsed().as_secs_f64(),
            );
        }

        if self.checkpoint_lsmr_enabled && matches!(self.design.effects().len(), 2 | 3) {
            if let Some(error) = self.take_checkpoint_lsmr_fatal_failure() {
                return Err(error);
            }
            let solver_weights = match balanced_weights.as_deref() {
                Some(w) => validate_weights(BorrowedMapWeights::Provided(w), values.nrows())?,
                None => weights,
            };
            // True unless a planned LSMR fault must reach LSMR without an
            // explicitly planned preceding Schur rejection; otherwise an
            // accepted accelerator would bypass the boundary under test.
            let allow_schur = !(self.checkpoint_lsmr_certificate_failure
                || self.checkpoint_lsmr_retry_error.is_some())
                || self.checkpoint_schur_iteration_cap.is_some()
                || self.checkpoint_schur_certificate_failure;
            let (schur, schur_cost) = if balanced_weights.is_none() && allow_schur {
                if self.design.effects().len() == 2 {
                    self.try_checkpoint_schur(weights, &values, config)?
                } else {
                    (None, CheckpointSchurCost::default())
                }
            } else {
                (None, CheckpointSchurCost::default())
            };

            if let Some(mut certified) = schur {
                certified.report.setup_seconds += setup_seconds;
                certified.report.certification_seconds += initial_certification_seconds;
                return Ok(certified);
            }
            let force_rejection = std::mem::take(&mut self.checkpoint_lsmr_certificate_failure);
            let remaining = config
                .max_iterations
                .checked_sub(schur_cost.iterations)
                .filter(|n| *n > 0)
                .ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::NonConvergence,
                        "checkpoint graph exhausted its shared work budget",
                    )
                })?;
            let lsmr = LsmrConfig {
                tolerance: config.tolerance,
                max_iterations: (remaining / 3).max(1),
                threads: crate::options::ThreadConfig::from_requested(
                    rayon::current_num_threads(),
                    self.checkpoint_deterministic,
                )?,
                ..LsmrConfig::default()
            };
            let mut batch = CheckpointLsmrBatch::new(&self.design, solver_weights, &values, lsmr)?;
            let mut accumulated: Option<SolveReport> = None;
            let mut recurrence_tolerance = config.tolerance;
            let report = loop {
                if accumulated.is_some() {
                    if let Some(code) = self.checkpoint_lsmr_retry_error.take() {
                        return Err(FeregError::new(code, "injected checkpoint retry failure"));
                    }
                }
                let mut report = batch.solve_attempt(
                    recurrence_tolerance,
                    accumulated
                        .as_ref()
                        .map(|_| self.workspace.rhs_relative_infinity_norms.as_slice()),
                    config.tolerance,
                )?;
                let certification_started = Instant::now();
                let mut canonical_certificate = certify(
                    &self.design,
                    weights,
                    batch
                        .candidate
                        .as_ref()
                        .expect("checkpoint attempt has output"),
                    config.tolerance,
                    &mut self.workspace,
                )?;
                canonical_certificate.passed &= !force_rejection;
                report.certification_seconds += certification_started.elapsed().as_secs_f64();
                if crate::diagnostics::solver_trace_enabled() {
                    crate::diagnostics::emit_solver_trace(&format!(
                        "FEREG_SOLVER_TRACE|phase=checkpoint_lsmr_attempt|retry={}|recurrence_tolerance={:.17e}|canonical_tolerance={:.17e}|iterations={}|setup_seconds={:.17e}|solve_seconds={:.17e}|certificate_seconds={:.17e}|canonical_relinf={:.17e}|canonical_passed={}|rhs={}|batch_window={}|threads={}|rows={}",
                        u8::from(accumulated.is_some()), recurrence_tolerance, config.tolerance,
                        report.iterations, report.setup_seconds, report.solve_seconds,
                        report.certification_seconds, canonical_certificate.relative_infinity_norm,
                        u8::from(canonical_certificate.passed), values.ncols(),
                        crate::memory::lsmr_rhs_window(values.ncols(), rayon::current_num_threads()),
                        rayon::current_num_threads(), self.design.nobs(),
                    ));
                }
                if let Some(prior) = accumulated.take() {
                    report.checkpoint_retries = 1;
                    report.iterations = report.iterations.saturating_add(prior.iterations);
                    report.setup_seconds += prior.setup_seconds;
                    report.solve_seconds += prior.solve_seconds;
                    report.certification_seconds += prior.certification_seconds;
                    report.warnings.extend(prior.warnings);
                } else {
                    report.setup_seconds += setup_seconds;
                    report.certification_seconds += initial_certification_seconds;
                }
                let canonical_passed = canonical_certificate.passed;
                report.certificate = canonical_certificate;
                if canonical_passed {
                    report.stop_reason = SolveStopReason::Certified;
                    schur_cost.add_to(&mut report);
                    return CertifiedBorrowedMap::from_certified_parts(
                        batch.candidate.take().expect("certified checkpoint output"),
                        report,
                    );
                }
                report.stop_reason = SolveStopReason::Uncertified;
                let tighter = (config.tolerance * 0.01).max(1e-14);
                let unused = remaining.saturating_sub(report.iterations);
                if report.checkpoint_retries != 0 || tighter >= recurrence_tolerance || unused == 0
                {
                    break report;
                }
                // Only a failed original-weight certificate authorizes retry.
                // The scoped owner may retain certified columns if the partial
                // retry fits the admitted full-batch allowance. Every solved
                // RHS still starts from untouched original inputs.
                batch.config.max_iterations = batch.config.max_iterations.min(unused);
                recurrence_tolerance = tighter;
                accumulated = Some(report);
            };

            drop(batch);
            // The LSMR candidate never aliases `values`. A numerical failure
            // therefore leaves the authoritative checkpoint inputs untouched
            // and permits the existing MAP projector to certify them. Other
            // error classes returned above remain fatal.
            let raw_weights = borrowed_weights(weights);
            self.checkpoint_lsmr_enabled = false;
            let remaining = config
                .max_iterations
                .checked_sub(schur_cost.iterations + report.iterations)
                .filter(|n| *n > 0)
                .ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::NonConvergence,
                        "checkpoint projection exhausted its shared work budget",
                    )
                })?;
            let fallback = self.absorb_ppml(
                raw_weights,
                values,
                MapConfig {
                    max_iterations: remaining,
                    ..config
                },
            );
            self.checkpoint_lsmr_enabled = true;
            let fallback = fallback?;
            let (residualized, mut fallback_report, coefficients) = fallback.into_full_parts();
            fallback_report.requested_solver = FeSolverKind::Lsmr;
            fallback_report.iterations =
                fallback_report.iterations.saturating_add(report.iterations);
            fallback_report.setup_seconds += report.setup_seconds;
            fallback_report.solve_seconds += report.solve_seconds;
            fallback_report.certification_seconds += report.certification_seconds;
            fallback_report.checkpoint_retries = report.checkpoint_retries;
            fallback_report.fallback_used = true;
            fallback_report.requested_preconditioner = report.requested_preconditioner;
            fallback_report.used_preconditioner = report.used_preconditioner;
            fallback_report.warnings.insert(
                0,
                format!(
                    "current-weight checkpoint LSMR did not certify after {} iterations; MAP certified the untouched authoritative checkpoint inputs",
                    report.iterations,
                ),
            );
            schur_cost.add_to(&mut fallback_report);
            return if let Some(coefficients) = coefficients {
                CertifiedBorrowedMap::from_certified_parts_with_coefficients(
                    residualized,
                    fallback_report,
                    coefficients,
                )
            } else {
                CertifiedBorrowedMap::from_certified_parts(residualized, fallback_report)
            };
        }

        // This owner always selects the two-FE Schur engine. Its equations
        // eliminate the left partition directly from untouched inputs; a MAP
        // warmup adds observation passes without supplying routing evidence.
        // Reconstruction and the full current-weight certificate remain the
        // acceptance gate, with the existing MAP continuation on a miss.
        if self.two_fe_pcg.is_some() {
            return self.absorb_two_fe_pcg(
                weights,
                values,
                config,
                refine_ols,
                0,
                1.0,
                setup_seconds,
                solve_started,
            );
        }

        for iteration in 1..=config.max_iterations {
            crate::interrupt::checkpoint()?;
            self.with_pilot_group_scratch(rhs_count, |design, workspace| {
                borrowed_residual_sweep_recording::<MapCoefficientAccumulator>(
                    design,
                    weights,
                    &mut values,
                    workspace,
                    None,
                    config.symmetric,
                )
            })?;
            if self.design.effects().len() == 1
                || iteration % config.check_every == 0
                || iteration == config.max_iterations
            {
                let certificate =
                    self.with_pilot_group_scratch(rhs_count, |design, workspace| {
                        certify(design, weights, &values, config.tolerance, workspace)
                    })?;
                if certificate.passed {
                    return certified_result(
                        values,
                        SolveStopReason::Certified,
                        iteration,
                        certificate,
                        setup_seconds,
                        solve_started.elapsed().as_secs_f64(),
                    );
                }
            }
        }
        Err(invalid_data(
            "reusable categorical MAP exhausted its iteration budget without certification",
        ))
    }

    fn try_checkpoint_schur(
        &mut self,
        weights: ValidatedWeights<'_>,
        values: &DenseMatrix,
        config: MapConfig,
    ) -> Result<(Option<CertifiedBorrowedMap>, CheckpointSchurCost)> {
        let mut cost = CheckpointSchurCost::default();
        if !self.checkpoint_lsmr_enabled
            || self.two_fe_pcg.is_none()
            || values.ncols() == 0
            || values.ncols() > self.max_rhs
            || config.max_iterations <= GRAPH_PROGRESS_CHECK_ITERATIONS
        {
            return Ok((None, cost));
        }
        let started = Instant::now();
        // The initial full-batch iterations are useful solve work. A bounded
        // progress check can decline without discarding a separate pilot or
        // rebuilding its current-weight group state.
        // Only checkpoint-LSMR callers enter here. Their preflight charges an
        // input plus the borrowed adapter's output/transport banks, while this
        // retained graph is already in the canonical session's selected plan.
        // One candidate replaces those serial LSMR banks, never overlaps them.
        let level_counts = [
            self.design.effects()[0].n_levels(),
            self.design.effects()[1].n_levels(),
        ];
        let transient = estimate_fe_solver_memory(
            values.nrows(),
            values.ncols(),
            &level_counts,
            rayon::current_num_threads(),
            Some(4),
        )?
        .lsmr_schwarz_bytes;
        let candidate = checkpoint_schur_copy(values, transient)?;
        cost.setup_seconds = started.elapsed().as_secs_f64();
        let cap = GRAPH_ATTEMPT_MAX_ITERATIONS.min(config.max_iterations);
        let cap = self
            .checkpoint_schur_iteration_cap
            .take()
            .unwrap_or(cap)
            .min(cap);
        let solve_started = Instant::now();
        let attempt = absorb_two_fe_pcg_reusing(
            &self.design,
            weights,
            candidate,
            config,
            0.5 * config.tolerance,
            0,
            cost.contraction,
            0.0,
            solve_started,
            self.max_rhs,
            self.two_fe_pcg.as_mut().unwrap(),
            &mut self.workspace,
            &mut self.cg_residual_square,
            &mut self.cg_initial_residual_square,
            &mut self.cg_next_residual_square,
            &mut self.cg_direction_projection,
            &mut self.cg_ratio,
            SchurWorkLimit::Checkpoint(cap),
            self.retain_coefficients,
        )?;
        cost.solve_seconds = solve_started.elapsed().as_secs_f64();
        match attempt {
            SchurAttempt::Certified(mut result) => {
                if std::mem::take(&mut self.checkpoint_schur_certificate_failure) {
                    cost.iterations = result.report.iterations;
                    cost.rejected = true;
                    cost.trace(values, "certificate_rejected");
                    return Ok((None, cost));
                }
                result.report.requested_solver = FeSolverKind::Lsmr;
                result.report.setup_seconds += cost.setup_seconds;
                cost.contraction = result.report.pilot_contraction;
                cost.iterations = result.report.iterations;
                cost.trace(values, "certified");
                Ok((Some(result), cost))
            }
            SchurAttempt::Continue {
                iterations,
                candidate,
                progress_contraction,
                ..
            } => {
                cost.iterations = iterations;
                cost.contraction = progress_contraction.unwrap_or(0.0);
                cost.rejected = true;
                cost.trace(
                    values,
                    if progress_contraction.is_some_and(|c| c > GRAPH_MAX_PROGRESS_CONTRACTION) {
                        "progress_rejected"
                    } else if candidate {
                        "certificate_rejected"
                    } else {
                        "capped_or_breakdown"
                    },
                );
                // `values` is borrowed and untouched. The finite attempt is
                // dropped before any LSMR setup or tighter retry can allocate.
                Ok((None, cost))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn absorb_two_fe_pcg(
        &mut self,
        weights: ValidatedWeights<'_>,
        values: DenseMatrix,
        config: MapConfig,
        refine_ols: bool,
        completed_pilot_iterations: usize,
        pilot_contraction: f64,
        setup_seconds: f64,
        solve_started: Instant,
    ) -> Result<CertifiedBorrowedMap> {
        let graph = self
            .two_fe_pcg
            .as_mut()
            .ok_or_else(|| invalid_data("two-FE categorical MAP PCG workspace is unavailable"))?;
        let cmg_schur = graph.cmg_schur.is_some();
        let result = absorb_two_fe_pcg_reusing(
            &self.design,
            weights,
            values,
            config,
            if cmg_schur {
                (config.tolerance * 0.01).max(1e-15).min(config.tolerance)
            } else {
                0.5 * config.tolerance
            },
            completed_pilot_iterations,
            pilot_contraction,
            setup_seconds,
            solve_started,
            self.max_rhs,
            graph,
            &mut self.workspace,
            &mut self.cg_residual_square,
            &mut self.cg_initial_residual_square,
            &mut self.cg_next_residual_square,
            &mut self.cg_direction_projection,
            &mut self.cg_ratio,
            if cmg_schur {
                SchurWorkLimit::CmgSchur
            } else {
                SchurWorkLimit::Map { refine: refine_ols }
            },
            self.retain_coefficients,
        );
        match result? {
            SchurAttempt::Certified(result) => Ok(result),
            SchurAttempt::Continue { .. } => {
                unreachable!("unbounded MAP accelerator continues internally")
            }
        }
    }
}

crate::fault::fault_plan! {
    static TWO_FE_CERTIFICATE_FAILURES: usize = 0;
    static TWO_FE_CORRECTIONS: usize = 0;
    static TWO_FE_BREAKDOWN: bool = false;
}

#[allow(clippy::too_many_arguments)]
fn absorb_two_fe_pcg_reusing(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    mut values: DenseMatrix,
    config: MapConfig,
    mut recurrence_tolerance: f64,
    completed_pilot_iterations: usize,
    pilot_contraction: f64,
    setup_seconds: f64,
    solve_started: Instant,
    stride: usize,
    graph: &mut TwoFePcgWorkspace,
    certificate_workspace: &mut BorrowedMapWorkspace,
    residual_square: &mut Vec<f64>,
    active_rhs_state: &mut Vec<f64>,
    next_residual_square: &mut Vec<f64>,
    direction_projection: &mut Vec<f64>,
    ratio: &mut Vec<f64>,
    work_limit: SchurWorkLimit,
    retain_coefficients: bool,
) -> Result<SchurAttempt> {
    let trace = crate::diagnostics::phase_timing_enabled();
    let mut operator_seconds = 0.0;
    let mut recurrence_seconds = 0.0;
    let rhs_count = values.ncols();
    let (iteration_cap, checkpoint_progress, refine) = match work_limit {
        SchurWorkLimit::Map { refine } => (None, false, refine),
        SchurWorkLimit::Checkpoint(cap) => (Some(cap), true, false),
        SchurWorkLimit::CmgSchur => (None, false, false),
    };
    let mut correction_tolerance =
        refine.then(|| super::auto::direct_lsmr_tolerance(config.tolerance));
    // Component banks belong only to the PPML constructor. A numerical work
    // cap must not turn off their current-weight scaling.
    let scale_components = !graph.component_roots.is_empty();
    let left = &design.effects()[graph.left_effect];
    let right = &design.effects()[graph.right_effect];
    let left_levels = left.n_levels();
    let right_levels = right.n_levels();
    if design.effects().len() != 2
        || stride < rhs_count
        || graph.left_weights.len() != left_levels
        || graph.right_weights.len() != right_levels
        || graph.left_rhs.len() != left_levels.saturating_mul(stride)
        || graph.left_work.len() != left_levels.saturating_mul(stride)
        || graph.solution.len() != right_levels.saturating_mul(stride)
        || graph.residual.len() != right_levels.saturating_mul(stride)
        || graph.direction.len() != right_levels.saturating_mul(stride)
        || graph.operator.len() != right_levels.saturating_mul(stride)
    {
        return Err(invalid_argument(
            "two-FE categorical MAP PCG workspace dimensions are inconsistent",
        ));
    }
    let hardware = crate::kernels::installed();
    let mut progress_contraction = None;
    let remaining_iterations = config
        .max_iterations
        .saturating_sub(completed_pilot_iterations);
    let cmg_schur = matches!(work_limit, SchurWorkLimit::CmgSchur);
    let map_fallback_reserve = if cmg_schur {
        0
    } else {
        remaining_iterations / 4
    };
    let pcg_iteration_limit = iteration_cap.map_or_else(
        || config.max_iterations.saturating_sub(map_fallback_reserve),
        |cap| cap.min(config.max_iterations),
    );
    let mut iterations = completed_pilot_iterations;
    let mut coefficients = if retain_coefficients {
        Some(MapCoefficientAccumulator::zeros(design, rhs_count)?)
    } else {
        None
    };
    let mut breakdown = false;
    loop {
        let phase_start = Instant::now();
        for scalars in [
            &mut *residual_square,
            &mut *active_rhs_state,
            &mut *next_residual_square,
            &mut *direction_projection,
            &mut *ratio,
        ] {
            if rhs_count > scalars.capacity() {
                return Err(invalid_argument(
                    "two-FE categorical MAP PCG scalar workspace exceeds fixed capacity",
                ));
            }
            scalars.resize(rhs_count, 0.0);
            // A capped attempt can leave nonzero energy in a reused bank before
            // the original-input MAP continuation reaches this same recurrence.
            scalars.fill(0.0);
        }

        {
            let left_codes = left.codes();
            graph
                .solution
                .par_chunks_mut(32768)
                .for_each(|part| part.fill(0.0));
            fill_two_fe_group_rhs(
                left,
                weights,
                &values,
                stride,
                (&mut graph.left_rhs, &mut graph.left_weights),
                0..rhs_count,
                stride,
                |_, _, _| {},
            )?;
            fill_two_fe_group_rhs(
                right,
                weights,
                &values,
                stride,
                (&mut graph.residual, &mut graph.right_weights),
                0..rhs_count,
                stride,
                |_, _, _| {},
            )?;
            if graph
                .left_weights
                .iter()
                .chain(&graph.right_weights)
                .any(|weight| !weight.is_finite() || *weight <= 0.0)
                || graph
                    .left_rhs
                    .iter()
                    .chain(&graph.residual)
                    .any(|value| !value.is_finite())
            {
                return Err(invalid_data(
                    "two-FE categorical MAP PCG group state is invalid",
                ));
            }
            graph
                .left_rhs
                .par_chunks_mut(stride)
                .zip(&graph.left_weights)
                .for_each(|(sum, &denominator)| {
                    for value in &mut sum[..rhs_count] {
                        *value /= denominator;
                    }
                });
            let left_rhs = &graph.left_rhs;
            graph
                .residual
                .par_chunks_mut(stride)
                .enumerate()
                .try_for_each(|(level, sum)| -> Result<()> {
                    subtract_two_fe_left_projection(
                        right.level_index().checked_members(level)?,
                        left_codes,
                        weights,
                        stride,
                        left_rhs,
                        &mut sum[..rhs_count],
                    );
                    Ok(())
                })?;
            if scale_components {
                // A positive scalar per disconnected block leaves the projected
                // solution unchanged, while keeping low-mass blocks visible to CG.
                // Only the recurrence is scaled; all acceptance uses original weights.
                graph.refresh_component_scales();
                for (row, &scale) in graph
                    .residual
                    .chunks_mut(stride)
                    .zip(&graph.component_scales)
                {
                    for value in &mut row[..rhs_count] {
                        *value /= scale;
                    }
                }
            }
            if let Some(preconditioner) = graph.cmg_schur.as_mut() {
                preconditioner.apply_batch(
                    &graph.residual,
                    &mut graph.direction,
                    stride,
                    rhs_count,
                    scale_components.then_some(graph.component_scales.as_slice()),
                )?;
                crate::kernels::dispatch!(
                    hardware,
                    column_products(&graph.residual, &graph.direction, stride, residual_square)
                );
            } else {
                crate::kernels::dispatch!(
                    crate::kernels::installed(),
                    pcg_initialize(
                        &graph.right_weights,
                        scale_components.then_some(graph.component_scales.as_slice()),
                        stride,
                        &graph.residual,
                        &mut graph.direction,
                        residual_square
                    )
                );
            }
        }
        if residual_square
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(invalid_data(
                "two-FE categorical MAP PCG initial residual is invalid",
            ));
        }
        let mut active_rhs = 0_usize;
        let recurrence_plan = crate::parallel::StagePlan::current(
            right_levels as u128 * rhs_count as u128,
            right_levels,
        );
        let parallel_recurrence = recurrence_plan.lanes > 1;
        recurrence_plan.trace("two_fe_recurrence", right_levels);
        active_rhs_state.fill(0.0);
        for rhs in 0..rhs_count {
            if two_fe_pcg_rhs_ready(
                graph,
                certificate_workspace,
                rhs,
                stride,
                recurrence_tolerance,
                scale_components,
            )? {
                trace_two_fe_pcg_stop(rhs, iterations, "proxy_target", recurrence_tolerance);
                residual_square[rhs] = 0.0;
                for level in 0..right_levels {
                    let index = level * stride + rhs;
                    graph.residual[index] = 0.0;
                    graph.direction[index] = 0.0;
                }
            } else {
                // Keep initial energy in the already admitted activity bank.
                // Zero marks a completed RHS; a negative sentinel represents an
                // active recurrence whose initial energy underflowed to zero.
                active_rhs_state[rhs] = if residual_square[rhs] > 0.0 {
                    residual_square[rhs]
                } else {
                    -1.0
                };
                active_rhs += 1;
            }
        }
        let mut reconstruct = active_rhs == 0;
        let initialization_seconds = phase_start.elapsed().as_secs_f64();
        while !reconstruct && iterations < pcg_iteration_limit {
            crate::interrupt::checkpoint()?;
            let op_start = Instant::now();
            graph.apply_schur(design, weights, rhs_count, stride)?;
            // Count actual operator lanes, including converged zeros. Only the
            // PPML owner exposes this scalar as a later-call routing hint.
            graph.ppml_operator_rhs_count = graph
                .ppml_operator_rhs_count
                .saturating_add(rhs_count as u128);
            if scale_components {
                for (row, &scale) in graph
                    .operator
                    .chunks_mut(stride)
                    .zip(&graph.component_scales)
                {
                    for value in &mut row[..rhs_count] {
                        *value /= scale;
                    }
                }
            }
            operator_seconds += op_start.elapsed().as_secs_f64();
            let rec_start = Instant::now();
            direction_projection.fill(0.0);
            crate::kernels::dispatch!(
                hardware,
                column_products(
                    &graph.direction[..right_levels * stride],
                    &graph.operator,
                    stride,
                    direction_projection
                )
            );
            for rhs in 0..rhs_count {
                if active_rhs_state[rhs] == 0.0 {
                    ratio[rhs] = 0.0;
                    continue;
                }
                let numerator = residual_square[rhs];
                let denominator = direction_projection[rhs];
                if numerator == 0.0 {
                    ratio[rhs] = 0.0;
                } else if numerator.is_finite()
                    && numerator > 0.0
                    && denominator.is_finite()
                    && denominator > 0.0
                {
                    ratio[rhs] = numerator / denominator;
                } else {
                    breakdown = true;
                    break;
                }
            }
            breakdown |= TWO_FE_BREAKDOWN.replace(false);
            if breakdown {
                break;
            }
            next_residual_square.fill(0.0);
            if parallel_recurrence {
                let level_chunk = recurrence_plan.chunk(right_levels);
                let cell_chunk = level_chunk
                    .checked_mul(stride)
                    .ok_or_else(|| allocation("two-FE PCG recurrence chunk dimensions overflow"))?;
                let direction = &graph.direction;
                let operator = &graph.operator;
                graph
                    .solution
                    .par_chunks_mut(cell_chunk)
                    .zip(graph.residual.par_chunks_mut(cell_chunk))
                    .enumerate()
                    .for_each(|(chunk, (solution, residual))| {
                        let start = chunk * cell_chunk;
                        let end = start + solution.len();
                        crate::kernels::dispatch!(
                            hardware,
                            pcg_step(
                                stride,
                                ratio,
                                &direction[start..end],
                                &operator[start..end],
                                solution,
                                residual
                            )
                        );
                    });
                // The state update is level-disjoint. Keep this scalar reduction
                // serial in canonical level/RHS order so thread count cannot alter
                // the recurrence or its stopping decision.
                if !cmg_schur {
                    crate::kernels::dispatch!(
                        hardware,
                        pcg_energy(
                            &graph.right_weights,
                            scale_components.then_some(graph.component_scales.as_slice()),
                            stride,
                            &graph.residual,
                            next_residual_square
                        )
                    );
                }
            } else if cmg_schur {
                crate::kernels::dispatch!(
                    hardware,
                    pcg_step(
                        stride,
                        ratio,
                        &graph.direction,
                        &graph.operator,
                        &mut graph.solution,
                        &mut graph.residual
                    )
                );
            } else {
                crate::kernels::dispatch!(
                    hardware,
                    pcg_step_energy(
                        &graph.right_weights,
                        scale_components.then_some(graph.component_scales.as_slice()),
                        stride,
                        ratio,
                        &graph.direction,
                        &graph.operator,
                        &mut graph.solution,
                        &mut graph.residual,
                        next_residual_square
                    )
                );
            }
            if let Some(preconditioner) = graph.cmg_schur.as_mut() {
                preconditioner.apply_batch(
                    &graph.residual,
                    &mut graph.operator,
                    stride,
                    rhs_count,
                    scale_components.then_some(graph.component_scales.as_slice()),
                )?;
                crate::kernels::dispatch!(
                    hardware,
                    column_products(
                        &graph.residual,
                        &graph.operator,
                        stride,
                        next_residual_square
                    )
                );
            }
            iterations += 1;
            if next_residual_square
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
            {
                breakdown = true;
                break;
            }
            for rhs in 0..rhs_count {
                if active_rhs_state[rhs] != 0.0
                    && two_fe_pcg_rhs_ready(
                        graph,
                        certificate_workspace,
                        rhs,
                        stride,
                        recurrence_tolerance,
                        scale_components,
                    )?
                {
                    trace_two_fe_pcg_stop(rhs, iterations, "proxy_target", recurrence_tolerance);
                    active_rhs_state[rhs] = 0.0;
                    next_residual_square[rhs] = 0.0;
                    active_rhs -= 1;
                    for level in 0..right_levels {
                        let index = level * stride + rhs;
                        graph.residual[index] = 0.0;
                        graph.direction[index] = 0.0;
                    }
                }
            }
            reconstruct = active_rhs == 0 || iterations == pcg_iteration_limit;
            if reconstruct {
                residual_square.copy_from_slice(next_residual_square);
                recurrence_seconds += rec_start.elapsed().as_secs_f64();
                break;
            }
            for rhs in 0..rhs_count {
                if active_rhs_state[rhs] == 0.0 {
                    ratio[rhs] = 0.0;
                    continue;
                }
                ratio[rhs] = if residual_square[rhs] == 0.0 {
                    0.0
                } else {
                    next_residual_square[rhs] / residual_square[rhs]
                };
                if !ratio[rhs].is_finite() || ratio[rhs] < 0.0 {
                    breakdown = true;
                    break;
                }
            }
            if breakdown {
                break;
            }
            if cmg_schur {
                // The operator bank now holds z = B r; the prior S p has
                // already been consumed by the solution/residual update.
                for (direction, preconditioned) in graph
                    .direction
                    .chunks_mut(stride)
                    .zip(graph.operator.chunks(stride))
                {
                    for rhs in 0..rhs_count {
                        direction[rhs] = if active_rhs_state[rhs] == 0.0 {
                            0.0
                        } else {
                            preconditioned[rhs] + ratio[rhs] * direction[rhs]
                        };
                    }
                }
            } else if parallel_recurrence {
                let level_chunk = recurrence_plan.chunk(right_levels);
                let cell_chunk = level_chunk
                    .checked_mul(stride)
                    .ok_or_else(|| allocation("two-FE PCG direction chunk dimensions overflow"))?;
                let right_weights = &graph.right_weights;
                let component_scales = &graph.component_scales;
                let residual = &graph.residual;
                graph
                    .direction
                    .par_chunks_mut(cell_chunk)
                    .enumerate()
                    .for_each(|(chunk, direction)| {
                        let first_level = chunk * level_chunk;
                        let end_level = first_level + direction.len() / stride;
                        let first = chunk * cell_chunk;
                        let end = first + direction.len();
                        crate::kernels::dispatch!(
                            hardware,
                            pcg_direction(
                                &right_weights[first_level..end_level],
                                scale_components.then(|| &component_scales[first_level..end_level]),
                                stride,
                                ratio,
                                active_rhs_state,
                                &residual[first..end],
                                direction
                            )
                        );
                    });
            } else {
                crate::kernels::dispatch!(
                    hardware,
                    pcg_direction(
                        &graph.right_weights,
                        scale_components.then_some(graph.component_scales.as_slice()),
                        stride,
                        ratio,
                        active_rhs_state,
                        &graph.residual,
                        &mut graph.direction
                    )
                );
            }
            residual_square.copy_from_slice(next_residual_square);
            recurrence_seconds += rec_start.elapsed().as_secs_f64();
            if checkpoint_progress && iterations == GRAPH_PROGRESS_CHECK_ITERATIONS && !reconstruct
            {
                let contraction = (0..rhs_count)
                    .map(|rhs| {
                        if active_rhs_state[rhs] <= 0.0 {
                            0.0
                        } else {
                            (residual_square[rhs] / active_rhs_state[rhs]).sqrt()
                        }
                    })
                    .fold(0.0_f64, f64::max);
                progress_contraction = Some(contraction);
                if crate::diagnostics::solver_trace_enabled() {
                    crate::diagnostics::emit_solver_trace(&format!(
                        "FEREG_SOLVER_TRACE|phase=checkpoint_schur_progress|iterations={iterations}|rhs={rhs_count}|contraction={contraction:.17e}|admitted={}",
                        u8::from(contraction <= GRAPH_MAX_PROGRESS_CONTRACTION),
                    ));
                }
                if contraction > GRAPH_MAX_PROGRESS_CONTRACTION {
                    break;
                }
            }
        }

        if crate::diagnostics::solver_trace_enabled() {
            let reason = if breakdown {
                "breakdown"
            } else if iterations == pcg_iteration_limit {
                "recurrence_budget"
            } else {
                "checkpoint_progress"
            };
            for (rhs, &active) in active_rhs_state.iter().enumerate() {
                if active != 0.0 {
                    trace_two_fe_pcg_stop(rhs, iterations, reason, recurrence_tolerance);
                }
            }
        }
        if iteration_cap.is_some() && (active_rhs > 0 || breakdown) {
            return Ok(SchurAttempt::Continue {
                iterations,
                candidate: false,
                progress_contraction,
            });
        }
        if reconstruct || iterations > completed_pilot_iterations {
            let reconstruct_start = Instant::now();
            reconstruct_two_fe_pcg_candidate(
                design,
                weights,
                graph,
                &mut values,
                rhs_count,
                stride,
            )?;
            if let Some(c) = coefficients.as_mut() {
                for (effect, bank) in [
                    (graph.left_effect, &graph.left_work),
                    (graph.right_effect, &graph.solution),
                ] {
                    for level in 0..design.effects()[effect].n_levels() {
                        c.add_intercept_level(
                            effect,
                            level,
                            &bank[level * stride..level * stride + rhs_count],
                        )?;
                    }
                }
            }
            let reconstruct_seconds = reconstruct_start.elapsed().as_secs_f64();
            let cert_start = Instant::now();
            let mut certificate = certify(
                design,
                weights,
                &values,
                config.tolerance,
                certificate_workspace,
            )?;
            if TWO_FE_CERTIFICATE_FAILURES.get() > 0 {
                TWO_FE_CERTIFICATE_FAILURES.set(TWO_FE_CERTIFICATE_FAILURES.get() - 1);
                certificate.passed = false;
            }
            if trace {
                eprintln!("FEREG_PHASE|schur|init={initialization_seconds:.9}|operator={operator_seconds:.9}|recurrence={recurrence_seconds:.9}|reconstruct={reconstruct_seconds:.9}|certificate={:.9}|iterations={iterations}|passed={}",cert_start.elapsed().as_secs_f64(),certificate.passed);
            }
            if certificate.passed {
                let mut result = certified_result(
                    values,
                    SolveStopReason::Certified,
                    iterations,
                    certificate,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                )?;
                result.report.pilot_iterations = completed_pilot_iterations;
                result.report.pilot_contraction = progress_contraction.unwrap_or(pilot_contraction);
                return Ok(SchurAttempt::Certified(attach_optional_map_coefficients(
                    result,
                    coefficients,
                )?));
            }
        }

        if iteration_cap.is_some() {
            return Ok(SchurAttempt::Continue {
                iterations,
                candidate: true,
                progress_contraction,
            });
        }
        // Correct the reconstructed residual, retaining its already removed FE
        // contribution and the original RHS reference norms. This is a fresh
        // residual restart: converged PCG lanes were deliberately zeroed above.
        // Both attempts share the original PCG limit and MAP reserve.
        if !breakdown && iterations < pcg_iteration_limit {
            if let Some(tighter) = correction_tolerance
                .take()
                .filter(|t| *t < recurrence_tolerance)
            {
                if crate::diagnostics::solver_trace_enabled() {
                    crate::diagnostics::emit_solver_trace(&format!(
                        "FEREG_SOLVER_TRACE|phase=two_fe_pcg_correction|iterations={iterations}|recurrence_tolerance={tighter:.17e}"
                    ));
                }
                TWO_FE_CORRECTIONS.set(TWO_FE_CORRECTIONS.get() + 1);
                recurrence_tolerance = tighter;
                continue;
            }
        }
        break;
    }

    if cmg_schur {
        return Err(FeregError::new(
            ErrorCode::NonConvergence,
            "cmgschur did not certify within its iteration budget",
        ));
    }

    // PCG is an acceleration only. A finite breakdown or a full-certificate
    // miss continues with ordinary symmetric MAP from the reconstructed
    // observation-space candidate. One quarter of the original iteration
    // budget remains reserved for this fail-safe continuation.
    for iteration in iterations.saturating_add(1)..=config.max_iterations {
        crate::interrupt::checkpoint()?;
        borrowed_residual_sweep_recording::<MapCoefficientAccumulator>(
            design,
            weights,
            &mut values,
            certificate_workspace,
            coefficients.as_mut(),
            config.symmetric,
        )?;
        if iteration % config.check_every == 0 || iteration == config.max_iterations {
            let certificate = certify(
                design,
                weights,
                &values,
                config.tolerance,
                certificate_workspace,
            )?;
            if certificate.passed {
                let mut result = certified_result(
                    values,
                    SolveStopReason::Certified,
                    iteration,
                    certificate,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                )?;
                result.report.pilot_iterations = completed_pilot_iterations;
                result.report.pilot_contraction = progress_contraction.unwrap_or(pilot_contraction);
                return Ok(SchurAttempt::Certified(attach_optional_map_coefficients(
                    result,
                    coefficients,
                )?));
            }
        }
    }
    let reason = if breakdown {
        "two-FE categorical MAP PCG broke down and the MAP continuation did not certify"
    } else {
        "two-FE categorical MAP PCG exhausted its iteration budget without certification"
    };
    Err(invalid_data(reason))
}

// The finish operation runs in the same level task as accumulation. Probes
// normalize/project immediately; full solves validate raw group products first.
// clear_width preserves each caller's ownership of unused workspace lanes.
#[allow(clippy::too_many_arguments)]
fn fill_two_fe_group_rhs<C: ExactSizeIterator<Item = usize> + Clone + Sync>(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    stride: usize,
    bank: (&mut [f64], &mut [f64]),
    columns: C,
    clear_width: usize,
    finish: impl Fn(&[usize], &mut [f64], f64) + Sync,
) -> Result<()> {
    let width = columns.len();
    let source_width = values.ncols();
    let data = values.as_slice();
    bank.0
        .par_chunks_mut(stride)
        .zip(bank.1.par_iter_mut())
        .enumerate()
        .try_for_each(|(level, (sum, total))| -> Result<()> {
            sum[..clear_width].fill(0.0);
            *total = 0.0;
            let members = effect.level_index().checked_members(level)?;
            for &row in members {
                let weight = weights.normalized(row);
                *total += weight;
                let source = &data[row * source_width..(row + 1) * source_width];
                for (out, column) in sum[..width].iter_mut().zip(columns.clone()) {
                    *out += weight * source[column];
                }
            }
            finish(members, &mut sum[..width], *total);
            Ok(())
        })
}

fn subtract_two_fe_left_projection(
    members: &[usize],
    left_codes: &[u32],
    weights: ValidatedWeights<'_>,
    stride: usize,
    left_rhs: &[f64],
    sum: &mut [f64],
) {
    for &row in members {
        let weight = weights.normalized(row);
        let a = left_codes[row] as usize;
        let source = &left_rhs[a * stride..a * stride + sum.len()];
        for (out, &value) in sum.iter_mut().zip(source) {
            *out -= weight * value;
        }
    }
}

/// Independent RHS coordinates share a fixed-width tile, but each coordinate
/// still receives one separately rounded product and add/subtract per row.
#[inline(always)]
fn schur_row_update<const SUBTRACT: bool>(weight: f64, source: &[f64], output: &mut [f64]) {
    debug_assert_eq!(source.len(), output.len());
    const TILE: usize = 4;
    let end = source.len() / TILE * TILE;
    let (source_tiles, source_tail) = source.split_at(end);
    let (output_tiles, output_tail) = output.split_at_mut(end);
    for (source, output) in source_tiles
        .chunks_exact(TILE)
        .zip(output_tiles.chunks_exact_mut(TILE))
    {
        for rhs in 0..TILE {
            let product = weight * source[rhs];
            if SUBTRACT {
                output[rhs] -= product;
            } else {
                output[rhs] += product;
            }
        }
    }
    for (&source, output) in source_tail.iter().zip(output_tail) {
        let product = weight * source;
        if SUBTRACT {
            *output -= product;
        } else {
            *output += product;
        }
    }
}

impl TwoFePcgWorkspace {
    fn apply_schur(
        &mut self,
        design: &BorrowedInterceptFeDesign<'_>,
        weights: ValidatedWeights<'_>,
        rhs_count: usize,
        stride: usize,
    ) -> Result<()> {
        if crate::parallel::StagePlan::current(
            design.nobs() as u128 * rhs_count as u128,
            design
                .effects()
                .iter()
                .map(|e| e.n_levels())
                .max()
                .unwrap_or(0),
        )
        .lanes
            > 1
        {
            return self.apply_schur_parallel(design, weights, rhs_count, stride);
        }
        self.left_work.fill(0.0);
        self.operator.fill(0.0);
        let left = &design.effects()[self.left_effect];
        let right = &design.effects()[self.right_effect];
        let left_codes = left.codes();
        let right_codes = right.codes();
        for row in 0..design.nobs() {
            let weight = weights.normalized(row);
            let left_level = left_codes[row] as usize;
            let right_level = right_codes[row] as usize;
            schur_row_update::<false>(
                weight,
                &self.direction[right_level * stride..right_level * stride + rhs_count],
                &mut self.left_work[left_level * stride..left_level * stride + rhs_count],
            );
        }
        for (work, &denominator) in self
            .left_work
            .chunks_exact_mut(stride)
            .zip(&self.left_weights)
        {
            for value in &mut work[..rhs_count] {
                *value /= denominator;
            }
        }
        for ((operator, direction), &right_weight) in self
            .operator
            .chunks_exact_mut(stride)
            .zip(self.direction.chunks_exact(stride))
            .zip(&self.right_weights)
        {
            for (value, &term) in operator[..rhs_count]
                .iter_mut()
                .zip(&direction[..rhs_count])
            {
                *value = right_weight * term;
            }
        }
        for row in 0..design.nobs() {
            let weight = weights.normalized(row);
            let left_level = left_codes[row] as usize;
            let right_level = right_codes[row] as usize;
            schur_row_update::<true>(
                weight,
                &self.left_work[left_level * stride..left_level * stride + rhs_count],
                &mut self.operator[right_level * stride..right_level * stride + rhs_count],
            );
        }
        if self.operator.iter().any(|value| !value.is_finite()) {
            Err(invalid_data(
                "two-FE categorical MAP PCG operator product is nonfinite",
            ))
        } else {
            Ok(())
        }
    }

    fn apply_schur_parallel(
        &mut self,
        design: &BorrowedInterceptFeDesign<'_>,
        weights: ValidatedWeights<'_>,
        rhs_count: usize,
        stride: usize,
    ) -> Result<()> {
        let left = &design.effects()[self.left_effect];
        let right = &design.effects()[self.right_effect];
        let left_codes = left.codes();
        let right_codes = right.codes();
        let direction = &self.direction;
        let left_weights = &self.left_weights;
        let left_plan = crate::parallel::StagePlan::current(
            design.nobs() as u128 * rhs_count as u128,
            left.n_levels(),
        );
        left_plan.groups_mut(
            &mut self.left_work,
            stride,
            left.level_index().membership_offsets(),
            |first_level, work| {
                work.fill(0.0);
                for (local_level, level_work) in work.chunks_exact_mut(stride).enumerate() {
                    let level = first_level + local_level;
                    for &row in left.level_index().checked_members(level)? {
                        if row >= design.nobs() {
                            return Err(invalid_argument(
                                "two-FE PCG membership exceeds the observation axis",
                            ));
                        }
                        let weight = weights.normalized(row);
                        let right_level = right_codes[row] as usize;
                        let source =
                            &direction[right_level * stride..right_level * stride + rhs_count];
                        schur_row_update::<false>(weight, source, &mut level_work[..rhs_count]);
                    }
                    for value in &mut level_work[..rhs_count] {
                        *value /= left_weights[level];
                    }
                }
                Ok(())
            },
        )?;
        let left_work = &self.left_work;
        let right_weights = &self.right_weights;
        let right_plan = crate::parallel::StagePlan::current(
            design.nobs() as u128 * rhs_count as u128,
            right.n_levels(),
        );
        right_plan.groups_mut(
            &mut self.operator,
            stride,
            right.level_index().membership_offsets(),
            |first_level, operator| {
                operator.fill(0.0);
                for (local_level, level_operator) in operator.chunks_exact_mut(stride).enumerate() {
                    let level = first_level + local_level;
                    let source = &direction[level * stride..level * stride + rhs_count];
                    for (value, &term) in level_operator[..rhs_count].iter_mut().zip(source) {
                        *value = right_weights[level] * term;
                    }
                    for &row in right.level_index().checked_members(level)? {
                        if row >= design.nobs() {
                            return Err(invalid_argument(
                                "two-FE PCG membership exceeds the observation axis",
                            ));
                        }
                        let weight = weights.normalized(row);
                        let left_level = left_codes[row] as usize;
                        let source =
                            &left_work[left_level * stride..left_level * stride + rhs_count];
                        schur_row_update::<true>(weight, source, &mut level_operator[..rhs_count]);
                    }
                }
                Ok(())
            },
        )?;
        if self.operator.iter().any(|value| !value.is_finite()) {
            Err(invalid_data(
                "two-FE categorical MAP PCG operator product is nonfinite",
            ))
        } else {
            Ok(())
        }
    }
}

fn trace_two_fe_pcg_stop(rhs: usize, iterations: usize, reason: &str, target: f64) {
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=two_fe_pcg_rhs_stop|rhs={rhs}|iterations={iterations}|reason={reason}|recurrence_tolerance={target:.17e}"
        ));
    }
}

fn two_fe_pcg_rhs_ready(
    graph: &TwoFePcgWorkspace,
    certificate_workspace: &BorrowedMapWorkspace,
    rhs: usize,
    stride: usize,
    recurrence_tolerance: f64,
    scale_components: bool,
) -> Result<bool> {
    for level in 0..graph.right_weights.len() {
        let denominator = graph.right_weights[level];
        let scaled_denominator = denominator
            / if scale_components {
                graph.component_scales[level]
            } else {
                1.0
            };
        let mean = graph.residual[level * stride + rhs] / scaled_denominator;
        let reference = certificate_workspace.reference_norms[rhs];
        let relative = if reference == 0.0 {
            if mean == 0.0 {
                0.0
            } else {
                f64::INFINITY
            }
        } else {
            // Match the weighted group-moment certificate; a mean alone can
            // stop early for large groups and trigger expensive MAP polishing.
            (mean.abs() / reference) * denominator.sqrt().max(1.0)
        };
        if !relative.is_finite() || relative > recurrence_tolerance {
            return Ok(false);
        }
    }
    Ok(true)
}

fn reconstruct_two_fe_pcg_candidate(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    graph: &mut TwoFePcgWorkspace,
    values: &mut DenseMatrix,
    rhs_count: usize,
    stride: usize,
) -> Result<()> {
    let left = &design.effects()[graph.left_effect];
    let right = &design.effects()[graph.right_effect];
    let left_codes = left.codes();
    let right_codes = right.codes();
    let solution = &graph.solution;
    let left_weights = &graph.left_weights;
    let left_rhs = &graph.left_rhs;
    graph
        .left_work
        .par_chunks_mut(stride)
        .enumerate()
        .try_for_each(|(level, sum)| -> Result<()> {
            sum.fill(0.0);
            for &row in left.level_index().checked_members(level)? {
                let weight = weights.normalized(row);
                let right_level = right_codes[row] as usize;
                let source = &solution[right_level * stride..right_level * stride + rhs_count];
                for (out, &value) in sum[..rhs_count].iter_mut().zip(source) {
                    *out += weight * value;
                }
            }
            let source = &left_rhs[level * stride..level * stride + rhs_count];
            for (out, &value) in sum[..rhs_count].iter_mut().zip(source) {
                *out = value - *out / left_weights[level];
            }
            Ok(())
        })?;
    let left_work = &graph.left_work;
    values
        .as_mut_slice()
        .par_chunks_mut(rhs_count)
        .enumerate()
        .try_for_each(|(row, output)| -> Result<()> {
            let left_level = left_codes[row] as usize;
            let right_level = right_codes[row] as usize;
            let a = &left_work[left_level * stride..left_level * stride + rhs_count];
            let b = &solution[right_level * stride..right_level * stride + rhs_count];
            for ((out, &av), &bv) in output.iter_mut().zip(a).zip(b) {
                *out -= av + bv;
                if !out.is_finite() {
                    return Err(invalid_data(
                        "two-FE categorical MAP PCG reconstruction is nonfinite",
                    ));
                }
            }
            Ok(())
        })?;

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct ValidatedWeights<'a> {
    values: Option<&'a [f64]>,
    raw_scale: f64,
}

impl ValidatedWeights<'_> {
    #[must_use]
    fn normalized(self, observation: usize) -> f64 {
        self.values
            .map_or(1.0, |values| values[observation] / self.raw_scale)
    }
}

/// Residualize a nonempty dense matrix against a validated borrowed,
/// intercept-only FE design.
///
/// The matrix is consumed so a failure cannot expose partially residualized
/// caller state. Every numerical workspace is fallibly allocated before the
/// first projection. A successful return always has a passing independently
/// recomputed fixed-effect certificate.
pub fn absorb_borrowed_intercept_map_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    absorb_borrowed_intercept_map(design, weights, values, config, false, false)
}

/// Symmetric MAP over a borrowed design. With `structural_constant`, an empty
/// explicit design still owns a one-level constant whose global projection
/// uses only the existing dense RHS plus O(K) workspace; it never materializes
/// N constant codes. `retain_coefficients` also returns the coefficient
/// solutions that produced the certified residuals.
pub(crate) fn absorb_borrowed_intercept_map(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    structural_constant: bool,
    retain_coefficients: bool,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_with_reservation_mode(
        design,
        weights,
        values,
        config,
        structural_constant,
        retain_coefficients,
        &mut reservation,
    )
}

/// Options for one borrowed CG solve; the default is an ordinary solve.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct BorrowedCgOptions<'a> {
    /// An empty explicit design owns a structural one-level constant.
    pub(crate) structural_constant: bool,
    /// Also return the coefficient solutions behind the certified residuals.
    pub(crate) retain_coefficients: bool,
    /// Stable group actions avoid subtracting nearly identical
    /// observation-space projections during the recurrence; saved-model
    /// recovery needs them for the complete product of a fresh raw RHS.
    pub(crate) stable_projection: bool,
    /// Certify against these original right-hand-side norms instead of the
    /// input's own.
    pub(crate) reference_norms: Option<&'a [f64]>,
}

impl<'a> BorrowedCgOptions<'a> {
    /// Certify a candidate residual against the original right-hand-side norms
    /// and use CG only for any remaining FE component. This is the polishing
    /// boundary of coefficient-space solvers such as CMG, whose bulk solve
    /// happens outside the observation-space CG recurrence.
    pub(crate) fn polish(reference_norms: &'a [f64]) -> Self {
        Self {
            stable_projection: true,
            reference_norms: Some(reference_norms),
            ..Self::default()
        }
    }
}

/// Residualize with batched weighted conjugate gradients over the borrowed
/// symmetric-Kaczmarz projection operator. No observation-scale categorical
/// codes or unit-weight vector are copied.
pub(crate) fn absorb_borrowed_intercept_cg(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    options: BorrowedCgOptions<'_>,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_cg_with_reservation(
        design,
        weights,
        values,
        config,
        options.structural_constant,
        CgContinuation {
            reference_norms: options.reference_norms,
            retain_coefficients: options.retain_coefficients,
            stable_projection: options.stable_projection,
            ..CgContinuation::ORDINARY
        },
        &mut reservation,
    )
}

/// Residualize through modified LSMR and optionally retain the coefficient
/// solutions that produced the certified observation-space residuals.
pub(crate) fn absorb_borrowed_intercept_lsmr(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: LsmrConfig,
    retain_coefficients: bool,
) -> Result<CertifiedBorrowedMap> {
    let validated = validate_weights(weights, values.nrows())?;
    let (residualized, report, _, coefficients) = run_borrowed_lsmr(
        design,
        validated,
        &values,
        config.tolerance,
        config.max_iterations,
        super::auto::explicit_lsmr_recurrence(config),
        None,
        retain_coefficients,
    )?;
    if let Some(coefficients) = coefficients {
        CertifiedBorrowedMap::from_certified_parts_with_coefficients(
            residualized,
            report,
            coefficients,
        )
    } else {
        CertifiedBorrowedMap::from_certified_parts(residualized, report)
    }
}

fn matrix_infinity_norm(values: &DenseMatrix) -> Result<f64> {
    let maximum = if values.as_slice().len() >= PARALLEL_PROJECTION_MIN_ROWS
        && rayon::current_num_threads() > 1
    {
        values
            .as_slice()
            .par_iter()
            .map(|value| value.abs())
            .reduce(|| 0.0_f64, f64::max)
    } else {
        values
            .as_slice()
            .iter()
            .fold(0.0_f64, |maximum, value| maximum.max(value.abs()))
    };
    if maximum.is_finite() {
        Ok(maximum)
    } else {
        Err(invalid_data(
            "borrowed CG recovery infinity norm is not finite",
        ))
    }
}

fn relative_reconstruction_infinity_norm(
    residual: &DenseMatrix,
    target_infinity: f64,
) -> Result<f64> {
    if !target_infinity.is_finite() || target_infinity < 0.0 {
        return Err(invalid_argument(
            "borrowed CG recovery target scale is invalid",
        ));
    }
    let residual_infinity = matrix_infinity_norm(residual)?;
    let relative = if target_infinity == 0.0 {
        residual_infinity
    } else {
        residual_infinity / target_infinity
    };
    if relative.is_finite() {
        Ok(relative)
    } else {
        Err(invalid_data(
            "borrowed CG recovery reconstruction ratio is not finite",
        ))
    }
}

/// Residualize with safeguarded Irons-Tuck extrapolation while optionally
/// carrying the same linear combinations through compact FE coefficient
/// space. This avoids a second recovery solve for explicit `solver(it)`.
pub(crate) fn absorb_borrowed_intercept_irons_tuck_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    mut values: DenseMatrix,
    config: MapConfig,
    retain_coefficients: bool,
) -> Result<CertifiedBorrowedMap> {
    validate_config(config)?;
    if design.nobs() != values.nrows()
        || values.nrows() == 0
        || values.ncols() == 0
        || design.effects().is_empty()
    {
        return Err(invalid_argument(
            "borrowed Irons-Tuck inputs are empty or dimensionally inconsistent",
        ));
    }
    if values.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(invalid_data("borrowed Irons-Tuck values must be finite"));
    }
    let weights = validate_weights(weights, values.nrows())?;
    let setup_started = Instant::now();
    let mut reservation = SystemWorkspaceReservation;
    let mut workspace =
        BorrowedMapWorkspace::new(design, values.nrows(), values.ncols(), &mut reservation)?;
    update_workspace_norms(&values, weights, &mut workspace, true)?;
    let setup_seconds = setup_started.elapsed().as_secs_f64();
    let solve_started = Instant::now();
    let initial_certificate = certify(design, weights, &values, config.tolerance, &mut workspace)?;
    let mut coefficients = retain_coefficients
        .then(|| FeCoefficientBatch::intercept_zeros(design, values.ncols()))
        .transpose()?;
    // A passing certificate earns one confirming sweep when the budget
    // allows; the solve finishes only if the confirmed certificate passes.
    let confirm = |values: &mut DenseMatrix,
                   workspace: &mut BorrowedMapWorkspace,
                   coefficients: Option<&mut FeCoefficientBatch>,
                   iterations: &mut usize,
                   certificate: FeCertificate|
     -> Result<FeCertificate> {
        if *iterations >= config.max_iterations {
            return Ok(certificate);
        }
        borrowed_residual_sweep_recording(
            design,
            weights,
            values,
            workspace,
            coefficients,
            config.symmetric,
        )?;
        *iterations += 1;
        certify(design, weights, values, config.tolerance, workspace)
    };
    let finish = |values: DenseMatrix,
                  coefficients: Option<FeCoefficientBatch>,
                  iterations: usize,
                  certificate: FeCertificate|
     -> Result<CertifiedBorrowedMap> {
        let normal_equation_residual = certificate.relative_two_norm;
        let result = certified_solver_result(
            values,
            FeSolverKind::IronsTuck,
            SolveStopReason::Certified,
            iterations,
            certificate,
            normal_equation_residual,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        )?;
        attach_optional_coefficients(result, coefficients)
    };
    if initial_certificate.passed {
        let result = certified_solver_result(
            values,
            FeSolverKind::IronsTuck,
            SolveStopReason::Certified,
            0,
            initial_certificate,
            0.0,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        )?;
        return attach_optional_coefficients(result, coefficients);
    }

    let mut iterations = 0_usize;
    while iterations < config.max_iterations {
        crate::interrupt::checkpoint()?;
        let x0 = fallible_matrix_copy(&values, "unable to reserve Irons-Tuck x0")?;
        let c0 = coefficients
            .as_ref()
            .map(FeCoefficientBatch::try_clone)
            .transpose()?;
        borrowed_residual_sweep_recording(
            design,
            weights,
            &mut values,
            &mut workspace,
            coefficients.as_mut(),
            config.symmetric,
        )?;
        iterations += 1;
        let mut certificate = certify(design, weights, &values, config.tolerance, &mut workspace)?;
        if certificate.passed {
            certificate = confirm(
                &mut values,
                &mut workspace,
                coefficients.as_mut(),
                &mut iterations,
                certificate,
            )?;
            if !certificate.passed {
                continue;
            }
            return finish(values, coefficients, iterations, certificate);
        }
        if iterations == config.max_iterations {
            break;
        }

        let x1 = fallible_matrix_copy(&values, "unable to reserve Irons-Tuck x1")?;
        let c1 = coefficients
            .as_ref()
            .map(FeCoefficientBatch::try_clone)
            .transpose()?;
        borrowed_residual_sweep_recording(
            design,
            weights,
            &mut values,
            &mut workspace,
            coefficients.as_mut(),
            config.symmetric,
        )?;
        iterations += 1;
        let mut baseline = certify(design, weights, &values, config.tolerance, &mut workspace)?;
        if baseline.passed {
            baseline = confirm(
                &mut values,
                &mut workspace,
                coefficients.as_mut(),
                &mut iterations,
                baseline,
            )?;
            if !baseline.passed {
                continue;
            }
            return finish(values, coefficients, iterations, baseline);
        }

        if let Some((candidate, alpha)) = borrowed_irons_tuck_candidate(&x0, &x1, &values)? {
            let mut candidate_certificate = certify(
                design,
                weights,
                &candidate,
                config.tolerance,
                &mut workspace,
            )?;
            if candidate_certificate.relative_two_norm < baseline.relative_two_norm {
                let candidate_coefficients = match (c0.as_ref(), c1.as_ref(), coefficients.as_ref())
                {
                    (Some(c0), Some(c1), Some(c2)) => Some(
                        FeCoefficientBatch::extrapolate_irons_tuck(c0, c1, c2, alpha)?,
                    ),
                    _ => None,
                };
                values = candidate;
                coefficients = candidate_coefficients;
                if candidate_certificate.passed {
                    candidate_certificate = confirm(
                        &mut values,
                        &mut workspace,
                        coefficients.as_mut(),
                        &mut iterations,
                        candidate_certificate,
                    )?;
                    if !candidate_certificate.passed {
                        continue;
                    }
                    return finish(values, coefficients, iterations, candidate_certificate);
                }
            }
        }
    }

    Err(invalid_data(
        "borrowed Irons-Tuck exhausted its iteration budget without certification",
    ))
}

/// Check an unchanged candidate against the same independent borrowed FE
/// certificate used at solver entry. This performs no projection or solver
/// iteration and is used when a caller expects a residual to already lie in
/// the within space.
#[cfg(test)]
pub(crate) fn certify_borrowed_intercept_candidate(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: &DenseMatrix,
    config: MapConfig,
) -> Result<FeCertificate> {
    certify_borrowed_intercept_candidate_with_workspace(design, weights, values, config)
        .map(|(certificate, _)| certificate)
}

pub(crate) fn certify_borrowed_intercept_candidate_with_workspace(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: &DenseMatrix,
    config: MapConfig,
) -> Result<(FeCertificate, SolverWorkspacePeak)> {
    validate_config(config)?;
    if design.nobs() != values.nrows() || values.nrows() == 0 || values.ncols() == 0 {
        return Err(invalid_argument(
            "borrowed certificate design and matrix dimensions are inconsistent",
        ));
    }
    if values.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(invalid_data(
            "borrowed certificate candidate contains a nonfinite value",
        ));
    }
    let weights = validate_weights(weights, values.nrows())?;
    let mut reservation = SystemWorkspaceReservation;
    let mut workspace =
        BorrowedMapWorkspace::new(design, values.nrows(), values.ncols(), &mut reservation)?;
    update_workspace_norms(values, weights, &mut workspace, true)?;
    let peak = workspace.payload_peak()?;
    let certificate = certify_initial(design, weights, values, config.tolerance, &mut workspace)?;
    Ok((certificate, peak))
}

/// Shared structural automatic policy. Descriptor copies contain only borrowed
/// memberships/codes; every current-weight certificate remains independent.
#[allow(clippy::too_many_arguments)]
pub(crate) fn project_borrowed_intercept_structural(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: crate::ols::OlsProjectionInput,
    map: MapConfig,
    lsmr: LsmrConfig,
    budget: Option<FeSolverMemoryBudget>,
    structural_constant: bool,
    retain: bool,
    final_residual: bool,
    dense_norms: bool,
    three_fe_geometry: Option<three_fe_map::Geometry>,
) -> Result<CertifiedBorrowedMap> {
    validate_config(map)?;
    if values.nrows() != design.nobs() || values.nrows() == 0 || values.ncols() == 0 {
        return Err(invalid_argument(
            "structural FE projection dimensions are inconsistent",
        ));
    }
    values.validate_finite("structural FE projection values must be finite")?;
    let values = values.into_values();
    validate_weights(weights, values.nrows())?;
    if design.effects().len() <= 1 {
        let bytes = BorrowedMapWorkspace::planned_payload_bytes(
            design,
            values.nrows(),
            values.ncols(),
            rayon::current_num_threads(),
        )?;
        if budget.is_some_and(|b| !b.fits(bytes)) {
            return Err(allocation(
                "direct FE projection exceeds admitted workspace",
            ));
        }
        return mark_auto(
            absorb_borrowed_intercept_map(
                design,
                weights,
                values,
                map,
                structural_constant,
                false,
            )?,
            0,
            0.,
            0.,
            0.,
        );
    }
    let levels = design
        .effects()
        .iter()
        .map(|e| e.n_levels())
        .collect::<Vec<_>>();
    let estimate = estimate_fe_solver_memory(
        values.nrows(),
        values.ncols(),
        &levels,
        rayon::current_num_threads(),
        lsmr.local_size,
    )?;
    if design.effects().len() == 2 {
        let graph_bytes = ReusableCategoricalMapAbsorber::owner_bytes(
            design,
            values.ncols(),
            rayon::current_num_threads(),
        )?;
        let product_bytes = if retain {
            estimate
                .coefficient_batch_bytes
                .checked_mul(3)
                .ok_or_else(|| allocation("FE retained graph product overflows"))?
        } else {
            0
        };
        let bytes = graph_bytes
            .checked_add(product_bytes)
            .ok_or_else(|| allocation("FE graph overlap overflows"))?;
        if budget.is_none_or(|b| b.fits(bytes)) {
            let mut effects = Vec::new();
            effects
                .try_reserve_exact(2)
                .map_err(|_| allocation("unable to reserve FE binding"))?;
            effects.extend_from_slice(design.effects());
            let binding = BorrowedInterceptFeDesign::new(design.nobs(), effects)?;
            let mut graph = ReusableCategoricalMapAbsorber::new(
                binding,
                values.ncols(),
                rayon::current_num_threads(),
            )?;
            graph.retain_coefficients = retain;
            let mut result = graph.absorb_ols(weights, values.into(), map, final_residual)?;
            result.report.requested_solver = FeSolverKind::Auto;
            result.report.solver_workspace_peak = result
                .report
                .solver_workspace_peak
                .max(SolverWorkspacePeak::from_bytes(bytes)?);
            // The owner accounts for its fixed banks, while the standalone
            // borrowed result predates owner-level admission.
            if result.report.solver_workspace_peak.bytes().is_none() {
                result.report.solver_workspace_peak = SolverWorkspacePeak::from_bytes(bytes)?;
            }
            return Ok(result);
        }
        let compact = if retain {
            estimate
                .cg_bytes
                .checked_add(
                    estimate
                        .coefficient_batch_bytes
                        .checked_mul(5)
                        .ok_or_else(|| allocation("compact coefficient banks overflow"))?,
                )
                .ok_or_else(|| allocation("compact FE overlap overflows"))?
        } else {
            estimate.cg_bytes
        };
        if budget.is_none_or(|b| b.fits(compact)) {
            let options = BorrowedCgOptions {
                retain_coefficients: retain,
                stable_projection: retain,
                ..BorrowedCgOptions::default()
            };
            let mut result = absorb_borrowed_intercept_cg(design, weights, values, map, options)?;
            result.report.requested_solver = FeSolverKind::Auto;
            result.report.solver_workspace_peak = result
                .report
                .solver_workspace_peak
                .max(SolverWorkspacePeak::from_bytes(compact)?);
            return Ok(result);
        }
    }
    let map_first = three_fe_geometry.and_then(|g| g.map_checkpoint(weights, map, final_residual));
    project_general_categorical(
        design,
        weights,
        values,
        map,
        lsmr,
        budget,
        retain,
        estimate,
        dense_norms,
        map_first,
    )
}

#[allow(clippy::too_many_arguments)]
fn project_general_categorical(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    map: MapConfig,
    mut lsmr: LsmrConfig,
    budget: Option<FeSolverMemoryBudget>,
    retain: bool,
    estimate: crate::memory::FeSolverMemoryEstimate,
    dense_norms: bool,
    map_first: Option<usize>,
) -> Result<CertifiedBorrowedMap> {
    lsmr.tolerance = super::auto::direct_lsmr_tolerance(map.tolerance);
    lsmr.max_iterations = lsmr.max_iterations.min((map.max_iterations / 2).max(1));
    let mut pilot = if let Some(checkpoint) =
        map_first.filter(|_| lsmr.max_iterations > three_fe_map::SWEEPS)
    {
        three_fe_map::attempt(
            design,
            &values,
            map,
            budget,
            retain,
            estimate.coefficient_batch_bytes,
            checkpoint,
        )?
    } else {
        three_fe_map::Attempt::default()
    };
    if let Some(mut result) = pilot.result.take() {
        result.report.requested_solver = FeSolverKind::Auto;
        result.report.pilot_iterations = pilot.iterations;
        return Ok(result);
    }
    lsmr.max_iterations -= pilot.iterations;
    let validated = validate_weights(weights, values.nrows())?;
    let preconditioner = resolve_borrowed_lsmr_preconditioner(
        design.effects().len(),
        design.total_term_slots(),
        lsmr.preconditioner,
    );
    let bytes = estimate.lsmr_bytes(preconditioner);
    let mut attempted_peak = pilot.peak;
    let mut attempted = None;
    if budget.is_none_or(|b| b.fits(bytes)) {
        let mut memory = BorrowedLsmrCallMemory::new(estimate, preconditioner, budget, 0)?;
        let mut norms = Vec::new();
        let (candidate, mut report, _, coefficients) = run_borrowed_lsmr_with_prepared(
            design,
            validated,
            &values,
            lsmr.tolerance,
            lsmr.max_iterations,
            lsmr,
            None,
            retain,
            None,
            &mut memory,
            dense_norms.then_some(&mut norms),
        )?;
        attempted_peak = attempted_peak.max(report.solver_workspace_peak);
        report.requested_solver = FeSolverKind::Auto;
        if report.certified() {
            pilot.record(&mut report);
            let result = if let Some(c) = coefficients {
                CertifiedBorrowedMap::from_certified_parts_with_coefficients(candidate, report, c)
            } else {
                CertifiedBorrowedMap::from_certified_parts(candidate, report)
            }?;
            return if norms.is_empty() {
                Ok(result)
            } else {
                result.with_residual_norms(norms)
            };
        }
        // The candidate and coefficient bank are released before continuation.
        // A report survives the failed certificate; input remains unchanged.
        attempted = Some(report);
    }
    let consumed = pilot.iterations + attempted.as_ref().map_or(0, |r| r.iterations);
    let remaining = map
        .max_iterations
        .checked_sub(consumed)
        .filter(|n| *n > 0)
        .ok_or_else(|| invalid_data("general FE attempt exhausted the shared iteration budget"))?;
    let map_bytes = BorrowedMapWorkspace::planned_payload_bytes(
        design,
        values.nrows(),
        values.ncols(),
        rayon::current_num_threads(),
    )?
    .checked_add(if retain {
        estimate.coefficient_batch_bytes
    } else {
        0
    })
    .ok_or_else(|| allocation("FE MAP continuation overlap overflows"))?;
    if budget.is_some_and(|b| !b.fits(map_bytes)) {
        return Err(allocation(
            "general FE continuation exceeds admitted workspace",
        ));
    }
    let config = MapConfig {
        max_iterations: remaining,
        ..map
    };
    let mut result = absorb_borrowed_intercept_map(design, weights, values, config, false, retain)?;
    result.report.requested_solver = FeSolverKind::Auto;
    result.report.solver_workspace_peak = result
        .report
        .solver_workspace_peak
        .max(attempted_peak)
        .max(SolverWorkspacePeak::from_bytes(map_bytes)?);
    if let Some(prior) = attempted {
        result.report.iterations += prior.iterations;
        result.report.setup_seconds += prior.setup_seconds;
        result.report.solve_seconds += prior.solve_seconds;
        result.report.certification_seconds += prior.certification_seconds;
        result.report.requested_preconditioner = prior.requested_preconditioner;
        result.report.used_preconditioner = prior.used_preconditioner;
        result.report.fallback_used = true;
        result.report.warnings.push("general FE solve did not certify; MAP used original inputs within the remaining budget".to_owned());
    }
    pilot.record(&mut result.report);
    Ok(result)
}

fn borrowed_weights(weights: ValidatedWeights<'_>) -> BorrowedMapWeights<'_> {
    match weights.values {
        Some(values) => BorrowedMapWeights::Provided(values),
        None => BorrowedMapWeights::Unit,
    }
}

/// A checkpoint's original RHSs, topology and solver weights are immutable for
/// this entire scope. Numerical setup is released before another weight frame
/// or MAP fallback; only fresh original-weight certification can accept output.
struct CheckpointLsmrBatch<'s, 'a> {
    design: &'s BorrowedInterceptFeDesign<'a>,
    values: &'s DenseMatrix,
    weights: ValidatedWeights<'s>,
    config: LsmrConfig,
    prepared: BorrowedLsmrPrepared<'a>,
    candidate: Option<DenseMatrix>,
    memory: BorrowedLsmrCallMemory,
    allowance: u128,
    // Developer fault plan and a fixed attempt record; shipped code runs the
    // same checks with no override and never allocates for the record.
    selective_allowance: Option<u128>,
    attempted_rhs: [usize; 4],
    attempt_count: usize,
}

impl<'s, 'a> CheckpointLsmrBatch<'s, 'a> {
    fn new(
        design: &'s BorrowedInterceptFeDesign<'a>,
        weights: ValidatedWeights<'s>,
        values: &'s DenseMatrix,
        config: LsmrConfig,
    ) -> Result<Self> {
        let levels: Vec<_> = design
            .effects()
            .iter()
            .map(|effect| effect.n_levels())
            .collect();
        let estimate = estimate_fe_solver_memory(
            values.nrows(),
            values.ncols(),
            &levels,
            rayon::current_num_threads(),
            config.local_size,
        )?;
        let preconditioner = resolve_borrowed_lsmr_preconditioner(
            levels.len(),
            design.total_term_slots(),
            config.preconditioner,
        );
        let allowance = estimate.lsmr_bytes(preconditioner);
        Ok(Self {
            design,
            values,
            weights,
            config,
            prepared: BorrowedLsmrPrepared::default(),
            candidate: None,
            memory: BorrowedLsmrCallMemory::new(
                estimate,
                preconditioner,
                Some(FeSolverMemoryBudget {
                    execution_bytes: 0,
                    usable_bytes: allowance,
                }),
                0,
            )?,
            allowance,
            selective_allowance: None,
            attempted_rhs: [0; 4],
            attempt_count: 0,
        })
    }

    fn selected_retry(&self, norms: &[f64], tolerance: f64) -> Result<Option<Vec<usize>>> {
        if norms.len() != self.values.ncols() || norms.iter().any(|v| !v.is_finite() || *v < 0.) {
            return Err(invalid_argument(
                "checkpoint retry certificate axis is invalid",
            ));
        }
        let count = norms.iter().filter(|&&norm| norm > tolerance).count();
        if count == 0 || count == norms.len() {
            return Ok(None);
        }
        let retained = self
            .candidate
            .as_ref()
            .ok_or_else(|| invalid_argument("selective checkpoint retry has no candidate"))?
            .storage_capacity() as u128
            * 8;
        let levels: Vec<_> = self
            .design
            .effects()
            .iter()
            .map(|effect| effect.n_levels())
            .collect();
        let estimate = estimate_fe_solver_memory(
            self.values.nrows(),
            count,
            &levels,
            rayon::current_num_threads(),
            self.config.local_size,
        )?;
        let preconditioner = resolve_borrowed_lsmr_preconditioner(
            levels.len(),
            self.design.total_term_slots(),
            self.config.preconditioner,
        );
        // The narrow model already charges one current-weight solver. Keep the
        // full result metadata allowance and explicitly add retained columns
        // plus the selection axis. If that cannot fit, release the candidate
        // before the established full retry instead of raising its memory peak.
        let without_positions = estimate
            .lsmr_bytes(preconditioner)
            .checked_add(retained)
            .and_then(|v| {
                v.checked_add(
                    self.memory
                        .metadata_bytes
                        .saturating_sub(estimate.lsmr_result_metadata_bytes),
                )
            })
            .ok_or_else(|| allocation("selective checkpoint overlap overflows"))?;
        let fits = |capacity: usize| {
            without_positions
                .checked_add(capacity as u128 * std::mem::size_of::<usize>() as u128)
                .is_some_and(|bytes| bytes <= self.allowance)
        };
        if self
            .selective_allowance
            .is_some_and(|limit| without_positions > limit)
        {
            return Ok(None);
        }
        if !fits(count) {
            return Ok(None);
        }
        let mut positions = Vec::new();
        if positions.try_reserve_exact(count).is_err() {
            return Ok(None);
        }
        if !fits(positions.capacity()) {
            return Ok(None);
        }
        positions.extend(
            norms
                .iter()
                .enumerate()
                .filter_map(|(rhs, &norm)| (norm > tolerance).then_some(rhs)),
        );
        Ok(Some(positions))
    }

    fn solve_attempt(
        &mut self,
        recurrence_tolerance: f64,
        retry_norms: Option<&[f64]>,
        canonical_tolerance: f64,
    ) -> Result<SolveReport> {
        let started = Instant::now();
        let selected = retry_norms
            .map(|norms| self.selected_retry(norms, canonical_tolerance))
            .transpose()?
            .flatten();
        if selected.is_none() {
            // Full retries never overlap the rejected output bank.
            self.candidate = None;
        }
        let rhs = match &selected {
            Some(positions) => super::lsmr_batch::selected_rhs_columns(self.values, positions)?,
            None => super::lsmr_batch::rhs_columns(self.values)?,
        };
        let views: Vec<_> = rhs.iter().map(Vec::as_slice).collect();
        let resolved = resolve_borrowed_lsmr_preconditioner(
            self.design.effects().len(),
            self.design.total_term_slots(),
            self.config.preconditioner,
        );
        let (mut solved, preconditioner_fallback) = self.prepared.solve(
            self.design,
            self.weights,
            &views,
            &LsmrOptions {
                tol: recurrence_tolerance,
                maxiter: self.config.max_iterations,
                local_size: self.config.local_size,
            },
            self.config.preconditioner,
            borrowed_within_preconditioner(resolved),
            self.config.threads.deterministic,
        )?;
        if let Some(slot) = self.attempted_rhs.get_mut(self.attempt_count) {
            *slot = rhs.len();
        }
        self.attempt_count = self.attempt_count.saturating_add(1);
        if crate::diagnostics::solver_trace_enabled() {
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=checkpoint_batch|rhs={}|full_rhs={}|selective={}|builds={}|batches={}",
                rhs.len(), self.values.ncols(), u8::from(selected.is_some()),
                self.prepared.builds, self.prepared.batches,
            ));
            for rhs in 0..solved.iterations.len() {
                let position = selected.as_ref().map_or(rhs, |positions| positions[rhs]);
                crate::diagnostics::emit_solver_trace(&format!(
                    "FEREG_SOLVER_TRACE|phase=checkpoint_batch_rhs|rhs={position}|converged={}|iterations={}|within_normal={:.17e}",
                    solved.converged[rhs], solved.iterations[rhs], solved.residual[rhs],
                ));
            }
        }
        drop(views);
        drop(rhs);
        let position_bytes = selected.as_ref().map_or(0, |v| {
            v.capacity() as u128 * std::mem::size_of::<usize>() as u128
        });
        let candidate_cells = self.candidate.as_ref().map_or(
            self.values.nrows() as u128 * self.values.ncols() as u128,
            |values| values.storage_capacity() as u128,
        );
        self.memory.include_payload(
            self.prepared.retained_bytes,
            (candidate_cells + solved.demeaned.capacity() as u128 + solved.x.capacity() as u128)
                * 8
                + position_bytes,
        )?;
        if self.candidate.is_none() {
            self.candidate = Some(DenseMatrix::zeros(
                self.values.nrows(),
                self.values.ncols(),
            )?);
        }
        let candidate = self
            .candidate
            .as_mut()
            .expect("checkpoint result was allocated");
        self.memory.include_payload(
            self.prepared.retained_bytes,
            (candidate.storage_capacity() as u128
                + solved.demeaned.capacity() as u128
                + solved.x.capacity() as u128)
                * 8
                + position_bytes,
        )?;
        if let Some(positions) = &selected {
            super::lsmr_batch::copy_selected_demeaned(&solved.demeaned, candidate, positions);
        } else {
            super::lsmr_batch::copy_demeaned(&solved.demeaned, candidate);
        }
        drop(std::mem::take(&mut solved.demeaned));
        drop(std::mem::take(&mut solved.x));
        let mut warnings: Vec<_> = solved
            .warnings
            .iter()
            .map(|warning| format!("within preconditioner: {warning:?}"))
            .collect();
        if preconditioner_fallback {
            warnings.insert(0, "automatic checkpoint LSMR preconditioner failed its positive-definiteness guard; retried unpreconditioned".to_owned());
        }
        Ok(SolveReport {
            checkpoint_retries: 0,
            solver_workspace_peak: self.memory.peak,
            requested_solver: FeSolverKind::Lsmr,
            used_solver: FeSolverKind::Lsmr,
            iterations: solved.iterations.iter().copied().max().unwrap_or(0),
            stop_reason: SolveStopReason::Uncertified,
            // The caller replaces this only with its fresh original-weight
            // certificate. A recurrence stop never authorizes this checkpoint.
            certificate: FeCertificate {
                passed: false,
                ..zero_certificate()
            },
            normal_equation_residual: solved.residual.iter().copied().fold(0., f64::max),
            setup_seconds: solved.time_setup
                + (started.elapsed().as_secs_f64() - solved.time_total).max(0.),
            solve_seconds: (solved.time_total - solved.time_setup).max(0.),
            certification_seconds: 0.,
            fallback_used: preconditioner_fallback,
            requested_preconditioner: Some(self.config.preconditioner),
            used_preconditioner: Some(if preconditioner_fallback {
                LsmrPreconditioner::Off
            } else {
                resolved
            }),
            pilot_iterations: 0,
            pilot_contraction: 0.,
            warnings,
        })
    }
}

/// Own one fixed operator during low-memory OLS projection. The estimator drops
/// it before QR/VCE; a CG continuation releases it before allocating CG state.
pub(crate) struct BorrowedLsmrAbsorber<'a> {
    design: BorrowedInterceptFeDesign<'a>,
    generation: Option<super::MapGeometryGeneration>,
    prepared: BorrowedLsmrPrepared<'a>,
}

impl<'a> BorrowedLsmrAbsorber<'a> {
    pub(crate) fn new(
        design: BorrowedInterceptFeDesign<'a>,
        weights: BorrowedMapWeights<'_>,
        config: super::AbsorbConfig,
    ) -> Result<Self> {
        if design.effects().is_empty()
            || !matches!(config.solver, FeSolverKind::Auto | FeSolverKind::Lsmr)
        {
            return Err(invalid_argument(
                "retained LSMR requires a fixed nonempty LSMR operator",
            ));
        }
        validate_weights(weights, design.nobs())?;
        Ok(Self {
            design,
            generation: None,
            prepared: BorrowedLsmrPrepared::default(),
        })
    }

    pub(crate) fn into_design(self) -> BorrowedInterceptFeDesign<'a> {
        self.design
    }

    pub(crate) fn absorb(
        &mut self,
        raw_weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        generation: super::MapGeometryGeneration,
        config: super::AbsorbConfig,
        dense_norms: bool,
    ) -> Result<CertifiedBorrowedMap> {
        // Invalidate numerical geometry before a fallible current-frame check.
        // Only the enclosing immutable binding issues a generation identity.
        if self.generation.take() != Some(generation) {
            self.prepared = BorrowedLsmrPrepared::default();
            self.generation = None;
        }
        let weights = validate_weights(raw_weights, values.nrows())?;
        let levels = self
            .design
            .effects()
            .iter()
            .map(|effect| effect.n_levels())
            .collect::<Vec<_>>();
        let estimate = estimate_fe_solver_memory(
            values.nrows(),
            values.ncols(),
            &levels,
            config
                .lsmr
                .threads
                .resolved()
                .max(rayon::current_num_threads()),
            config.lsmr.local_size,
        )?;
        let preconditioner = resolve_borrowed_lsmr_preconditioner(
            levels.len(),
            self.design.total_term_slots(),
            config.lsmr.preconditioner,
        );
        let mut memory =
            BorrowedLsmrCallMemory::new(estimate, preconditioner, config.solver_memory_budget, 0)?;
        let mut norms = Vec::new();
        let (residualized, report, _, coefficients) = run_borrowed_lsmr_with_prepared(
            &self.design,
            weights,
            &values,
            config.lsmr.tolerance,
            config.lsmr.max_iterations,
            super::auto::explicit_lsmr_recurrence(config.lsmr),
            None,
            config.retain_coefficients,
            Some(&mut self.prepared),
            &mut memory,
            dense_norms.then_some(&mut norms),
        )?;
        let result = if let Some(coefficients) = coefficients {
            CertifiedBorrowedMap::from_certified_parts_with_coefficients(
                residualized,
                report,
                coefficients,
            )
        } else {
            CertifiedBorrowedMap::from_certified_parts(residualized, report)
        }?;
        self.generation = Some(generation);
        if norms.is_empty() {
            Ok(result)
        } else {
            result.with_residual_norms(norms)
        }
    }
}

// Keep the maximum even if postprocessing fails and auto continues to CG.
struct BorrowedLsmrCallMemory {
    peak: SolverWorkspacePeak,
    metadata_bytes: u128,
    budget: Option<FeSolverMemoryBudget>,
    parent_bytes: u128,
}

impl BorrowedLsmrCallMemory {
    fn new(
        estimate: crate::memory::FeSolverMemoryEstimate,
        preconditioner: LsmrPreconditioner,
        budget: Option<FeSolverMemoryBudget>,
        parent_bytes: u128,
    ) -> Result<Self> {
        let mut memory = Self {
            peak: SolverWorkspacePeak::ZERO,
            metadata_bytes: estimate.lsmr_result_metadata_bytes,
            budget,
            parent_bytes,
        };
        memory.include(estimate.lsmr_bytes(preconditioner))?;
        Ok(memory)
    }

    fn include(&mut self, bytes: u128) -> Result<()> {
        let bytes = bytes
            .checked_add(self.parent_bytes)
            .ok_or_else(|| allocation("LSMR parent overlap overflows"))?;
        if self.budget.is_some_and(|budget| !budget.fits(bytes)) {
            return Err(allocation(
                "LSMR workspace exceeds the joint execution plan",
            ));
        }
        self.peak = self.peak.max(SolverWorkspacePeak::from_bytes(bytes)?);
        Ok(())
    }

    fn include_payload(&mut self, retained: u128, payload: u128) -> Result<()> {
        self.include(
            retained
                .checked_add(payload)
                .and_then(|bytes| bytes.checked_add(self.metadata_bytes))
                .ok_or_else(|| allocation("LSMR result overlap overflows"))?,
        )
    }
}

#[derive(Default)]
struct BorrowedLsmrPrepared<'a> {
    solver: Option<within::Solver<'a>>,
    retained_bytes: u128,
    preconditioner_fallback: bool,
    builds: usize,
    batches: usize,
    disabled: bool,
}

impl<'a> BorrowedLsmrPrepared<'a> {
    #[allow(clippy::too_many_arguments)]
    fn solve(
        &mut self,
        design: &BorrowedInterceptFeDesign<'a>,
        weights: ValidatedWeights<'_>,
        rhs: &[&[f64]],
        options: &LsmrOptions,
        requested: LsmrPreconditioner,
        preconditioner: PreconditionerConfig,
        deterministic: bool,
    ) -> Result<(within::BatchSolveResult, bool)> {
        let levels = design
            .effects()
            .iter()
            .map(|effect| effect.n_levels())
            .collect::<Vec<_>>();
        let memory = estimate_fe_solver_memory(
            design.nobs(),
            rhs.len(),
            &levels,
            rayon::current_num_threads(),
            options.local_size,
        )?;
        let resolved = resolve_borrowed_lsmr_preconditioner(
            levels.len(),
            design.total_term_slots(),
            requested,
        );
        let started = Instant::now();
        let mut setup_seconds = 0.0;
        if self
            .solver
            .as_ref()
            .is_some_and(|solver| solver.reproducible() != deterministic)
        {
            self.solver = None;
            self.retained_bytes = 0;
        }
        if self.solver.is_none() {
            let setup = Instant::now();
            let preconditioner = if self.preconditioner_fallback {
                PreconditionerConfig::Off
            } else {
                preconditioner
            };
            self.solver = Some(build_retained_borrowed_lsmr(
                design,
                weights,
                preconditioner,
                deterministic,
            )?);
            self.retained_bytes = memory.lsmr_retained_bytes(if self.preconditioner_fallback {
                LsmrPreconditioner::Off
            } else {
                resolved
            });
            self.builds += 1;
            setup_seconds += setup.elapsed().as_secs_f64();
        }
        let initial = super::lsmr_batch::solve_prepared_batch(
            self.solver.as_ref().expect("LSMR preparation exists"),
            rhs,
            options,
        );
        let mut solved = match initial {
            Ok(solved) => solved,
            Err(error)
                if requested == LsmrPreconditioner::Auto
                    && !self.preconditioner_fallback
                    && error.preconditioner_breakdown() =>
            {
                // Release the rejected preconditioner before constructing its replacement.
                self.solver = None;
                self.retained_bytes = 0;
                let setup = Instant::now();
                self.solver = Some(build_retained_borrowed_lsmr(
                    design,
                    weights,
                    PreconditionerConfig::Off,
                    deterministic,
                )?);
                self.retained_bytes = memory.lsmr_retained_bytes(LsmrPreconditioner::Off);
                self.builds += 1;
                self.preconditioner_fallback = true;
                setup_seconds += setup.elapsed().as_secs_f64();
                super::lsmr_batch::solve_prepared_batch(
                    self.solver
                        .as_ref()
                        .expect("fallback LSMR preparation exists"),
                    rhs,
                    options,
                )
                .map_err(|error| {
                    error
                        .into_fereg("borrowed modified LSMR failed after unpreconditioned fallback")
                })?
            }
            Err(error) => return Err(error.into_fereg("borrowed modified LSMR failed")),
        };
        self.batches += 1;
        solved.time_setup += setup_seconds;
        solved.time_total = started.elapsed().as_secs_f64();
        if crate::diagnostics::solver_trace_enabled() {
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=lsmr_session|builds={}|batches={}|rhs={}|setup={setup_seconds:.9}",
                self.builds, self.batches, rhs.len(),
            ));
        }
        Ok((solved, self.preconditioner_fallback))
    }
}

fn borrowed_lsmr_effects<'a>(design: &BorrowedInterceptFeDesign<'a>) -> Result<Vec<Effect<'a>>> {
    design
        .effects()
        .iter()
        .map(|effect| {
            Effect::new(effect.codes(), true, std::iter::empty::<&[f64]>()).map_err(|error| {
                invalid_data(format!(
                    "unable to construct borrowed LSMR fixed-effect term: {error}"
                ))
            })
        })
        .collect()
}

fn build_retained_borrowed_lsmr<'a>(
    design: &BorrowedInterceptFeDesign<'a>,
    weights: ValidatedWeights<'_>,
    preconditioner: PreconditionerConfig,
    deterministic: bool,
) -> Result<within::Solver<'a>> {
    let effects = borrowed_lsmr_effects(design)?;
    let weights = weights
        .values
        .map(|values| {
            let mut copy = Vec::new();
            copy.try_reserve_exact(values.len())
                .map_err(|_| allocation("unable to reserve retained LSMR weights"))?;
            copy.extend_from_slice(values);
            Ok(copy)
        })
        .transpose()?;
    let solver = within::Solver::new(effects, weights, preconditioner).and_then(|solver| {
        if deterministic {
            solver.with_reproducible()
        } else {
            Ok(solver)
        }
    });
    solver.map_err(|error| {
        FeregError::new(
            if matches!(error, within::BuildError::Allocation { .. }) {
                ErrorCode::Allocation
            } else {
                ErrorCode::Internal
            },
            format!("borrowed modified LSMR failed: {error}"),
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn run_borrowed_lsmr(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    certificate_tolerance: f64,
    max_iterations: usize,
    lsmr_config: LsmrConfig,
    certificate_reference_norms: Option<&[f64]>,
    retain_coefficients: bool,
) -> Result<(DenseMatrix, SolveReport, f64, Option<FeCoefficientBatch>)> {
    let levels = design
        .effects()
        .iter()
        .map(|effect| effect.n_levels())
        .collect::<Vec<_>>();
    let estimate = estimate_fe_solver_memory(
        values.nrows(),
        values.ncols(),
        &levels,
        lsmr_config
            .threads
            .resolved()
            .max(rayon::current_num_threads()),
        lsmr_config.local_size,
    )?;
    let preconditioner = resolve_borrowed_lsmr_preconditioner(
        levels.len(),
        design.total_term_slots(),
        lsmr_config.preconditioner,
    );
    let mut memory = BorrowedLsmrCallMemory::new(estimate, preconditioner, None, 0)?;
    run_borrowed_lsmr_with_prepared(
        design,
        weights,
        values,
        certificate_tolerance,
        max_iterations,
        lsmr_config,
        certificate_reference_norms,
        retain_coefficients,
        None,
        &mut memory,
        None,
    )
}

// The recurrence stops at `lsmr_config.tolerance`; the independent FE
// certificate passes at `certificate_tolerance`.
#[allow(clippy::too_many_arguments)]
fn run_borrowed_lsmr_with_prepared<'a>(
    design: &BorrowedInterceptFeDesign<'a>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    certificate_tolerance: f64,
    max_iterations: usize,
    lsmr_config: LsmrConfig,
    certificate_reference_norms: Option<&[f64]>,
    retain_coefficients: bool,
    prepared: Option<&mut BorrowedLsmrPrepared<'a>>,
    memory: &mut BorrowedLsmrCallMemory,
    dense_norms: Option<&mut Vec<f64>>,
) -> Result<(DenseMatrix, SolveReport, f64, Option<FeCoefficientBatch>)> {
    crate::interrupt::checkpoint()?;
    let total_started = Instant::now();
    let effective_max_iterations = lsmr_config.max_iterations.min(max_iterations);
    for tolerance in [certificate_tolerance, lsmr_config.tolerance] {
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err(invalid_argument(
                "borrowed LSMR tolerance must be finite and positive",
            ));
        }
    }
    if effective_max_iterations == 0 {
        return Err(invalid_argument(
            "borrowed LSMR iteration budget must be positive",
        ));
    }
    if lsmr_config.local_size == Some(0) {
        return Err(invalid_argument(
            "borrowed LSMR local reorthogonalization size must be positive",
        ));
    }
    let lsmr_setup_started = Instant::now();
    let rhs = super::lsmr_batch::rhs_columns(values)?;
    let rhs_views = rhs.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let options = LsmrOptions {
        tol: lsmr_config.tolerance,
        maxiter: effective_max_iterations,
        local_size: lsmr_config.local_size,
    };
    let total_coefficients = design.total_term_slots();
    let resolved_preconditioner = resolve_borrowed_lsmr_preconditioner(
        design.effects().len(),
        total_coefficients,
        lsmr_config.preconditioner,
    );
    let preconditioner = borrowed_within_preconditioner(resolved_preconditioner);
    let owned_setup_seconds = lsmr_setup_started.elapsed().as_secs_f64();
    let mut prepared = prepared.filter(|p| !p.disabled);
    let (mut solved, preconditioner_fallback) = if let Some(prepared) = prepared.as_deref_mut() {
        prepared.solve(
            design,
            weights,
            &rhs_views,
            &options,
            lsmr_config.preconditioner,
            preconditioner,
            lsmr_config.threads.deterministic,
        )?
    } else {
        let effects = borrowed_lsmr_effects(design)?;
        let initial = super::lsmr_batch::solve_batch(
            effects.clone(),
            &rhs_views,
            weights.values,
            &options,
            preconditioner,
            lsmr_config.threads.deterministic,
        );
        match initial {
            Ok(solved) => (solved, false),
            Err(error)
                if lsmr_config.preconditioner == LsmrPreconditioner::Auto
                    && error.preconditioner_breakdown() =>
            {
                let solved = super::lsmr_batch::solve_batch(
                    effects,
                    &rhs_views,
                    weights.values,
                    &options,
                    PreconditionerConfig::Off,
                    lsmr_config.threads.deterministic,
                )
                .map_err(|fallback_error| {
                    fallback_error
                        .into_fereg("borrowed modified LSMR failed after unpreconditioned fallback")
                })?;
                (solved, true)
            }
            Err(error) => {
                return Err(error.into_fereg("borrowed modified LSMR failed"));
            }
        }
    };
    // The candidate is wholly overwritten by the returned residuals. Allocate
    // it after Krylov work, once the RHS transport can be released; preserve
    // the caller's values for independent reference norms and any continuation.
    let retained = prepared.as_ref().map_or(0, |p| p.retained_bytes);
    drop(rhs_views);
    drop(rhs);
    let result_cells = (values.nrows() as u128) * (values.ncols() as u128);
    memory.include_payload(
        retained,
        (result_cells + solved.demeaned.capacity() as u128 + solved.x.capacity() as u128) * 8,
    )?;
    let mut lsmr_values = DenseMatrix::zeros(values.nrows(), values.ncols())?;
    memory.include_payload(
        retained,
        (lsmr_values.storage_capacity() as u128
            + solved.demeaned.capacity() as u128
            + solved.x.capacity() as u128)
            * 8,
    )?;
    super::lsmr_batch::copy_demeaned(&solved.demeaned, &mut lsmr_values);
    drop(std::mem::take(&mut solved.demeaned));
    if !retain_coefficients {
        drop(std::mem::take(&mut solved.x));
    }
    memory.include_payload(
        retained,
        (lsmr_values.storage_capacity() as u128 + solved.x.capacity() as u128) * 8
            + BorrowedMapWorkspace::planned_payload_bytes(
                design,
                values.nrows(),
                values.ncols(),
                rayon::current_num_threads(),
            )?,
    )?;
    let mut reservation = SystemWorkspaceReservation;
    let mut certificate_workspace = BorrowedMapWorkspace::new(
        design,
        lsmr_values.nrows(),
        lsmr_values.ncols(),
        &mut reservation,
    )?;
    let certificate_bytes = modeled_bytes(certificate_workspace.payload_peak()?)?;
    memory.include_payload(
        retained,
        (lsmr_values.storage_capacity() as u128 + solved.x.capacity() as u128) * 8
            + certificate_bytes,
    )?;
    update_workspace_norms(values, weights, &mut certificate_workspace, true)?;
    if let Some(reference_norms) = certificate_reference_norms {
        if reference_norms.len() != values.ncols()
            || reference_norms
                .iter()
                .any(|norm| !norm.is_finite() || *norm < 0.0)
        {
            return Err(invalid_argument(
                "borrowed LSMR continuation reference norms are invalid",
            ));
        }
        certificate_workspace
            .reference_norms
            .copy_from_slice(reference_norms);
    }
    let certification_started = Instant::now();
    let (certificate, dense_valid) = lsmr_norms::certify(
        design,
        weights,
        &lsmr_values,
        certificate_tolerance,
        &mut certificate_workspace,
        dense_norms.is_some(),
    )?;
    let certification_seconds = certification_started.elapsed().as_secs_f64();
    if crate::diagnostics::solver_trace_enabled() {
        for rhs in 0..lsmr_values.ncols() {
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=lsmr_rhs|rhs={rhs}|converged={}|iterations={}|within_normal={:.17e}|fereg_relinf={:.17e}",
                solved.converged[rhs],
                solved.iterations[rhs],
                solved.residual[rhs],
                certificate_workspace.rhs_relative_infinity_norms[rhs],
            ));
        }
    }
    let recurrence_converged = solved.converged.iter().copied().all(|value| value);
    let stop_reason = if certificate.passed {
        SolveStopReason::Certified
    } else if recurrence_converged {
        SolveStopReason::Uncertified
    } else {
        SolveStopReason::MaximumIterations
    };
    let mut warnings = solved
        .warnings
        .iter()
        .map(|warning| format!("within preconditioner: {warning:?}"))
        .collect::<Vec<_>>();
    if preconditioner_fallback {
        warnings.insert(
            0,
            "automatic borrowed LSMR preconditioner failed its positive-definiteness guard; retried unpreconditioned and certified the returned residuals".to_owned(),
        );
    }
    if recurrence_converged && !certificate.passed {
        warnings.push(
            "borrowed LSMR recurrence claimed convergence but the recomputed FE certificate failed"
                .to_owned(),
        );
    }
    let accounted_seconds = owned_setup_seconds + solved.time_total + certification_seconds;
    let postprocess_seconds = (total_started.elapsed().as_secs_f64() - accounted_seconds).max(0.0);
    let mut report = SolveReport {
        iterations: solved.iterations.iter().copied().max().unwrap_or(0),
        normal_equation_residual: solved.residual.iter().copied().fold(0.0_f64, f64::max),
        setup_seconds: solved.time_setup + postprocess_seconds,
        solve_seconds: (solved.time_total - solved.time_setup).max(0.0),
        certification_seconds,
        fallback_used: preconditioner_fallback,
        requested_preconditioner: Some(lsmr_config.preconditioner),
        used_preconditioner: Some(if preconditioner_fallback {
            LsmrPreconditioner::Off
        } else {
            resolved_preconditioner
        }),
        warnings,
        ..SolveReport::single_route(FeSolverKind::Lsmr, stop_reason, certificate)
    };
    let layout_bytes =
        (design.effects().len() as u128 * 3 + 1) * std::mem::size_of::<usize>() as u128;
    if retain_coefficients {
        memory.include_payload(
            retained,
            (lsmr_values.storage_capacity() as u128 + solved.x.capacity() as u128) * 8
                + certificate_bytes
                + layout_bytes,
        )?;
    }
    let coefficients = if retain_coefficients {
        Some(FeCoefficientBatch::from_values(
            vec![1; design.effects().len()],
            design
                .effects()
                .iter()
                .map(crate::fe::BorrowedInterceptEffect::n_levels)
                .collect(),
            lsmr_values.ncols(),
            solved.x,
        )?)
    } else {
        None
    };
    memory.include_payload(
        retained,
        lsmr_values.storage_capacity() as u128 * 8
            + certificate_bytes
            + coefficients
                .as_ref()
                .map_or(0, FeCoefficientBatch::retained_bytes),
    )?;
    if report.certified() && dense_valid {
        if let Some(norms) = dense_norms {
            *norms = std::mem::take(&mut certificate_workspace.reference_norms);
        }
    }
    report.solver_workspace_peak = memory.peak;
    Ok((lsmr_values, report, owned_setup_seconds, coefficients))
}

fn resolve_borrowed_lsmr_preconditioner(
    effect_count: usize,
    total_coefficients: usize,
    requested: LsmrPreconditioner,
) -> LsmrPreconditioner {
    match requested {
        LsmrPreconditioner::Auto if effect_count >= 2 && total_coefficients >= 256 => {
            LsmrPreconditioner::Schwarz
        }
        LsmrPreconditioner::Auto => LsmrPreconditioner::Diagonal,
        explicit => explicit,
    }
}

fn borrowed_within_preconditioner(preconditioner: LsmrPreconditioner) -> PreconditionerConfig {
    match preconditioner {
        LsmrPreconditioner::Auto => unreachable!("borrowed automatic preconditioner is resolved"),
        LsmrPreconditioner::Off => PreconditionerConfig::Off,
        LsmrPreconditioner::Diagonal => PreconditionerConfig::Diagonal,
        LsmrPreconditioner::Schwarz => PreconditionerConfig::default(),
    }
}

fn mark_auto(
    mut result: CertifiedBorrowedMap,
    pilot_iterations: usize,
    contraction: f64,
    additional_setup_seconds: f64,
    additional_solve_seconds: f64,
) -> Result<CertifiedBorrowedMap> {
    result.report.requested_solver = FeSolverKind::Auto;
    result.report.pilot_iterations = pilot_iterations;
    result.report.pilot_contraction = contraction;
    result.report.setup_seconds += additional_setup_seconds;
    result.report.solve_seconds += additional_solve_seconds;
    if !result.report.certified() {
        return Err(invalid_data(
            "borrowed MAP attempted to construct an uncertified result",
        ));
    }
    Ok(result)
}

fn attach_optional_coefficients(
    mut result: CertifiedBorrowedMap,
    coefficients: Option<FeCoefficientBatch>,
) -> Result<CertifiedBorrowedMap> {
    if let Some(coefficients) = coefficients {
        if coefficients.rhs_count() != result.residualized.ncols()
            || coefficients.coefficients_per_rhs() == 0
        {
            return Err(invalid_data("FE coefficient product is not aligned"));
        }
        result.coefficients = Some(coefficients);
    }
    Ok(result)
}

fn attach_optional_map_coefficients(
    mut result: CertifiedBorrowedMap,
    coefficients: Option<MapCoefficientAccumulator>,
) -> Result<CertifiedBorrowedMap> {
    let coefficients = if let Some(coefficients) = coefficients {
        let (batch, conversion_bytes) = coefficients.into_batch()?;
        result.report.solver_workspace_peak = result
            .report
            .solver_workspace_peak
            .with_retained(conversion_bytes)?;
        Some(batch)
    } else {
        None
    };
    attach_optional_coefficients(result, coefficients)
}

fn absorb_cg_with_reservation<R: WorkspaceReservation>(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    mut values: DenseMatrix,
    config: MapConfig,
    empty_design_is_structural_constant: bool,
    continuation: CgContinuation<'_>,
    reservation: &mut R,
) -> Result<CertifiedBorrowedMap> {
    let CgContinuation {
        reference_norms: continuation_reference_norms,
        refinement,
        retain_coefficients,
        stable_projection,
        reconstruction_tolerance,
    } = continuation;
    validate_config(config)?;
    if refinement.minimum_iterations > config.max_iterations {
        return Err(invalid_argument(
            "forced borrowed CG refinement exceeds its iteration budget",
        ));
    }
    if design.nobs() != values.nrows() {
        return Err(invalid_argument(
            "borrowed CG design and matrix observation counts differ",
        ));
    }
    if values.nrows() == 0 || values.ncols() == 0 {
        return Err(invalid_data(
            "borrowed CG requires nonempty observation and RHS axes",
        ));
    }
    if values.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(invalid_data("borrowed CG values must be finite"));
    }
    let weights = validate_weights(weights, values.nrows())?;

    let setup_started = Instant::now();
    let mut workspace =
        BorrowedMapWorkspace::new(design, values.nrows(), values.ncols(), reservation)?;
    update_workspace_norms(&values, weights, &mut workspace, true)?;
    let rhs_count = values.ncols();
    if reconstruction_tolerance.is_some() && rhs_count != 1 {
        return Err(invalid_argument(
            "borrowed CG recovery reconstruction checks require one RHS",
        ));
    }
    let reconstruction_scale = reconstruction_tolerance
        .map(|_| matrix_infinity_norm(&values))
        .transpose()?;
    let mut improvement_potential = reserved_zeros(
        rhs_count,
        reservation,
        "unable to reserve borrowed CG Hestenes improvement potential",
    )?;
    for (potential, norm) in improvement_potential
        .iter_mut()
        .zip(&workspace.reference_norms)
    {
        *potential = norm * norm;
        if !potential.is_finite() || *potential < 0.0 {
            return Err(invalid_data(
                "borrowed CG Hestenes improvement potential is invalid",
            ));
        }
    }
    if let Some(reference_norms) = continuation_reference_norms {
        if reference_norms.len() != values.ncols()
            || reference_norms
                .iter()
                .any(|norm| !norm.is_finite() || *norm < 0.0)
        {
            return Err(invalid_argument(
                "borrowed CG continuation reference norms are invalid",
            ));
        }
        workspace.reference_norms.copy_from_slice(reference_norms);
    }
    let mut residual_square = reserved_zeros(
        rhs_count,
        reservation,
        "unable to reserve borrowed CG residual norms",
    )?;
    let mut initial_residual_square = reserved_zeros(
        rhs_count,
        reservation,
        "unable to reserve borrowed CG initial residual norms",
    )?;
    let mut direction_projection = reserved_zeros(
        rhs_count,
        reservation,
        "unable to reserve borrowed CG direction products",
    )?;
    let mut next_residual_square = reserved_zeros(
        rhs_count,
        reservation,
        "unable to reserve borrowed CG updated residual norms",
    )?;
    let mut alpha = reserved_zeros(
        rhs_count,
        reservation,
        "unable to reserve borrowed CG alpha coefficients",
    )?;
    let mut beta = reserved_zeros(
        rhs_count,
        reservation,
        "unable to reserve borrowed CG beta coefficients",
    )?;
    let mut dot_corrections = reserved_zeros(
        rhs_count,
        reservation,
        "unable to reserve borrowed CG dot corrections",
    )?;
    let scalar_bytes = [
        &improvement_potential,
        &residual_square,
        &initial_residual_square,
        &direction_projection,
        &next_residual_square,
        &alpha,
        &beta,
        &dot_corrections,
    ]
    .iter()
    .map(|bank| bank.capacity() as u128)
    .sum::<u128>()
        * 8;
    let mut workspace_peak = workspace.payload_peak()?.with_retained(scalar_bytes)?;
    let setup_seconds = setup_started.elapsed().as_secs_f64();
    let solve_started = Instant::now();

    if design.effects().is_empty() && !empty_design_is_structural_constant {
        let mut result = certified_solver_result(
            values,
            FeSolverKind::Cg,
            SolveStopReason::NoFixedEffects,
            0,
            zero_certificate(),
            0.0,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        )?;
        result.set_solver_workspace_peak(workspace_peak);
        return Ok(result);
    }

    let mut solution_coefficients = retain_coefficients
        .then(|| FeCoefficientBatch::intercept_zeros(design, values.ncols()))
        .transpose()?;

    workspace_peak = workspace_peak.with_retained(
        solution_coefficients
            .as_ref()
            .map_or(0, FeCoefficientBatch::retained_bytes),
    )?;
    let initial_certificate = if design.effects().is_empty() {
        certify_structural_constant(weights, &values, config.tolerance, &mut workspace)?
    } else {
        certify(design, weights, &values, config.tolerance, &mut workspace)?
    };
    let initial_reconstruction_passed = match reconstruction_tolerance {
        Some(tolerance) => {
            relative_reconstruction_infinity_norm(
                &values,
                reconstruction_scale.expect("reconstruction scale exists with a tolerance"),
            )? <= tolerance
        }
        None => true,
    };
    if initial_certificate.passed
        && initial_reconstruction_passed
        && refinement.minimum_iterations == 0
    {
        let mut result = certified_solver_result(
            values,
            FeSolverKind::Cg,
            SolveStopReason::Certified,
            0,
            initial_certificate,
            0.0,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        )?;
        result.set_solver_workspace_peak(workspace_peak);
        return attach_optional_coefficients(result, solution_coefficients);
    }

    if design.effects().is_empty() {
        project_structural_constant(weights, &mut values, &mut workspace)?;
        let certificate =
            certify_structural_constant(weights, &values, config.tolerance, &mut workspace)?;
        if !certificate.passed {
            return Err(invalid_data(
                "borrowed CG structural-constant projection failed certification",
            ));
        }
        let mut result = certified_solver_result(
            values,
            FeSolverKind::Cg,
            SolveStopReason::Certified,
            1,
            certificate,
            0.0,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        )?;
        result.set_solver_workspace_peak(workspace_peak);
        return Ok(result);
    }

    let mut residual = fallible_matrix_zeros(
        values.nrows(),
        values.ncols(),
        "unable to reserve borrowed CG residual matrix",
    )?;
    let mut residual_coefficients = retain_coefficients
        .then(|| FeCoefficientBatch::intercept_zeros(design, values.ncols()))
        .transpose()?;
    cg_projection_action(
        design,
        weights,
        &values,
        &mut residual,
        &mut workspace,
        residual_coefficients.as_mut(),
        stable_projection,
    )?;
    let mut direction =
        fallible_matrix_copy(&residual, "unable to reserve borrowed CG direction matrix")?;
    let mut direction_coefficients = residual_coefficients
        .as_ref()
        .map(FeCoefficientBatch::try_clone)
        .transpose()?;
    let mut projection = fallible_matrix_zeros(
        values.nrows(),
        values.ncols(),
        "unable to reserve borrowed CG projection matrix",
    )?;
    let mut projection_coefficients = retain_coefficients
        .then(|| FeCoefficientBatch::intercept_zeros(design, values.ncols()))
        .transpose()?;
    // These banks coexist for the recurrence and are never resized. The
    // solution coefficient bank was already charged before the early exit.
    let matrix_bytes = (residual.storage_capacity() as u128
        + direction.storage_capacity() as u128
        + projection.storage_capacity() as u128)
        * 8;
    let coefficient_bytes = [
        &residual_coefficients,
        &direction_coefficients,
        &projection_coefficients,
    ]
    .iter()
    .map(|bank| bank.as_ref().map_or(0, FeCoefficientBatch::retained_bytes))
    .sum::<u128>();
    workspace_peak = workspace_peak.with_retained(matrix_bytes + coefficient_bytes)?;
    weighted_dot_columns_into(
        &residual,
        &residual,
        weights,
        &mut residual_square,
        &mut dot_corrections,
        &mut workspace.parallel_dot_sums,
    )?;
    initial_residual_square.copy_from_slice(&residual_square);
    let mut recurrence_residual = 1.0_f64;
    let mut hestenes_error = 1.0_f64;
    let mut reconstruction_check_active = false;

    for iteration in 1..=config.max_iterations {
        crate::interrupt::checkpoint()?;
        if let Some(coefficients) = projection_coefficients.as_mut() {
            coefficients.fill_zero();
        }
        cg_projection_action(
            design,
            weights,
            &direction,
            &mut projection,
            &mut workspace,
            projection_coefficients.as_mut(),
            stable_projection,
        )?;
        weighted_dot_columns_into(
            &direction,
            &projection,
            weights,
            &mut direction_projection,
            &mut dot_corrections,
            &mut workspace.parallel_dot_sums,
        )?;
        let alpha_result = cg_ratios_into(
            &residual_square,
            &direction_projection,
            &mut alpha,
            "borrowed CG direction denominator is nonfinite or nonpositive",
        );
        if let Err(error) = alpha_result {
            // A semidefinite projection operator can lose its remaining
            // direction to roundoff exactly at convergence. Batched RHSs can
            // reach that boundary at different iterations, so independently
            // certify the current iterate and deactivate only broken columns
            // that already satisfy their own certificate. Other columns keep
            // iterating, and publication still requires the final joint
            // certificate.
            let certificate = certify(design, weights, &values, config.tolerance, &mut workspace)?;
            let reconstruction_passed = match reconstruction_tolerance {
                Some(tolerance) => {
                    relative_reconstruction_infinity_norm(
                        &values,
                        reconstruction_scale.expect("reconstruction scale exists with a tolerance"),
                    )? <= tolerance
                }
                None => true,
            };
            if certificate.passed
                && reconstruction_passed
                && iteration.saturating_sub(1) >= refinement.minimum_iterations
            {
                let mut result = certified_solver_result(
                    values,
                    FeSolverKind::Cg,
                    SolveStopReason::Certified,
                    iteration - 1,
                    certificate,
                    recurrence_residual,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                )?;
                result.set_solver_workspace_peak(workspace_peak);
                return attach_optional_coefficients(result, solution_coefficients);
            }
            let mut recovered = false;
            for rhs in 0..residual_square.len() {
                let numerator = residual_square[rhs];
                let denominator = direction_projection[rhs];
                if !numerator.is_finite() || numerator < 0.0 {
                    return Err(error);
                }
                if numerator == 0.0 {
                    alpha[rhs] = 0.0;
                } else if denominator.is_finite() && denominator > 0.0 {
                    let ratio = numerator / denominator;
                    if !ratio.is_finite() || ratio < 0.0 {
                        return Err(error);
                    }
                    alpha[rhs] = ratio;
                } else if denominator.is_finite() {
                    if workspace.rhs_relative_infinity_norms[rhs] > config.tolerance {
                        // Finite nonpositive curvature for this PSD operator is
                        // a roundoff loss of conjugacy. Setting p=0 with
                        // alpha=0 makes the Fletcher--Reeves update below
                        // restart this RHS from its unchanged residual.
                        for row in 0..values.nrows() {
                            direction[(row, rhs)] = 0.0;
                        }
                        if let Some(coefficients) = direction_coefficients.as_mut() {
                            coefficients.zero_rhs(rhs)?;
                        }
                        alpha[rhs] = 0.0;
                        recovered = true;
                        continue;
                    }
                    for row in 0..values.nrows() {
                        residual[(row, rhs)] = 0.0;
                        direction[(row, rhs)] = 0.0;
                    }
                    if let Some(coefficients) = residual_coefficients.as_mut() {
                        coefficients.zero_rhs(rhs)?;
                    }
                    if let Some(coefficients) = direction_coefficients.as_mut() {
                        coefficients.zero_rhs(rhs)?;
                    }
                    residual_square[rhs] = 0.0;
                    alpha[rhs] = 0.0;
                    recovered = true;
                } else {
                    return Err(error);
                }
            }
            if !recovered {
                return Err(error);
            }
        }
        hestenes_error = cg_hestenes_error_and_update(
            &alpha,
            &residual_square,
            &mut improvement_potential,
            &values,
            &direction,
            weights,
        )?;
        cg_update_solution_and_residual(
            &mut values,
            &mut residual,
            &direction,
            &projection,
            &alpha,
        )?;
        if let (
            Some(solution),
            Some(residual_coefficients),
            Some(direction_coefficients),
            Some(projection_coefficients),
        ) = (
            solution_coefficients.as_mut(),
            residual_coefficients.as_mut(),
            direction_coefficients.as_ref(),
            projection_coefficients.as_ref(),
        ) {
            solution.add_scaled_rhs(direction_coefficients, &alpha, 1.0)?;
            residual_coefficients.add_scaled_rhs(projection_coefficients, &alpha, -1.0)?;
        }
        weighted_dot_columns_into(
            &residual,
            &residual,
            weights,
            &mut next_residual_square,
            &mut dot_corrections,
            &mut workspace.parallel_dot_sums,
        )?;
        recurrence_residual =
            cg_largest_relative_residual(&next_residual_square, &initial_residual_square)?;

        // Match reghdfe's Hestenes stopping rule, then require the independent
        // FE-moment certificate as a second condition before publication.
        // This avoids accepting a weak-graph iterate merely because its FE
        // moments are small while keeping certification out of the hot loop.
        let ordinary_ready =
            hestenes_error <= config.tolerance && iteration >= refinement.minimum_iterations;
        reconstruction_check_active |= reconstruction_tolerance.is_some() && ordinary_ready;
        let reconstruction_relative = if reconstruction_check_active
            || (reconstruction_tolerance.is_some() && iteration == config.max_iterations)
        {
            Some(relative_reconstruction_infinity_norm(
                &values,
                reconstruction_scale.expect("reconstruction scale exists with a tolerance"),
            )?)
        } else {
            None
        };
        let reconstruction_passed = reconstruction_tolerance.is_none_or(|tolerance| {
            reconstruction_relative.is_some_and(|relative| relative <= tolerance)
        });
        let recurrence_ready = if reconstruction_tolerance.is_some() {
            reconstruction_passed && iteration >= refinement.minimum_iterations
        } else {
            ordinary_ready
        };
        if recurrence_ready || iteration == config.max_iterations {
            let certificate = certify(design, weights, &values, config.tolerance, &mut workspace)?;
            if certificate.passed && recurrence_ready {
                let mut result = certified_solver_result(
                    values,
                    FeSolverKind::Cg,
                    SolveStopReason::Certified,
                    iteration,
                    certificate,
                    recurrence_residual,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                )?;
                result.set_solver_workspace_peak(workspace_peak);
                return attach_optional_coefficients(result, solution_coefficients);
            }
        }

        cg_ratios_into(
            &next_residual_square,
            &residual_square,
            &mut beta,
            "borrowed CG residual denominator is nonfinite or nonpositive",
        )?;
        cg_update_direction(&mut direction, &residual, &beta)?;
        if let (Some(direction_coefficients), Some(residual_coefficients)) = (
            direction_coefficients.as_mut(),
            residual_coefficients.as_ref(),
        ) {
            direction_coefficients.assign_sum_scaled_rhs(residual_coefficients, &beta)?;
        }
        residual_square.copy_from_slice(&next_residual_square);
    }

    Err(invalid_data(format!(
        "borrowed CG exhausted {} iterations without comparator-equivalent recurrence convergence and a passing final certificate (Hestenes error {hestenes_error:e}, recurrence residual {recurrence_residual:e})",
        config.max_iterations,
    )))
}

fn absorb_with_reservation_mode<R: WorkspaceReservation>(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    mut values: DenseMatrix,
    config: MapConfig,
    empty_design_is_structural_constant: bool,
    retain_coefficients: bool,
    reservation: &mut R,
) -> Result<CertifiedBorrowedMap> {
    validate_config(config)?;
    if design.nobs() != values.nrows() {
        return Err(invalid_argument(
            "borrowed MAP design and matrix observation counts differ",
        ));
    }
    if values.nrows() == 0 || values.ncols() == 0 {
        return Err(invalid_data(
            "borrowed MAP requires nonempty observation and RHS axes",
        ));
    }
    let invalid_values = values.as_slice().iter().any(|value| !value.is_finite());
    if invalid_values {
        return Err(invalid_data("borrowed MAP values must be finite"));
    }
    let weights = validate_weights(weights, values.nrows())?;

    let setup_started = Instant::now();
    let mut workspace =
        BorrowedMapWorkspace::new(design, values.nrows(), values.ncols(), reservation)?;
    update_workspace_norms(&values, weights, &mut workspace, true)?;
    let setup_seconds = setup_started.elapsed().as_secs_f64();
    let solve_started = Instant::now();
    let workspace_peak = workspace.payload_peak()?;
    let mut coefficients = retain_coefficients
        .then(|| MapCoefficientAccumulator::zeros(design, values.ncols()))
        .transpose()?;

    if design.effects().is_empty() && !empty_design_is_structural_constant {
        let mut result = certified_result(
            values,
            SolveStopReason::NoFixedEffects,
            0,
            zero_certificate(),
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        )?;
        result.set_solver_workspace_peak(workspace_peak);
        return attach_optional_map_coefficients(result, coefficients);
    }

    {
        let initial_certificate = if design.effects().is_empty() {
            certify_initial_structural_constant(weights, &values, config.tolerance, &mut workspace)?
        } else {
            certify_initial(design, weights, &values, config.tolerance, &mut workspace)?
        };
        if initial_certificate.passed {
            let mut result = certified_result(
                values,
                SolveStopReason::Certified,
                0,
                initial_certificate,
                setup_seconds,
                solve_started.elapsed().as_secs_f64(),
            )?;
            result.set_solver_workspace_peak(workspace_peak);
            return attach_optional_map_coefficients(
                result.with_residual_norms(std::mem::take(&mut workspace.residual_norms))?,
                coefficients,
            );
        }
    }

    if design.effects().is_empty() {
        project_structural_constant(weights, &mut values, &mut workspace)?;
        let certificate =
            certify_structural_constant(weights, &values, config.tolerance, &mut workspace)?;
        if !certificate.passed {
            return Err(invalid_data(
                "borrowed MAP structural-constant projection failed certification",
            ));
        }
        let mut result = certified_result(
            values,
            SolveStopReason::Certified,
            1,
            certificate,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        )?;
        result.set_solver_workspace_peak(workspace_peak);
        return attach_optional_map_coefficients(
            result.with_residual_norms(std::mem::take(&mut workspace.residual_norms))?,
            coefficients,
        );
    }

    for iteration in 1..=config.max_iterations {
        crate::interrupt::checkpoint()?;
        borrowed_residual_sweep_recording(
            design,
            weights,
            &mut values,
            &mut workspace,
            coefficients.as_mut(),
            config.symmetric,
        )?;
        // One intercept effect is removed exactly by its first projection, so
        // certify immediately instead of waiting for the periodic multiway
        // cadence. Multiway designs retain periodic plus final certification.
        if design.effects().len() <= 1
            || iteration % config.check_every == 0
            || iteration == config.max_iterations
        {
            let certificate = certify(design, weights, &values, config.tolerance, &mut workspace)?;
            if certificate.passed {
                let mut result = certified_result(
                    values,
                    SolveStopReason::Certified,
                    iteration,
                    certificate,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                )?;
                result.set_solver_workspace_peak(workspace_peak);
                return attach_optional_map_coefficients(
                    result.with_residual_norms(std::mem::take(&mut workspace.residual_norms))?,
                    coefficients,
                );
            }
        }
    }

    Err(invalid_data(
        "borrowed MAP exhausted its iteration budget without a passing final certificate",
    ))
}

fn validate_config(config: MapConfig) -> Result<()> {
    if !config.tolerance.is_finite() || config.tolerance <= 0.0 {
        return Err(invalid_argument(
            "borrowed MAP tolerance must be finite and positive",
        ));
    }
    if config.max_iterations == 0 || config.check_every == 0 {
        return Err(invalid_argument(
            "borrowed MAP iteration and check budgets must be positive",
        ));
    }
    Ok(())
}

fn validate_weights(weights: BorrowedMapWeights<'_>, nrows: usize) -> Result<ValidatedWeights<'_>> {
    match weights {
        BorrowedMapWeights::Unit => Ok(ValidatedWeights {
            values: None,
            raw_scale: 1.0,
        }),
        BorrowedMapWeights::Provided(values) => {
            if values.len() != nrows {
                return Err(invalid_argument(
                    "borrowed MAP weight and observation counts differ",
                ));
            }
            if values
                .iter()
                .any(|weight| !weight.is_finite() || *weight <= 0.0)
            {
                return Err(invalid_data(
                    "borrowed MAP weights must be finite and positive",
                ));
            }
            let raw_scale = values.iter().copied().fold(0.0_f64, f64::max);
            if values.iter().any(|weight| *weight / raw_scale == 0.0) {
                return Err(invalid_data(
                    "borrowed MAP normalized weight underflowed to zero",
                ));
            }
            Ok(ValidatedWeights {
                values: Some(values),
                raw_scale,
            })
        }
    }
}

fn project_structural_constant(
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<()> {
    structural_constant_means(weights, values, workspace)?;
    for observation in 0..values.nrows() {
        for rhs in 0..values.ncols() {
            let residual = values[(observation, rhs)] - workspace.group_sums[rhs];
            if !residual.is_finite() {
                return Err(invalid_data(
                    "borrowed MAP structural-constant projection produced a nonfinite residual",
                ));
            }
            values[(observation, rhs)] = residual;
        }
    }
    Ok(())
}

fn structural_constant_means(
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<f64> {
    workspace.value_scales.fill(0.0);
    workspace.group_sums.fill(0.0);
    workspace.group_corrections.fill(0.0);
    let mut weight_sum = 0.0_f64;
    let mut weight_correction = 0.0_f64;
    for observation in 0..values.nrows() {
        compensated_add(
            weights.normalized(observation),
            &mut weight_sum,
            &mut weight_correction,
        )?;
        for rhs in 0..values.ncols() {
            workspace.value_scales[rhs] =
                workspace.value_scales[rhs].max(values[(observation, rhs)].abs());
        }
    }
    let total_weight = weight_sum + weight_correction;
    if !total_weight.is_finite() || total_weight <= 0.0 {
        return Err(invalid_data(
            "borrowed MAP structural-constant weight sum is invalid",
        ));
    }
    for observation in 0..values.nrows() {
        let weight = weights.normalized(observation);
        for rhs in 0..values.ncols() {
            let scale = workspace.value_scales[rhs];
            if scale != 0.0 {
                compensated_add(
                    weight * (values[(observation, rhs)] / scale),
                    &mut workspace.group_sums[rhs],
                    &mut workspace.group_corrections[rhs],
                )?;
            }
        }
    }
    for rhs in 0..values.ncols() {
        let scale = workspace.value_scales[rhs];
        let mean = if scale == 0.0 {
            0.0
        } else {
            scale * ((workspace.group_sums[rhs] + workspace.group_corrections[rhs]) / total_weight)
        };
        if !mean.is_finite() {
            return Err(invalid_data(
                "borrowed MAP structural-constant mean is not finite",
            ));
        }
        workspace.group_sums[rhs] = mean;
    }
    Ok(total_weight)
}

fn certify_structural_constant(
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    update_workspace_norms(residuals, weights, workspace, false)?;
    certify_structural_constant_with_current_norms(weights, residuals, tolerance, workspace)
}

fn certify_initial_structural_constant(
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    workspace
        .residual_norms
        .copy_from_slice(&workspace.reference_norms);
    certify_structural_constant_with_current_norms(weights, residuals, tolerance, workspace)
}

fn certify_structural_constant_with_current_norms(
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    for rhs in 0..residuals.ncols() {
        let scale = workspace.reference_norms[rhs].max(workspace.residual_norms[rhs]);
        if !scale.is_finite() || scale < 0.0 {
            return Err(invalid_data(
                "borrowed MAP structural-constant RHS scale is invalid",
            ));
        }
        workspace.rhs_scales[rhs] = scale;
    }
    let hardware = crate::kernels::installed();
    let group_weight = structural_constant_means(weights, residuals, workspace)?;
    let root_group_weight = group_weight.sqrt();
    let mut absolute_infinity_norm = 0.0_f64;
    let mut relative_infinity_norm = 0.0_f64;
    let mut relative_square_sum = 0.0_f64;
    for rhs in 0..residuals.ncols() {
        let mean = workspace.group_sums[rhs];
        let denominator = workspace.rhs_scales[rhs];
        let relative = if denominator == 0.0 {
            if mean == 0.0 {
                0.0
            } else {
                return Err(invalid_data(
                    "borrowed MAP structural-constant zero-scale certificate failed",
                ));
            }
        } else {
            (mean.abs() / denominator) * root_group_weight
        };
        if !relative.is_finite() {
            return Err(invalid_data(
                "borrowed MAP structural-constant certificate ratio is not finite",
            ));
        }
        relative_infinity_norm = relative_infinity_norm.max(relative);
        relative_square_sum =
            crate::kernels::fused(hardware, relative, relative, relative_square_sum);
        let normalized_moment = mean.abs() * group_weight;
        let raw_moment = normalized_moment * weights.raw_scale;
        absolute_infinity_norm = absolute_infinity_norm.max(if raw_moment.is_finite() {
            raw_moment
        } else if normalized_moment == 0.0 {
            0.0
        } else {
            f64::INFINITY
        });
    }
    let relative_two_norm = (relative_square_sum / residuals.ncols() as f64).sqrt();
    if !relative_two_norm.is_finite() {
        return Err(invalid_data(
            "borrowed MAP structural-constant certificate norm is not finite",
        ));
    }
    Ok(FeCertificate {
        absolute_infinity_norm,
        relative_infinity_norm,
        relative_two_norm,
        passed: relative_infinity_norm <= tolerance,
    })
}

fn project_effect_recording<R: InterceptCoefficientRecorder>(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    coefficients: Option<(&mut R, usize)>,
) -> Result<()> {
    project_effect_recording_impl::<R, false, false>(
        effect,
        weights,
        values,
        workspace,
        coefficients,
        None,
    )
}

fn project_effect_recording_with_complement<R: InterceptCoefficientRecorder>(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    coefficients: Option<(&mut R, usize)>,
    complement_from: &DenseMatrix,
) -> Result<()> {
    project_effect_recording_impl::<R, false, true>(
        effect,
        weights,
        values,
        workspace,
        coefficients,
        Some(complement_from),
    )
}

fn project_effect_recording_from_input<R: InterceptCoefficientRecorder>(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    input: &DenseMatrix,
    output: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    coefficients: Option<(&mut R, usize)>,
) -> Result<()> {
    project_effect_recording_impl::<R, true, false>(
        effect,
        weights,
        output,
        workspace,
        coefficients,
        Some(input),
    )
}

fn project_effect_recording_impl<
    R: InterceptCoefficientRecorder,
    const FROM_INPUT: bool,
    const COMPLEMENT: bool,
>(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    coefficients: Option<(&mut R, usize)>,
    auxiliary: Option<&DenseMatrix>,
) -> Result<()> {
    debug_assert!(!(FROM_INPUT && COMPLEMENT));
    if effect.codes().len() != values.nrows() {
        return Err(invalid_argument(
            "borrowed MAP effect and matrix observation counts differ",
        ));
    }
    if (FROM_INPUT || COMPLEMENT)
        && auxiliary.is_none_or(|source| {
            source.nrows() != values.nrows() || source.ncols() != values.ncols()
        })
    {
        return Err(invalid_argument(
            "borrowed MAP auxiliary projection matrix dimensions differ",
        ));
    }
    let parallel_mean_count = effect
        .n_levels()
        .checked_mul(values.ncols())
        .ok_or_else(|| allocation("borrowed MAP parallel group-mean dimensions overflow"))?;
    if crate::parallel::StagePlan::current(
        values.nrows() as u128 * values.ncols() as u128,
        effect.n_levels(),
    )
    .lanes
        > 1
        && workspace.parallel_group_means.len() >= parallel_mean_count
    {
        return project_effect_parallel::<R, FROM_INPUT, COMPLEMENT>(
            effect,
            weights,
            values,
            workspace,
            parallel_mean_count,
            coefficients,
            auxiliary,
        );
    }
    project_effect_serial::<R, FROM_INPUT, COMPLEMENT>(
        effect,
        weights,
        values,
        workspace,
        coefficients,
        auxiliary,
    )
}

fn project_effect_serial<
    R: InterceptCoefficientRecorder,
    const FROM_INPUT: bool,
    const COMPLEMENT: bool,
>(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    mut coefficients: Option<(&mut R, usize)>,
    auxiliary: Option<&DenseMatrix>,
) -> Result<()> {
    for level in 0..effect.n_levels() {
        let members = effect.level_index().checked_members(level)?;
        if members.is_empty() {
            return Err(invalid_argument(
                "borrowed MAP effect contains an empty final level",
            ));
        }
        if FROM_INPUT {
            group_means(
                members,
                weights,
                auxiliary.expect("validated input projection source"),
                workspace,
                GroupMeanRoute::Scaled,
            )?;
        } else {
            group_means(members, weights, values, workspace, GroupMeanRoute::Scaled)?;
        }
        if let Some((batch, effect_index)) = coefficients.as_mut() {
            batch.add_intercept_level(
                *effect_index,
                level,
                &workspace.group_sums[..values.ncols()],
            )?;
        }
        for &observation in members {
            if observation >= values.nrows() {
                return Err(invalid_argument(
                    "borrowed MAP membership exceeds the observation axis",
                ));
            }
            for rhs in 0..values.ncols() {
                let source = if FROM_INPUT {
                    auxiliary.expect("validated input projection source")[(observation, rhs)]
                } else {
                    values[(observation, rhs)]
                };
                let projected = source - workspace.group_sums[rhs];
                let output = if COMPLEMENT {
                    auxiliary.expect("validated complement")[(observation, rhs)] - projected
                } else {
                    projected
                };
                if !output.is_finite() {
                    return Err(invalid_data(
                        "borrowed MAP projection produced a nonfinite residual",
                    ));
                }
                values[(observation, rhs)] = output;
            }
        }
    }
    Ok(())
}

fn project_effect_parallel<
    R: InterceptCoefficientRecorder,
    const FROM_INPUT: bool,
    const COMPLEMENT: bool,
>(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    parallel_mean_count: usize,
    coefficients: Option<(&mut R, usize)>,
    auxiliary: Option<&DenseMatrix>,
) -> Result<()> {
    let levels = effect.n_levels();
    let columns = values.ncols();
    if columns <= PARALLEL_GROUP_MAX_RHS {
        return project_effect_parallel_fused::<R, FROM_INPUT, COMPLEMENT>(
            effect,
            weights,
            values,
            workspace,
            parallel_mean_count,
            coefficients,
            auxiliary,
        );
    }
    {
        let values_read = if FROM_INPUT {
            auxiliary.expect("validated input projection source")
        } else {
            &*values
        };
        fill_parallel_group_statistics(
            effect,
            weights,
            values_read,
            &mut workspace.parallel_group_weights[..levels],
            &mut workspace.parallel_group_means[..parallel_mean_count],
            GroupMeanRoute::Scaled,
        )?;
    }

    let means = &workspace.parallel_group_means[..parallel_mean_count];
    if let Some((batch, effect_index)) = coefficients {
        record_parallel_group_means(batch, effect_index, means, levels, columns)?;
    }
    subtract_parallel_level_means::<FROM_INPUT, COMPLEMENT>(
        effect.codes(),
        levels,
        means,
        values,
        auxiliary,
    )
}

fn project_effect_parallel_fused<
    R: InterceptCoefficientRecorder,
    const FROM_INPUT: bool,
    const COMPLEMENT: bool,
>(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    parallel_mean_count: usize,
    coefficients: Option<(&mut R, usize)>,
    auxiliary: Option<&DenseMatrix>,
) -> Result<()> {
    let levels = effect.n_levels();
    let columns = values.ncols();
    let values_read = if FROM_INPUT {
        auxiliary.expect("validated input projection source")
    } else {
        &*values
    };
    let plan =
        crate::parallel::StagePlan::current(values.nrows() as u128 * columns as u128, levels);
    let mut coefficients = coefficients;
    let mut recorded_inline = false;
    if let Some((batch, effect_index)) = coefficients.as_mut() {
        if batch.rhs_count() != columns {
            return Err(invalid_argument(
                "parallel MAP coefficient means are not aligned",
            ));
        }
        if let Some(accumulated) = batch.level_major_values_mut(*effect_index, levels)? {
            let mut lanes = vec![(); plan.lanes];
            crate::parallel::group_blocks(
                crate::parallel::GroupBuffers {
                    first: &mut workspace.parallel_group_means[..parallel_mean_count],
                    first_stride: columns,
                    second: accumulated,
                    second_stride: columns,
                    scratch: &mut lanes,
                },
                effect.level_index().membership_offsets(),
                0,
                &|first_level, means, accumulated, _| {
                    for local_level in 0..means.len() / columns {
                        let level = first_level + local_level;
                        let start = local_level * columns;
                        let level_means = &mut means[start..start + columns];
                        projection_level_means(effect, level, weights, values_read, level_means)?;
                        for (destination, &mean) in accumulated[start..start + columns]
                            .iter_mut()
                            .zip(level_means.iter())
                        {
                            *destination += mean;
                            if !destination.is_finite() {
                                return Err(invalid_data(
                                    "MAP coefficient group accumulation is nonfinite",
                                ));
                            }
                        }
                    }
                    Ok(())
                },
            )?;
            recorded_inline = true;
        }
    }
    if !recorded_inline {
        plan.groups_mut(
            &mut workspace.parallel_group_means[..parallel_mean_count],
            columns,
            effect.level_index().membership_offsets(),
            |first_level, means| {
                for local_level in 0..means.len() / columns {
                    let start = local_level * columns;
                    projection_level_means(
                        effect,
                        first_level + local_level,
                        weights,
                        values_read,
                        &mut means[start..start + columns],
                    )?;
                }
                Ok(())
            },
        )?;
    }

    let means = &workspace.parallel_group_means[..parallel_mean_count];
    if !recorded_inline {
        if let Some((batch, effect_index)) = coefficients {
            record_parallel_group_means(batch, effect_index, means, levels, columns)?;
        }
    }
    subtract_parallel_level_means::<FROM_INPUT, COMPLEMENT>(
        effect.codes(),
        levels,
        means,
        values,
        auxiliary,
    )
}

// Subtract each row's level means in parallel. FROM_INPUT reads the source
// row from `auxiliary`; COMPLEMENT returns `auxiliary - projected`.
fn subtract_parallel_level_means<const FROM_INPUT: bool, const COMPLEMENT: bool>(
    codes: &[u32],
    levels: usize,
    means: &[f64],
    values: &mut DenseMatrix,
    auxiliary: Option<&DenseMatrix>,
) -> Result<()> {
    let columns = values.ncols();
    let update_row = |observation: usize, row: &mut [f64], auxiliary: Option<&[f64]>| {
        let level = codes[observation] as usize;
        if level >= levels {
            return Err(invalid_argument(
                "borrowed MAP code exceeds the finalized level count",
            ));
        }
        for (rhs, value) in row.iter_mut().enumerate() {
            let source = if FROM_INPUT {
                auxiliary.expect("validated input projection row")[rhs]
            } else {
                *value
            };
            let projected = source - means[level * columns + rhs];
            let output = if COMPLEMENT {
                auxiliary.expect("validated complement row")[rhs] - projected
            } else {
                projected
            };
            if !output.is_finite() {
                return Err(invalid_data(
                    "borrowed MAP projection produced a nonfinite residual",
                ));
            }
            *value = output;
        }
        Ok(())
    };
    let row_work = values.nrows() as u128 * columns as u128;
    if FROM_INPUT || COMPLEMENT {
        let auxiliary = auxiliary.expect("validated auxiliary projection matrix");
        crate::parallel::stage_try_for_each(
            values
                .as_mut_slice()
                .par_chunks_mut(columns)
                .zip(auxiliary.as_slice().par_chunks(columns))
                .enumerate(),
            row_work,
            |(observation, (row, auxiliary))| update_row(observation, row, Some(auxiliary)),
        )?;
    } else {
        crate::parallel::stage_try_for_each(
            values.as_mut_slice().par_chunks_mut(columns).enumerate(),
            row_work,
            |(observation, row)| update_row(observation, row, None),
        )?;
    }
    Ok(())
}

fn fill_parallel_group_statistics(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    group_weights: &mut [f64],
    means: &mut [f64],
    route: GroupMeanRoute,
) -> Result<()> {
    let levels = effect.n_levels();
    let columns = values.ncols();
    if group_weights.len() != levels
        || means.len()
            != levels
                .checked_mul(columns)
                .ok_or_else(|| allocation("borrowed MAP parallel group dimensions overflow"))?
    {
        return Err(invalid_argument(
            "borrowed MAP parallel group workspaces are not aligned",
        ));
    }
    let plan =
        crate::parallel::StagePlan::current(values.nrows() as u128 * columns as u128, levels);
    let mut lanes = vec![(); plan.lanes];
    crate::parallel::group_blocks(
        crate::parallel::GroupBuffers {
            first: group_weights,
            first_stride: 1,
            second: means,
            second_stride: columns,
            scratch: &mut lanes,
        },
        effect.level_index().membership_offsets(),
        0,
        &|first, group_weights, means, _| {
            for (local_level, group_weight) in group_weights.iter_mut().enumerate() {
                let level = first + local_level;
                let start = local_level * columns;
                let level_means = &mut means[start..start + columns];
                if columns <= PARALLEL_GROUP_MAX_RHS {
                    *group_weight = fused_group_means_for_level(
                        effect,
                        level,
                        weights,
                        values,
                        level_means,
                        route,
                    )?;
                } else {
                    let members = effect.level_index().checked_members(level)?;
                    if members.is_empty() {
                        return Err(invalid_argument(
                            "borrowed MAP effect contains an empty final level",
                        ));
                    }
                    // Sixty-four-column tiles read each member row segment
                    // once; every column keeps its member-order recurrence.
                    let data = values.as_slice();
                    for (tile, output) in level_means.chunks_mut(PARALLEL_GROUP_MAX_RHS).enumerate()
                    {
                        let start = tile * PARALLEL_GROUP_MAX_RHS;
                        *group_weight = fused_group_means_by_value(
                            effect,
                            level,
                            weights,
                            values.nrows(),
                            output,
                            |row, rhs| data[row * columns + start + rhs],
                        )?;
                    }
                }
            }
            Ok(())
        },
    )
}
/// Projection-route per-level means (scaled two-pass arithmetic).
fn projection_level_means(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    level: usize,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    means: &mut [f64],
) -> Result<f64> {
    fused_group_means_for_level(
        effect,
        level,
        weights,
        values,
        means,
        GroupMeanRoute::Scaled,
    )
}

/// Parallel-lane group means for at most `PARALLEL_GROUP_MAX_RHS` columns,
/// with stack scratch and the shared (possibly hardware-installed) kernels.
fn fused_group_means_for_level(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    level: usize,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    means: &mut [f64],
    route: GroupMeanRoute,
) -> Result<f64> {
    let members = effect.level_index().checked_members(level)?;
    let columns = values.ncols();
    if members.is_empty() || means.len() != columns || columns > PARALLEL_GROUP_MAX_RHS {
        return Err(invalid_argument(
            "borrowed MAP fused group dimensions are invalid",
        ));
    }
    let mut scales = [0.0_f64; PARALLEL_GROUP_MAX_RHS];
    let mut corrections = [0.0_f64; PARALLEL_GROUP_MAX_RHS];
    kernel_group_means(
        members,
        weights,
        values,
        &mut scales[..columns],
        means,
        &mut corrections[..columns],
        route,
    )
}

fn fused_group_means_by_value(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    level: usize,
    weights: ValidatedWeights<'_>,
    nrows: usize,
    means: &mut [f64],
    value: impl Fn(usize, usize) -> f64,
) -> Result<f64> {
    let members = effect.level_index().checked_members(level)?;
    if members.is_empty() || means.len() > PARALLEL_GROUP_MAX_RHS {
        return Err(invalid_argument(
            "borrowed MAP fused group dimensions are invalid",
        ));
    }
    let mut scales = [0.0_f64; PARALLEL_GROUP_MAX_RHS];
    let mut sums = [0.0_f64; PARALLEL_GROUP_MAX_RHS];
    let mut corrections = [0.0_f64; PARALLEL_GROUP_MAX_RHS];
    let scales = &mut scales[..means.len()];
    let sums = &mut sums[..means.len()];
    let corrections = &mut corrections[..means.len()];
    let mut weight_sum = 0.0_f64;
    let mut weight_correction = 0.0_f64;
    for &observation in members {
        if observation >= nrows {
            return Err(invalid_argument(
                "borrowed MAP membership exceeds the observation axis",
            ));
        }
        if weights.values.is_some() {
            neumaier_add(
                weights.normalized(observation),
                &mut weight_sum,
                &mut weight_correction,
            );
        }
        for (rhs, scale) in scales.iter_mut().enumerate() {
            *scale = scale.max(value(observation, rhs).abs());
        }
    }
    compensated_finite(weight_sum, weight_correction)?;
    let total_weight = if weights.values.is_some() {
        weight_sum + weight_correction
    } else {
        members.len() as f64
    };
    if !total_weight.is_finite() || total_weight <= 0.0 {
        return Err(invalid_data(
            "borrowed MAP group weight sum is not finite and positive",
        ));
    }
    if let Some(weight_values) = weights.values {
        for &observation in members {
            let weight = weight_values[observation] / weights.raw_scale;
            for rhs in 0..means.len() {
                if scales[rhs] != 0.0 {
                    neumaier_add(
                        weight * (value(observation, rhs) / scales[rhs]),
                        &mut sums[rhs],
                        &mut corrections[rhs],
                    );
                }
            }
        }
    } else {
        for &observation in members {
            for rhs in 0..means.len() {
                if scales[rhs] != 0.0 {
                    neumaier_add(
                        value(observation, rhs) / scales[rhs],
                        &mut sums[rhs],
                        &mut corrections[rhs],
                    );
                }
            }
        }
    }
    for (&sum, &correction) in sums.iter().zip(corrections.iter()) {
        compensated_finite(sum, correction)?;
    }
    for rhs in 0..means.len() {
        let mean = if scales[rhs] == 0.0 {
            0.0
        } else {
            scales[rhs] * ((sums[rhs] + corrections[rhs]) / total_weight)
        };
        if !mean.is_finite() {
            return Err(invalid_data("borrowed MAP group mean is not finite"));
        }
        means[rhs] = mean;
    }
    Ok(total_weight)
}

fn record_parallel_group_means<R: InterceptCoefficientRecorder>(
    batch: &mut R,
    effect: usize,
    means: &[f64],
    levels: usize,
    columns: usize,
) -> Result<()> {
    if means.len() != levels.saturating_mul(columns) || batch.rhs_count() != columns {
        return Err(invalid_argument(
            "parallel MAP coefficient means are not aligned",
        ));
    }
    batch.add_intercept_level_major(effect, means, levels)
}

fn group_means(
    members: &[usize],
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    route: GroupMeanRoute,
) -> Result<f64> {
    workspace.value_scales.fill(0.0);
    workspace.group_sums.fill(0.0);
    workspace.group_corrections.fill(0.0);
    let columns = values.ncols();
    kernel_group_means(
        members,
        weights,
        values,
        &mut workspace.value_scales[..columns],
        &mut workspace.group_sums[..columns],
        &mut workspace.group_corrections[..columns],
        route,
    )
}

/// Weighted group means of every column of `values` over `members`, written
/// into `sums`; `scales`, `sums` and `corrections` have one entry per column.
/// Returns the group's total normalized weight.
/// Which group-mean arithmetic a caller uses. Certificates use the one-pass
/// sums; projections, whose solvers can amplify rounding changes, keep the
/// scaled two-pass sums.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupMeanRoute {
    Scaled,
    OnePass,
}

/// Columns whose largest member magnitude is below this bound take the scaled
/// two-pass route, so unscaled weighted products cannot lose precision to
/// underflow.
const GROUP_SUM_MIN_MAGNITUDE: f64 = 1.0e-150;

/// Group means of every column of `values` over `members`, written into
/// `sums`; `scales`, `sums` and `corrections` have one entry per column.
/// Returns the group's total normalized weight.
///
/// One pass accumulates compensated weighted sums of values shifted by the
/// first member, and the column maxima, so a group of equal values has exactly
/// that value as its mean. The scaled two-pass route recomputes the group when
/// a sum or mean is not finite or a nonzero column lies below
/// [`GROUP_SUM_MIN_MAGNITUDE`].
fn kernel_group_means(
    members: &[usize],
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    scales: &mut [f64],
    sums: &mut [f64],
    corrections: &mut [f64],
    route: GroupMeanRoute,
) -> Result<f64> {
    if route == GroupMeanRoute::Scaled {
        return scaled_kernel_group_means(members, weights, values, scales, sums, corrections);
    }
    scales.fill(0.0);
    sums.fill(0.0);
    corrections.fill(0.0);
    let data = values.as_slice();
    let (rows, raw) = (values.nrows(), weights.raw_scale);
    let one_pass = match crate::kernels::installed() {
        Some(kernels) => (kernels.group_sums)(
            weights.values,
            raw,
            data,
            rows,
            members,
            sums,
            corrections,
            scales,
        ),
        None => crate::kernels::group_sums(
            weights.values,
            raw,
            data,
            rows,
            members,
            sums,
            corrections,
            scales,
        ),
    };
    let (weight_sum, weight_correction) = one_pass
        .ok_or_else(|| invalid_argument("borrowed MAP membership exceeds the observation axis"))?;
    compensated_finite(weight_sum, weight_correction)?;
    let total_weight = weight_sum + weight_correction;
    if !total_weight.is_finite() || total_weight <= 0.0 {
        return Err(invalid_data(
            "borrowed MAP group weight sum is not finite and positive",
        ));
    }
    let representable = scales.iter().zip(sums.iter()).zip(corrections.iter()).all(
        |((&maximum, &sum), &correction)| {
            (maximum == 0.0 || maximum >= GROUP_SUM_MIN_MAGNITUDE)
                && ((sum + correction) / total_weight).is_finite()
        },
    );
    if representable {
        let columns = sums.len();
        let first = members[0];
        let shifts = &data[first * columns..(first + 1) * columns];
        for ((sum, &correction), &shift) in sums.iter_mut().zip(corrections.iter()).zip(shifts) {
            *sum = shift + (*sum + correction) / total_weight;
        }
        if sums.iter().all(|mean| mean.is_finite()) {
            return Ok(total_weight);
        }
    }
    scaled_kernel_group_means(members, weights, values, scales, sums, corrections)
}

/// The scaled two-pass group means: divide every member value by its column
/// maximum before the compensated weighted sum.
fn scaled_kernel_group_means(
    members: &[usize],
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    scales: &mut [f64],
    sums: &mut [f64],
    corrections: &mut [f64],
) -> Result<f64> {
    scales.fill(0.0);
    sums.fill(0.0);
    corrections.fill(0.0);
    let data = values.as_slice();

    // An installed hardware table runs the same bodies; otherwise they inline.
    let hardware = crate::kernels::installed();
    let (rows, raw) = (values.nrows(), weights.raw_scale);
    let first_pass = match hardware {
        Some(kernels) => (kernels.group_scales)(weights.values, raw, data, rows, members, scales),
        None => crate::kernels::group_scales(weights.values, raw, data, rows, members, scales),
    };
    let (weight_sum, weight_correction) = first_pass
        .ok_or_else(|| invalid_argument("borrowed MAP membership exceeds the observation axis"))?;
    compensated_finite(weight_sum, weight_correction)?;
    let total_weight = weight_sum + weight_correction;
    if !total_weight.is_finite() || total_weight <= 0.0 {
        return Err(invalid_data(
            "borrowed MAP group weight sum is not finite and positive",
        ));
    }

    // Coordinates are independent; each keeps its scalar Neumaier row order.
    let second_pass = hardware.map_or(
        crate::kernels::scaled_group_sums as crate::kernels::ScaledGroupSums,
        |kernels| kernels.scaled_group_sums,
    );
    second_pass(
        weights.values,
        raw,
        data,
        members,
        scales,
        sums,
        corrections,
    );
    for (&sum, &correction) in sums.iter().zip(corrections.iter()) {
        compensated_finite(sum, correction)?;
    }

    for ((&scale, sum), &correction) in scales.iter().zip(sums.iter_mut()).zip(corrections.iter()) {
        let normalized_sum = *sum + correction;
        let mean = if scale == 0.0 {
            0.0
        } else {
            scale * (normalized_sum / total_weight)
        };
        if !mean.is_finite() {
            return Err(invalid_data("borrowed MAP group mean is not finite"));
        }
        *sum = mean;
    }
    Ok(total_weight)
}

fn certify(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    update_workspace_norms(residuals, weights, workspace, false)?;
    certify_with_current_norms(design, weights, residuals, tolerance, workspace)
}

fn certify_initial(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    workspace
        .residual_norms
        .copy_from_slice(&workspace.reference_norms);
    certify_with_current_norms(design, weights, residuals, tolerance, workspace)
}

/// Reject only the reusable owner's zero-iteration shortcut. Its caller has
/// just computed these norms from the unchanged matrix. The unit and bounded
/// weighted finite-error proofs are documented in ARCHITECTURE.md; their
/// conservative exponent windows are not convergence tolerances.
fn initial_group_rejects(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<bool> {
    let unit = weights.values.is_none();
    if (unit && weights.raw_scale != 1.0)
        || design.effects().is_empty()
        || workspace.reference_norms.len() != values.ncols()
        || workspace
            .reference_norms
            .iter()
            .any(|&norm| !(unit && norm == 0.0 || (1e-20..=1e20).contains(&norm)))
        || design
            .total_term_slots()
            .checked_mul(values.ncols())
            .is_none()
    {
        return Ok(false);
    }
    if let Some(raw) = weights.values {
        const LIMIT: f64 = 4_294_967_296.0;
        if !(1.0 / LIMIT..=LIMIT).contains(&weights.raw_scale)
            || values.nrows() as u128 > (1_u128 << 32)
        {
            return Ok(false);
        }
        // Positive division by the fixed scale is monotone. This one raw
        // minimum scan bounds every computed normalized weight without a bank.
        // A weighted zero norm is excluded above: sqrt(q)*x can underflow even
        // within this window while a later group mean remains nonzero.
        let minimum = raw.iter().copied().fold(weights.raw_scale, f64::min);
        if minimum / weights.raw_scale < 1.0 / LIMIT {
            return Ok(false);
        }
    }
    // Validated immutable design owners establish nonempty, in-range
    // memberships; set_rhs_count establishes all per-RHS workspace lengths.
    // Evaluate this while the pilot scratch loan is active, exactly where the
    // original certificate selects its serial/narrow/wide group arithmetic.
    let parallel = certificate_parallel_workspaces_fit(design, values, workspace)?;
    workspace
        .residual_norms
        .copy_from_slice(&workspace.reference_norms);
    workspace
        .rhs_scales
        .copy_from_slice(&workspace.reference_norms);
    workspace.rhs_relative_infinity_norms.fill(0.0);
    let effect = &design.effects()[0];
    let mut group_weight = 0.0;
    if parallel && values.ncols() > PARALLEL_GROUP_MAX_RHS {
        for (tile, means) in workspace
            .group_sums
            .chunks_mut(PARALLEL_GROUP_MAX_RHS)
            .enumerate()
        {
            let start = tile * PARALLEL_GROUP_MAX_RHS;
            group_weight = fused_group_means_by_value(
                effect,
                0,
                weights,
                values.nrows(),
                means,
                |row, rhs| values[(row, start + rhs)],
            )?;
        }
    } else {
        // All RHSs must share the original whole-group fallback decision.
        group_weight = group_means(
            effect.level_index().checked_members(0)?,
            weights,
            values,
            workspace,
            GroupMeanRoute::OnePass,
        )?;
    }
    let mut fold = CertificateFold::default();
    fold.level(
        group_weight,
        &workspace.group_sums,
        &workspace.rhs_scales,
        &mut workspace.rhs_relative_infinity_norms,
        weights.raw_scale,
    )?;
    let rejected = fold.relative_infinity_norm > tolerance;
    if rejected && crate::diagnostics::solver_trace_enabled() {
        eprintln!(
            "FEREG_SOLVER_TRACE|phase=initial_certificate_screen|rejected=1|weighted={}|rows={}|rhs={}|parallel={}|wide={}",
            usize::from(!unit), values.nrows(), values.ncols(), usize::from(parallel),
            usize::from(parallel && values.ncols() > PARALLEL_GROUP_MAX_RHS),
        );
    }
    Ok(rejected)
}

fn certificate_parallel_workspaces_fit(
    design: &BorrowedInterceptFeDesign<'_>,
    residuals: &DenseMatrix,
    workspace: &BorrowedMapWorkspace,
) -> Result<bool> {
    let maximum_levels = design
        .effects()
        .iter()
        .map(crate::fe::BorrowedInterceptEffect::n_levels)
        .max()
        .unwrap_or(0);
    let maximum_mean_count = maximum_levels
        .checked_mul(residuals.ncols())
        .ok_or_else(|| allocation("borrowed MAP certificate dimensions overflow"))?;
    Ok(!design.effects().is_empty()
        && crate::parallel::StagePlan::current(residuals.as_slice().len() as u128, maximum_levels)
            .lanes
            > 1
        && workspace.parallel_group_weights.len() >= maximum_levels
        && workspace.parallel_group_means.len() >= maximum_mean_count)
}

fn certify_with_current_norms(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    workspace.rhs_relative_infinity_norms.fill(0.0);
    for rhs in 0..residuals.ncols() {
        let scale = workspace.reference_norms[rhs].max(workspace.residual_norms[rhs]);
        if !scale.is_finite() || scale < 0.0 {
            return Err(invalid_data("borrowed MAP RHS scale is invalid"));
        }
        workspace.rhs_scales[rhs] = scale;
    }

    certify_with_current_scales(design, weights, residuals, tolerance, workspace)
}

fn certify_with_current_scales(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    if certificate_parallel_workspaces_fit(design, residuals, workspace)? {
        return certify_effects_parallel(design, weights, residuals, tolerance, workspace);
    }

    let columns = residuals.ncols();
    let mut fold = CertificateFold::default();
    for effect in design.effects() {
        for level in 0..effect.n_levels() {
            let members = effect.level_index().checked_members(level)?;
            if members.is_empty() {
                return Err(invalid_argument(
                    "borrowed MAP certificate contains an empty final level",
                ));
            }
            let group_weight = group_means(
                members,
                weights,
                residuals,
                workspace,
                GroupMeanRoute::OnePass,
            )?;
            fold.level(
                group_weight,
                &workspace.group_sums[..columns],
                &workspace.rhs_scales,
                &mut workspace.rhs_relative_infinity_norms,
                weights.raw_scale,
            )?;
        }
    }
    fold.finish(tolerance)
}

fn certify_effects_parallel(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    let columns = residuals.ncols();
    let mut fold = CertificateFold::default();
    for effect in design.effects() {
        let levels = effect.n_levels();
        let mean_count = levels
            .checked_mul(columns)
            .ok_or_else(|| allocation("borrowed MAP certificate dimensions overflow"))?;
        fill_parallel_group_statistics(
            effect,
            weights,
            residuals,
            &mut workspace.parallel_group_weights[..levels],
            &mut workspace.parallel_group_means[..mean_count],
            GroupMeanRoute::OnePass,
        )?;
        let group_weights = &workspace.parallel_group_weights[..levels];
        let means = &workspace.parallel_group_means[..mean_count];
        for level in 0..levels {
            fold.level(
                group_weights[level],
                &means[level * columns..(level + 1) * columns],
                &workspace.rhs_scales,
                &mut workspace.rhs_relative_infinity_norms,
                weights.raw_scale,
            )?;
        }
    }
    fold.finish(tolerance)
}

/// Running FE moment certificate, folded level-major and RHS-minor.
#[derive(Default)]
struct CertificateFold {
    absolute_infinity_norm: f64,
    relative_infinity_norm: f64,
    relative_square_sum: f64,
    relative_count: usize,
}

impl CertificateFold {
    /// Fold one level's group means, one per RHS, at its group weight.
    fn level(
        &mut self,
        group_weight: f64,
        means: &[f64],
        rhs_scales: &[f64],
        rhs_relative_infinity_norms: &mut [f64],
        raw_scale: f64,
    ) -> Result<()> {
        let root_group_weight = group_weight.sqrt();
        if !root_group_weight.is_finite() || root_group_weight <= 0.0 {
            return Err(invalid_data(
                "borrowed MAP certificate group weight is invalid",
            ));
        }
        let hardware = crate::kernels::installed();
        for (rhs, &mean) in means.iter().enumerate() {
            let denominator = rhs_scales[rhs];
            let relative = if denominator == 0.0 {
                if mean == 0.0 {
                    0.0
                } else {
                    return Err(invalid_data(
                        "borrowed MAP zero-scale certificate has a nonzero moment",
                    ));
                }
            } else {
                (mean.abs() / denominator) * root_group_weight
            };
            if !relative.is_finite() {
                return Err(invalid_data("borrowed MAP certificate ratio is not finite"));
            }
            self.relative_infinity_norm = self.relative_infinity_norm.max(relative);
            rhs_relative_infinity_norms[rhs] = rhs_relative_infinity_norms[rhs].max(relative);
            self.relative_square_sum =
                crate::kernels::fused(hardware, relative, relative, self.relative_square_sum);
            if !self.relative_square_sum.is_finite() {
                return Err(invalid_data(
                    "borrowed MAP certificate two-norm accumulation is not finite",
                ));
            }
            self.relative_count = self
                .relative_count
                .checked_add(1)
                .ok_or_else(|| allocation("borrowed MAP certificate moment count overflows"))?;
            let normalized_moment = mean.abs() * group_weight;
            let raw_moment = normalized_moment * raw_scale;
            self.absolute_infinity_norm =
                self.absolute_infinity_norm.max(if raw_moment.is_finite() {
                    raw_moment
                } else if normalized_moment == 0.0 {
                    0.0
                } else {
                    f64::INFINITY
                });
        }
        Ok(())
    }

    fn finish(self, tolerance: f64) -> Result<FeCertificate> {
        let relative_two_norm = if self.relative_count == 0 {
            0.0
        } else {
            (self.relative_square_sum / self.relative_count as f64).sqrt()
        };
        if !relative_two_norm.is_finite() {
            return Err(invalid_data(
                "borrowed MAP certificate two norm is not finite",
            ));
        }
        Ok(FeCertificate {
            absolute_infinity_norm: self.absolute_infinity_norm,
            relative_infinity_norm: self.relative_infinity_norm,
            relative_two_norm,
            passed: self.relative_infinity_norm <= tolerance,
        })
    }
}

/// Form I-S from its FE contributions, not by subtracting two observation
/// vectors. The latter introduces off-model-space cancellation error that CG
/// can amplify along weak graph directions despite passing FE normal equations.
/// Reuses admitted mean buffers; optional products record the same action.
/// PPML and CMG polishing share this construction without subtracting two
/// observation-scale matrices. Ordinary OLS absorption retains its arithmetic.
fn borrowed_symmetric_projection_from_group_means_recording(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    input: &DenseMatrix,
    projection: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    mut coefficients: Option<&mut FeCoefficientBatch>,
) -> Result<()> {
    let columns = input.ncols();
    if projection.nrows() != input.nrows()
        || projection.ncols() != columns
        || input.nrows() != design.nobs()
        || columns == 0
    {
        return Err(invalid_argument(
            "categorical projection action dimensions differ",
        ));
    }
    projection.as_mut_slice().fill(0.0);
    let effects = design.effects().len();
    for effect_index in (0..effects).chain((0..effects.saturating_sub(1)).rev()) {
        let effect = &design.effects()[effect_index];
        let cells = effect
            .n_levels()
            .checked_mul(columns)
            .ok_or_else(|| allocation("categorical projection group dimensions overflow"))?;
        let plan = crate::parallel::StagePlan::current(
            input.nrows() as u128 * columns as u128,
            effect.n_levels(),
        );
        if plan.lanes > 1 && workspace.parallel_group_means.len() >= cells {
            plan.groups_mut(
                &mut workspace.parallel_group_means[..cells],
                columns,
                effect.level_index().membership_offsets(),
                |first, means| {
                    for (local, means) in means.chunks_mut(columns).enumerate() {
                        projection_group_means(
                            effect,
                            first + local,
                            weights,
                            input.nrows(),
                            means,
                            input,
                            projection,
                        )?;
                    }
                    Ok(())
                },
            )?;
            let means = &workspace.parallel_group_means[..cells];
            if let Some(batch) = coefficients.as_mut() {
                batch.add_intercept_level_major(effect_index, means, effect.n_levels())?;
            }
            crate::parallel::stage_try_for_each(
                projection
                    .as_mut_slice()
                    .par_chunks_mut(columns)
                    .enumerate(),
                input.nrows() as u128 * columns as u128,
                |(row, values)| -> Result<()> {
                    let start = effect.codes()[row] as usize * columns;
                    for (value, mean) in values.iter_mut().zip(&means[start..start + columns]) {
                        *value += mean;
                        if !value.is_finite() {
                            return Err(invalid_data("categorical projection action is nonfinite"));
                        }
                    }
                    Ok(())
                },
            )?;
        } else {
            let means = &mut workspace.group_sums[..columns];
            for level in 0..effect.n_levels() {
                projection_group_means(
                    effect,
                    level,
                    weights,
                    input.nrows(),
                    means,
                    input,
                    projection,
                )?;
                if let Some(batch) = coefficients.as_mut() {
                    batch.add_intercept_level(effect_index, level, means)?;
                }
                for &row in effect.level_index().checked_members(level)? {
                    for rhs in 0..columns {
                        projection[(row, rhs)] += means[rhs];
                        if !projection[(row, rhs)].is_finite() {
                            return Err(invalid_data("categorical projection action is nonfinite"));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn projection_group_means(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    level: usize,
    weights: ValidatedWeights<'_>,
    nrows: usize,
    means: &mut [f64],
    input: &DenseMatrix,
    projection: &DenseMatrix,
) -> Result<()> {
    for (tile, output) in means.chunks_mut(PARALLEL_GROUP_MAX_RHS).enumerate() {
        let start = tile * PARALLEL_GROUP_MAX_RHS;
        fused_group_means_by_value(effect, level, weights, nrows, output, |row, rhs| {
            input[(row, start + rhs)] - projection[(row, start + rhs)]
        })?;
    }
    Ok(())
}

fn cg_projection_action(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    input: &DenseMatrix,
    projection: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    coefficients: Option<&mut FeCoefficientBatch>,
    stable: bool,
) -> Result<()> {
    if stable {
        borrowed_symmetric_projection_from_group_means_recording(
            design,
            weights,
            input,
            projection,
            workspace,
            coefficients,
        )
    } else {
        borrowed_symmetric_projection_into_recording(
            design,
            weights,
            input,
            projection,
            workspace,
            coefficients,
        )
    }
}

fn borrowed_symmetric_projection_into_recording(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    input: &DenseMatrix,
    residual: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    mut coefficients: Option<&mut FeCoefficientBatch>,
) -> Result<()> {
    if residual.nrows() != input.nrows() || residual.ncols() != input.ncols() {
        return Err(invalid_argument(
            "borrowed CG projection buffer dimensions differ",
        ));
    }
    let effect_count = design.effects().len();
    if effect_count == 0 {
        residual.as_mut_slice().fill(0.0);
        return Ok(());
    }
    if effect_count > 1 {
        project_effect_recording_from_input(
            &design.effects()[0],
            weights,
            input,
            residual,
            workspace,
            coefficients
                .as_deref_mut()
                .map(|coefficients| (coefficients, 0)),
        )?;
        for effect_index in 1..effect_count {
            project_effect_recording(
                &design.effects()[effect_index],
                weights,
                residual,
                workspace,
                coefficients
                    .as_deref_mut()
                    .map(|coefficients| (coefficients, effect_index)),
            )?;
        }
        for effect_index in (1..effect_count - 1).rev() {
            project_effect_recording(
                &design.effects()[effect_index],
                weights,
                residual,
                workspace,
                coefficients
                    .as_deref_mut()
                    .map(|coefficients| (coefficients, effect_index)),
            )?;
        }
    } else {
        residual.as_mut_slice().copy_from_slice(input.as_slice());
    }
    project_effect_recording_with_complement(
        &design.effects()[0],
        weights,
        residual,
        workspace,
        coefficients.map(|coefficients| (coefficients, 0)),
        input,
    )
}

fn borrowed_residual_sweep_recording<R: InterceptCoefficientRecorder>(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    mut coefficients: Option<&mut R>,
    symmetric: bool,
) -> Result<()> {
    for (effect_index, effect) in design.effects().iter().enumerate() {
        project_effect_recording(
            effect,
            weights,
            values,
            workspace,
            coefficients
                .as_deref_mut()
                .map(|coefficients| (coefficients, effect_index)),
        )?;
    }
    if symmetric && design.effects().len() > 1 {
        for effect_index in (0..design.effects().len() - 1).rev() {
            project_effect_recording(
                &design.effects()[effect_index],
                weights,
                values,
                workspace,
                coefficients
                    .as_deref_mut()
                    .map(|coefficients| (coefficients, effect_index)),
            )?;
        }
    }
    Ok(())
}

fn borrowed_irons_tuck_candidate(
    x0: &DenseMatrix,
    x1: &DenseMatrix,
    x2: &DenseMatrix,
) -> Result<Option<(DenseMatrix, f64)>> {
    if x0.nrows() != x1.nrows()
        || x0.ncols() != x1.ncols()
        || x0.nrows() != x2.nrows()
        || x0.ncols() != x2.ncols()
    {
        return Err(invalid_argument(
            "borrowed Irons-Tuck states have inconsistent dimensions",
        ));
    }
    let mut d1d1 = 0.0_f64;
    let mut d1d2 = 0.0_f64;
    let mut d2d2 = 0.0_f64;
    for ((a, b), c) in x0.as_slice().iter().zip(x1.as_slice()).zip(x2.as_slice()) {
        let d1 = b - a;
        let d2 = c - 2.0 * b + a;
        d1d1 = d1.mul_add(d1, d1d1);
        d1d2 = d1.mul_add(d2, d1d2);
        d2d2 = d2.mul_add(d2, d2d2);
    }
    if !d2d2.is_finite() || d2d2 <= f64::EPSILON * d1d1.max(1.0) {
        return Ok(None);
    }
    let alpha = (-d1d2 / d2d2).clamp(-8.0, 8.0);
    if !alpha.is_finite() {
        return Ok(None);
    }
    let mut candidate = fallible_matrix_copy(x0, "unable to reserve Irons-Tuck candidate")?;
    for (((output, a), b), c) in candidate
        .as_mut_slice()
        .iter_mut()
        .zip(x0.as_slice())
        .zip(x1.as_slice())
        .zip(x2.as_slice())
    {
        let d1 = b - a;
        let d2 = c - 2.0 * b + a;
        *output = a + 2.0 * alpha * d1 + alpha * alpha * d2;
        if !output.is_finite() {
            return Ok(None);
        }
    }
    Ok(Some((candidate, alpha)))
}

fn weighted_dot_columns_into(
    left: &DenseMatrix,
    right: &DenseMatrix,
    weights: ValidatedWeights<'_>,
    output: &mut [f64],
    corrections: &mut [f64],
    parallel_sums: &mut [f64],
) -> Result<()> {
    if left.nrows() != right.nrows()
        || left.ncols() != right.ncols()
        || output.len() != left.ncols()
        || corrections.len() != left.ncols()
    {
        return Err(invalid_argument(
            "borrowed CG weighted-dot dimensions differ",
        ));
    }
    output.fill(0.0);
    corrections.fill(0.0);
    let chunks = left.nrows().div_ceil(PARALLEL_DOT_CHUNK_ROWS);
    let parallel_elements = chunks
        .checked_mul(left.ncols())
        .ok_or_else(|| allocation("borrowed CG parallel dot dimensions overflow"))?;
    if left.nrows() >= PARALLEL_PROJECTION_MIN_ROWS
        && left.ncols() <= FUSED_PARALLEL_MAX_RHS
        && rayon::current_num_threads() > 1
        && parallel_sums.len() >= parallel_elements
    {
        parallel_sums[..parallel_elements]
            .par_chunks_mut(left.ncols())
            .enumerate()
            .try_for_each(|(chunk, sums)| -> Result<()> {
                sums.fill(0.0);
                let mut chunk_corrections = [0.0_f64; FUSED_PARALLEL_MAX_RHS];
                let start = chunk * PARALLEL_DOT_CHUNK_ROWS;
                let end = (start + PARALLEL_DOT_CHUNK_ROWS).min(left.nrows());
                if let Some(weight_values) = weights.values {
                    for row in start..end {
                        let weight = weight_values[row] / weights.raw_scale;
                        for rhs in 0..left.ncols() {
                            let term = weight * left[(row, rhs)] * right[(row, rhs)];
                            if !term.is_finite() {
                                return Err(invalid_data(
                                    "borrowed CG weighted inner product produced a nonfinite term",
                                ));
                            }
                            compensated_add(term, &mut sums[rhs], &mut chunk_corrections[rhs])?;
                        }
                    }
                } else {
                    for row in start..end {
                        for rhs in 0..left.ncols() {
                            let term = left[(row, rhs)] * right[(row, rhs)];
                            if !term.is_finite() {
                                return Err(invalid_data(
                                    "borrowed CG weighted inner product produced a nonfinite term",
                                ));
                            }
                            compensated_add(term, &mut sums[rhs], &mut chunk_corrections[rhs])?;
                        }
                    }
                }
                for rhs in 0..left.ncols() {
                    sums[rhs] += chunk_corrections[rhs];
                    if !sums[rhs].is_finite() {
                        return Err(invalid_data(
                            "borrowed CG weighted inner product is nonfinite",
                        ));
                    }
                }
                Ok(())
            })?;
        for chunk in 0..chunks {
            for rhs in 0..left.ncols() {
                compensated_add(
                    parallel_sums[chunk * left.ncols() + rhs],
                    &mut output[rhs],
                    &mut corrections[rhs],
                )?;
            }
        }
        for rhs in 0..output.len() {
            output[rhs] += corrections[rhs];
            if !output[rhs].is_finite() {
                return Err(invalid_data(
                    "borrowed CG weighted inner product is nonfinite",
                ));
            }
        }
        return Ok(());
    }
    for row in 0..left.nrows() {
        let weight = weights.normalized(row);
        for rhs in 0..left.ncols() {
            let term = weight * left[(row, rhs)] * right[(row, rhs)];
            if !term.is_finite() {
                return Err(invalid_data(
                    "borrowed CG weighted inner product produced a nonfinite term",
                ));
            }
            compensated_add(term, &mut output[rhs], &mut corrections[rhs])?;
        }
    }
    for rhs in 0..output.len() {
        output[rhs] += corrections[rhs];
        if !output[rhs].is_finite() {
            return Err(invalid_data(
                "borrowed CG weighted inner product is nonfinite",
            ));
        }
    }
    Ok(())
}

fn cg_ratios_into(
    numerators: &[f64],
    denominators: &[f64],
    output: &mut [f64],
    message: &'static str,
) -> Result<()> {
    if numerators.len() != denominators.len() || output.len() != numerators.len() {
        return Err(invalid_argument("borrowed CG recurrence dimensions differ"));
    }
    for rhs in 0..output.len() {
        let numerator = numerators[rhs];
        let denominator = denominators[rhs];
        if !numerator.is_finite() || numerator < 0.0 {
            return Err(invalid_data("borrowed CG recurrence numerator is invalid"));
        }
        if numerator == 0.0 {
            output[rhs] = 0.0;
            continue;
        }
        if !denominator.is_finite() || denominator <= 0.0 {
            return Err(invalid_data(message));
        }
        let ratio = numerator / denominator;
        if !ratio.is_finite() || ratio < 0.0 {
            return Err(invalid_data("borrowed CG recurrence ratio is invalid"));
        }
        output[rhs] = ratio;
    }
    Ok(())
}

fn cg_hestenes_error_and_update(
    alpha: &[f64],
    residual_square: &[f64],
    improvement_potential: &mut [f64],
    values: &DenseMatrix,
    direction: &DenseMatrix,
    weights: ValidatedWeights<'_>,
) -> Result<f64> {
    if alpha.len() != residual_square.len()
        || alpha.len() != improvement_potential.len()
        || alpha.len() != values.ncols()
        || values.nrows() != direction.nrows()
        || values.ncols() != direction.ncols()
    {
        return Err(invalid_argument(
            "borrowed CG Hestenes recurrence dimensions differ",
        ));
    }
    let mut largest_error = 0.0_f64;
    for rhs in 0..alpha.len() {
        let decrease = alpha[rhs] * residual_square[rhs];
        let previous = improvement_potential[rhs];
        if !decrease.is_finite() || decrease < 0.0 || !previous.is_finite() || previous < 0.0 {
            return Err(invalid_data(
                "borrowed CG Hestenes improvement values are invalid",
            ));
        }
        let mut remaining = previous - decrease;
        let roundoff_floor = f64::EPSILON * previous.max(1.0);
        if remaining < -roundoff_floor {
            remaining = cg_audit_remaining_energy(
                values, direction, weights, rhs, alpha[rhs], previous, decrease,
            )?;
        }
        improvement_potential[rhs] = remaining.max(f64::EPSILON);
        let numerator = if decrease < 1e-15 { 0.0 } else { decrease };
        let error = (numerator / improvement_potential[rhs]).sqrt();
        if !error.is_finite() {
            return Err(invalid_data(
                "borrowed CG Hestenes recurrence error is nonfinite",
            ));
        }
        largest_error = largest_error.max(error);
    }
    Ok(largest_error)
}

fn cg_audit_remaining_energy(
    values: &DenseMatrix,
    direction: &DenseMatrix,
    weights: ValidatedWeights<'_>,
    rhs: usize,
    alpha: f64,
    previous: f64,
    decrease: f64,
) -> Result<f64> {
    // A nearly absorbed RHS can lose its last energy bits in subtraction.
    // Audit only a small arithmetic discrepancy; invalid steps still fail.
    // This bound checks energy consistency, not FE or recurrence convergence.
    let allowance = 64.0 * f64::EPSILON * previous.max(decrease).max(1.0);
    if decrease - previous > allowance {
        return Err(invalid_data(
            "borrowed CG Hestenes improvement potential became negative",
        ));
    }
    let mut scales = [0.0_f64; 2];
    for row in 0..values.nrows() {
        let before = values[(row, rhs)];
        // Match the pending solution update, including its separate multiply
        // and subtraction. The audit never mutates the iterate or direction.
        let after = before - alpha * direction[(row, rhs)];
        let root_weight = weights.normalized(row).sqrt();
        for (index, value) in [before, after].into_iter().enumerate() {
            let weighted = root_weight * value;
            if !weighted.is_finite() {
                return Err(invalid_data("borrowed CG energy audit is nonfinite"));
            }
            scales[index] = scales[index].max(weighted.abs());
        }
    }
    let mut sums = [0.0_f64; 2];
    let mut corrections = [0.0_f64; 2];
    for row in 0..values.nrows() {
        let before = values[(row, rhs)];
        let after = before - alpha * direction[(row, rhs)];
        let root_weight = weights.normalized(row).sqrt();
        for (index, value) in [before, after].into_iter().enumerate() {
            if scales[index] != 0.0 {
                let scaled = (root_weight * value) / scales[index];
                compensated_add(scaled * scaled, &mut sums[index], &mut corrections[index])?;
            }
        }
    }
    let energies =
        [0, 1].map(|index| scales[index] * (scales[index] * (sums[index] + corrections[index])));
    let [before, after] = energies;
    if energies
        .iter()
        .any(|energy| !energy.is_finite() || *energy < 0.0)
        || (before - previous).abs() > allowance
        || ((before - after) - decrease).abs() > allowance
    {
        return Err(invalid_data(
            "borrowed CG Hestenes step disagrees with independent weighted energy",
        ));
    }
    if crate::diagnostics::phase_timing_enabled() {
        eprintln!("FEREG_PHASE|cg_energy_audit|rhs={rhs}|previous={previous:.17e}|decrease={decrease:.17e}|before={before:.17e}|after={after:.17e}");
    }
    Ok(after)
}

fn cg_update_solution_and_residual(
    values: &mut DenseMatrix,
    residual: &mut DenseMatrix,
    direction: &DenseMatrix,
    projection: &DenseMatrix,
    alpha: &[f64],
) -> Result<()> {
    if alpha.len() != values.ncols()
        || residual.nrows() != values.nrows()
        || residual.ncols() != values.ncols()
        || direction.nrows() != values.nrows()
        || direction.ncols() != values.ncols()
        || projection.nrows() != values.nrows()
        || projection.ncols() != values.ncols()
    {
        return Err(invalid_argument("borrowed CG update dimensions differ"));
    }
    let columns = values.ncols();
    if values.nrows() >= PARALLEL_PROJECTION_MIN_ROWS && rayon::current_num_threads() > 1 {
        values
            .as_mut_slice()
            .par_chunks_mut(columns)
            .zip(residual.as_mut_slice().par_chunks_mut(columns))
            .enumerate()
            .try_for_each(|(row, (values, residuals))| -> Result<()> {
                for rhs in 0..columns {
                    values[rhs] -= alpha[rhs] * direction[(row, rhs)];
                    residuals[rhs] -= alpha[rhs] * projection[(row, rhs)];
                    if !values[rhs].is_finite() || !residuals[rhs].is_finite() {
                        return Err(invalid_data(
                            "borrowed CG update produced a nonfinite value",
                        ));
                    }
                }
                Ok(())
            })?;
        return Ok(());
    }
    for row in 0..values.nrows() {
        for rhs in 0..columns {
            values[(row, rhs)] -= alpha[rhs] * direction[(row, rhs)];
            residual[(row, rhs)] -= alpha[rhs] * projection[(row, rhs)];
            if !values[(row, rhs)].is_finite() || !residual[(row, rhs)].is_finite() {
                return Err(invalid_data(
                    "borrowed CG update produced a nonfinite value",
                ));
            }
        }
    }
    Ok(())
}

fn cg_update_direction(
    direction: &mut DenseMatrix,
    residual: &DenseMatrix,
    beta: &[f64],
) -> Result<()> {
    if direction.nrows() != residual.nrows()
        || direction.ncols() != residual.ncols()
        || beta.len() != direction.ncols()
    {
        return Err(invalid_argument("borrowed CG direction dimensions differ"));
    }
    if direction.nrows() >= PARALLEL_PROJECTION_MIN_ROWS && rayon::current_num_threads() > 1 {
        let columns = direction.ncols();
        direction
            .as_mut_slice()
            .par_chunks_mut(columns)
            .zip(residual.as_slice().par_chunks(columns))
            .try_for_each(|(direction, residual)| -> Result<()> {
                for rhs in 0..columns {
                    direction[rhs] = residual[rhs] + beta[rhs] * direction[rhs];
                    if !direction[rhs].is_finite() {
                        return Err(invalid_data(
                            "borrowed CG direction update produced a nonfinite value",
                        ));
                    }
                }
                Ok(())
            })?;
        return Ok(());
    }
    for row in 0..direction.nrows() {
        for rhs in 0..direction.ncols() {
            direction[(row, rhs)] = residual[(row, rhs)] + beta[rhs] * direction[(row, rhs)];
            if !direction[(row, rhs)].is_finite() {
                return Err(invalid_data(
                    "borrowed CG direction update produced a nonfinite value",
                ));
            }
        }
    }
    Ok(())
}

/// Largest relative CG residual over RHS columns, from squared norms. A
/// column solved exactly at entry that later carries residual fails closed.
pub(super) fn cg_largest_relative_residual(current: &[f64], initial: &[f64]) -> Result<f64> {
    if current.len() != initial.len() {
        return Err(invalid_argument("CG residual-norm dimensions differ"));
    }
    let mut largest = 0.0_f64;
    for (current, initial) in current.iter().zip(initial) {
        if !current.is_finite() || *current < 0.0 || !initial.is_finite() || *initial < 0.0 {
            return Err(invalid_data("CG residual norm is invalid"));
        }
        let relative = if *initial == 0.0 {
            if *current == 0.0 {
                0.0
            } else {
                return Err(invalid_data(
                    "CG acquired residual from a zero initial norm",
                ));
            }
        } else {
            (current / initial).sqrt()
        };
        if !relative.is_finite() {
            return Err(invalid_data("CG relative residual is nonfinite"));
        }
        largest = largest.max(relative);
    }
    Ok(largest)
}

/// Bytes of a workspace peak that must carry a complete model.
fn modeled_bytes(peak: SolverWorkspacePeak) -> Result<u128> {
    peak.bytes()
        .map(u128::from)
        .ok_or_else(|| FeregError::new(ErrorCode::Internal, "solver workspace peak is unmodeled"))
}

fn fallible_matrix_copy(input: &DenseMatrix, label: &'static str) -> Result<DenseMatrix> {
    let mut output = fallible_matrix_zeros(input.nrows(), input.ncols(), label)?;
    output.as_mut_slice().copy_from_slice(input.as_slice());
    Ok(output)
}

fn fallible_matrix_zeros(nrows: usize, ncols: usize, label: &'static str) -> Result<DenseMatrix> {
    DenseMatrix::zeros(nrows, ncols).map_err(|error| {
        if error.code == ErrorCode::Allocation {
            allocation(label)
        } else {
            error
        }
    })
}

fn weighted_column_norms(
    values: &DenseMatrix,
    weights: ValidatedWeights<'_>,
    output: &mut [f64],
    scales: &mut [f64],
    scaled_square_sums: &mut [f64],
    partials: &mut [f64],
) -> Result<()> {
    if output.len() != values.ncols()
        || scales.len() != values.ncols()
        || scaled_square_sums.len() != values.ncols()
    {
        return Err(invalid_argument(
            "borrowed MAP norm workspace dimensions differ",
        ));
    }
    super::norms::column_norms(
        values,
        |row| {
            let root = weights.normalized(row).sqrt();
            if root.is_finite() && root > 0.0 {
                Ok(root)
            } else {
                Err(invalid_data(
                    "borrowed MAP normalized weight is not finite and positive",
                ))
            }
        },
        output,
        scales,
        scaled_square_sums,
        partials,
        "borrowed MAP weighted value is not finite",
        "borrowed MAP weighted column norm is not finite",
    )
}

fn compensated_add(value: f64, sum: &mut f64, correction: &mut f64) -> Result<()> {
    neumaier_add(value, sum, correction);
    if !sum.is_finite() || !correction.is_finite() {
        return Err(invalid_data(
            "borrowed MAP compensated accumulation is not finite",
        ));
    }
    Ok(())
}

fn compensated_finite(sum: f64, correction: f64) -> Result<()> {
    if sum.is_finite() && correction.is_finite() {
        Ok(())
    } else {
        Err(invalid_data(
            "borrowed MAP compensated accumulation is not finite",
        ))
    }
}

fn certified_result(
    residualized: DenseMatrix,
    stop_reason: SolveStopReason,
    iterations: usize,
    certificate: FeCertificate,
    setup_seconds: f64,
    solve_seconds: f64,
) -> Result<CertifiedBorrowedMap> {
    let normal_equation_residual = certificate.relative_two_norm;
    certified_solver_result(
        residualized,
        FeSolverKind::Map,
        stop_reason,
        iterations,
        certificate,
        normal_equation_residual,
        setup_seconds,
        solve_seconds,
    )
}

#[allow(clippy::too_many_arguments)]
fn certified_solver_result(
    residualized: DenseMatrix,
    solver: FeSolverKind,
    stop_reason: SolveStopReason,
    iterations: usize,
    certificate: FeCertificate,
    normal_equation_residual: f64,
    setup_seconds: f64,
    solve_seconds: f64,
) -> Result<CertifiedBorrowedMap> {
    let report = SolveReport {
        iterations,
        normal_equation_residual,
        setup_seconds,
        solve_seconds,
        ..SolveReport::single_route(solver, stop_reason, certificate)
    };
    if !report.certified() {
        return Err(invalid_data(
            "borrowed MAP attempted to return an uncertified result",
        ));
    }
    Ok(CertifiedBorrowedMap {
        residualized,
        report,
        coefficients: None,
        residual_norms: Vec::new(),
    })
}

const fn zero_certificate() -> FeCertificate {
    FeCertificate {
        absolute_infinity_norm: 0.0,
        relative_infinity_norm: 0.0,
        relative_two_norm: 0.0,
        passed: true,
    }
}

fn reserved_zeros<R: WorkspaceReservation>(
    elements: usize,
    reservation: &mut R,
    label: &'static str,
) -> Result<Vec<f64>> {
    let mut output = Vec::new();
    reservation.reserve_f64(&mut output, elements, label)?;
    if output.capacity() < elements {
        return Err(allocation(
            "borrowed MAP workspace reservation returned insufficient capacity",
        ));
    }
    crate::parallel::initialize_reserved(&mut output, elements, 0.0_f64);
    Ok(output)
}

pub(crate) const GRAPH_PROGRESS_CHECK_ITERATIONS: usize = 8;
pub(crate) const GRAPH_ATTEMPT_MAX_ITERATIONS: usize = 64;
pub(crate) const GRAPH_MAX_PROGRESS_CONTRACTION: f64 = 0.01;

#[derive(Default)]
struct CheckpointSchurCost {
    iterations: usize,
    contraction: f64,
    setup_seconds: f64,
    solve_seconds: f64,
    certification_seconds: f64,
    rejected: bool,
}

impl CheckpointSchurCost {
    fn add_to(&self, report: &mut SolveReport) {
        if !self.rejected {
            return;
        }
        report.iterations = report.iterations.saturating_add(self.iterations);
        report.setup_seconds += self.setup_seconds;
        report.solve_seconds += self.solve_seconds;
        report.certification_seconds += self.certification_seconds;
        report.pilot_iterations = 0;
        report.pilot_contraction = self.contraction;
        report.checkpoint_retries += usize::from(self.rejected);
        if self.rejected {
            report.warnings.push(format!(
                "bounded current-weight Schur checkpoint rejected after {} iterations; LSMR used untouched authoritative inputs",
                self.iterations,
            ));
        }
    }

    fn trace(&self, values: &DenseMatrix, outcome: &str) {
        if crate::diagnostics::solver_trace_enabled() {
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=checkpoint_schur_attempt|outcome={outcome}|rows={}|rhs={}|probe_budget=0|contraction={:.17e}|iterations={}|setup_seconds={:.17e}|solve_seconds={:.17e}",
                values.nrows(), values.ncols(), self.contraction,
                self.iterations, self.setup_seconds, self.solve_seconds,
            ));
        }
    }
}

fn checkpoint_schur_copy(
    values: &DenseMatrix,
    admitted_transient_bytes: u128,
) -> Result<DenseMatrix> {
    let payload = (values.as_slice().len() as u128)
        .checked_mul(8)
        .ok_or_else(|| allocation("checkpoint Schur candidate size overflow"))?;
    if payload > admitted_transient_bytes {
        return Err(allocation(
            "checkpoint Schur candidate exceeds admitted transient workspace",
        ));
    }
    fallible_matrix_copy(values, "unable to reserve checkpoint Schur candidate")
}

#[derive(Clone, Copy)]
enum SchurWorkLimit {
    Map { refine: bool },
    Checkpoint(usize),
    CmgSchur,
}

// Values remain authoritative input until reconstruction. A failed final
// certificate can instead hand off its finite observation-space candidate.
// Keep certified results inline; boxing would change admitted allocation lifetimes.
#[allow(clippy::large_enum_variant)]
enum SchurAttempt {
    Certified(CertifiedBorrowedMap),
    Continue {
        iterations: usize,
        candidate: bool,
        progress_contraction: Option<f64>,
    },
}

fn update_workspace_norms(
    values: &DenseMatrix,
    weights: ValidatedWeights<'_>,
    workspace: &mut BorrowedMapWorkspace,
    reference: bool,
) -> Result<()> {
    let output = if reference {
        &mut workspace.reference_norms
    } else {
        &mut workspace.residual_norms
    };
    // Dot products initialize their entire active bank. Group statistics are
    // rebuilt for every effect before projection/certification reads them.
    // Both banks are dead at every norm call, including pilot scratch loans;
    // the PCG pilot restores ownership, not numerical contents.
    let partials = if workspace.parallel_dot_sums.len() >= workspace.parallel_group_means.len() {
        &mut workspace.parallel_dot_sums
    } else {
        &mut workspace.parallel_group_means
    };
    weighted_column_norms(
        values,
        weights,
        output,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
        partials,
    )
}

#[cfg(test)]
mod tests;
