use std::time::Instant;

use super::auto::{auto_lsmr_route_at_cap, direct_lsmr_tolerance, AutoLsmrRoute};
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    BorrowedInterceptFeDesign, FeCertificate, FeSolverKind, LsmrConfig, LsmrPreconditioner,
    MapConfig, SolveReport, SolveStopReason,
};
use crate::memory::{estimate_fe_solver_memory, FeSolverMemoryBudget};
use rayon::prelude::*;
use within::{Effect, LsmrOptions, PreconditionerConfig};

const PARALLEL_PROJECTION_MIN_ROWS: usize = 65_536;
const FUSED_PARALLEL_MAX_RHS: usize = 8;
const PARALLEL_DOT_CHUNK_ROWS: usize = 32_768;
pub(crate) const SD_RECOVERY_PILOT_SWEEPS: usize = 4;
pub(crate) const SD_RECOVERY_CONTINUATION_CAP: usize = 4;
const SD_RECOVERY_LSMR_TOLERANCE_CAP: f64 = 1e-4;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BorrowedInterceptRecoveryMethod {
    Direct,
    SteepestDescent,
    HybridLsmr,
}

#[derive(Debug)]
pub(crate) struct BorrowedInterceptRecoveryCoefficients {
    pub(crate) coefficients: FeCoefficientBatch,
    pub(crate) iterations: usize,
    pub(crate) method: BorrowedInterceptRecoveryMethod,
}

impl FeCoefficientBatch {
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

    fn into_batch(self) -> Result<FeCoefficientBatch> {
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
        FeCoefficientBatch::from_values(effect_channels, level_counts, rhs_count, rhs_major)
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
    #[must_use]
    pub const fn residualized(&self) -> &DenseMatrix {
        &self.residualized
    }

    #[must_use]
    pub const fn report(&self) -> &SolveReport {
        &self.report
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
    reconstruction_tolerance: Option<f64>,
}

impl CgContinuation<'_> {
    const ORDINARY: Self = Self {
        reference_norms: None,
        refinement: CgRefinementPolicy::ORDINARY,
        retain_coefficients: false,
        reconstruction_tolerance: None,
    };
}

#[derive(Debug, Clone, Copy)]
struct AutoRunOptions {
    empty_design_is_structural_constant: bool,
    retain_coefficients: bool,
}

#[derive(Debug, Clone, Copy)]
struct AutoLsmrPlan {
    config: Option<LsmrConfig>,
    memory_budget: Option<FeSolverMemoryBudget>,
}

impl AutoLsmrPlan {
    const DISABLED: Self = Self {
        config: None,
        memory_budget: None,
    };

    const fn routed(config: LsmrConfig, memory_budget: Option<FeSolverMemoryBudget>) -> Self {
        Self {
            config: Some(config),
            memory_budget,
        }
    }
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
    fn new<R: WorkspaceReservation>(
        design: &BorrowedInterceptFeDesign<'_>,
        observation_count: usize,
        rhs_count: usize,
        reservation: &mut R,
    ) -> Result<Self> {
        let parallel_group_levels = if observation_count >= PARALLEL_PROJECTION_MIN_ROWS
            && rayon::current_num_threads() > 1
        {
            design
                .effects()
                .iter()
                .map(crate::fe::BorrowedInterceptEffect::n_levels)
                .max()
                .unwrap_or(0)
        } else {
            0
        };
        let parallel_group_mean_count = if rhs_count > 1 || parallel_group_levels > 0 {
            parallel_group_levels
                .checked_mul(rhs_count)
                .ok_or_else(|| allocation("borrowed MAP parallel group-mean dimensions overflow"))?
        } else {
            0
        };
        let parallel_dot_sum_count = if observation_count >= PARALLEL_PROJECTION_MIN_ROWS
            && rhs_count <= FUSED_PARALLEL_MAX_RHS
            && rayon::current_num_threads() > 1
        {
            observation_count
                .div_ceil(PARALLEL_DOT_CHUNK_ROWS)
                .checked_mul(rhs_count)
                .ok_or_else(|| allocation("borrowed CG parallel dot dimensions overflow"))?
        } else {
            0
        };
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
    let mut reservation = SystemWorkspaceReservation;
    absorb_with_reservation(design, weights, values, config, false, &mut reservation)
}

pub(crate) fn absorb_borrowed_intercept_map_retaining_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_with_reservation_mode(
        design,
        weights,
        values,
        config,
        false,
        false,
        true,
        &mut reservation,
    )
}

/// Residualize with batched weighted conjugate gradients over the borrowed
/// symmetric-Kaczmarz projection operator. No observation-scale categorical
/// codes or unit-weight vector are copied.
pub fn absorb_borrowed_intercept_cg_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_cg_with_reservation(
        design,
        weights,
        values,
        config,
        false,
        CgContinuation::ORDINARY,
        &mut reservation,
    )
}

pub(crate) fn absorb_borrowed_intercept_cg_retaining_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_cg_with_reservation(
        design,
        weights,
        values,
        config,
        false,
        CgContinuation {
            retain_coefficients: true,
            ..CgContinuation::ORDINARY
        },
        &mut reservation,
    )
}

/// Recover an intercept-only coefficient product with CG and continue the
/// existing recurrence until the observation-space reconstruction is strict
/// enough for categorical publication. Ordinary absorption intentionally does
/// not pay for this stronger coefficient-recovery condition.
pub(crate) fn absorb_borrowed_intercept_cg_recovery_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    reconstruction_tolerance: f64,
) -> Result<CertifiedBorrowedMap> {
    if !reconstruction_tolerance.is_finite() || reconstruction_tolerance <= 0.0 {
        return Err(invalid_argument(
            "borrowed CG recovery tolerance must be finite and positive",
        ));
    }
    let mut reservation = SystemWorkspaceReservation;
    absorb_cg_with_reservation(
        design,
        weights,
        values,
        config,
        false,
        CgContinuation {
            retain_coefficients: true,
            reconstruction_tolerance: Some(reconstruction_tolerance),
            ..CgContinuation::ORDINARY
        },
        &mut reservation,
    )
}

/// Residualize through modified LSMR and optionally retain the coefficient
/// solutions that produced the certified observation-space residuals.
pub(crate) fn absorb_borrowed_intercept_lsmr_certified(
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
        config,
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

/// Recover one intercept-only FE coefficient vector with a deterministic
/// forward-Kaczmarz/steepest-descent pilot. Fast-converging designs continue
/// the same recurrence; slow designs pass the already-reduced remainder to
/// modified LSMR and add its coefficient correction.
pub(crate) fn recover_borrowed_intercept_sd_lsmr_coefficients(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    target: &[f64],
    map: MapConfig,
    lsmr: LsmrConfig,
    reconstruction_tolerance: f64,
) -> Result<BorrowedInterceptRecoveryCoefficients> {
    validate_config(map)?;
    if !lsmr.tolerance.is_finite()
        || lsmr.tolerance <= 0.0
        || lsmr.max_iterations == 0
        || lsmr.local_size == Some(0)
        || !reconstruction_tolerance.is_finite()
        || reconstruction_tolerance <= 0.0
    {
        return Err(invalid_argument(
            "borrowed intercept recovery policy is invalid",
        ));
    }
    if design.nobs() != target.len()
        || target.is_empty()
        || design.effects().is_empty()
        || target.iter().any(|value| !value.is_finite())
    {
        return Err(invalid_data(
            "borrowed intercept recovery inputs are not finite and aligned",
        ));
    }

    let weights = validate_weights(weights, target.len())?;
    let mut reservation = SystemWorkspaceReservation;
    let mut residual =
        fallible_matrix_zeros(target.len(), 1, "unable to reserve SD recovery residual")?;
    residual.as_mut_slice().copy_from_slice(target);
    let mut direction =
        fallible_matrix_zeros(target.len(), 1, "unable to reserve SD recovery direction")?;
    let mut workspace = BorrowedMapWorkspace::new(design, target.len(), 1, &mut reservation)?;
    weighted_column_norms(
        &residual,
        weights,
        &mut workspace.reference_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
    let mut coefficients = FeCoefficientBatch::intercept_zeros(design, 1)?;
    if workspace.reference_norms[0] == 0.0 {
        return Ok(BorrowedInterceptRecoveryCoefficients {
            coefficients,
            iterations: 0,
            method: BorrowedInterceptRecoveryMethod::Direct,
        });
    }
    let mut increment = FeCoefficientBatch::intercept_zeros(design, 1)?;
    let mut dot = [0.0_f64; 1];
    let mut dot_correction = [0.0_f64; 1];

    if design.effects().len() == 1 {
        borrowed_residual_sweep_recording(
            design,
            weights,
            &mut residual,
            &mut workspace,
            Some(&mut coefficients),
            false,
        )?;
        return Ok(BorrowedInterceptRecoveryCoefficients {
            coefficients,
            iterations: 1,
            method: BorrowedInterceptRecoveryMethod::Direct,
        });
    }

    let pilot_limit = map.max_iterations.min(SD_RECOVERY_PILOT_SWEEPS);
    let mut iterations = 0_usize;
    while iterations < pilot_limit {
        sd_recovery_step(
            design,
            weights,
            &mut residual,
            &mut direction,
            &mut workspace,
            &mut coefficients,
            &mut increment,
            &mut dot,
            &mut dot_correction,
        )?;
        iterations += 1;
        if sd_recovery_residual_certified(
            design,
            weights,
            target,
            &residual,
            map.tolerance,
            reconstruction_tolerance,
            &mut workspace,
        )? {
            return Ok(BorrowedInterceptRecoveryCoefficients {
                coefficients,
                iterations,
                method: BorrowedInterceptRecoveryMethod::SteepestDescent,
            });
        }
    }

    let current_reconstruction = relative_reconstruction_residual(target, residual.as_slice())?;
    let predicted_iterations = predicted_sd_recovery_iterations(
        current_reconstruction,
        reconstruction_tolerance,
        iterations,
    );
    let continuation_limit = map.max_iterations.min(SD_RECOVERY_CONTINUATION_CAP);
    if predicted_iterations.is_some_and(|predicted| predicted <= continuation_limit) {
        while iterations < continuation_limit {
            sd_recovery_step(
                design,
                weights,
                &mut residual,
                &mut direction,
                &mut workspace,
                &mut coefficients,
                &mut increment,
                &mut dot,
                &mut dot_correction,
            )?;
            iterations += 1;
            if sd_recovery_residual_certified(
                design,
                weights,
                target,
                &residual,
                map.tolerance,
                reconstruction_tolerance,
                &mut workspace,
            )? {
                return Ok(BorrowedInterceptRecoveryCoefficients {
                    coefficients,
                    iterations,
                    method: BorrowedInterceptRecoveryMethod::SteepestDescent,
                });
            }
        }
    }

    weighted_column_norms(
        &residual,
        weights,
        &mut workspace.residual_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
    let contraction = workspace.residual_norms[0] / workspace.reference_norms[0];
    if !contraction.is_finite() || contraction <= 0.0 {
        return Err(invalid_data(
            "SD recovery pilot produced an invalid residual contraction",
        ));
    }
    let scaled_lsmr_tolerance = (map.tolerance / contraction)
        .max(lsmr.tolerance)
        .min(SD_RECOVERY_LSMR_TOLERANCE_CAP);
    let (lsmr_residual, report, _, correction) = run_borrowed_lsmr(
        design,
        weights,
        &residual,
        scaled_lsmr_tolerance,
        lsmr.max_iterations,
        LsmrConfig {
            tolerance: scaled_lsmr_tolerance,
            ..lsmr
        },
        true,
    )?;
    drop(lsmr_residual);
    if !report.certified() {
        return Err(invalid_data(
            "hybrid recovery LSMR correction did not certify its remainder",
        ));
    }
    coefficients.add_assign(
        &correction.ok_or_else(|| {
            invalid_data("hybrid recovery LSMR correction did not retain coefficients")
        })?,
        1.0,
    )?;
    Ok(BorrowedInterceptRecoveryCoefficients {
        coefficients,
        iterations: iterations.saturating_add(report.iterations),
        method: BorrowedInterceptRecoveryMethod::HybridLsmr,
    })
}

#[allow(clippy::too_many_arguments)]
fn sd_recovery_step(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    residual: &mut DenseMatrix,
    direction: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    coefficients: &mut FeCoefficientBatch,
    increment: &mut FeCoefficientBatch,
    dot: &mut [f64; 1],
    dot_correction: &mut [f64; 1],
) -> Result<()> {
    direction
        .as_mut_slice()
        .copy_from_slice(residual.as_slice());
    increment.fill_zero();
    borrowed_residual_sweep_recording(
        design,
        weights,
        direction,
        workspace,
        Some(increment),
        false,
    )?;
    for (projected, &original) in direction.as_mut_slice().iter_mut().zip(residual.as_slice()) {
        *projected = original - *projected;
        if !projected.is_finite() {
            return Err(invalid_data(
                "SD recovery direction contains a nonfinite value",
            ));
        }
    }
    weighted_dot_columns_into(
        residual,
        direction,
        weights,
        dot,
        dot_correction,
        &mut workspace.parallel_dot_sums,
    )?;
    let numerator = dot[0];
    weighted_dot_columns_into(
        direction,
        direction,
        weights,
        dot,
        dot_correction,
        &mut workspace.parallel_dot_sums,
    )?;
    let denominator = dot[0];
    if !numerator.is_finite() || !denominator.is_finite() || numerator <= 0.0 || denominator <= 0.0
    {
        return Err(invalid_data(
            "SD recovery line search lost positive finite curvature",
        ));
    }
    let step = numerator / denominator;
    if !step.is_finite() || step <= 0.0 {
        return Err(invalid_data(
            "SD recovery line-search step is not finite and positive",
        ));
    }
    for (value, &projected) in residual.as_mut_slice().iter_mut().zip(direction.as_slice()) {
        *value = (-step).mul_add(projected, *value);
        if !value.is_finite() {
            return Err(invalid_data(
                "SD recovery update produced a nonfinite residual",
            ));
        }
    }
    coefficients.add_assign(increment, step)
}

fn sd_recovery_residual_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    target: &[f64],
    residual: &DenseMatrix,
    tolerance: f64,
    reconstruction_tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<bool> {
    if relative_reconstruction_residual(target, residual.as_slice())? > reconstruction_tolerance {
        return Ok(false);
    }
    Ok(certify(design, weights, residual, tolerance, workspace)?.passed)
}

fn relative_reconstruction_residual(target: &[f64], residual: &[f64]) -> Result<f64> {
    if target.len() != residual.len() {
        return Err(invalid_argument(
            "SD recovery target and residual dimensions differ",
        ));
    }
    let mut target_infinity = 0.0_f64;
    let mut residual_infinity = 0.0_f64;
    for (&expected, &remainder) in target.iter().zip(residual) {
        target_infinity = target_infinity.max(expected.abs());
        residual_infinity = residual_infinity.max(remainder.abs());
    }
    let relative = if target_infinity == 0.0 {
        residual_infinity
    } else {
        residual_infinity / target_infinity
    };
    if relative.is_finite() {
        Ok(relative)
    } else {
        Err(invalid_data(
            "SD recovery reconstruction ratio is not finite",
        ))
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

fn predicted_sd_recovery_iterations(
    current_relative: f64,
    target_relative: f64,
    completed_iterations: usize,
) -> Option<usize> {
    if completed_iterations == 0
        || !current_relative.is_finite()
        || current_relative <= 0.0
        || current_relative >= 1.0
        || !target_relative.is_finite()
        || target_relative <= 0.0
    {
        return None;
    }
    if current_relative <= target_relative {
        return Some(completed_iterations);
    }
    let per_iteration = current_relative.powf(1.0 / completed_iterations as f64);
    if !per_iteration.is_finite() || per_iteration <= 0.0 || per_iteration >= 1.0 {
        return None;
    }
    let remaining = (target_relative / current_relative).ln() / per_iteration.ln();
    if !remaining.is_finite() || remaining < 0.0 || remaining > usize::MAX as f64 {
        return None;
    }
    Some(completed_iterations.saturating_add(remaining.ceil() as usize))
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
    weighted_column_norms(
        &values,
        weights,
        &mut workspace.reference_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
    let setup_seconds = setup_started.elapsed().as_secs_f64();
    let solve_started = Instant::now();
    let initial_certificate = certify(design, weights, &values, config.tolerance, &mut workspace)?;
    let mut coefficients = retain_coefficients
        .then(|| FeCoefficientBatch::intercept_zeros(design, values.ncols()))
        .transpose()?;
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
            if iterations < config.max_iterations {
                borrowed_residual_sweep_recording(
                    design,
                    weights,
                    &mut values,
                    &mut workspace,
                    coefficients.as_mut(),
                    config.symmetric,
                )?;
                iterations += 1;
                certificate = certify(design, weights, &values, config.tolerance, &mut workspace)?;
            }
            if !certificate.passed {
                continue;
            }
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
            return attach_optional_coefficients(result, coefficients);
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
            if iterations < config.max_iterations {
                borrowed_residual_sweep_recording(
                    design,
                    weights,
                    &mut values,
                    &mut workspace,
                    coefficients.as_mut(),
                    config.symmetric,
                )?;
                iterations += 1;
                baseline = certify(design, weights, &values, config.tolerance, &mut workspace)?;
            }
            if !baseline.passed {
                continue;
            }
            let normal_equation_residual = baseline.relative_two_norm;
            let result = certified_solver_result(
                values,
                FeSolverKind::IronsTuck,
                SolveStopReason::Certified,
                iterations,
                baseline,
                normal_equation_residual,
                setup_seconds,
                solve_started.elapsed().as_secs_f64(),
            )?;
            return attach_optional_coefficients(result, coefficients);
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
                    if iterations < config.max_iterations {
                        borrowed_residual_sweep_recording(
                            design,
                            weights,
                            &mut values,
                            &mut workspace,
                            coefficients.as_mut(),
                            config.symmetric,
                        )?;
                        iterations += 1;
                        candidate_certificate =
                            certify(design, weights, &values, config.tolerance, &mut workspace)?;
                    }
                    if !candidate_certificate.passed {
                        continue;
                    }
                    let normal_equation_residual = candidate_certificate.relative_two_norm;
                    let result = certified_solver_result(
                        values,
                        FeSolverKind::IronsTuck,
                        SolveStopReason::Certified,
                        iterations,
                        candidate_certificate,
                        normal_equation_residual,
                        setup_seconds,
                        solve_started.elapsed().as_secs_f64(),
                    )?;
                    return attach_optional_coefficients(result, coefficients);
                }
            }
        }
    }

    Err(invalid_data(
        "borrowed Irons-Tuck exhausted its iteration budget without certification",
    ))
}

/// Automatic borrowed-intercept policy: direct one-pass MAP for one effect;
/// otherwise run at most four symmetric MAP pilot sweeps and continue with
/// genuine CG unless the pilot already certifies.
pub fn absorb_borrowed_intercept_auto_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_auto_with_reservation(
        design,
        weights,
        values,
        config,
        AutoLsmrPlan::DISABLED,
        AutoRunOptions {
            empty_design_is_structural_constant: false,
            retain_coefficients: false,
        },
        &mut reservation,
    )
}

/// Prepared-data automatic policy. Categorical codes stay borrowed for the
/// MAP pilot, direct LSMR, and CG continuation. The LSMR route reuses prepared
/// memberships and does not create an owned observation-scale code copy.
pub(crate) fn absorb_borrowed_intercept_auto_routed_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    lsmr: LsmrConfig,
    solver_memory_budget: Option<FeSolverMemoryBudget>,
    retain_coefficients: bool,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_auto_with_reservation(
        design,
        weights,
        values,
        config,
        AutoLsmrPlan::routed(lsmr, solver_memory_budget),
        AutoRunOptions {
            empty_design_is_structural_constant: false,
            retain_coefficients,
        },
        &mut reservation,
    )
}

/// Prepared OLS compatibility path: an empty explicit FE design still owns a
/// structural one-level constant. The global projection uses only the existing
/// dense RHS plus O(K) workspace; it never materializes N constant codes.
pub(crate) fn absorb_borrowed_intercept_map_with_structural_constant_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_with_reservation(design, weights, values, config, true, &mut reservation)
}

/// Prepared OLS compatibility path for CG when the empty explicit design owns
/// a structural one-level constant.
pub(crate) fn absorb_borrowed_intercept_cg_with_structural_constant_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_cg_with_reservation(
        design,
        weights,
        values,
        config,
        true,
        CgContinuation::ORDINARY,
        &mut reservation,
    )
}

/// Certify a candidate residual against the original right-hand-side norms and
/// use the existing CG implementation only for any remaining FE component.
/// This is the polishing boundary used by coefficient-space solvers such as
/// CMG, whose bulk solve happens outside the observation-space CG recurrence.
pub(crate) fn polish_borrowed_intercept_cg_with_reference_norms_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    reference_norms: &[f64],
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_cg_with_reservation(
        design,
        weights,
        values,
        config,
        false,
        CgContinuation {
            reference_norms: Some(reference_norms),
            refinement: CgRefinementPolicy::ORDINARY,
            retain_coefficients: false,
            reconstruction_tolerance: None,
        },
        &mut reservation,
    )
}

/// Check an unchanged candidate against the same independent borrowed FE
/// certificate used at solver entry. This performs no projection or solver
/// iteration and is used when a caller expects a residual to already lie in
/// the within space.
pub(crate) fn certify_borrowed_intercept_candidate(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: &DenseMatrix,
    config: MapConfig,
) -> Result<FeCertificate> {
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
    weighted_column_norms(
        values,
        weights,
        &mut workspace.reference_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
    certify_initial(design, weights, values, config.tolerance, &mut workspace)
}

pub(crate) fn absorb_borrowed_intercept_auto_with_structural_constant_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    lsmr: LsmrConfig,
    solver_memory_budget: Option<FeSolverMemoryBudget>,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_auto_with_reservation(
        design,
        weights,
        values,
        config,
        AutoLsmrPlan::routed(lsmr, solver_memory_budget),
        AutoRunOptions {
            empty_design_is_structural_constant: true,
            retain_coefficients: false,
        },
        &mut reservation,
    )
}

/// Prepared OLS initial-RHS path. A one-effect projection is exact, so the raw
/// input is projected once and independently certified once. Residual reuse
/// continues to use the ordinary auto entry point, whose initial certificate
/// can return an already-within residual without another projection.
pub(crate) fn absorb_borrowed_intercept_auto_initial_with_structural_constant_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    lsmr: LsmrConfig,
    solver_memory_budget: Option<FeSolverMemoryBudget>,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    if design.effects().len() <= 1 {
        let direct = absorb_initial_with_reservation(
            design,
            weights,
            values,
            config,
            true,
            &mut reservation,
        )?;
        mark_auto(direct, 0, 0.0, 0.0, 0.0)
    } else {
        absorb_auto_with_reservation(
            design,
            weights,
            values,
            config,
            AutoLsmrPlan::routed(lsmr, solver_memory_budget),
            AutoRunOptions {
                empty_design_is_structural_constant: true,
                retain_coefficients: false,
            },
            &mut reservation,
        )
    }
}

fn absorb_auto_with_reservation<R: WorkspaceReservation>(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    lsmr_plan: AutoLsmrPlan,
    options: AutoRunOptions,
    reservation: &mut R,
) -> Result<CertifiedBorrowedMap> {
    absorb_auto_with_reservation_at_lsmr_cap(
        design,
        weights,
        values,
        config,
        lsmr_plan,
        options.empty_design_is_structural_constant,
        options.retain_coefficients,
        reservation,
        usize::MAX,
    )
}

#[allow(clippy::too_many_arguments)]
fn absorb_auto_with_reservation_at_lsmr_cap<R: WorkspaceReservation>(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    mut values: DenseMatrix,
    config: MapConfig,
    lsmr_plan: AutoLsmrPlan,
    empty_design_is_structural_constant: bool,
    retain_coefficients: bool,
    reservation: &mut R,
    direct_lsmr_cap: usize,
) -> Result<CertifiedBorrowedMap> {
    let AutoLsmrPlan {
        config: lsmr_config,
        memory_budget: solver_memory_budget,
    } = lsmr_plan;
    validate_config(config)?;
    if design.nobs() != values.nrows() {
        return Err(invalid_argument(
            "borrowed auto design and matrix observation counts differ",
        ));
    }
    if values.nrows() == 0 || values.ncols() == 0 {
        return Err(invalid_data(
            "borrowed auto requires nonempty observation and RHS axes",
        ));
    }
    if values.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(invalid_data("borrowed auto values must be finite"));
    }

    if design.effects().len() <= 1 {
        let direct = absorb_with_reservation(
            design,
            weights,
            values,
            config,
            empty_design_is_structural_constant,
            reservation,
        )?;
        return mark_auto(direct, 0, 0.0, 0.0, 0.0);
    }

    let weights = validate_weights(weights, values.nrows())?;
    let setup_started = Instant::now();
    let mut workspace =
        BorrowedMapWorkspace::new(design, values.nrows(), values.ncols(), reservation)?;
    let mut retained_coefficients = retain_coefficients
        .then(|| FeCoefficientBatch::intercept_zeros(design, values.ncols()))
        .transpose()?;
    weighted_column_norms(
        &values,
        weights,
        &mut workspace.reference_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
    let initial_certificate = certify(design, weights, &values, config.tolerance, &mut workspace)?;
    let pilot_setup_seconds = setup_started.elapsed().as_secs_f64();
    if initial_certificate.passed {
        let result = certified_solver_result(
            values,
            FeSolverKind::Map,
            SolveStopReason::Certified,
            0,
            initial_certificate,
            0.0,
            pilot_setup_seconds,
            0.0,
        )?;
        let result = if let Some(coefficients) = retained_coefficients {
            let (residualized, report) = result.into_parts();
            CertifiedBorrowedMap::from_certified_parts_with_coefficients(
                residualized,
                report,
                coefficients,
            )?
        } else {
            result
        };
        return mark_auto(result, 0, 0.0, 0.0, 0.0);
    }

    let initial_moment = initial_certificate.relative_two_norm;
    let pilot_budget = config.max_iterations.min(4);
    let pilot_started = Instant::now();
    let mut final_pilot_certificate = initial_certificate;
    for pilot_iteration in 1..=pilot_budget {
        for (effect_index, effect) in design.effects().iter().enumerate() {
            project_effect_recording(
                effect,
                weights,
                &mut values,
                &mut workspace,
                retained_coefficients
                    .as_mut()
                    .map(|coefficients| (coefficients, effect_index)),
            )?;
        }
        for effect_index in (0..design.effects().len() - 1).rev() {
            project_effect_recording(
                &design.effects()[effect_index],
                weights,
                &mut values,
                &mut workspace,
                retained_coefficients
                    .as_mut()
                    .map(|coefficients| (coefficients, effect_index)),
            )?;
        }
        final_pilot_certificate =
            certify(design, weights, &values, config.tolerance, &mut workspace)?;
        let contraction =
            pilot_contraction(initial_moment, final_pilot_certificate.relative_two_norm)?;
        if final_pilot_certificate.passed {
            let pilot_seconds = pilot_started.elapsed().as_secs_f64();
            let result = certified_solver_result(
                values,
                FeSolverKind::Map,
                SolveStopReason::Certified,
                pilot_iteration,
                final_pilot_certificate,
                contraction,
                pilot_setup_seconds,
                pilot_seconds,
            )?;
            let result = if let Some(coefficients) = retained_coefficients {
                let (residualized, report) = result.into_parts();
                CertifiedBorrowedMap::from_certified_parts_with_coefficients(
                    residualized,
                    report,
                    coefficients,
                )?
            } else {
                result
            };
            return mark_auto(result, pilot_iteration, contraction, 0.0, 0.0);
        }
    }
    let pilot_seconds = pilot_started.elapsed().as_secs_f64();
    let contraction = pilot_contraction(initial_moment, final_pilot_certificate.relative_two_norm)?;
    if pilot_budget == config.max_iterations {
        return Err(invalid_data(
            "borrowed auto exhausted its iteration budget during the MAP pilot",
        ));
    }

    // CG continues the original absorption problem. Keep the raw RHS norms
    // used by the pilot certificate: recomputing the reference scale from the
    // already-residualized pilot state makes the same requested tolerance
    // artificially stricter and can erase the pilot's benefit on slow graphs.
    let original_reference_norms = std::mem::take(&mut workspace.reference_norms);
    drop(workspace);
    let remaining_iterations = config.max_iterations - pilot_budget;
    let mut candidate_failure = None;
    let total_fe_coefficients = design.effects().iter().try_fold(0_usize, |total, effect| {
        total
            .checked_add(effect.n_levels())
            .ok_or_else(|| invalid_data("borrowed auto FE coefficient count overflows usize"))
    })?;
    let level_counts = design
        .effects()
        .iter()
        .map(crate::fe::BorrowedInterceptEffect::n_levels)
        .collect::<Vec<_>>();
    let solver_memory = if solver_memory_budget.is_some() {
        let local_size = lsmr_config.and_then(|lsmr| lsmr.local_size);
        Some(estimate_fe_solver_memory(
            values.nrows(),
            values.ncols(),
            &level_counts,
            lsmr_config.map_or(1, |lsmr| lsmr.threads.resolved()),
            local_size,
        )?)
    } else {
        None
    };
    let cg_fits = if let (Some(budget), Some(estimate)) = (solver_memory_budget, solver_memory) {
        let coefficient_bytes = if retain_coefficients {
            estimate
                .coefficient_batch_bytes
                .checked_mul(5)
                .ok_or_else(|| allocation("automatic CG retained coefficient memory overflows"))?
        } else {
            0
        };
        let required = estimate
            .cg_bytes
            .checked_add(coefficient_bytes)
            .ok_or_else(|| allocation("automatic CG workspace memory overflows"))?;
        budget.fits(required)
    } else {
        true
    };
    if let Some(lsmr_config) = lsmr_config {
        let resolved_preconditioner = resolve_borrowed_lsmr_preconditioner(
            design.effects().len(),
            total_fe_coefficients,
            lsmr_config.preconditioner,
        );
        let lsmr_fits = solver_memory_budget
            .zip(solver_memory)
            .is_none_or(|(budget, estimate)| {
                budget.fits(estimate.lsmr_bytes(resolved_preconditioner))
            });
        let route = auto_lsmr_route_at_cap(contraction, total_fe_coefficients, direct_lsmr_cap);
        if route != AutoLsmrRoute::None && !lsmr_fits {
            candidate_failure = Some(
                "automatic LSMR route did not fit the joint execution/solver memory plan; selected CG"
                    .to_owned(),
            );
        } else {
            match route {
                AutoLsmrRoute::Direct => {
                    match run_borrowed_lsmr(
                        design,
                        weights,
                        &values,
                        direct_lsmr_tolerance(config.tolerance),
                        remaining_iterations,
                        lsmr_config,
                        retain_coefficients,
                    ) {
                        Ok((lsmr_values, mut report, owned_setup_seconds, lsmr_coefficients))
                            if report.certified() =>
                        {
                            report.requested_solver = FeSolverKind::Auto;
                            report.iterations = report.iterations.saturating_add(pilot_budget);
                            report.pilot_iterations = pilot_budget;
                            report.pilot_contraction = contraction;
                            report.setup_seconds += pilot_setup_seconds + owned_setup_seconds;
                            report.solve_seconds += pilot_seconds;
                            if let (Some(pilot), Some(lsmr)) =
                                (retained_coefficients.as_mut(), lsmr_coefficients.as_ref())
                            {
                                pilot.add_assign(lsmr, 1.0)?;
                            }
                            return if let Some(coefficients) = retained_coefficients {
                                CertifiedBorrowedMap::from_certified_parts_with_coefficients(
                                    lsmr_values,
                                    report,
                                    coefficients,
                                )
                            } else {
                                CertifiedBorrowedMap::from_certified_parts(lsmr_values, report)
                            };
                        }
                        Ok((_, report, _, _)) => {
                            candidate_failure = Some(format!(
                            "automatic LSMR path did not certify after {} iterations; retried CG",
                            report.iterations
                        ));
                        }
                        Err(error) => {
                            candidate_failure =
                                Some(format!("automatic LSMR path failed ({error}); retried CG"));
                        }
                    }
                }
                AutoLsmrRoute::CgPolish => {
                    let bulk_tolerance = lsmr_cg_bulk_tolerance(config.tolerance);
                    match run_borrowed_lsmr(
                        design,
                        weights,
                        &values,
                        bulk_tolerance,
                        remaining_iterations,
                        lsmr_config,
                        false,
                    ) {
                        Ok((lsmr_values, lsmr_report, owned_setup_seconds, _)) => {
                            let lsmr_iterations = lsmr_report.iterations;
                            let lsmr_certified = lsmr_report.certified();
                            let lsmr_setup_seconds = lsmr_report.setup_seconds;
                            let lsmr_solve_seconds = lsmr_report.solve_seconds;
                            let requested_preconditioner = lsmr_report.requested_preconditioner;
                            let used_preconditioner = lsmr_report.used_preconditioner;
                            let lsmr_fallback = lsmr_report.fallback_used;
                            let mut lsmr_warnings = lsmr_report.warnings;
                            let polish_config = MapConfig {
                                max_iterations: remaining_iterations,
                                ..config
                            };
                            match absorb_cg_with_reservation(
                                design,
                                borrowed_weights(weights),
                                lsmr_values,
                                polish_config,
                                false,
                                CgContinuation {
                                    reference_norms: Some(&original_reference_norms),
                                    refinement: CgRefinementPolicy::ORDINARY,
                                    retain_coefficients: false,
                                    reconstruction_tolerance: None,
                                },
                                reservation,
                            ) {
                                Ok(polished) => {
                                    let (residualized, mut report) = polished.into_parts();
                                    let polish_iterations = report.iterations;
                                    report.requested_solver = FeSolverKind::Auto;
                                    report.requested_preconditioner = requested_preconditioner;
                                    report.used_preconditioner = used_preconditioner;
                                    report.pilot_iterations = pilot_budget;
                                    report.pilot_contraction = contraction;
                                    report.setup_seconds += pilot_setup_seconds
                                        + owned_setup_seconds
                                        + lsmr_setup_seconds;
                                    report.solve_seconds += pilot_seconds + lsmr_solve_seconds;
                                    report.fallback_used |= lsmr_fallback;
                                    report.warnings.insert(
                                    0,
                                    format!(
                                        "development-only LSMR-CG prototype: LSMR bulk tolerance={bulk_tolerance:e}, LSMR iterations={lsmr_iterations}, LSMR bulk-certified={lsmr_certified}, CG polish iterations={polish_iterations}"
                                    ),
                                );
                                    report.warnings.append(&mut lsmr_warnings);
                                    return CertifiedBorrowedMap::from_certified_parts(
                                        residualized,
                                        report,
                                    );
                                }
                                Err(error) => {
                                    candidate_failure = Some(format!(
                                    "development-only LSMR-CG polish failed after LSMR used {lsmr_iterations} iterations ({error}); retried CG from the MAP pilot"
                                ));
                                }
                            }
                        }
                        Err(error) => {
                            candidate_failure = Some(format!(
                            "development-only LSMR-CG bulk phase failed ({error}); retried CG from the MAP pilot"
                        ));
                        }
                    }
                }
                AutoLsmrRoute::None => {}
            }
        }
    }
    if !cg_fits {
        return Err(allocation(
            "automatic FE solver memory preflight found neither LSMR nor CG able to fit the joint execution plan",
        ));
    }
    let cg_config = MapConfig {
        max_iterations: remaining_iterations,
        ..config
    };
    let cg = absorb_cg_with_reservation(
        design,
        match weights.values {
            Some(values) => BorrowedMapWeights::Provided(values),
            None => BorrowedMapWeights::Unit,
        },
        values,
        cg_config,
        false,
        CgContinuation {
            reference_norms: Some(&original_reference_norms),
            refinement: CgRefinementPolicy::ORDINARY,
            retain_coefficients,
            reconstruction_tolerance: None,
        },
        reservation,
    )?;
    let cg = if retain_coefficients {
        let (residualized, report, cg_coefficients) = cg.into_full_parts();
        match (retained_coefficients, cg_coefficients) {
            (Some(mut pilot), Some(cg_coefficients)) => {
                pilot.add_assign(&cg_coefficients, 1.0)?;
                CertifiedBorrowedMap::from_certified_parts_with_coefficients(
                    residualized,
                    report,
                    pilot,
                )?
            }
            _ => CertifiedBorrowedMap::from_certified_parts(residualized, report)?,
        }
    } else {
        cg
    };
    let marked = mark_auto(
        cg,
        pilot_budget,
        contraction,
        pilot_setup_seconds,
        pilot_seconds,
    )?;
    if let Some(warning) = candidate_failure {
        let (residualized, mut report, coefficients) = marked.into_full_parts();
        report.fallback_used = true;
        report.warnings.insert(0, warning);
        return if let Some(coefficients) = coefficients {
            CertifiedBorrowedMap::from_certified_parts_with_coefficients(
                residualized,
                report,
                coefficients,
            )
        } else {
            CertifiedBorrowedMap::from_certified_parts(residualized, report)
        };
    }
    Ok(marked)
}

fn borrowed_weights(weights: ValidatedWeights<'_>) -> BorrowedMapWeights<'_> {
    match weights.values {
        Some(values) => BorrowedMapWeights::Provided(values),
        None => BorrowedMapWeights::Unit,
    }
}

fn lsmr_cg_bulk_tolerance(final_tolerance: f64) -> f64 {
    // The scale-free square-root target makes LSMR a reducer rather than a
    // second final solver. CG still receives the caller's exact tolerance and
    // the original RHS norms, so this cannot weaken publication criteria.
    final_tolerance.max(final_tolerance.sqrt())
}

fn run_borrowed_lsmr(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    tolerance: f64,
    max_iterations: usize,
    lsmr_config: LsmrConfig,
    retain_coefficients: bool,
) -> Result<(DenseMatrix, SolveReport, f64, Option<FeCoefficientBatch>)> {
    let effective_max_iterations = lsmr_config.max_iterations.min(max_iterations);
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(invalid_argument(
            "borrowed LSMR tolerance must be finite and positive",
        ));
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
    let mut lsmr_values =
        fallible_matrix_copy(values, "unable to reserve automatic LSMR fallback matrix")?;
    let effects = design
        .effects()
        .iter()
        .map(|effect| {
            Effect::new(effect.codes(), true, std::iter::empty::<&[f64]>()).map_err(|error| {
                invalid_data(format!(
                    "unable to construct borrowed LSMR fixed-effect term: {error}"
                ))
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let rhs = (0..lsmr_values.ncols())
        .map(|column| lsmr_values.column(column))
        .collect::<Result<Vec<_>>>()?;
    let rhs_views = rhs.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let options = LsmrOptions {
        tol: tolerance,
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
    let initial = within::solve_batch(
        effects.clone(),
        &rhs_views,
        weights.values,
        Some(&options),
        preconditioner,
    );
    let (solved, preconditioner_fallback) = match initial {
        Ok(solved) => (solved, false),
        Err(error)
            if lsmr_config.preconditioner == LsmrPreconditioner::Auto
                && borrowed_lsmr_preconditioner_breakdown(&error) =>
        {
            let solved = within::solve_batch(
                effects,
                &rhs_views,
                weights.values,
                Some(&options),
                PreconditionerConfig::Off,
            )
            .map_err(|fallback_error| {
                FeregError::new(
                    ErrorCode::Internal,
                    format!(
                        "borrowed modified LSMR failed after unpreconditioned fallback: {fallback_error}"
                    ),
                )
            })?;
            (solved, true)
        }
        Err(error) => {
            return Err(FeregError::new(
                ErrorCode::Internal,
                format!("borrowed modified LSMR failed: {error}"),
            ));
        }
    };
    for column in 0..lsmr_values.ncols() {
        for (row, value) in solved.demeaned(column).iter().copied().enumerate() {
            lsmr_values[(row, column)] = value;
        }
    }
    let mut reservation = SystemWorkspaceReservation;
    let mut certificate_workspace = BorrowedMapWorkspace::new(
        design,
        lsmr_values.nrows(),
        lsmr_values.ncols(),
        &mut reservation,
    )?;
    weighted_column_norms(
        values,
        weights,
        &mut certificate_workspace.reference_norms,
        &mut certificate_workspace.value_scales,
        &mut certificate_workspace.group_sums,
    )?;
    let certificate = certify(
        design,
        weights,
        &lsmr_values,
        tolerance,
        &mut certificate_workspace,
    )?;
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
    let report = SolveReport {
        requested_solver: FeSolverKind::Lsmr,
        used_solver: FeSolverKind::Lsmr,
        iterations: solved.iterations.iter().copied().max().unwrap_or(0),
        stop_reason,
        certificate,
        normal_equation_residual: solved.residual.iter().copied().fold(0.0_f64, f64::max),
        setup_seconds: solved.time_setup,
        solve_seconds: solved.time_solve.iter().copied().sum(),
        fallback_used: preconditioner_fallback,
        requested_preconditioner: Some(lsmr_config.preconditioner),
        used_preconditioner: Some(if preconditioner_fallback {
            LsmrPreconditioner::Off
        } else {
            resolved_preconditioner
        }),
        pilot_iterations: 0,
        pilot_contraction: 0.0,
        warnings,
    };
    let coefficients = if retain_coefficients && report.certified() {
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

fn borrowed_lsmr_preconditioner_breakdown(error: &within::WithinError) -> bool {
    matches!(
        error,
        within::WithinError::Solve(within::SolveError::InvalidInput { context, message })
            if *context == "mlsmr" && message.contains("preconditioner not positive definite")
    )
}

fn pilot_contraction(initial: f64, current: f64) -> Result<f64> {
    if !initial.is_finite() || initial < 0.0 || !current.is_finite() || current < 0.0 {
        return Err(invalid_data("borrowed auto pilot moment is invalid"));
    }
    let contraction = if initial == 0.0 {
        if current == 0.0 {
            0.0
        } else {
            return Err(invalid_data(
                "borrowed auto pilot acquired a moment from a zero initial norm",
            ));
        }
    } else {
        current / initial
    };
    if !contraction.is_finite() || contraction < 0.0 {
        return Err(invalid_data("borrowed auto pilot contraction is invalid"));
    }
    Ok(contraction)
}

fn mark_auto(
    result: CertifiedBorrowedMap,
    pilot_iterations: usize,
    contraction: f64,
    additional_setup_seconds: f64,
    additional_solve_seconds: f64,
) -> Result<CertifiedBorrowedMap> {
    let (residualized, mut report, coefficients) = result.into_full_parts();
    report.requested_solver = FeSolverKind::Auto;
    report.pilot_iterations = pilot_iterations;
    report.pilot_contraction = contraction;
    report.setup_seconds += additional_setup_seconds;
    report.solve_seconds += additional_solve_seconds;
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

fn attach_optional_coefficients(
    result: CertifiedBorrowedMap,
    coefficients: Option<FeCoefficientBatch>,
) -> Result<CertifiedBorrowedMap> {
    if let Some(coefficients) = coefficients {
        let (residualized, report) = result.into_parts();
        CertifiedBorrowedMap::from_certified_parts_with_coefficients(
            residualized,
            report,
            coefficients,
        )
    } else {
        Ok(result)
    }
}

fn attach_optional_map_coefficients(
    result: CertifiedBorrowedMap,
    coefficients: Option<MapCoefficientAccumulator>,
) -> Result<CertifiedBorrowedMap> {
    attach_optional_coefficients(
        result,
        coefficients
            .map(MapCoefficientAccumulator::into_batch)
            .transpose()?,
    )
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
    weighted_column_norms(
        &values,
        weights,
        &mut workspace.reference_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
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
    let setup_seconds = setup_started.elapsed().as_secs_f64();
    let solve_started = Instant::now();

    if design.effects().is_empty() && !empty_design_is_structural_constant {
        return certified_solver_result(
            values,
            FeSolverKind::Cg,
            SolveStopReason::NoFixedEffects,
            0,
            zero_certificate(),
            0.0,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        );
    }

    let mut solution_coefficients = retain_coefficients
        .then(|| FeCoefficientBatch::intercept_zeros(design, values.ncols()))
        .transpose()?;

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
        let result = certified_solver_result(
            values,
            FeSolverKind::Cg,
            SolveStopReason::Certified,
            0,
            initial_certificate,
            0.0,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        )?;
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
        return certified_solver_result(
            values,
            FeSolverKind::Cg,
            SolveStopReason::Certified,
            1,
            certificate,
            0.0,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        );
    }

    let mut residual = fallible_matrix_zeros(
        values.nrows(),
        values.ncols(),
        "unable to reserve borrowed CG residual matrix",
    )?;
    let mut residual_coefficients = retain_coefficients
        .then(|| FeCoefficientBatch::intercept_zeros(design, values.ncols()))
        .transpose()?;
    borrowed_symmetric_projection_into_recording(
        design,
        weights,
        &values,
        &mut residual,
        &mut workspace,
        residual_coefficients.as_mut(),
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
        if let Some(coefficients) = projection_coefficients.as_mut() {
            coefficients.fill_zero();
        }
        borrowed_symmetric_projection_into_recording(
            design,
            weights,
            &direction,
            &mut projection,
            &mut workspace,
            projection_coefficients.as_mut(),
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
                let result = certified_solver_result(
                    values,
                    FeSolverKind::Cg,
                    SolveStopReason::Certified,
                    iteration - 1,
                    certificate,
                    recurrence_residual,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                )?;
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
        hestenes_error =
            cg_hestenes_error_and_update(&alpha, &residual_square, &mut improvement_potential)?;
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
                let result = certified_solver_result(
                    values,
                    FeSolverKind::Cg,
                    SolveStopReason::Certified,
                    iteration,
                    certificate,
                    recurrence_residual,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                )?;
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

fn absorb_with_reservation<R: WorkspaceReservation>(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    empty_design_is_structural_constant: bool,
    reservation: &mut R,
) -> Result<CertifiedBorrowedMap> {
    absorb_with_reservation_mode(
        design,
        weights,
        values,
        config,
        empty_design_is_structural_constant,
        false,
        false,
        reservation,
    )
}

fn absorb_initial_with_reservation<R: WorkspaceReservation>(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    empty_design_is_structural_constant: bool,
    reservation: &mut R,
) -> Result<CertifiedBorrowedMap> {
    absorb_with_reservation_mode(
        design,
        weights,
        values,
        config,
        empty_design_is_structural_constant,
        true,
        false,
        reservation,
    )
}

#[allow(clippy::too_many_arguments)]
fn absorb_with_reservation_mode<R: WorkspaceReservation>(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    mut values: DenseMatrix,
    config: MapConfig,
    empty_design_is_structural_constant: bool,
    skip_initial_one_effect_certificate: bool,
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
    if values.as_slice().iter().any(|value| !value.is_finite()) {
        return Err(invalid_data("borrowed MAP values must be finite"));
    }
    let weights = validate_weights(weights, values.nrows())?;

    let setup_started = Instant::now();
    let mut workspace =
        BorrowedMapWorkspace::new(design, values.nrows(), values.ncols(), reservation)?;
    weighted_column_norms(
        &values,
        weights,
        &mut workspace.reference_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
    let setup_seconds = setup_started.elapsed().as_secs_f64();
    let solve_started = Instant::now();
    let mut coefficients = retain_coefficients
        .then(|| MapCoefficientAccumulator::zeros(design, values.ncols()))
        .transpose()?;

    if design.effects().is_empty() && !empty_design_is_structural_constant {
        let result = certified_result(
            values,
            SolveStopReason::NoFixedEffects,
            0,
            zero_certificate(),
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        )?;
        return attach_optional_map_coefficients(result, coefficients);
    }

    if !skip_initial_one_effect_certificate {
        let initial_certificate = if design.effects().is_empty() {
            certify_initial_structural_constant(weights, &values, config.tolerance, &mut workspace)?
        } else {
            certify_initial(design, weights, &values, config.tolerance, &mut workspace)?
        };
        if initial_certificate.passed {
            let result = certified_result(
                values,
                SolveStopReason::Certified,
                0,
                initial_certificate,
                setup_seconds,
                solve_started.elapsed().as_secs_f64(),
            )?;
            return attach_optional_map_coefficients(result, coefficients);
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
        let result = certified_result(
            values,
            SolveStopReason::Certified,
            1,
            certificate,
            setup_seconds,
            solve_started.elapsed().as_secs_f64(),
        )?;
        return attach_optional_map_coefficients(result, coefficients);
    }

    for iteration in 1..=config.max_iterations {
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
                let result = certified_result(
                    values,
                    SolveStopReason::Certified,
                    iteration,
                    certificate,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                )?;
                return attach_optional_map_coefficients(result, coefficients);
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
    weighted_column_norms(
        residuals,
        weights,
        &mut workspace.residual_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
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
        relative_square_sum = relative.mul_add(relative, relative_square_sum);
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
    if effect.codes().len() != values.nrows() {
        return Err(invalid_argument(
            "borrowed MAP effect and matrix observation counts differ",
        ));
    }
    let parallel_mean_count = effect
        .n_levels()
        .checked_mul(values.ncols())
        .ok_or_else(|| allocation("borrowed MAP parallel group-mean dimensions overflow"))?;
    if rayon::current_num_threads() > 1
        && workspace.parallel_group_means.len() >= parallel_mean_count
        && (values.ncols() > 1 || values.nrows() >= PARALLEL_PROJECTION_MIN_ROWS)
    {
        return project_effect_parallel(
            effect,
            weights,
            values,
            workspace,
            parallel_mean_count,
            coefficients,
        );
    }
    project_effect_serial(effect, weights, values, workspace, coefficients)
}

fn project_effect_serial<R: InterceptCoefficientRecorder>(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    mut coefficients: Option<(&mut R, usize)>,
) -> Result<()> {
    for level in 0..effect.n_levels() {
        let members = effect.level_index().checked_members(level)?;
        if members.is_empty() {
            return Err(invalid_argument(
                "borrowed MAP effect contains an empty final level",
            ));
        }
        group_means(members, weights, values, workspace)?;
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
                let residual = values[(observation, rhs)] - workspace.group_sums[rhs];
                if !residual.is_finite() {
                    return Err(invalid_data(
                        "borrowed MAP projection produced a nonfinite residual",
                    ));
                }
                values[(observation, rhs)] = residual;
            }
        }
    }
    Ok(())
}

fn project_effect_parallel<R: InterceptCoefficientRecorder>(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    parallel_mean_count: usize,
    coefficients: Option<(&mut R, usize)>,
) -> Result<()> {
    let levels = effect.n_levels();
    let columns = values.ncols();
    if columns <= FUSED_PARALLEL_MAX_RHS {
        return project_effect_parallel_fused(
            effect,
            weights,
            values,
            workspace,
            parallel_mean_count,
            coefficients,
        );
    }
    workspace.parallel_group_weights[..levels]
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(level, group_weight)| -> Result<()> {
            let members = effect.level_index().checked_members(level)?;
            *group_weight = group_weight_sum(members, weights, values.nrows())?;
            Ok(())
        })?;
    let group_weights = &workspace.parallel_group_weights[..levels];
    let values_read = &*values;
    workspace.parallel_group_means[..parallel_mean_count]
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(slot, mean)| -> Result<()> {
            let level = slot / columns;
            let rhs = slot % columns;
            let members = effect.level_index().checked_members(level)?;
            if members.is_empty() {
                return Err(invalid_argument(
                    "borrowed MAP effect contains an empty final level",
                ));
            }
            *mean = group_mean_for_column_with_weight(
                members,
                rhs,
                weights,
                group_weights[level],
                values_read,
            )?;
            Ok(())
        })?;

    let means = &workspace.parallel_group_means[..parallel_mean_count];
    if let Some((batch, effect_index)) = coefficients {
        record_parallel_group_means(batch, effect_index, means, levels, columns)?;
    }
    let codes = effect.codes();
    values
        .as_mut_slice()
        .par_chunks_mut(columns)
        .enumerate()
        .try_for_each(|(observation, row)| -> Result<()> {
            let level = codes[observation] as usize;
            if level >= levels {
                return Err(invalid_argument(
                    "borrowed MAP code exceeds the finalized level count",
                ));
            }
            for (rhs, value) in row.iter_mut().enumerate() {
                let residual = *value - means[level * columns + rhs];
                if !residual.is_finite() {
                    return Err(invalid_data(
                        "borrowed MAP projection produced a nonfinite residual",
                    ));
                }
                *value = residual;
            }
            Ok(())
        })?;
    Ok(())
}

fn project_effect_parallel_fused<R: InterceptCoefficientRecorder>(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
    parallel_mean_count: usize,
    coefficients: Option<(&mut R, usize)>,
) -> Result<()> {
    let levels = effect.n_levels();
    let columns = values.ncols();
    let values_read = &*values;
    let mut coefficients = coefficients;
    let mut recorded_inline = false;
    if let Some((batch, effect_index)) = coefficients.as_mut() {
        if batch.rhs_count() != columns {
            return Err(invalid_argument(
                "parallel MAP coefficient means are not aligned",
            ));
        }
        if let Some(accumulated) = batch.level_major_values_mut(*effect_index, levels)? {
            workspace.parallel_group_means[..parallel_mean_count]
                .par_chunks_mut(columns)
                .zip(accumulated.par_chunks_mut(columns))
                .enumerate()
                .try_for_each(|(level, (means, accumulated))| -> Result<()> {
                    fused_group_means_for_level(effect, level, weights, values_read, means)?;
                    for (destination, &mean) in accumulated.iter_mut().zip(means.iter()) {
                        *destination += mean;
                        if !destination.is_finite() {
                            return Err(invalid_data(
                                "MAP coefficient group accumulation is nonfinite",
                            ));
                        }
                    }
                    Ok(())
                })?;
            recorded_inline = true;
        }
    }
    if !recorded_inline {
        workspace.parallel_group_means[..parallel_mean_count]
            .par_chunks_mut(columns)
            .enumerate()
            .try_for_each(|(level, means)| {
                fused_group_means_for_level(effect, level, weights, values_read, means)
            })?;
    }

    let means = &workspace.parallel_group_means[..parallel_mean_count];
    if !recorded_inline {
        if let Some((batch, effect_index)) = coefficients {
            record_parallel_group_means(batch, effect_index, means, levels, columns)?;
        }
    }
    let codes = effect.codes();
    values
        .as_mut_slice()
        .par_chunks_mut(columns)
        .enumerate()
        .try_for_each(|(observation, row)| -> Result<()> {
            let level = codes[observation] as usize;
            if level >= levels {
                return Err(invalid_argument(
                    "borrowed MAP code exceeds the finalized level count",
                ));
            }
            for (rhs, value) in row.iter_mut().enumerate() {
                let residual = *value - means[level * columns + rhs];
                if !residual.is_finite() {
                    return Err(invalid_data(
                        "borrowed MAP projection produced a nonfinite residual",
                    ));
                }
                *value = residual;
            }
            Ok(())
        })?;
    Ok(())
}

fn fused_group_means_for_level(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    level: usize,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    means: &mut [f64],
) -> Result<()> {
    let members = effect.level_index().checked_members(level)?;
    if members.is_empty() || means.len() > FUSED_PARALLEL_MAX_RHS {
        return Err(invalid_argument(
            "borrowed MAP fused group dimensions are invalid",
        ));
    }
    let mut scales = [0.0_f64; FUSED_PARALLEL_MAX_RHS];
    let mut sums = [0.0_f64; FUSED_PARALLEL_MAX_RHS];
    let mut corrections = [0.0_f64; FUSED_PARALLEL_MAX_RHS];
    let mut weight_sum = 0.0_f64;
    let mut weight_correction = 0.0_f64;
    for &observation in members {
        if observation >= values.nrows() {
            return Err(invalid_argument(
                "borrowed MAP membership exceeds the observation axis",
            ));
        }
        if weights.values.is_some() {
            compensated_add(
                weights.normalized(observation),
                &mut weight_sum,
                &mut weight_correction,
            )?;
        }
        for rhs in 0..means.len() {
            scales[rhs] = scales[rhs].max(values[(observation, rhs)].abs());
        }
    }
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
                    compensated_add(
                        weight * (values[(observation, rhs)] / scales[rhs]),
                        &mut sums[rhs],
                        &mut corrections[rhs],
                    )?;
                }
            }
        }
    } else {
        for &observation in members {
            for rhs in 0..means.len() {
                if scales[rhs] != 0.0 {
                    compensated_add(
                        values[(observation, rhs)] / scales[rhs],
                        &mut sums[rhs],
                        &mut corrections[rhs],
                    )?;
                }
            }
        }
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
    Ok(())
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

fn group_weight_sum(members: &[usize], weights: ValidatedWeights<'_>, nrows: usize) -> Result<f64> {
    let mut weight_sum = 0.0_f64;
    let mut weight_correction = 0.0_f64;
    for &observation in members {
        if observation >= nrows {
            return Err(invalid_argument(
                "borrowed MAP membership exceeds the observation axis",
            ));
        }
        compensated_add(
            weights.normalized(observation),
            &mut weight_sum,
            &mut weight_correction,
        )?;
    }
    let total_weight = weight_sum + weight_correction;
    if total_weight.is_finite() && total_weight > 0.0 {
        Ok(total_weight)
    } else {
        Err(invalid_data(
            "borrowed MAP group weight sum is not finite and positive",
        ))
    }
}

fn group_mean_for_column_with_weight(
    members: &[usize],
    rhs: usize,
    weights: ValidatedWeights<'_>,
    total_weight: f64,
    values: &DenseMatrix,
) -> Result<f64> {
    let mut scale = 0.0_f64;
    for &observation in members {
        if observation >= values.nrows() {
            return Err(invalid_argument(
                "borrowed MAP membership exceeds the observation axis",
            ));
        }
        scale = scale.max(values[(observation, rhs)].abs());
    }

    let mut normalized_sum = 0.0_f64;
    let mut correction = 0.0_f64;
    if scale != 0.0 {
        for &observation in members {
            compensated_add(
                weights.normalized(observation) * (values[(observation, rhs)] / scale),
                &mut normalized_sum,
                &mut correction,
            )?;
        }
    }
    let mean = if scale == 0.0 {
        0.0
    } else {
        scale * ((normalized_sum + correction) / total_weight)
    };
    if !mean.is_finite() {
        return Err(invalid_data("borrowed MAP group mean is not finite"));
    }
    Ok(mean)
}

fn group_means(
    members: &[usize],
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<f64> {
    workspace.value_scales.fill(0.0);
    workspace.group_sums.fill(0.0);
    workspace.group_corrections.fill(0.0);

    let mut weight_sum = 0.0_f64;
    let mut weight_correction = 0.0_f64;
    for &observation in members {
        if observation >= values.nrows() {
            return Err(invalid_argument(
                "borrowed MAP membership exceeds the observation axis",
            ));
        }
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
            "borrowed MAP group weight sum is not finite and positive",
        ));
    }

    for &observation in members {
        let weight = weights.normalized(observation);
        for rhs in 0..values.ncols() {
            let scale = workspace.value_scales[rhs];
            if scale == 0.0 {
                continue;
            }
            let term = weight * (values[(observation, rhs)] / scale);
            compensated_add(
                term,
                &mut workspace.group_sums[rhs],
                &mut workspace.group_corrections[rhs],
            )?;
        }
    }

    for rhs in 0..values.ncols() {
        let scale = workspace.value_scales[rhs];
        let normalized_sum = workspace.group_sums[rhs] + workspace.group_corrections[rhs];
        let mean = if scale == 0.0 {
            0.0
        } else {
            scale * (normalized_sum / total_weight)
        };
        if !mean.is_finite() {
            return Err(invalid_data("borrowed MAP group mean is not finite"));
        }
        workspace.group_sums[rhs] = mean;
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
    weighted_column_norms(
        residuals,
        weights,
        &mut workspace.residual_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
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

    if design.effects().len() == 1
        && residuals.nrows() >= PARALLEL_PROJECTION_MIN_ROWS
        && rayon::current_num_threads() > 1
        && workspace.parallel_group_weights.len() >= design.effects()[0].n_levels()
        && workspace.parallel_group_means.len()
            >= design.effects()[0]
                .n_levels()
                .checked_mul(residuals.ncols())
                .ok_or_else(|| allocation("borrowed MAP certificate dimensions overflow"))?
    {
        return certify_one_effect_parallel(
            &design.effects()[0],
            weights,
            residuals,
            tolerance,
            workspace,
        );
    }

    let mut absolute_infinity_norm = 0.0_f64;
    let mut relative_infinity_norm = 0.0_f64;
    let mut relative_square_sum = 0.0_f64;
    let mut relative_count = 0_usize;
    for effect in design.effects() {
        for level in 0..effect.n_levels() {
            let members = effect.level_index().checked_members(level)?;
            if members.is_empty() {
                return Err(invalid_argument(
                    "borrowed MAP certificate contains an empty final level",
                ));
            }
            let group_weight = group_means(members, weights, residuals, workspace)?;
            let root_group_weight = group_weight.sqrt();
            if !root_group_weight.is_finite() || root_group_weight <= 0.0 {
                return Err(invalid_data(
                    "borrowed MAP certificate group weight is invalid",
                ));
            }
            for rhs in 0..residuals.ncols() {
                let mean = workspace.group_sums[rhs];
                let denominator = workspace.rhs_scales[rhs];
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
                relative_infinity_norm = relative_infinity_norm.max(relative);
                workspace.rhs_relative_infinity_norms[rhs] =
                    workspace.rhs_relative_infinity_norms[rhs].max(relative);
                relative_square_sum = relative.mul_add(relative, relative_square_sum);
                if !relative_square_sum.is_finite() {
                    return Err(invalid_data(
                        "borrowed MAP certificate two-norm accumulation is not finite",
                    ));
                }
                relative_count = relative_count
                    .checked_add(1)
                    .ok_or_else(|| allocation("borrowed MAP certificate moment count overflows"))?;

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
        }
    }

    let relative_two_norm = if relative_count == 0 {
        0.0
    } else {
        (relative_square_sum / relative_count as f64).sqrt()
    };
    if !relative_two_norm.is_finite() {
        return Err(invalid_data(
            "borrowed MAP certificate two norm is not finite",
        ));
    }
    Ok(FeCertificate {
        absolute_infinity_norm,
        relative_infinity_norm,
        relative_two_norm,
        passed: relative_infinity_norm <= tolerance,
    })
}

fn certify_one_effect_parallel(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    let levels = effect.n_levels();
    let columns = residuals.ncols();
    workspace.parallel_group_weights[..levels]
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(level, group_weight)| -> Result<()> {
            let members = effect.level_index().checked_members(level)?;
            if members.is_empty() {
                return Err(invalid_argument(
                    "borrowed MAP certificate contains an empty final level",
                ));
            }
            *group_weight = group_weight_sum(members, weights, residuals.nrows())?;
            Ok(())
        })?;
    let group_weights = &workspace.parallel_group_weights[..levels];
    let means = &mut workspace.parallel_group_means[..levels * columns];
    means
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(slot, mean)| -> Result<()> {
            let level = slot / columns;
            let rhs = slot % columns;
            let members = effect.level_index().checked_members(level)?;
            *mean = group_mean_for_column_with_weight(
                members,
                rhs,
                weights,
                group_weights[level],
                residuals,
            )?;
            Ok(())
        })?;

    let mut absolute_infinity_norm = 0.0_f64;
    let mut relative_infinity_norm = 0.0_f64;
    let mut relative_square_sum = 0.0_f64;
    let mut relative_count = 0_usize;
    for level in 0..levels {
        let group_weight = group_weights[level];
        let root_group_weight = group_weight.sqrt();
        if !root_group_weight.is_finite() || root_group_weight <= 0.0 {
            return Err(invalid_data(
                "borrowed MAP certificate group weight is invalid",
            ));
        }
        for rhs in 0..columns {
            let mean = means[level * columns + rhs];
            let denominator = workspace.rhs_scales[rhs];
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
            relative_infinity_norm = relative_infinity_norm.max(relative);
            workspace.rhs_relative_infinity_norms[rhs] =
                workspace.rhs_relative_infinity_norms[rhs].max(relative);
            relative_square_sum = relative.mul_add(relative, relative_square_sum);
            if !relative_square_sum.is_finite() {
                return Err(invalid_data(
                    "borrowed MAP certificate two-norm accumulation is not finite",
                ));
            }
            relative_count = relative_count
                .checked_add(1)
                .ok_or_else(|| allocation("borrowed MAP certificate moment count overflows"))?;
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
    }
    let relative_two_norm = if relative_count == 0 {
        0.0
    } else {
        (relative_square_sum / relative_count as f64).sqrt()
    };
    if !relative_two_norm.is_finite() {
        return Err(invalid_data(
            "borrowed MAP certificate two norm is not finite",
        ));
    }
    Ok(FeCertificate {
        absolute_infinity_norm,
        relative_infinity_norm,
        relative_two_norm,
        passed: relative_infinity_norm <= tolerance,
    })
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
    residual.as_mut_slice().copy_from_slice(input.as_slice());
    for (effect_index, effect) in design.effects().iter().enumerate() {
        project_effect_recording(
            effect,
            weights,
            residual,
            workspace,
            coefficients
                .as_deref_mut()
                .map(|coefficients| (coefficients, effect_index)),
        )?;
    }
    if design.effects().len() > 1 {
        for effect_index in (0..design.effects().len() - 1).rev() {
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
    }
    for (output, original) in residual.as_mut_slice().iter_mut().zip(input.as_slice()) {
        *output = *original - *output;
        if !output.is_finite() {
            return Err(invalid_data(
                "borrowed CG symmetric projection produced a nonfinite value",
            ));
        }
    }
    Ok(())
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
) -> Result<f64> {
    if alpha.len() != residual_square.len() || alpha.len() != improvement_potential.len() {
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
        let remaining = previous - decrease;
        let roundoff_floor = f64::EPSILON * previous.max(1.0);
        if remaining < -roundoff_floor {
            return Err(invalid_data(
                "borrowed CG Hestenes improvement potential became negative",
            ));
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

fn cg_largest_relative_residual(current: &[f64], initial: &[f64]) -> Result<f64> {
    if current.len() != initial.len() {
        return Err(invalid_argument(
            "borrowed CG residual-norm dimensions differ",
        ));
    }
    let mut largest = 0.0_f64;
    for (current, initial) in current.iter().zip(initial) {
        if !current.is_finite() || *current < 0.0 || !initial.is_finite() || *initial < 0.0 {
            return Err(invalid_data("borrowed CG residual norm is invalid"));
        }
        let relative = if *initial == 0.0 {
            if *current == 0.0 {
                0.0
            } else {
                return Err(invalid_data(
                    "borrowed CG acquired residual from a zero initial norm",
                ));
            }
        } else {
            (current / initial).sqrt()
        };
        if !relative.is_finite() {
            return Err(invalid_data("borrowed CG relative residual is nonfinite"));
        }
        largest = largest.max(relative);
    }
    Ok(largest)
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
) -> Result<()> {
    if output.len() != values.ncols()
        || scales.len() != values.ncols()
        || scaled_square_sums.len() != values.ncols()
    {
        return Err(invalid_argument(
            "borrowed MAP norm workspace dimensions differ",
        ));
    }
    scales.fill(0.0);
    scaled_square_sums.fill(1.0);
    for row in 0..values.nrows() {
        let root_weight = weights.normalized(row).sqrt();
        if !root_weight.is_finite() || root_weight <= 0.0 {
            return Err(invalid_data(
                "borrowed MAP normalized weight is not finite and positive",
            ));
        }
        for rhs in 0..values.ncols() {
            let value = root_weight * values[(row, rhs)];
            if !value.is_finite() {
                return Err(invalid_data("borrowed MAP weighted value is not finite"));
            }
            update_scaled_sum_of_squares(value, &mut scales[rhs], &mut scaled_square_sums[rhs]);
        }
    }
    for rhs in 0..values.ncols() {
        let norm = if scales[rhs] == 0.0 {
            0.0
        } else {
            scales[rhs] * scaled_square_sums[rhs].sqrt()
        };
        if !norm.is_finite() {
            return Err(invalid_data(
                "borrowed MAP weighted column norm is not finite",
            ));
        }
        output[rhs] = norm;
    }
    Ok(())
}

fn update_scaled_sum_of_squares(value: f64, scale: &mut f64, sum: &mut f64) {
    let absolute = value.abs();
    if absolute == 0.0 {
        return;
    }
    if *scale < absolute {
        let ratio = *scale / absolute;
        *sum = 1.0 + *sum * ratio * ratio;
        *scale = absolute;
    } else {
        let ratio = absolute / *scale;
        *sum += ratio * ratio;
    }
}

fn compensated_add(value: f64, sum: &mut f64, correction: &mut f64) -> Result<()> {
    let updated = *sum + value;
    let adjustment = if sum.abs() >= value.abs() {
        (*sum - updated) + value
    } else {
        (value - updated) + *sum
    };
    *correction += adjustment;
    *sum = updated;
    if !sum.is_finite() || !correction.is_finite() {
        return Err(invalid_data(
            "borrowed MAP compensated accumulation is not finite",
        ));
    }
    Ok(())
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
        requested_solver: solver,
        used_solver: solver,
        iterations,
        stop_reason,
        normal_equation_residual,
        certificate,
        setup_seconds,
        solve_seconds,
        fallback_used: false,
        requested_preconditioner: None,
        used_preconditioner: None,
        pilot_iterations: 0,
        pilot_contraction: 0.0,
        warnings: Vec::new(),
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
    output.resize(elements, 0.0);
    Ok(output)
}

fn allocation(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

fn invalid_argument(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};

    #[derive(Default)]
    struct InjectedReservation {
        fail_at: Option<usize>,
        skip_at: Option<usize>,
        calls: usize,
        records: Vec<(usize, &'static str)>,
    }

    impl WorkspaceReservation for InjectedReservation {
        fn reserve_f64(
            &mut self,
            target: &mut Vec<f64>,
            additional: usize,
            label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls = self.calls.checked_add(1).expect("small reservation count");
            self.records.push((additional, label));
            if self.fail_at == Some(call) {
                return Err(allocation("injected borrowed MAP allocation failure"));
            }
            if self.skip_at == Some(call) {
                return Ok(());
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test borrowed MAP reservation failed"))
        }
    }

    fn config() -> MapConfig {
        MapConfig {
            tolerance: 1e-11,
            max_iterations: 10_000,
            check_every: 1,
            symmetric: true,
        }
    }

    #[test]
    fn every_workspace_reservation_and_capacity_guard_fails_closed() {
        let codes = [0_u32, 0, 1, 1];
        let index = LevelIndex::from_codes(&codes, 2).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).expect("source");
        let design = BorrowedInterceptFeDesign::new(4, vec![source.effect()]).expect("design");
        let columns = [vec![1.0, 2.0, 4.0, 8.0], vec![2.0, 3.0, 1.0, -1.0]];

        let mut successful = InjectedReservation::default();
        absorb_with_reservation(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&columns).expect("matrix"),
            config(),
            false,
            &mut successful,
        )
        .expect("successful run");
        assert_eq!(successful.calls, 7);

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = absorb_with_reservation(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&columns).expect("matrix"),
                config(),
                false,
                &mut injected,
            )
            .expect_err("injected allocation must fail");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }

        for skip_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                skip_at: Some(skip_at),
                ..InjectedReservation::default()
            };
            let error = absorb_with_reservation(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&columns).expect("matrix"),
                config(),
                false,
                &mut injected,
            )
            .expect_err("undersized allocation must fail");
            assert_eq!(error.code, ErrorCode::Allocation, "call {skip_at}");
        }
    }

    #[test]
    fn structural_constant_empty_design_allocations_are_fallible_and_only_rhs_scale() {
        let observations = 4096;
        let rhs_count = 3;
        let design =
            BorrowedInterceptFeDesign::new(observations, Vec::new()).expect("empty design");
        let columns = (0..rhs_count)
            .map(|column| {
                (0..observations)
                    .map(|row| (row as f64).sin() + column as f64)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let labels = [
            "unable to reserve borrowed MAP reference norms",
            "unable to reserve borrowed MAP residual norms",
            "unable to reserve borrowed MAP RHS scales",
            "unable to reserve borrowed MAP per-RHS certificate norms",
            "unable to reserve borrowed MAP value scales",
            "unable to reserve borrowed MAP group sums",
            "unable to reserve borrowed MAP group corrections",
        ];

        let mut successful = InjectedReservation::default();
        let result = absorb_with_reservation(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&columns).expect("structural RHS"),
            config(),
            true,
            &mut successful,
        )
        .expect("structural constant projection");
        assert!(result.report().certified());
        assert_eq!(result.report().iterations, 1);
        assert_eq!(successful.calls, labels.len());
        assert_eq!(
            successful.records,
            labels
                .into_iter()
                .map(|label| (rhs_count, label))
                .collect::<Vec<_>>()
        );
        assert!(successful
            .records
            .iter()
            .all(|(additional, _)| *additional < observations));

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = absorb_with_reservation(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&columns).expect("structural RHS"),
                config(),
                true,
                &mut injected,
            )
            .expect_err("structural constant allocation must fail closed");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }
        for skip_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                skip_at: Some(skip_at),
                ..InjectedReservation::default()
            };
            let error = absorb_with_reservation(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&columns).expect("structural RHS"),
                config(),
                true,
                &mut injected,
            )
            .expect_err("undersized structural workspace must fail closed");
            assert_eq!(error.code, ErrorCode::Allocation, "call {skip_at}");
        }
    }

    #[test]
    fn large_parallel_projection_is_bit_stable_and_its_workspace_fails_closed() {
        let observations = PARALLEL_PROJECTION_MIN_ROWS;
        let levels = 4_096;
        let codes = (0..observations)
            .map(|row| (row % levels) as u32)
            .collect::<Vec<_>>();
        let index = LevelIndex::from_codes(&codes, levels).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, levels, &index).expect("source");
        let design =
            BorrowedInterceptFeDesign::new(observations, vec![source.effect()]).expect("design");
        let columns = (0..3)
            .map(|column| {
                (0..observations)
                    .map(|row| {
                        let centered = (row % 97) as f64 - 48.0;
                        centered.mul_add(column as f64 + 0.5, (row % levels) as f64 / 17.0)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let weights = (0..observations)
            .map(|row| 1.0 + (row % 11) as f64 / 13.0)
            .collect::<Vec<_>>();

        let run = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("pool")
                .install(|| {
                    absorb_borrowed_intercept_map_certified(
                        &design,
                        BorrowedMapWeights::Provided(&weights),
                        DenseMatrix::from_columns(&columns).expect("matrix"),
                        config(),
                    )
                    .expect("certified projection")
                })
        };
        let serial = run(1);
        let parallel = run(4);
        assert_eq!(serial.residualized(), parallel.residualized());
        assert_eq!(serial.report().iterations, parallel.report().iterations);
        assert_eq!(serial.report().stop_reason, parallel.report().stop_reason);
        assert_eq!(serial.report().certificate, parallel.report().certificate);

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .expect("pool");
        let mut successful = InjectedReservation::default();
        pool.install(|| {
            absorb_with_reservation(
                &design,
                BorrowedMapWeights::Provided(&weights),
                DenseMatrix::from_columns(&columns).expect("matrix"),
                config(),
                false,
                &mut successful,
            )
            .expect("parallel reservation succeeds");
        });
        assert_eq!(successful.calls, 10);
        assert_eq!(
            successful.records.last(),
            Some(&(
                observations.div_ceil(PARALLEL_DOT_CHUNK_ROWS) * columns.len(),
                "unable to reserve borrowed CG parallel dot sums",
            )),
        );

        for reservation in 7..successful.calls {
            for skip in [false, true] {
                let mut injected = InjectedReservation {
                    fail_at: (!skip).then_some(reservation),
                    skip_at: skip.then_some(reservation),
                    ..InjectedReservation::default()
                };
                let error = pool
                    .install(|| {
                        absorb_with_reservation(
                            &design,
                            BorrowedMapWeights::Provided(&weights),
                            DenseMatrix::from_columns(&columns).expect("matrix"),
                            config(),
                            false,
                            &mut injected,
                        )
                    })
                    .expect_err("parallel workspace allocation must fail closed");
                assert_eq!(error.code, ErrorCode::Allocation);
            }
        }
    }

    #[test]
    fn large_one_rhs_one_fe_projection_is_bit_stable_through_sixteen_threads() {
        let observations = PARALLEL_PROJECTION_MIN_ROWS;
        let levels = 4_096;
        let codes = (0..observations)
            .map(|row| (row % levels) as u32)
            .collect::<Vec<_>>();
        let index = LevelIndex::from_codes(&codes, levels).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, levels, &index).expect("source");
        let design =
            BorrowedInterceptFeDesign::new(observations, vec![source.effect()]).expect("design");
        let column = (0..observations)
            .map(|row| {
                let centered = (row % 97) as f64 - 48.0;
                centered.mul_add(0.5, (row % levels) as f64 / 17.0)
            })
            .collect::<Vec<_>>();
        let weights = (0..observations)
            .map(|row| 1.0 + (row % 11) as f64 / 13.0)
            .collect::<Vec<_>>();

        let run = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("pool")
                .install(|| {
                    absorb_borrowed_intercept_map_certified(
                        &design,
                        BorrowedMapWeights::Provided(&weights),
                        DenseMatrix::from_columns(std::slice::from_ref(&column)).expect("matrix"),
                        config(),
                    )
                    .expect("certified one-RHS projection")
                })
        };
        let serial = run(1);
        for threads in [2, 4, 8, 16] {
            let parallel = run(threads);
            assert_eq!(serial.residualized(), parallel.residualized());
            assert_eq!(
                serial.report().requested_solver,
                parallel.report().requested_solver
            );
            assert_eq!(serial.report().used_solver, parallel.report().used_solver);
            assert_eq!(serial.report().iterations, parallel.report().iterations);
            assert_eq!(serial.report().stop_reason, parallel.report().stop_reason);
            assert_eq!(serial.report().certificate, parallel.report().certificate);
            assert_eq!(
                serial.report().normal_equation_residual,
                parallel.report().normal_equation_residual
            );
            assert_eq!(
                serial.report().fallback_used,
                parallel.report().fallback_used
            );
        }
    }

    #[test]
    fn success_is_always_certified_and_nonconvergence_is_fatal() {
        let first_codes = [0_u32, 0, 0, 1, 1, 1];
        let second_codes = [0_u32, 0, 1, 0, 1, 1];
        let first_index = LevelIndex::from_codes(&first_codes, 2).expect("first index");
        let second_index = LevelIndex::from_codes(&second_codes, 2).expect("second index");
        let first =
            ValidatedBorrowedInterceptSource::new(&first_codes, 2, &first_index).expect("first");
        let second =
            ValidatedBorrowedInterceptSource::new(&second_codes, 2, &second_index).expect("second");
        let design = BorrowedInterceptFeDesign::new(6, vec![first.effect(), second.effect()])
            .expect("design");
        let columns = [vec![1.0, -3.0, 2.0, 5.0, -4.0, 0.5]];

        let success = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Provided(&[1.0, 2.0, 0.7, 3.0, 1.5, 0.5]),
            DenseMatrix::from_columns(&columns).expect("matrix"),
            config(),
        )
        .expect("certified");
        assert!(success.report().certified());

        let error = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Provided(&[1.0, 2.0, 0.7, 3.0, 1.5, 0.5]),
            DenseMatrix::from_columns(&columns).expect("matrix"),
            MapConfig {
                tolerance: 1e-16,
                max_iterations: 1,
                check_every: 1,
                symmetric: false,
            },
        )
        .expect_err("uncertified solve must be fatal");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    #[test]
    fn empty_nonfinite_and_invalid_weight_inputs_fail_before_workspace_allocation() {
        let design = BorrowedInterceptFeDesign::new(0, Vec::new()).expect("empty design");
        let mut reservation = InjectedReservation::default();
        let error = absorb_with_reservation(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::zeros(0, 1).expect("matrix"),
            config(),
            false,
            &mut reservation,
        )
        .expect_err("empty rows");
        assert_eq!(error.code, ErrorCode::InvalidData);
        assert_eq!(reservation.calls, 0);

        let codes = [0_u32, 0];
        let index = LevelIndex::from_codes(&codes, 1).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, 1, &index).expect("source");
        let design = BorrowedInterceptFeDesign::new(2, vec![source.effect()]).expect("design");
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let error = absorb_borrowed_intercept_map_certified(
                &design,
                BorrowedMapWeights::Provided(&[1.0, bad]),
                DenseMatrix::from_columns(&[vec![1.0, 2.0]]).expect("matrix"),
                config(),
            )
            .expect_err("invalid weight");
            assert_eq!(error.code, ErrorCode::InvalidData);
        }
        let error = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Provided(&[f64::MAX, f64::MIN_POSITIVE]),
            DenseMatrix::from_columns(&[vec![1.0, 2.0]]).expect("matrix"),
            config(),
        )
        .expect_err("normalized underflow");
        assert_eq!(error.code, ErrorCode::InvalidData);

        for bad in [f64::NAN, f64::INFINITY] {
            let error = absorb_borrowed_intercept_map_certified(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&[vec![1.0, bad]]).expect("matrix"),
                config(),
            )
            .expect_err("invalid value");
            assert_eq!(error.code, ErrorCode::InvalidData);
        }
    }

    #[test]
    fn no_effects_and_already_certified_inputs_return_without_iteration() {
        let design = BorrowedInterceptFeDesign::new(3, Vec::new()).expect("design");
        let result = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0]]).expect("matrix"),
            config(),
        )
        .expect("no effects");
        assert_eq!(result.report().stop_reason, SolveStopReason::NoFixedEffects);
        assert_eq!(result.report().iterations, 0);

        let codes = [0_u32, 0, 1, 1];
        let index = LevelIndex::from_codes(&codes, 2).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).expect("source");
        let design = BorrowedInterceptFeDesign::new(4, vec![source.effect()]).expect("design");
        let result = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![-1.0, 1.0, -2.0, 2.0]]).expect("matrix"),
            config(),
        )
        .expect("already certified");
        assert_eq!(result.report().iterations, 0);
        assert!(result.report().certified());

        let zero = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![0.0; 4]]).expect("zero matrix"),
            config(),
        )
        .expect("zero RHS");
        assert_eq!(zero.report().iterations, 0);
        assert_eq!(zero.report().certificate.relative_infinity_norm, 0.0);
    }

    #[test]
    fn one_way_projection_is_exact_and_invalid_shapes_and_configurations_fail() {
        let codes = [0_u32, 0, 1, 1];
        let index = LevelIndex::from_codes(&codes, 2).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).expect("source");
        let design = BorrowedInterceptFeDesign::new(4, vec![source.effect()]).expect("design");
        let result = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Provided(&[1.0, 3.0, 2.0, 2.0]),
            DenseMatrix::from_columns(&[vec![1.0, 5.0, 4.0, 8.0]]).expect("matrix"),
            config(),
        )
        .expect("one-way projection");
        assert_eq!(result.report().iterations, 1);
        assert_close(
            result.residualized().as_slice(),
            &[-3.0, 1.0, -2.0, 2.0],
            1e-14,
        );

        let short = BorrowedInterceptFeDesign::new(3, Vec::new()).expect("short design");
        assert_eq!(
            absorb_borrowed_intercept_map_certified(
                &short,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0, 4.0]]).expect("matrix"),
                config(),
            )
            .expect_err("dimension mismatch")
            .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            absorb_borrowed_intercept_map_certified(
                &design,
                BorrowedMapWeights::Provided(&[1.0; 3]),
                DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0, 4.0]]).expect("matrix"),
                config(),
            )
            .expect_err("weight mismatch")
            .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            absorb_borrowed_intercept_map_certified(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::zeros(4, 0).expect("empty RHS matrix"),
                config(),
            )
            .expect_err("empty RHS")
            .code,
            ErrorCode::InvalidData
        );

        for bad in [
            MapConfig {
                tolerance: 0.0,
                ..config()
            },
            MapConfig {
                tolerance: f64::NAN,
                ..config()
            },
            MapConfig {
                max_iterations: 0,
                ..config()
            },
            MapConfig {
                check_every: 0,
                ..config()
            },
        ] {
            assert_eq!(
                absorb_borrowed_intercept_map_certified(
                    &design,
                    BorrowedMapWeights::Unit,
                    DenseMatrix::from_columns(&[vec![1.0, 2.0, 3.0, 4.0]]).expect("matrix"),
                    bad,
                )
                .expect_err("invalid config")
                .code,
                ErrorCode::InvalidArgument
            );
        }
    }

    #[test]
    fn nonrepresentable_weighted_norm_fails_closed() {
        let codes = [0_u32, 0];
        let index = LevelIndex::from_codes(&codes, 1).expect("index");
        let source = ValidatedBorrowedInterceptSource::new(&codes, 1, &index).expect("source");
        let design = BorrowedInterceptFeDesign::new(2, vec![source.effect()]).expect("design");
        let error = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![f64::MAX, f64::MAX]]).expect("matrix"),
            config(),
        )
        .expect_err("overflowing weighted norm");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    #[test]
    fn borrowed_cg_is_weighted_batched_certified_and_deterministic() {
        let worker_codes = [0_u32, 0, 1, 1, 2, 2, 3, 3];
        let firm_codes = [0_u32, 1, 1, 2, 2, 0, 0, 2];
        let worker_index = LevelIndex::from_codes(&worker_codes, 4).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm_codes, 3).expect("firm index");
        let worker =
            ValidatedBorrowedInterceptSource::new(&worker_codes, 4, &worker_index).expect("worker");
        let firm =
            ValidatedBorrowedInterceptSource::new(&firm_codes, 3, &firm_index).expect("firm");
        let design = BorrowedInterceptFeDesign::new(
            worker_codes.len(),
            vec![worker.effect(), firm.effect()],
        )
        .expect("design");
        let weights = [1.0, 2.0, 1.5, 4.0, 0.5, 3.0, 2.5, 1.25];
        let columns = [
            vec![1.0, 7.0, 2.0, 9.0, 4.0, 3.0, 8.0, -2.0],
            vec![2.0, -1.0, 4.0, 5.0, 0.0, 6.0, 3.0, 10.0],
        ];
        let run = || {
            absorb_borrowed_intercept_cg_certified(
                &design,
                BorrowedMapWeights::Provided(&weights),
                DenseMatrix::from_columns(&columns).expect("matrix"),
                config(),
            )
            .expect("CG")
        };
        let first = run();
        let second = run();
        assert!(first.report().certified());
        assert_eq!(first.report().used_solver, FeSolverKind::Cg);
        assert_eq!(first.residualized(), second.residualized());
        assert_eq!(first.report().iterations, second.report().iterations);

        let map = absorb_borrowed_intercept_map_certified(
            &design,
            BorrowedMapWeights::Provided(&weights),
            DenseMatrix::from_columns(&columns).expect("matrix"),
            config(),
        )
        .expect("MAP");
        assert_close(
            first.residualized().as_slice(),
            map.residualized().as_slice(),
            2e-10,
        );

        let input = DenseMatrix::from_columns(&columns).expect("matrix");
        let retained = absorb_borrowed_intercept_cg_retaining_certified(
            &design,
            BorrowedMapWeights::Provided(&weights),
            input.clone(),
            config(),
        )
        .expect("retaining CG");
        let (residualized, report, coefficients) = retained.into_full_parts();
        assert!(report.certified());
        let coefficients = coefficients.expect("CG coefficient product");
        for rhs_index in 0..input.ncols() {
            let rhs = coefficients.rhs(rhs_index).expect("coefficient RHS");
            for row in 0..input.nrows() {
                let fitted = rhs[coefficients.effect_offsets()[0] + worker_codes[row] as usize]
                    + rhs[coefficients.effect_offsets()[1] + firm_codes[row] as usize];
                assert!(
                    (fitted - (input[(row, rhs_index)] - residualized[(row, rhs_index)])).abs()
                        <= 2e-10
                );
            }
        }
    }

    #[test]
    fn retained_map_level_major_accumulator_reconstructs_every_rhs() {
        let worker_codes = [0_u32, 0, 1, 1, 2, 2, 3, 3];
        let firm_codes = [0_u32, 1, 1, 2, 2, 0, 0, 2];
        let worker_index = LevelIndex::from_codes(&worker_codes, 4).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm_codes, 3).expect("firm index");
        let worker =
            ValidatedBorrowedInterceptSource::new(&worker_codes, 4, &worker_index).expect("worker");
        let firm =
            ValidatedBorrowedInterceptSource::new(&firm_codes, 3, &firm_index).expect("firm");
        let design = BorrowedInterceptFeDesign::new(
            worker_codes.len(),
            vec![worker.effect(), firm.effect()],
        )
        .expect("design");
        let weights = [1.0, 2.0, 1.5, 4.0, 0.5, 3.0, 2.5, 1.25];
        let columns = [
            vec![1.0, 7.0, 2.0, 9.0, 4.0, 3.0, 8.0, -2.0],
            vec![2.0, -1.0, 4.0, 5.0, 0.0, 6.0, 3.0, 10.0],
            vec![-3.0, 0.5, 1.0, 2.5, 7.0, -4.0, 6.0, 8.0],
        ];
        let input = DenseMatrix::from_columns(&columns).expect("matrix");
        let retained = absorb_borrowed_intercept_map_retaining_certified(
            &design,
            BorrowedMapWeights::Provided(&weights),
            input.clone(),
            config(),
        )
        .expect("retaining MAP");
        let (residualized, report, coefficients) = retained.into_full_parts();
        assert!(report.certified());
        let coefficients = coefficients.expect("MAP coefficient product");
        for rhs_index in 0..input.ncols() {
            let rhs = coefficients.rhs(rhs_index).expect("coefficient RHS");
            for row in 0..input.nrows() {
                let fitted = rhs[coefficients.effect_offsets()[0] + worker_codes[row] as usize]
                    + rhs[coefficients.effect_offsets()[1] + firm_codes[row] as usize];
                assert!(
                    (fitted - (input[(row, rhs_index)] - residualized[(row, rhs_index)])).abs()
                        <= 2e-10
                );
            }
        }
    }

    #[test]
    fn borrowed_cg_breakdown_and_exhaustion_fail_closed() {
        let mut output = [0.0, 0.0];
        let error = cg_ratios_into(
            &[1.0, 0.0],
            &[0.0, 0.0],
            &mut output,
            "injected denominator breakdown",
        )
        .expect_err("breakdown");
        assert_eq!(error.code, ErrorCode::InvalidData);

        let mut improvement = [10.0, 20.0];
        let error = cg_hestenes_error_and_update(&[0.5, 0.25], &[4.0, 8.0], &mut improvement)
            .expect("Hestenes update");
        assert!((error - 0.5).abs() <= f64::EPSILON);
        assert_eq!(improvement, [8.0, 18.0]);
        assert_eq!(
            cg_hestenes_error_and_update(&[2.0], &[1.0], &mut [1.0])
                .expect_err("negative improvement potential")
                .code,
            ErrorCode::InvalidData
        );

        let first_codes = [0_u32, 0, 0, 1, 1, 1];
        let second_codes = [0_u32, 0, 1, 0, 1, 1];
        let first_index = LevelIndex::from_codes(&first_codes, 2).expect("first index");
        let second_index = LevelIndex::from_codes(&second_codes, 2).expect("second index");
        let first =
            ValidatedBorrowedInterceptSource::new(&first_codes, 2, &first_index).expect("first");
        let second =
            ValidatedBorrowedInterceptSource::new(&second_codes, 2, &second_index).expect("second");
        let design = BorrowedInterceptFeDesign::new(
            first_codes.len(),
            vec![first.effect(), second.effect()],
        )
        .expect("design");
        let error = absorb_borrowed_intercept_cg_certified(
            &design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![1.0, 7.0, 2.0, 9.0, 4.0, 3.0]]).expect("matrix"),
            MapConfig {
                tolerance: 1e-16,
                max_iterations: 1,
                check_every: 1,
                symmetric: true,
            },
        )
        .expect_err("iteration exhaustion");
        assert_eq!(error.code, ErrorCode::InvalidData);
    }

    #[test]
    fn borrowed_irons_tuck_retains_a_reconstructing_coefficient_product() {
        let worker_codes = [0_u32, 0, 1, 1, 2, 2, 3, 3, 0, 1, 2, 3];
        let firm_codes = [0_u32, 1, 1, 2, 2, 0, 0, 2, 2, 0, 1, 2];
        let worker_index = LevelIndex::from_codes(&worker_codes, 4).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm_codes, 3).expect("firm index");
        let worker =
            ValidatedBorrowedInterceptSource::new(&worker_codes, 4, &worker_index).expect("worker");
        let firm =
            ValidatedBorrowedInterceptSource::new(&firm_codes, 3, &firm_index).expect("firm");
        let design = BorrowedInterceptFeDesign::new(
            worker_codes.len(),
            vec![worker.effect(), firm.effect()],
        )
        .expect("design");
        let input = DenseMatrix::from_columns(&[
            (0..worker_codes.len())
                .map(|row| (row as f64 * 0.37).sin())
                .collect(),
            (0..worker_codes.len())
                .map(|row| (row as f64 * 0.19).cos())
                .collect(),
        ])
        .expect("matrix");
        let result = absorb_borrowed_intercept_irons_tuck_certified(
            &design,
            BorrowedMapWeights::Unit,
            input.clone(),
            config(),
            true,
        )
        .expect("retaining Irons-Tuck");
        let (residualized, report, coefficients) = result.into_full_parts();
        assert!(report.certified());
        assert_eq!(report.used_solver, FeSolverKind::IronsTuck);
        let coefficients = coefficients.expect("Irons-Tuck coefficient product");
        for rhs_index in 0..input.ncols() {
            let rhs = coefficients.rhs(rhs_index).expect("coefficient RHS");
            for row in 0..input.nrows() {
                let fitted = rhs[coefficients.effect_offsets()[0] + worker_codes[row] as usize]
                    + rhs[coefficients.effect_offsets()[1] + firm_codes[row] as usize];
                assert!(
                    (fitted - (input[(row, rhs_index)] - residualized[(row, rhs_index)])).abs()
                        <= 2e-10
                );
            }
        }
    }

    #[test]
    fn borrowed_auto_uses_direct_one_fe_early_map_and_cg_routes() {
        let one_codes = [0_u32, 0, 1, 1];
        let one_index = LevelIndex::from_codes(&one_codes, 2).expect("one index");
        let one = ValidatedBorrowedInterceptSource::new(&one_codes, 2, &one_index).expect("one");
        let one_design = BorrowedInterceptFeDesign::new(4, vec![one.effect()]).expect("one design");
        let direct = absorb_borrowed_intercept_auto_certified(
            &one_design,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![1.0, 3.0, 4.0, 10.0]]).expect("one matrix"),
            config(),
        )
        .expect("direct auto");
        assert_eq!(direct.report().requested_solver, FeSolverKind::Auto);
        assert_eq!(direct.report().used_solver, FeSolverKind::Map);
        assert_eq!(direct.report().pilot_iterations, 0);

        let row_codes = [0_u32, 0, 1, 1];
        let column_codes = [0_u32, 1, 0, 1];
        let row_index = LevelIndex::from_codes(&row_codes, 2).expect("row index");
        let column_index = LevelIndex::from_codes(&column_codes, 2).expect("column index");
        let row = ValidatedBorrowedInterceptSource::new(&row_codes, 2, &row_index).expect("row");
        let column =
            ValidatedBorrowedInterceptSource::new(&column_codes, 2, &column_index).expect("column");
        let orthogonal = BorrowedInterceptFeDesign::new(4, vec![row.effect(), column.effect()])
            .expect("orthogonal design");
        let early = absorb_borrowed_intercept_auto_certified(
            &orthogonal,
            BorrowedMapWeights::Unit,
            DenseMatrix::from_columns(&[vec![1.0, 2.0, 4.0, 8.0]]).expect("early matrix"),
            config(),
        )
        .expect("early MAP");
        assert_eq!(early.report().used_solver, FeSolverKind::Map);
        assert_eq!(early.report().pilot_iterations, 1);

        let worker_codes = [0_u32, 0, 1, 1, 2, 2, 3, 3];
        let firm_codes = [0_u32, 1, 1, 2, 2, 0, 0, 2];
        let worker_index = LevelIndex::from_codes(&worker_codes, 4).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm_codes, 3).expect("firm index");
        let worker =
            ValidatedBorrowedInterceptSource::new(&worker_codes, 4, &worker_index).expect("worker");
        let firm =
            ValidatedBorrowedInterceptSource::new(&firm_codes, 3, &firm_index).expect("firm");
        let hard = BorrowedInterceptFeDesign::new(
            worker_codes.len(),
            vec![worker.effect(), firm.effect()],
        )
        .expect("hard design");
        let cg = absorb_borrowed_intercept_auto_certified(
            &hard,
            BorrowedMapWeights::Provided(&[1.0, 2.0, 1.5, 4.0, 0.5, 3.0, 2.5, 1.25]),
            DenseMatrix::from_columns(&[vec![1.0, 7.0, 2.0, 9.0, 4.0, 3.0, 8.0, -2.0]])
                .expect("CG matrix"),
            MapConfig {
                tolerance: 1e-14,
                ..config()
            },
        )
        .expect("CG auto");
        assert_eq!(cg.report().requested_solver, FeSolverKind::Auto);
        assert_eq!(cg.report().used_solver, FeSolverKind::Cg);
        assert_eq!(cg.report().pilot_iterations, 4);
        assert!(cg.report().pilot_contraction.is_finite());
        assert!(cg.report().certified());
    }

    #[test]
    fn routed_auto_selects_lsmr_for_a_slow_chain_graph() {
        let workers = 160_usize;
        let mut worker_codes = Vec::with_capacity(workers * 2);
        let mut firm_codes = Vec::with_capacity(workers * 2);
        let mut outcome = Vec::with_capacity(workers * 2);
        for worker in 0..workers {
            worker_codes.extend([worker as u32, worker as u32]);
            firm_codes.extend([worker as u32, (worker + 1) as u32]);
            outcome.extend([(worker as f64 * 0.17).sin(), (worker as f64 * 0.11).cos()]);
        }
        let worker_index = LevelIndex::from_codes(&worker_codes, workers).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm_codes, workers + 1).expect("firm index");
        let worker = ValidatedBorrowedInterceptSource::new(&worker_codes, workers, &worker_index)
            .expect("worker");
        let firm = ValidatedBorrowedInterceptSource::new(&firm_codes, workers + 1, &firm_index)
            .expect("firm");
        let design = BorrowedInterceptFeDesign::new(
            worker_codes.len(),
            vec![worker.effect(), firm.effect()],
        )
        .expect("design");
        let input = DenseMatrix::from_columns(&[outcome]).expect("matrix");
        let result = absorb_borrowed_intercept_auto_routed_certified(
            &design,
            BorrowedMapWeights::Unit,
            input.clone(),
            config(),
            LsmrConfig::default(),
            None,
            true,
        )
        .expect("routed auto");
        assert_eq!(result.report().requested_solver, FeSolverKind::Auto);
        assert_eq!(result.report().used_solver, FeSolverKind::Lsmr);
        assert_eq!(result.report().pilot_iterations, 4);
        assert!(result.report().pilot_contraction >= 0.008);
        assert!(result.report().certified());
        let (residualized, _, coefficients) = result.into_full_parts();
        let coefficients = coefficients.expect("retained AUTO coefficient product");
        let rhs = coefficients.rhs(0).expect("coefficient RHS");
        for row in 0..input.nrows() {
            let fitted = rhs[coefficients.effect_offsets()[0] + worker_codes[row] as usize]
                + rhs[coefficients.effect_offsets()[1] + firm_codes[row] as usize];
            assert!((fitted - (input[(row, 0)] - residualized[(row, 0)])).abs() <= 1e-9);
        }
    }

    #[test]
    fn constrained_joint_memory_routes_lsmr_to_cg_and_fails_cleanly_when_neither_fits() {
        let workers = 160_usize;
        let observations = workers * 2;
        let mut worker_codes = Vec::with_capacity(observations);
        let mut firm_codes = Vec::with_capacity(observations);
        let mut outcome = Vec::with_capacity(observations);
        for worker in 0..workers {
            worker_codes.extend([worker as u32, worker as u32]);
            firm_codes.extend([worker as u32, (worker + 1) as u32]);
            outcome.extend([(worker as f64 * 0.17).sin(), (worker as f64 * 0.11).cos()]);
        }
        let worker_index = LevelIndex::from_codes(&worker_codes, workers).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm_codes, workers + 1).expect("firm index");
        let worker = ValidatedBorrowedInterceptSource::new(&worker_codes, workers, &worker_index)
            .expect("worker");
        let firm = ValidatedBorrowedInterceptSource::new(&firm_codes, workers + 1, &firm_index)
            .expect("firm");
        let design =
            BorrowedInterceptFeDesign::new(observations, vec![worker.effect(), firm.effect()])
                .expect("design");
        let lsmr = LsmrConfig {
            threads: crate::options::ThreadConfig::from_requested(1, true).expect("threads"),
            ..LsmrConfig::default()
        };
        let memory =
            estimate_fe_solver_memory(observations, 1, &[workers, workers + 1], 1, lsmr.local_size)
                .expect("solver memory");
        assert!(memory.cg_bytes < memory.lsmr_schwarz_bytes);
        let run = |usable_bytes| {
            absorb_borrowed_intercept_auto_routed_certified(
                &design,
                BorrowedMapWeights::Unit,
                DenseMatrix::from_columns(std::slice::from_ref(&outcome)).expect("matrix"),
                config(),
                lsmr,
                Some(FeSolverMemoryBudget {
                    execution_bytes: 0,
                    usable_bytes,
                }),
                false,
            )
        };
        let cg = run(memory.cg_bytes).expect("memory-routed CG");
        assert_eq!(cg.report().used_solver, FeSolverKind::Cg);
        assert!(cg.report().fallback_used);
        assert!(cg
            .report()
            .warnings
            .first()
            .is_some_and(|warning| { warning.contains("joint execution/solver memory plan") }));

        let error = run(memory.cg_bytes - 1).expect_err("neither route may allocate");
        assert_eq!(error.code, ErrorCode::Allocation);
        assert!(error.message.contains("neither LSMR nor CG"));
    }

    #[test]
    fn forced_large_axis_route_uses_schwarz_lsmr_then_certified_cg() {
        let workers = 160_usize;
        let observations = workers * 2;
        let mut worker_codes = Vec::with_capacity(observations);
        let mut firm_codes = Vec::with_capacity(observations);
        let mut region_codes = Vec::with_capacity(observations);
        let mut first = Vec::with_capacity(observations);
        let mut second = Vec::with_capacity(observations);
        let mut weights = Vec::with_capacity(observations);
        for worker in 0..workers {
            worker_codes.extend([worker as u32, worker as u32]);
            firm_codes.extend([worker as u32, (worker + 1) as u32]);
            region_codes.extend([
                ((worker * 37) % 97) as u32,
                ((worker * 37 + 17) % 97) as u32,
            ]);
            first.extend([(worker as f64 * 0.17).sin(), (worker as f64 * 0.11).cos()]);
            second.extend([(worker as f64 * 0.07).cos(), (worker as f64 * 0.19).sin()]);
            weights.extend([
                1.0 + (worker % 7) as f64 * 0.125,
                1.0 + (worker % 5) as f64 * 0.25,
            ]);
        }
        let worker_index = LevelIndex::from_codes(&worker_codes, workers).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm_codes, workers + 1).expect("firm index");
        let region_index = LevelIndex::from_codes(&region_codes, 97).expect("region index");
        let worker = ValidatedBorrowedInterceptSource::new(&worker_codes, workers, &worker_index)
            .expect("worker");
        let firm = ValidatedBorrowedInterceptSource::new(&firm_codes, workers + 1, &firm_index)
            .expect("firm");
        let region = ValidatedBorrowedInterceptSource::new(&region_codes, 97, &region_index)
            .expect("region");
        let design = BorrowedInterceptFeDesign::new(
            observations,
            vec![worker.effect(), firm.effect(), region.effect()],
        )
        .expect("design");
        let columns = [first, second];
        let raw = DenseMatrix::from_columns(&columns).expect("matrix");
        let expected = absorb_borrowed_intercept_cg_certified(
            &design,
            BorrowedMapWeights::Provided(&weights),
            raw.clone(),
            config(),
        )
        .expect("direct CG");

        let run_hybrid = |lsmr: LsmrConfig| {
            let mut reservation = SystemWorkspaceReservation;
            absorb_auto_with_reservation_at_lsmr_cap(
                &design,
                BorrowedMapWeights::Provided(&weights),
                raw.clone(),
                config(),
                AutoLsmrPlan::routed(lsmr, None),
                false,
                false,
                &mut reservation,
                0,
            )
        };
        let first_hybrid = run_hybrid(LsmrConfig::default()).expect("hybrid");
        let second_hybrid = run_hybrid(LsmrConfig::default()).expect("deterministic hybrid");
        let report = first_hybrid.report();
        assert_eq!(report.requested_solver, FeSolverKind::Auto);
        assert_eq!(report.used_solver, FeSolverKind::Cg);
        assert_eq!(
            report.requested_preconditioner,
            Some(crate::fe::LsmrPreconditioner::Auto)
        );
        assert_eq!(
            report.used_preconditioner,
            Some(crate::fe::LsmrPreconditioner::Schwarz)
        );
        assert_eq!(report.pilot_iterations, 4);
        assert!(
            report.iterations > 0,
            "CG must perform the final polish: {report:?}"
        );
        assert!(report.certified());
        assert!(report
            .warnings
            .first()
            .is_some_and(|warning| warning.contains("LSMR-CG prototype")));
        assert_eq!(first_hybrid.residualized(), second_hybrid.residualized());
        assert_eq!(report.iterations, second_hybrid.report().iterations);
        assert_close(
            first_hybrid.residualized().as_slice(),
            expected.residualized().as_slice(),
            1e-8,
        );

        let fallback = run_hybrid(LsmrConfig {
            max_iterations: 0,
            ..LsmrConfig::default()
        })
        .expect("fallback CG");
        assert_eq!(fallback.report().used_solver, FeSolverKind::Cg);
        assert!(fallback.report().fallback_used);
        assert!(fallback
            .report()
            .warnings
            .first()
            .is_some_and(|warning| warning.contains("bulk phase failed")));
        assert_close(
            fallback.residualized().as_slice(),
            expected.residualized().as_slice(),
            1e-8,
        );
    }

    #[test]
    fn hybrid_bulk_tolerance_is_looser_without_weakening_the_final_target() {
        assert_eq!(lsmr_cg_bulk_tolerance(1e-10), 1e-5);
        assert_eq!(lsmr_cg_bulk_tolerance(1.0), 1.0);
        assert_eq!(lsmr_cg_bulk_tolerance(4.0), 4.0);
    }

    #[test]
    fn qualification_matrix_exercises_direct_tolerances_preconditioners_and_forced_cg() {
        let workers = 48_usize;
        let observations = workers * 3;
        let mut worker_codes = Vec::with_capacity(observations);
        let mut firm_codes = Vec::with_capacity(observations);
        let mut first = Vec::with_capacity(observations);
        let mut second = Vec::with_capacity(observations);
        let mut raw_weights = Vec::with_capacity(observations);
        for worker in 0..workers {
            worker_codes.extend([worker as u32, worker as u32, worker as u32]);
            firm_codes.extend([
                worker as u32,
                worker as u32,
                ((worker + 1) % workers) as u32,
            ]);
            first.extend([
                (worker as f64 * 0.17).sin(),
                (worker as f64 * 0.11).cos(),
                (worker as f64 * 0.07).sin(),
            ]);
            second.extend([
                (worker as f64 * 0.13).cos(),
                (worker as f64 * 0.19).sin(),
                (worker as f64 * 0.23).cos(),
            ]);
            raw_weights.extend([0.25, 1.0 + (worker % 5) as f64, 3.5]);
        }
        let worker_index = LevelIndex::from_codes(&worker_codes, workers).expect("worker index");
        let firm_index = LevelIndex::from_codes(&firm_codes, workers).expect("firm index");
        let worker = ValidatedBorrowedInterceptSource::new(&worker_codes, workers, &worker_index)
            .expect("worker");
        let firm =
            ValidatedBorrowedInterceptSource::new(&firm_codes, workers, &firm_index).expect("firm");
        let design =
            BorrowedInterceptFeDesign::new(observations, vec![worker.effect(), firm.effect()])
                .expect("design");
        let raw = DenseMatrix::from_columns(&[first, second]).expect("matrix");
        let weights = validate_weights(BorrowedMapWeights::Provided(&raw_weights), observations)
            .expect("weights");
        let requested_tolerance = 1e-8;
        assert_eq!(direct_lsmr_tolerance(requested_tolerance), 1e-10);
        assert_eq!(direct_lsmr_tolerance(1e-15), 1e-15);

        let reference = absorb_borrowed_intercept_cg_certified(
            &design,
            BorrowedMapWeights::Provided(&raw_weights),
            raw.clone(),
            MapConfig {
                tolerance: requested_tolerance,
                ..config()
            },
        )
        .expect("CG reference");

        for preconditioner in [
            crate::fe::LsmrPreconditioner::Off,
            crate::fe::LsmrPreconditioner::Diagonal,
            crate::fe::LsmrPreconditioner::Schwarz,
        ] {
            for tolerance in [
                requested_tolerance,
                direct_lsmr_tolerance(requested_tolerance),
            ] {
                let (candidate, report, _, _) = run_borrowed_lsmr(
                    &design,
                    weights,
                    &raw,
                    tolerance,
                    16_000,
                    LsmrConfig {
                        tolerance,
                        preconditioner,
                        ..LsmrConfig::default()
                    },
                    false,
                )
                .expect("qualification direct LSMR arm");
                assert!(
                    report.certified(),
                    "{preconditioner:?} {tolerance:e}: {report:?}"
                );
                assert_close(
                    candidate.as_slice(),
                    reference.residualized().as_slice(),
                    2e-7,
                );
            }
        }

        let (direct, direct_report, _, _) = run_borrowed_lsmr(
            &design,
            weights,
            &raw,
            direct_lsmr_tolerance(requested_tolerance),
            16_000,
            LsmrConfig {
                tolerance: direct_lsmr_tolerance(requested_tolerance),
                preconditioner: crate::fe::LsmrPreconditioner::Schwarz,
                ..LsmrConfig::default()
            },
            false,
        )
        .expect("direct Schwarz LSMR");
        assert!(direct_report.certified());
        for forced_iterations in [4_usize, 8, 16] {
            let mut reservation = SystemWorkspaceReservation;
            let refined = absorb_cg_with_reservation(
                &design,
                BorrowedMapWeights::Provided(&raw_weights),
                direct.clone(),
                MapConfig {
                    tolerance: requested_tolerance,
                    max_iterations: forced_iterations,
                    ..config()
                },
                false,
                CgContinuation {
                    reference_norms: None,
                    refinement: CgRefinementPolicy::forced(forced_iterations),
                    retain_coefficients: false,
                    reconstruction_tolerance: None,
                },
                &mut reservation,
            )
            .expect("forced restarted CG arm");
            assert_eq!(refined.report().iterations, forced_iterations);
            assert!(refined.report().certified());
            assert_close(
                refined.residualized().as_slice(),
                reference.residualized().as_slice(),
                2e-7,
            );
        }
    }

    fn assert_close(left: &[f64], right: &[f64], tolerance: f64) {
        assert_eq!(left.len(), right.len());
        for (left, right) in left.iter().zip(right) {
            assert!((left - right).abs() <= tolerance, "{left} != {right}");
        }
    }
}
