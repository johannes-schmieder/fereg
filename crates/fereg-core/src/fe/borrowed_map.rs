use std::fs::OpenOptions;
use std::io::Write;
use std::time::Instant;

use super::auto::{auto_lsmr_route_at_cap, direct_lsmr_tolerance, AutoLsmrRoute};
use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::{
    BorrowedInterceptFeDesign, FeCertificate, FeSolverKind, LsmrConfig, LsmrPreconditioner,
    MapConfig, SolveReport, SolveStopReason,
};
use crate::memory::{
    estimate_fe_solver_memory, estimate_fe_solver_memory_with_batch_policy,
    FeSolverMemoryBudget, LsmrBatchPolicy, SolverWorkspacePeak,
};
use rayon::prelude::*;
use within::{Effect, LsmrOptions, PreconditionerConfig};

use super::map::ReusableMapCapacities;

pub(crate) const PARALLEL_PROJECTION_MIN_ROWS: usize = 16_384;
pub(crate) const FUSED_PARALLEL_MAX_RHS: usize = CATEGORICAL_MAP_ACCELERATOR_MAX_RHS;
// Fixed row partitions make the compensated dot reduction independent of the
// installed team width. The 512-row crossover is measured on the wide PPML
// checkpoint shape; smaller chunks add scheduling overhead and larger chunks
// leave the deterministic reduction under-parallelized.
pub(crate) const PARALLEL_DOT_CHUNK_ROWS: usize = 512;
const PARALLEL_GROUP_MAX_RHS: usize = CATEGORICAL_MAP_ACCELERATOR_MAX_RHS;
const PARALLEL_LEVEL_CHUNKS_PER_THREAD: usize = 4;
const TWO_FE_PCG_PARALLEL_MIN_ROWS: usize = 16_384;
// The level-disjoint recurrence wins through seven workers on the difficult
// chain, then loses to scheduling and memory traffic. Above the measured
// crossover retain the canonical serial recurrence inside the threaded solve.
const TWO_FE_PCG_PARALLEL_RECURRENCE_MAX_THREADS: usize = 7;
/// Keep the PPML accelerator available through the development matrix's
/// control-heavy cases. The exact three-matrix state is charged by the PPML
/// memory planner, so wider fits fall back to the bounded route rather than
/// allocating this state outside the admitted budget.
pub(crate) const CATEGORICAL_MAP_ACCELERATOR_MAX_RHS: usize = 64;
const CATEGORICAL_MAP_PILOT_SWEEPS: usize = 4;
pub(crate) const SD_RECOVERY_PILOT_SWEEPS: usize = 4;
pub(crate) const SD_RECOVERY_CONTINUATION_CAP: usize = 4;
const SD_RECOVERY_LSMR_TOLERANCE_CAP: f64 = 1e-4;

fn emit_solver_trace(line: &str) {
    eprintln!("{line}");
    let Some(path) = std::env::var_os("FEREG_SOLVER_TRACE_FILE") else {
        return;
    };
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

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
    reconstruct_projection: bool,
    reconstruction_tolerance: Option<f64>,
}

impl CgContinuation<'_> {
    const ORDINARY: Self = Self {
        reference_norms: None,
        refinement: CgRefinementPolicy::ORDINARY,
        retain_coefficients: false,
        reconstruct_projection: false,
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
    skip_map_pilot: bool,
    batch_policy: LsmrBatchPolicy,
}

#[derive(Debug, Clone, Copy)]
struct AutoLsmrAttemptAccounting {
    iterations: usize,
    setup_seconds: f64,
    solve_seconds: f64,
    requested_preconditioner: Option<LsmrPreconditioner>,
    used_preconditioner: Option<LsmrPreconditioner>,
}

impl AutoLsmrPlan {
    const DISABLED: Self = Self {
        config: None,
        memory_budget: None,
        skip_map_pilot: false,
        batch_policy: LsmrBatchPolicy::Default,
    };

    const fn routed(config: LsmrConfig, memory_budget: Option<FeSolverMemoryBudget>) -> Self {
        Self {
            config: Some(config),
            memory_budget,
            skip_map_pilot: false,
            batch_policy: LsmrBatchPolicy::Default,
        }
    }

    const fn direct_lsmr(config: LsmrConfig, memory_budget: Option<FeSolverMemoryBudget>) -> Self {
        Self {
            config: Some(config),
            memory_budget,
            skip_map_pilot: true,
            batch_policy: LsmrBatchPolicy::Default,
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
    ols_norm_reduction: bool,
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
        let parallel_group_levels =
            if observation_count >= PARALLEL_PROJECTION_MIN_ROWS && parallel_threads > 1 {
                maximum_levels
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
            && parallel_threads > 1
        {
            observation_count
                .div_ceil(PARALLEL_DOT_CHUNK_ROWS)
                .checked_mul(rhs_count)
                .ok_or_else(|| allocation("borrowed CG parallel dot dimensions overflow"))?
        } else {
            0
        };
        Ok([
            parallel_group_levels,
            parallel_group_mean_count,
            parallel_dot_sum_count,
        ])
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
            ols_norm_reduction: false,
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
    accelerator_x0: Option<DenseMatrix>,
    accelerator_x1: Option<DenseMatrix>,
    accelerator_x2: Option<DenseMatrix>,
    cg_residual_square: Vec<f64>,
    cg_initial_residual_square: Vec<f64>,
    cg_next_residual_square: Vec<f64>,
    cg_direction_projection: Vec<f64>,
    cg_ratio: Vec<f64>,
    max_rhs: usize,
    max_level_members: usize,
    checkpoint_lsmr_enabled: bool,
    #[cfg(test)]
    checkpoint_lsmr_certificate_failure: bool,
    #[cfg(test)]
    checkpoint_lsmr_fatal_failure: u8,
    #[cfg(test)]
    checkpoint_lsmr_retry_error: Option<ErrorCode>,
    #[cfg(test)]
    checkpoint_schur_iteration_cap: Option<usize>,
    #[cfg(test)]
    checkpoint_schur_certificate_failure: bool,
}

/// Reusable level-space PCG state for two-way categorical graphs.
///
/// Eliminating the larger effect turns two-way demeaning into a weighted
/// Schur-complement system on the smaller effect. This is still an
/// acceleration of the requested MAP projection: the published result is
/// reconstructed in observation space and must pass the ordinary MAP FE
/// certificate before it can leave the solver.
struct TwoFePcgWorkspace {
    left_effect: usize,
    right_effect: usize,
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
        let left_cells = left_levels
            .checked_mul(max_rhs)
            .ok_or_else(|| allocation("two-FE PCG left workspace dimensions overflow"))?;
        let right_cells = right_levels
            .checked_mul(max_rhs)
            .ok_or_else(|| allocation("two-FE PCG right workspace dimensions overflow"))?;
        Ok(Self {
            left_effect,
            right_effect,
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
    }
}

impl<'a> ReusableCategoricalMapAbsorber<'a> {
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
        let two_fe_pcg = if design.effects().len() == 2
            && design.nobs() >= 128
            && max_rhs <= CATEGORICAL_MAP_ACCELERATOR_MAX_RHS
        {
            Some(TwoFePcgWorkspace::new(&design, max_rhs, &mut reservation)?)
        } else {
            None
        };
        // A single categorical projection is exact and needs no observation-sized accelerator.
        let (accelerator_x0, accelerator_x1, accelerator_x2) =
            if design.effects().len() > 1
                && max_rhs <= CATEGORICAL_MAP_ACCELERATOR_MAX_RHS
                && two_fe_pcg.is_none()
            {
                (
                    Some(DenseMatrix::zeros(design.nobs(), max_rhs)?),
                    Some(DenseMatrix::zeros(design.nobs(), max_rhs)?),
                    Some(DenseMatrix::zeros(design.nobs(), max_rhs)?),
                )
            } else {
                (None, None, None)
            };
        let cg_scalar_rhs = usize::from(accelerator_x0.is_some() || two_fe_pcg.is_some()) * max_rhs;
        Ok(Self {
            design,
            workspace,
            two_fe_pcg,
            accelerator_x0,
            accelerator_x1,
            accelerator_x2,
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
            checkpoint_lsmr_enabled: false,
            #[cfg(test)]
            checkpoint_lsmr_certificate_failure: false,
            #[cfg(test)]
            checkpoint_lsmr_fatal_failure: 0,
            #[cfg(test)]
            checkpoint_lsmr_retry_error: None,
            #[cfg(test)]
            checkpoint_schur_iteration_cap: None,
            #[cfg(test)]
            checkpoint_schur_certificate_failure: false,
        })
    }

    pub(crate) fn enable_checkpoint_lsmr(&mut self) {
        self.checkpoint_lsmr_enabled = true;
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

    #[cfg(test)]
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
            certificate_columns: self.workspace.parallel_dot_sums.capacity(),
            certificate_basis: 0,
            accelerator_state: self
                .two_fe_pcg
                .as_ref()
                .map_or(0, TwoFePcgWorkspace::capacity_elements)
                .saturating_add(
                    self.accelerator_x0
                        .as_ref()
                        .map_or(0, DenseMatrix::storage_capacity),
                )
                .saturating_add(
                    self.accelerator_x1
                        .as_ref()
                        .map_or(0, DenseMatrix::storage_capacity),
                )
                .saturating_add(
                    self.accelerator_x2
                        .as_ref()
                        .map_or(0, DenseMatrix::storage_capacity),
                )
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

    /// Copy trailing-column norms from this successful projection into retained
    /// caller storage. One-FE returns certify the exact returned values in this
    /// workspace; multi-FE solver handoffs need not leave the same norms here.
    pub(crate) fn absorb_with_trailing_norms(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        values: DenseMatrix,
        config: MapConfig,
        norm_weight_scale: f64,
        output: &mut [f64],
    ) -> Result<(CertifiedBorrowedMap, bool)> {
        if output.len() > values.ncols() {
            return Err(invalid_argument(
                "reusable categorical MAP norm output exceeds the RHS count",
            ));
        }
        let rhs_positions = values.ncols() - output.len()..values.ncols();
        self.absorb_with_selected_norms(
            weights, values, config, norm_weight_scale, rhs_positions, output,
        )
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
            || rhs_positions.clone().any(|position| position >= values.ncols())
        {
            return Err(invalid_argument(
                "reusable categorical MAP norm selection is not aligned",
            ));
        }
        let absorbed = self.absorb(weights, values, config)?;
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

    pub(crate) fn absorb(
        &mut self,
        weights: BorrowedMapWeights<'_>,
        mut values: DenseMatrix,
        config: MapConfig,
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
        if values.as_slice().iter().any(|value| !value.is_finite()) {
            return Err(invalid_data(
                "reusable categorical MAP values must be finite",
            ));
        }
        let weights = validate_weights(weights, values.nrows())?;
        self.workspace.set_rhs_count(rhs_count)?;

        let setup_started = Instant::now();
        weighted_column_norms(
            &values,
            weights,
            &mut self.workspace.reference_norms,
            &mut self.workspace.value_scales,
            &mut self.workspace.group_sums,
        )?;
        let setup_seconds = setup_started.elapsed().as_secs_f64();
        let solve_started = Instant::now();
        let initial_certification_started = Instant::now();
        let initial = certify_initial(
            &self.design,
            weights,
            &values,
            config.tolerance,
            &mut self.workspace,
        )?;
        let initial_certification_seconds = initial_certification_started.elapsed().as_secs_f64();
        if initial.passed {
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
            #[cfg(test)]
            if let Some(error) = self.take_checkpoint_lsmr_fatal_failure() {
                return Err(error);
            }
            let (schur, schur_cost) = self.try_checkpoint_schur(weights, &values, config)?;
            if let Some(mut certified) = schur {
                certified.report.setup_seconds += setup_seconds;
                certified.report.certification_seconds += initial_certification_seconds;
                return Ok(certified);
            }
            #[cfg(not(test))]
            let force_rejection = false;
            #[cfg(test)]
            let force_rejection = std::mem::take(&mut self.checkpoint_lsmr_certificate_failure);
            let mut accumulated: Option<SolveReport> = None;
            let mut recurrence_tolerance = config.tolerance;
            let report = loop {
                #[cfg(test)]
                if accumulated.is_some() {
                    if let Some(code) = self.checkpoint_lsmr_retry_error.take() {
                        return Err(FeregError::new(code, "injected checkpoint retry failure"));
                    }
                }
                let lsmr = LsmrConfig {
                    tolerance: recurrence_tolerance,
                    max_iterations: config.max_iterations,
                    ..LsmrConfig::default()
                };
                let (candidate, mut report, owned_setup_seconds, _) = run_borrowed_lsmr(
                    &self.design,
                    weights,
                    &values,
                    recurrence_tolerance,
                    config.max_iterations,
                    lsmr,
                    Some(&self.workspace.reference_norms),
                    false,
                )?;
                report.setup_seconds += owned_setup_seconds;
                let certification_started = Instant::now();
                let mut canonical_certificate = certify(
                    &self.design,
                    weights,
                    &candidate,
                    config.tolerance,
                    &mut self.workspace,
                )?;
                canonical_certificate.passed &= !force_rejection;
                report.certification_seconds += certification_started.elapsed().as_secs_f64();
                if std::env::var_os("FEREG_SOLVER_TRACE").is_some() {
                    emit_solver_trace(&format!(
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
                    return CertifiedBorrowedMap::from_certified_parts(candidate, report);
                }
                report.stop_reason = SolveStopReason::Uncertified;
                drop(candidate);
                let tighter = (config.tolerance * 0.01).max(1e-14);
                if report.checkpoint_retries != 0 || tighter >= recurrence_tolerance {
                    break report;
                }
                // Only a failed fresh canonical certificate authorizes this
                // retry. Both independent batches use untouched inputs, and
                // their transient peaks are serial; publication tolerance,
                // fallback inputs, rank policy, and downstream checks do not change.
                recurrence_tolerance = tighter;
                accumulated = Some(report);
            };

            // The LSMR candidate never aliases `values`. A numerical failure
            // therefore leaves the authoritative checkpoint inputs untouched
            // and permits the existing MAP projector to certify them. Other
            // error classes returned above remain fatal.
            let raw_weights = borrowed_weights(weights);
            self.checkpoint_lsmr_enabled = false;
            let fallback = self.absorb(raw_weights, values, config);
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

        let pilot_limit = config.max_iterations.min(CATEGORICAL_MAP_PILOT_SWEEPS);
        for iteration in 1..=pilot_limit {
            borrowed_residual_sweep_recording::<MapCoefficientAccumulator>(
                &self.design,
                weights,
                &mut values,
                &mut self.workspace,
                None,
                config.symmetric,
            )?;
            if self.design.effects().len() == 1
                || iteration % config.check_every == 0
                || iteration == pilot_limit
            {
                let certificate = certify(
                    &self.design,
                    weights,
                    &values,
                    config.tolerance,
                    &mut self.workspace,
                )?;
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
                if iteration != pilot_limit {
                    continue;
                }
                let contraction =
                    pilot_contraction(initial.relative_two_norm, certificate.relative_two_norm)?
                        .max(pilot_contraction(
                            initial.relative_infinity_norm,
                            certificate.relative_infinity_norm,
                        )?);
                if pilot_limit == CATEGORICAL_MAP_PILOT_SWEEPS {
                    if self.two_fe_pcg.is_some() {
                        return self.absorb_two_fe_pcg_after_pilot(
                            weights,
                            values,
                            config,
                            pilot_limit,
                            contraction,
                            setup_seconds,
                            solve_started,
                        );
                    }
                    if self.accelerator_x0.is_none() {
                        continue;
                    }
                    let topology_prefers_cg = self.design.effects().len() == 3;
                    if topology_prefers_cg || config.tolerance <= 1.1e-12 {
                        return self.absorb_cg_after_pilot(
                            weights,
                            values,
                            config,
                            pilot_limit,
                            contraction,
                            setup_seconds,
                            solve_started,
                        );
                    }
                    return self.absorb_accelerated_after_pilot(
                        weights,
                        values,
                        config,
                        pilot_limit,
                        contraction,
                        setup_seconds,
                        solve_started,
                    );
                }
            }
        }
        for iteration in pilot_limit.saturating_add(1)..=config.max_iterations {
            borrowed_residual_sweep_recording::<MapCoefficientAccumulator>(
                &self.design,
                weights,
                &mut values,
                &mut self.workspace,
                None,
                config.symmetric,
            )?;
            if iteration % config.check_every == 0 || iteration == config.max_iterations {
                let certificate = certify(
                    &self.design,
                    weights,
                    &values,
                    config.tolerance,
                    &mut self.workspace,
                )?;
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
            || self.design.nobs() < PARALLEL_PROJECTION_MIN_ROWS
            || !(16..=64).contains(&values.ncols())
            || config.max_iterations <= OLS_SCHUR_PROBE_ITERATIONS
        {
            return Ok((None, cost));
        }
        let started = Instant::now();
        cost.contraction = schur_probe_reusing(
            &self.design,
            weights,
            values,
            self.two_fe_pcg.as_mut().unwrap(),
            self.max_rhs,
        )?;
        cost.probe_iterations = OLS_SCHUR_PROBE_ITERATIONS;
        cost.setup_seconds = started.elapsed().as_secs_f64();
        if cost.contraction > OLS_SCHUR_MAX_CONTRACTION {
            cost.trace(values, "probe_rejected");
            return Ok((None, cost));
        }

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
        let copy_started = Instant::now();
        let candidate = checkpoint_schur_copy(values, transient)?;
        cost.setup_seconds += copy_started.elapsed().as_secs_f64();
        let cap = OLS_SCHUR_MAX_ITERATIONS.min(config.max_iterations - cost.probe_iterations);
        #[cfg(test)]
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
            Some(cap),
            false,
            None,
        )?;
        cost.solve_seconds = solve_started.elapsed().as_secs_f64();
        match attempt {
            SchurAttempt::Certified(mut result) => {
                #[cfg(test)]
                if std::mem::take(&mut self.checkpoint_schur_certificate_failure) {
                    cost.iterations = result.report.iterations;
                    cost.rejected = true;
                    cost.trace(values, "certificate_rejected");
                    return Ok((None, cost));
                }
                result.report.requested_solver = FeSolverKind::Lsmr;
                result.report.setup_seconds += cost.setup_seconds;
                result.report.iterations += cost.probe_iterations;
                result.report.pilot_iterations = cost.probe_iterations;
                cost.iterations = result.report.iterations - cost.probe_iterations;
                cost.trace(values, "certified");
                Ok((Some(result), cost))
            }
            SchurAttempt::Continue {
                iterations,
                candidate,
                ..
            } => {
                cost.iterations = iterations;
                cost.rejected = true;
                cost.trace(
                    values,
                    if candidate {
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
    fn absorb_two_fe_pcg_after_pilot(
        &mut self,
        weights: ValidatedWeights<'_>,
        values: DenseMatrix,
        config: MapConfig,
        completed_pilot_iterations: usize,
        pilot_contraction: f64,
        setup_seconds: f64,
        solve_started: Instant,
    ) -> Result<CertifiedBorrowedMap> {
        let mut graph = self
            .two_fe_pcg
            .take()
            .ok_or_else(|| invalid_data("two-FE categorical MAP PCG workspace is unavailable"))?;
        let result = absorb_two_fe_pcg_reusing(
            &self.design,
            weights,
            values,
            config,
            completed_pilot_iterations,
            pilot_contraction,
            setup_seconds,
            solve_started,
            self.max_rhs,
            &mut graph,
            &mut self.workspace,
            &mut self.cg_residual_square,
            &mut self.cg_initial_residual_square,
            &mut self.cg_next_residual_square,
            &mut self.cg_direction_projection,
            &mut self.cg_ratio,
            None,
            false,
            None,
        );
        self.two_fe_pcg = Some(graph);
        match result? {
            SchurAttempt::Certified(result) => Ok(result),
            SchurAttempt::Continue { .. } => {
                unreachable!("unbounded MAP accelerator continues internally")
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn absorb_cg_after_pilot(
        &mut self,
        weights: ValidatedWeights<'_>,
        mut values: DenseMatrix,
        config: MapConfig,
        completed_pilot_iterations: usize,
        pilot_contraction: f64,
        setup_seconds: f64,
        solve_started: Instant,
    ) -> Result<CertifiedBorrowedMap> {
        let rhs_count = values.ncols();
        for scalars in [
            &mut self.cg_residual_square,
            &mut self.cg_initial_residual_square,
            &mut self.cg_next_residual_square,
            &mut self.cg_direction_projection,
            &mut self.cg_ratio,
        ] {
            if rhs_count > scalars.capacity() {
                return Err(invalid_argument(
                    "reusable categorical CG RHS workspaces exceed fixed capacity",
                ));
            }
            scalars.resize(rhs_count, 0.0);
        }
        let residual = self
            .accelerator_x0
            .as_mut()
            .ok_or_else(|| invalid_data("categorical CG residual state is unavailable"))?;
        let direction = self
            .accelerator_x1
            .as_mut()
            .ok_or_else(|| invalid_data("categorical CG direction state is unavailable"))?;
        let projection = self
            .accelerator_x2
            .as_mut()
            .ok_or_else(|| invalid_data("categorical CG projection state is unavailable"))?;
        residual.resize_zeroed_within_capacity(values.nrows(), rhs_count)?;
        direction.resize_zeroed_within_capacity(values.nrows(), rhs_count)?;
        projection.resize_zeroed_within_capacity(values.nrows(), rhs_count)?;

        borrowed_symmetric_projection_from_group_means(
            &self.design,
            weights,
            &values,
            residual,
            &mut self.workspace,
        )?;
        direction
            .as_mut_slice()
            .copy_from_slice(residual.as_slice());
        weighted_dot_columns_into(
            residual,
            residual,
            weights,
            &mut self.cg_residual_square,
            &mut self.workspace.group_corrections,
            &mut self.workspace.parallel_dot_sums,
        )?;
        self.cg_initial_residual_square
            .copy_from_slice(&self.cg_residual_square);

        let cg_tolerance = config.tolerance;
        let mut iterations = completed_pilot_iterations;
        while iterations < config.max_iterations {
            borrowed_symmetric_projection_from_group_means(
                &self.design,
                weights,
                direction,
                projection,
                &mut self.workspace,
            )?;
            weighted_dot_columns_into(
                direction,
                projection,
                weights,
                &mut self.cg_direction_projection,
                &mut self.workspace.group_corrections,
                &mut self.workspace.parallel_dot_sums,
            )?;
            if cg_ratios_into(
                &self.cg_residual_square,
                &self.cg_direction_projection,
                &mut self.cg_ratio,
                "reusable categorical CG direction denominator is nonfinite or nonpositive",
            )
            .is_err()
            {
                break;
            }
            cg_update_solution_residual_and_weighted_norm(
                &mut values,
                residual,
                direction,
                projection,
                &self.cg_ratio,
                weights,
                &mut self.cg_next_residual_square,
                &mut self.workspace.group_corrections,
                &mut self.workspace.parallel_dot_sums,
            )?;
            iterations += 1;
            let recurrence = cg_largest_relative_residual(
                &self.cg_next_residual_square,
                &self.cg_initial_residual_square,
            )?;
            if iterations % config.check_every == 0
                || recurrence <= cg_tolerance.sqrt()
                || iterations == config.max_iterations
            {
                let mut certificate = certify(
                    &self.design,
                    weights,
                    &values,
                    cg_tolerance,
                    &mut self.workspace,
                )?;
                if certificate.passed && iterations < config.max_iterations {
                    borrowed_residual_sweep_recording::<MapCoefficientAccumulator>(
                        &self.design,
                        weights,
                        &mut values,
                        &mut self.workspace,
                        None,
                        config.symmetric,
                    )?;
                    iterations += 1;
                    certificate = certify(
                        &self.design,
                        weights,
                        &values,
                        cg_tolerance,
                        &mut self.workspace,
                    )?;
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
                    result.report.pilot_contraction = pilot_contraction;
                    return Ok(result);
                }
            }
            if cg_ratios_into(
                &self.cg_next_residual_square,
                &self.cg_residual_square,
                &mut self.cg_ratio,
                "reusable categorical CG residual denominator is nonfinite or nonpositive",
            )
            .is_err()
            {
                break;
            }
            cg_update_direction(direction, residual, &self.cg_ratio)?;
            std::mem::swap(
                &mut self.cg_residual_square,
                &mut self.cg_next_residual_square,
            );
        }

        self.absorb_accelerated_after_pilot(
            weights,
            values,
            config,
            iterations,
            pilot_contraction,
            setup_seconds,
            solve_started,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn absorb_accelerated_after_pilot(
        &mut self,
        weights: ValidatedWeights<'_>,
        mut values: DenseMatrix,
        config: MapConfig,
        mut iterations: usize,
        pilot_contraction: f64,
        setup_seconds: f64,
        solve_started: Instant,
    ) -> Result<CertifiedBorrowedMap> {
        let rhs_count = values.ncols();
        let x0 = self
            .accelerator_x0
            .as_mut()
            .ok_or_else(|| invalid_data("categorical MAP accelerator state is unavailable"))?;
        let x1 = self
            .accelerator_x1
            .as_mut()
            .ok_or_else(|| invalid_data("categorical MAP accelerator state is unavailable"))?;
        x0.resize_zeroed_within_capacity(values.nrows(), rhs_count)?;
        x1.resize_zeroed_within_capacity(values.nrows(), rhs_count)?;
        while iterations < config.max_iterations {
            x0.as_mut_slice().copy_from_slice(values.as_slice());
            borrowed_residual_sweep_recording::<MapCoefficientAccumulator>(
                &self.design,
                weights,
                &mut values,
                &mut self.workspace,
                None,
                config.symmetric,
            )?;
            iterations += 1;
            if iterations == config.max_iterations {
                break;
            }
            x1.as_mut_slice().copy_from_slice(values.as_slice());
            borrowed_residual_sweep_recording::<MapCoefficientAccumulator>(
                &self.design,
                weights,
                &mut values,
                &mut self.workspace,
                None,
                config.symmetric,
            )?;
            iterations += 1;

            let baseline = certify(
                &self.design,
                weights,
                &values,
                config.tolerance,
                &mut self.workspace,
            )?;
            if baseline.passed {
                let mut result = certified_result(
                    values,
                    SolveStopReason::Certified,
                    iterations,
                    baseline,
                    setup_seconds,
                    solve_started.elapsed().as_secs_f64(),
                )?;
                result.report.pilot_iterations = CATEGORICAL_MAP_PILOT_SWEEPS;
                result.report.pilot_contraction = pilot_contraction;
                return Ok(result);
            }

            if borrowed_irons_tuck_candidate_in_place(
                x0,
                x1,
                &values,
                &mut self.workspace.group_sums,
                &mut self.workspace.group_corrections,
                &mut self.workspace.value_scales,
            )? {
                let candidate = certify(
                    &self.design,
                    weights,
                    x0,
                    config.tolerance,
                    &mut self.workspace,
                )?;
                if candidate.relative_two_norm < baseline.relative_two_norm
                    && candidate.relative_infinity_norm <= baseline.relative_infinity_norm
                {
                    values.as_mut_slice().copy_from_slice(x0.as_slice());
                    if candidate.passed {
                        let mut result = certified_result(
                            values,
                            SolveStopReason::Certified,
                            iterations,
                            candidate,
                            setup_seconds,
                            solve_started.elapsed().as_secs_f64(),
                        )?;
                        result.report.pilot_iterations = CATEGORICAL_MAP_PILOT_SWEEPS;
                        result.report.pilot_contraction = pilot_contraction;
                        return Ok(result);
                    }
                }
            }
        }

        let certificate = certify(
            &self.design,
            weights,
            &values,
            config.tolerance,
            &mut self.workspace,
        )?;
        if certificate.passed {
            let mut result = certified_result(
                values,
                SolveStopReason::Certified,
                iterations,
                certificate,
                setup_seconds,
                solve_started.elapsed().as_secs_f64(),
            )?;
            result.report.pilot_iterations = CATEGORICAL_MAP_PILOT_SWEEPS;
            result.report.pilot_contraction = pilot_contraction;
            return Ok(result);
        }
        Err(invalid_data(
            "accelerated reusable categorical MAP exhausted its iteration budget without certification",
        ))
    }
}

#[allow(clippy::too_many_arguments)]
fn absorb_two_fe_pcg_reusing(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    mut values: DenseMatrix,
    config: MapConfig,
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
    ols_cap: Option<usize>,
    allow_ols_rhs_scratch: bool,
    continuation: Option<&NarrowSchurState>,
) -> Result<SchurAttempt> {
    let trace = std::env::var_os("FEREG_PHASE_TIMING").is_some();
    let phase_start = Instant::now();
    let mut operator_seconds = 0.0;
    let mut recurrence_seconds = 0.0;
    let rhs_count = values.ncols();
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

    if let Some(state) = continuation {
        if !allow_ols_rhs_scratch || ols_cap.is_none() || rhs_count != state.rhs_count
            || completed_pilot_iterations != OLS_SCHUR_PROBE_ITERATIONS {
            return Err(invalid_argument("narrow Schur continuation identity is invalid"));
        }
        residual_square.copy_from_slice(&state.square[..rhs_count]);
    } else {
        let left_codes = left.codes();
        let data = values.as_slice();
        graph
            .solution
            .par_chunks_mut(32768)
            .for_each(|part| part.fill(0.0));
        fill_two_fe_left_rhs(left, weights, &values, stride,
            &mut graph.left_rhs, &mut graph.left_weights)?;
        graph
            .residual
            .par_chunks_mut(stride)
            .zip(graph.right_weights.par_iter_mut())
            .enumerate()
            .try_for_each(|(level, (sum, total))| -> Result<()> {
                sum.fill(0.0);
                *total = 0.0;
                for &row in right.level_index().checked_members(level)? {
                    let weight = weights.normalized(row);
                    *total += weight;
                    let source = &data[row * rhs_count..(row + 1) * rhs_count];
                    for (out, &value) in sum[..rhs_count].iter_mut().zip(source) {
                        *out += weight * value;
                    }
                }
                Ok(())
            })?;
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
                for &row in right.level_index().checked_members(level)? {
                    let weight = weights.normalized(row);
                    let left_level = left_codes[row] as usize;
                    let source = &left_rhs[left_level * stride..left_level * stride + rhs_count];
                    for (out, &value) in sum[..rhs_count].iter_mut().zip(source) {
                        *out -= weight * value;
                    }
                }
                Ok(())
            })?;
        for level in 0..right_levels {
            let denominator = graph.right_weights[level];
            for (rhs, residual_sum) in residual_square.iter_mut().enumerate() {
                let index = level * stride + rhs;
                let residual = graph.residual[index];
                graph.direction[index] = residual / denominator;
                *residual_sum = residual.mul_add(graph.direction[index], *residual_sum);
            }
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
    let remaining_iterations = config
        .max_iterations
        .saturating_sub(completed_pilot_iterations);
    let map_fallback_reserve = remaining_iterations / 4;
    let pcg_iteration_limit = ols_cap.map_or_else(
        || config.max_iterations.saturating_sub(map_fallback_reserve),
        |cap| cap.min(config.max_iterations),
    );
    let mut iterations = completed_pilot_iterations;
    let mut active_rhs = 0_usize;
    // The chain-tuned worker cap belongs to the shared iterative path. Wide
    // OLS has passed its own contraction probe and benefits from the disjoint
    // state updates at larger team sizes as well.
    let parallel_recurrence = design.nobs() >= TWO_FE_PCG_PARALLEL_MIN_ROWS
        && rayon::current_num_threads() > 1
        && (ols_cap.is_some()
            || rayon::current_num_threads() <= TWO_FE_PCG_PARALLEL_RECURRENCE_MAX_THREADS);
    active_rhs_state.fill(0.0);
    for rhs in 0..rhs_count {
        if two_fe_pcg_rhs_ready(graph, certificate_workspace, rhs, stride, config.tolerance)? {
            residual_square[rhs] = 0.0;
            for level in 0..right_levels {
                let index = level * stride + rhs;
                graph.residual[index] = 0.0;
                graph.direction[index] = 0.0;
            }
        } else {
            active_rhs_state[rhs] = 1.0;
            active_rhs += 1;
        }
    }
    let mut reconstruct = active_rhs == 0;
    let mut breakdown = false;
    // Group-statistics scratch is idle until the final certificate. In eligible
    // unit-weight OLS solves, combine repeated pairs into counted graph edges.
    // The private bank stores packed u32 codes/counts as bits; no floating-point
    // arithmetic touches that representation before certification overwrites it.
    let cached_unit = if ols_cap.is_some() && weights.values.is_none()
        && rayon::current_num_threads() >= 8 {
        cache_unit_schur_edges(design, graph, &mut certificate_workspace.parallel_group_means, allow_ols_rhs_scratch)?
    } else { UnitSchurEdges::None };
    let weighted_neighbors = allow_ols_rhs_scratch && ols_cap.is_some() && weights.values.is_some()
        && active_rhs > 0 && rayon::current_num_threads() >= 8
        && cache_weighted_schur_right_edges(design, weights, graph,
            &mut certificate_workspace.parallel_group_means)?;
    let weighted_offsets = if weighted_neighbors {
        compact_weighted_schur_edges(design, graph, &mut certificate_workspace.parallel_group_means)?
    } else { None };
    let initialization_seconds = phase_start.elapsed().as_secs_f64();
    while !reconstruct && iterations < pcg_iteration_limit {
        let op_start = Instant::now();
        if cached_unit != UnitSchurEdges::None {
            let scratch = &certificate_workspace.parallel_group_means;
            let (left, right) = match cached_unit {
                UnitSchurEdges::BothScratch => {
                    let (left, right) = scratch[..2 * design.nobs()].split_at(design.nobs());
                    (Some(SchurLeftNeighbors::Counted(left)), right)
                },
                UnitSchurEdges::BothRhsScratch => (Some(SchurLeftNeighbors::RhsScratch), &scratch[..design.nobs()]),
                _ => (None, &scratch[..design.nobs()]),
            };
            graph.apply_schur_parallel(design, weights, rhs_count, stride, left, Some(SchurRightNeighbors::Counted(right)))?;
        } else if weighted_neighbors {
            let scratch = &certificate_workspace.parallel_group_means;
            let (left, right) = if let Some(offsets) = weighted_offsets {
                (Some(SchurLeftNeighbors::WeightedRhsScratch { weights: scratch, offsets: offsets.left }),
                    SchurRightNeighbors::WeightedCompactRhsScratch { weights: scratch, offsets: offsets.right })
            } else { (None, SchurRightNeighbors::WeightedRhsScratch(&scratch[..design.nobs()])) };
            graph.apply_schur_parallel(design, weights, rhs_count, stride, left, Some(right))?;
        } else {
            graph.apply_schur(design, weights, rhs_count, stride)?;
        }
        operator_seconds += op_start.elapsed().as_secs_f64();
        let rec_start = Instant::now();
        direction_projection.fill(0.0);
        for level in 0..right_levels {
            for (rhs, projection_sum) in direction_projection.iter_mut().enumerate() {
                let index = level * stride + rhs;
                *projection_sum =
                    graph.direction[index].mul_add(graph.operator[index], *projection_sum);
            }
        }
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
        if breakdown {
            break;
        }
        next_residual_square.fill(0.0);
        if parallel_recurrence {
            let level_chunk = parallel_level_chunk_size(right_levels);
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
                    for local_level in 0..solution.len() / stride {
                        for (rhs, &step) in ratio.iter().enumerate() {
                            let local = local_level * stride + rhs;
                            let global = chunk * cell_chunk + local;
                            solution[local] = step.mul_add(direction[global], solution[local]);
                            residual[local] = (-step).mul_add(operator[global], residual[local]);
                        }
                    }
                });
            // The state update is level-disjoint. Keep this scalar reduction
            // serial in canonical level/RHS order so thread count cannot alter
            // the recurrence or its stopping decision.
            for level in 0..right_levels {
                let denominator = graph.right_weights[level];
                for (rhs, residual_square) in next_residual_square.iter_mut().enumerate() {
                    let index = level * stride + rhs;
                    let preconditioned = graph.residual[index] / denominator;
                    *residual_square =
                        graph.residual[index].mul_add(preconditioned, *residual_square);
                }
            }
        } else {
            for level in 0..right_levels {
                let denominator = graph.right_weights[level];
                for (rhs, &step) in ratio.iter().enumerate() {
                    let index = level * stride + rhs;
                    graph.solution[index] =
                        step.mul_add(graph.direction[index], graph.solution[index]);
                    graph.residual[index] =
                        (-step).mul_add(graph.operator[index], graph.residual[index]);
                    let preconditioned = graph.residual[index] / denominator;
                    next_residual_square[rhs] =
                        graph.residual[index].mul_add(preconditioned, next_residual_square[rhs]);
                }
            }
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
                    config.tolerance,
                )?
            {
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
        if parallel_recurrence {
            let level_chunk = parallel_level_chunk_size(right_levels);
            let cell_chunk = level_chunk
                .checked_mul(stride)
                .ok_or_else(|| allocation("two-FE PCG direction chunk dimensions overflow"))?;
            let right_weights = &graph.right_weights;
            let residual = &graph.residual;
            graph
                .direction
                .par_chunks_mut(cell_chunk)
                .enumerate()
                .for_each(|(chunk, direction)| {
                    let first_level = chunk * level_chunk;
                    for local_level in 0..direction.len() / stride {
                        let denominator = right_weights[first_level + local_level];
                        for rhs in 0..rhs_count {
                            if active_rhs_state[rhs] == 0.0 {
                                continue;
                            }
                            let local = local_level * stride + rhs;
                            let global = chunk * cell_chunk + local;
                            let preconditioned = residual[global] / denominator;
                            direction[local] =
                                preconditioned.mul_add(1.0, ratio[rhs] * direction[local]);
                        }
                    }
                });
        } else {
            for level in 0..right_levels {
                let denominator = graph.right_weights[level];
                for rhs in 0..rhs_count {
                    if active_rhs_state[rhs] == 0.0 {
                        continue;
                    }
                    let index = level * stride + rhs;
                    let preconditioned = graph.residual[index] / denominator;
                    graph.direction[index] =
                        preconditioned.mul_add(1.0, ratio[rhs] * graph.direction[index]);
                }
            }
        }
        residual_square.copy_from_slice(next_residual_square);
        recurrence_seconds += rec_start.elapsed().as_secs_f64();
    }

    if ols_cap.is_some() && (active_rhs > 0 || breakdown) {
        return Ok(SchurAttempt::Continue {
            values,
            iterations,
            candidate: false,
        });
    }
    if reconstruct || iterations > completed_pilot_iterations {
        let reconstruct_start = Instant::now();
        let restore_left_means = weighted_neighbors || cached_unit == UnitSchurEdges::BothRhsScratch;
        reconstruct_two_fe_pcg_candidate(design, weights, graph, &mut values,
            rhs_count, stride, restore_left_means)?;
        let reconstruct_seconds = reconstruct_start.elapsed().as_secs_f64();
        let cert_start = Instant::now();
        let certificate = certify(
            design,
            weights,
            &values,
            config.tolerance,
            certificate_workspace,
        )?;
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
            result.report.pilot_contraction = pilot_contraction;
            return Ok(SchurAttempt::Certified(result));
        }
    }

    if ols_cap.is_some() {
        return Ok(SchurAttempt::Continue {
            values,
            iterations,
            candidate: true,
        });
    }
    // PCG is an acceleration only. A finite breakdown or a full-certificate
    // miss continues with ordinary symmetric MAP from the reconstructed
    // observation-space candidate. One quarter of the original iteration
    // budget remains reserved for this fail-safe continuation.
    for iteration in iterations.saturating_add(1)..=config.max_iterations {
        borrowed_residual_sweep_recording::<MapCoefficientAccumulator>(
            design,
            weights,
            &mut values,
            certificate_workspace,
            None,
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
                result.report.pilot_contraction = pilot_contraction;
                return Ok(SchurAttempt::Certified(result));
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

fn fill_two_fe_left_rhs(
    left: &crate::fe::BorrowedInterceptEffect<'_>, weights: ValidatedWeights<'_>,
    values: &DenseMatrix, stride: usize, rhs: &mut [f64], totals: &mut [f64],
) -> Result<()> {
    let width = values.ncols();
    let data = values.as_slice();
    rhs.par_chunks_mut(stride).zip(totals.par_iter_mut()).enumerate()
        .try_for_each(|(level, (sum, total))| -> Result<()> {
            sum.fill(0.0);
            *total = 0.0;
            for &row in left.level_index().checked_members(level)? {
                let weight = weights.normalized(row);
                *total += weight;
                let source = &data[row * width..(row + 1) * width];
                for (out, &value) in sum[..width].iter_mut().zip(source) {
                    *out += weight * value;
                }
            }
            Ok(())
        })
}

fn cache_weighted_schur_right_edges(
    design: &BorrowedInterceptFeDesign<'_>, weights: ValidatedWeights<'_>,
    graph: &mut TwoFePcgWorkspace, scratch: &mut [f64],
) -> Result<bool> {
    let n = design.nobs();
    if n == 0 || n > u32::MAX as usize || scratch.len() < n || graph.left_rhs.len() < n {
        return Ok(false);
    }
    let left = &design.effects()[graph.left_effect];
    let right = &design.effects()[graph.right_effect];
    if !has_repeated_schur_edges(left, right.codes())? { return Ok(false); }
    // Left RHS means are idle during the recurrence, and group scratch is idle
    // until certification. Their disjoint banks hold exact codes and f64 edge
    // weight sums without a new allocation or reduced weight precision.
    cache_weighted_schur_edge_groups(right, left.codes(), weights, 0..right.n_levels(), 0,
        &mut graph.left_rhs[..n], &mut scratch[..n],
        parallel_level_chunk_size(right.n_levels()).max(64))?;
    Ok(true)
}

#[derive(Clone, Copy, Debug)]
struct WeightedSchurOffsets { left: usize, right: usize }

fn compact_weighted_schur_edges(
    design: &BorrowedInterceptFeDesign<'_>, graph: &mut TwoFePcgWorkspace, weights: &mut [f64],
) -> Result<Option<WeightedSchurOffsets>> {
    let n = design.nobs();
    let left_levels = graph.left_weights.len();
    let right = &design.effects()[graph.right_effect];
    if weights.len() < n || graph.left_rhs.len() < n { return Ok(None); }
    let mut edges = 0_usize;
    for level in 0..right.n_levels() {
        for &weight in &weights[right.level_index().checked_member_range(level)?] {
            if weight == 0.0 { break; }
            edges += 1;
        }
    }
    let Some(both) = edges.checked_mul(2) else { return Ok(None); };
    // Offsets start past both the old N-slot representation and the compact
    // edges, so compaction cannot overwrite unread codes. Capacity rejection
    // leaves the previously built one-sided cache valid.
    let offset_start = n.max(both);
    let Some(left_start) = offset_start.checked_add(right.n_levels()).and_then(|v| v.checked_add(1))
        else { return Ok(None); };
    let Some(end) = left_start.checked_add(left_levels).and_then(|v| v.checked_add(1))
        else { return Ok(None); };
    if graph.left_rhs.len() < end || weights.len() < offset_start { return Ok(None); }
    let (codes, offsets) = graph.left_rhs[..end].split_at_mut(offset_start);
    let (right_offsets, left_offsets) = offsets.split_at_mut(right.n_levels() + 1);
    left_offsets.fill(f64::from_bits(0));
    let mut written = 0;
    for level in 0..right.n_levels() {
        right_offsets[level] = f64::from_bits(written as u64);
        for read in right.level_index().checked_member_range(level)? {
            let weight = weights[read];
            if weight == 0.0 { break; }
            let code = codes[read].to_bits() as usize;
            if code >= left_levels {
                return Err(invalid_argument("weighted Schur compact code exceeds the left level axis"));
            }
            codes[written] = f64::from_bits(code as u64);
            weights[written] = weight;
            left_offsets[code] = f64::from_bits(left_offsets[code].to_bits() + 1);
            written += 1;
        }
    }
    right_offsets[right.n_levels()] = f64::from_bits(edges as u64);
    debug_assert_eq!(written, edges);
    let mut next = edges;
    for offset in &mut left_offsets[..left_levels] {
        let count = offset.to_bits() as usize;
        *offset = f64::from_bits(next as u64);
        next += count;
    }
    left_offsets[left_levels] = f64::from_bits(both as u64);
    debug_assert_eq!(next, both);
    // Transpose the compact right edges into the disjoint second half. Both
    // gathers use the same f64 sum, with canonical neighboring-level order.
    for level in 0..right.n_levels() {
        let range = right_offsets[level].to_bits() as usize..right_offsets[level + 1].to_bits() as usize;
        for read in range {
            let code = codes[read].to_bits() as usize;
            let destination = left_offsets[code].to_bits() as usize;
            codes[destination] = f64::from_bits(level as u64);
            weights[destination] = weights[read];
            left_offsets[code] = f64::from_bits((destination + 1) as u64);
        }
    }
    for level in (1..left_levels).rev() { left_offsets[level] = left_offsets[level - 1]; }
    left_offsets[0] = f64::from_bits(edges as u64);
    Ok(Some(WeightedSchurOffsets { left: left_start, right: offset_start }))
}

#[allow(clippy::too_many_arguments)]
fn cache_weighted_schur_edge_groups(
    effect: &crate::fe::BorrowedInterceptEffect<'_>, other_codes: &[u32],
    weights: ValidatedWeights<'_>, levels: std::ops::Range<usize>, first_offset: usize,
    codes: &mut [f64], edge_weights: &mut [f64], grain: usize,
) -> Result<()> {
    if levels.len() > grain {
        let middle = levels.start + levels.len() / 2;
        let offset = effect.level_index().checked_member_range(middle)?.start;
        let (left_codes, right_codes) = codes.split_at_mut(offset - first_offset);
        let (left_weights, right_weights) = edge_weights.split_at_mut(offset - first_offset);
        let (a, b) = rayon::join(
            || cache_weighted_schur_edge_groups(effect, other_codes, weights, levels.start..middle,
                first_offset, left_codes, left_weights, grain),
            || cache_weighted_schur_edge_groups(effect, other_codes, weights, middle..levels.end,
                offset, right_codes, right_weights, grain),
        );
        a?;
        return b;
    }
    for level in levels {
        let range = effect.level_index().checked_member_range(level)?;
        let range = range.start - first_offset..range.end - first_offset;
        let neighbors = &mut codes[range.clone()];
        let sums = &mut edge_weights[range];
        for (out, &row) in neighbors.iter_mut().zip(effect.level_index().checked_members(level)?) {
            let code = *other_codes.get(row)
                .ok_or_else(|| invalid_argument("weighted Schur cache exceeds the observation axis"))?;
            *out = f64::from_bits(u64::from(code) << 32 | row as u64);
        }
        // Row bits break code ties, preserving original observation order within
        // each summed edge. Packed code/row words never undergo floating math.
        neighbors.sort_unstable_by_key(|value| value.to_bits());
        let mut read = 0;
        let mut written = 0;
        while read < neighbors.len() {
            let code = neighbors[read].to_bits() >> 32;
            let mut total = 0.0;
            while read < neighbors.len() && neighbors[read].to_bits() >> 32 == code {
                total += weights.normalized(neighbors[read].to_bits() as u32 as usize);
                read += 1;
            }
            if !total.is_finite() || total <= 0.0 {
                return Err(invalid_data("weighted Schur edge sum is invalid"));
            }
            neighbors[written] = f64::from_bits(code);
            sums[written] = total;
            written += 1;
        }
        sums[written..].fill(0.0);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnitSchurEdges { None, RightScratch, BothScratch, BothRhsScratch }

fn cache_unit_schur_edges(
    design: &BorrowedInterceptFeDesign<'_>, graph: &mut TwoFePcgWorkspace, scratch: &mut [f64],
    allow_idle_rhs: bool,
) -> Result<UnitSchurEdges> {
    let n = design.nobs();
    if n == 0 || n > u32::MAX as usize || scratch.len() < n {
        return Ok(UnitSchurEdges::None);
    }
    let banks = (scratch.len() / n).min(2);
    let left = &design.effects()[graph.left_effect];
    let right = &design.effects()[graph.right_effect];
    if banks == 1 && !has_repeated_schur_edges(left, right.codes())? {
        return Ok(UnitSchurEdges::None);
    }
    // If group scratch fits only one gather, OLS can temporarily reuse idle
    // left means for the other. Those means must be restored before the
    // observation-space reconstruction; shared reusable callers opt out.
    let mut cache = UnitSchurEdges::RightScratch;
    if banks == 2 {
        cache_unit_schur_edge_groups(left, right.codes(), 0..left.n_levels(), 0,
            &mut scratch[..n], parallel_level_chunk_size(left.n_levels()).max(64))?;
        cache = UnitSchurEdges::BothScratch;
    } else if allow_idle_rhs && graph.left_rhs.len() >= n {
        cache_unit_schur_edge_groups(left, right.codes(), 0..left.n_levels(), 0,
            &mut graph.left_rhs[..n], parallel_level_chunk_size(left.n_levels()).max(64))?;
        cache = UnitSchurEdges::BothRhsScratch;
    }
    cache_unit_schur_edge_groups(right, left.codes(), 0..right.n_levels(), 0,
        &mut scratch[(banks - 1) * n..banks * n],
        parallel_level_chunk_size(right.n_levels()).max(64))?;
    Ok(cache)
}

fn has_repeated_schur_edges(
    effect: &crate::fe::BorrowedInterceptEffect<'_>, other_codes: &[u32],
) -> Result<bool> {
    // A bounded, allocation-free sample avoids sorting a whole distinct-edge
    // graph for the new one-bank route. This is only a cost decision; skipped
    // caches use the same certified observation operator.
    let sample_levels = effect.n_levels().min(64);
    let mut sampled = 0_usize;
    let mut repeated = 0_usize;
    for index in 0..sample_levels {
        let level = index * (effect.n_levels() / sample_levels)
            + index * (effect.n_levels() % sample_levels) / sample_levels;
        let members = effect.level_index().checked_members(level)?;
        let members = &members[..members.len().min(32)];
        for (position, &row) in members.iter().enumerate() {
            let code = other_codes.get(row)
                .ok_or_else(|| invalid_argument("Schur cache sample exceeds the observation axis"))?;
            repeated += usize::from(members[..position].iter().any(|&previous| other_codes.get(previous) == Some(code)));
        }
        sampled += members.len();
    }
    Ok(sampled > 0 && repeated * 4 >= sampled)
}

fn cache_unit_schur_edge_groups(
    effect: &crate::fe::BorrowedInterceptEffect<'_>, other_codes: &[u32],
    levels: std::ops::Range<usize>, first_offset: usize, bank: &mut [f64], grain: usize,
) -> Result<()> {
    if levels.len() > grain {
        let middle = levels.start + levels.len() / 2;
        let offset = effect.level_index().checked_member_range(middle)?.start;
        let (left, right) = bank.split_at_mut(offset - first_offset);
        let (a, b) = rayon::join(
            || cache_unit_schur_edge_groups(effect, other_codes, levels.start..middle,
                first_offset, left, grain),
            || cache_unit_schur_edge_groups(effect, other_codes, middle..levels.end,
                offset, right, grain),
        );
        a?;
        return b;
    }
    for level in levels {
        let range = effect.level_index().checked_member_range(level)?;
        let neighbors = &mut bank[range.start - first_offset..range.end - first_offset];
        for (out, &row) in neighbors.iter_mut().zip(effect.level_index().checked_members(level)?) {
            let code = *other_codes.get(row)
                .ok_or_else(|| invalid_argument("Schur cache membership exceeds the observation axis"))?;
            *out = f64::from_bits(u64::from(code));
        }
        neighbors.sort_unstable_by_key(|value| value.to_bits());
        let mut read = 0;
        let mut written = 0;
        while read < neighbors.len() {
            let code = neighbors[read].to_bits();
            let mut end = read + 1;
            while end < neighbors.len() && neighbors[end].to_bits() == code { end += 1; }
            neighbors[written] = f64::from_bits(((end - read) as u64) << 32 | code);
            written += 1;
            read = end;
        }
        neighbors[written..].fill(0.0);
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum SchurLeftNeighbors<'a> {
    Counted(&'a [f64]),
    RhsScratch,
    WeightedRhsScratch { weights: &'a [f64], offsets: usize },
}

#[derive(Clone, Copy)]
enum SchurRightNeighbors<'a> {
    Counted(&'a [f64]),
    // Codes temporarily occupy left_rhs; this slice contains their weights.
    WeightedRhsScratch(&'a [f64]),
    WeightedCompactRhsScratch { weights: &'a [f64], offsets: usize },
}

impl TwoFePcgWorkspace {
    fn apply_schur(
        &mut self,
        design: &BorrowedInterceptFeDesign<'_>,
        weights: ValidatedWeights<'_>,
        rhs_count: usize,
        stride: usize,
    ) -> Result<()> {
        if design.nobs() >= TWO_FE_PCG_PARALLEL_MIN_ROWS && rayon::current_num_threads() > 1 {
            return self.apply_schur_parallel(design, weights, rhs_count, stride, None, None);
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
            for rhs in 0..rhs_count {
                self.left_work[left_level * stride + rhs] +=
                    weight * self.direction[right_level * stride + rhs];
            }
        }
        for (level, &denominator) in self.left_weights.iter().enumerate() {
            for rhs in 0..rhs_count {
                self.left_work[level * stride + rhs] /= denominator;
            }
        }
        for (level, &right_weight) in self.right_weights.iter().enumerate() {
            for rhs in 0..rhs_count {
                let index = level * stride + rhs;
                self.operator[index] = right_weight * self.direction[index];
            }
        }
        for row in 0..design.nobs() {
            let weight = weights.normalized(row);
            let left_level = left_codes[row] as usize;
            let right_level = right_codes[row] as usize;
            for rhs in 0..rhs_count {
                self.operator[right_level * stride + rhs] -=
                    weight * self.left_work[left_level * stride + rhs];
            }
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
        left_neighbors: Option<SchurLeftNeighbors<'_>>,
        right_neighbors: Option<SchurRightNeighbors<'_>>,
    ) -> Result<()> {
        let left = &design.effects()[self.left_effect];
        let right = &design.effects()[self.right_effect];
        let left_codes = left.codes();
        let right_codes = right.codes();
        let direction = &self.direction;
        let left_weights = &self.left_weights;
        let counted_left_neighbors = match left_neighbors {
            Some(SchurLeftNeighbors::Counted(values)) => Some(values),
            Some(SchurLeftNeighbors::RhsScratch) => Some(self.left_rhs.as_slice()),
            _ => None,
        };
        let weighted_neighbor_codes = &self.left_rhs;
        let left_level_chunk = parallel_level_chunk_size(left.n_levels());
        let left_cell_chunk = left_level_chunk
            .checked_mul(stride)
            .ok_or_else(|| allocation("two-FE PCG left operator chunk dimensions overflow"))?;
        self.left_work
            .par_chunks_mut(left_cell_chunk)
            .enumerate()
            .try_for_each(|(chunk, work)| -> Result<()> {
                work.fill(0.0);
                let first_level = chunk * left_level_chunk;
                for local_level in 0..work.len() / stride {
                    let level = first_level + local_level;
                    let level_work = &mut work[local_level * stride..(local_level + 1) * stride];
                    if let Some(left_neighbors) = counted_left_neighbors {
                        for &neighbor in &left_neighbors[left.level_index().checked_member_range(level)?] {
                            let packed = neighbor.to_bits();
                            let count = (packed >> 32) as f64;
                            if count == 0.0 { break; }
                            let right_level = packed as u32 as usize;
                            let source = &direction[right_level * stride..right_level * stride + rhs_count];
                            for (value, &term) in level_work[..rhs_count].iter_mut().zip(source) {
                                *value += count * term;
                            }
                        }
                    } else if let Some(SchurLeftNeighbors::WeightedRhsScratch { weights: edge_weights, offsets }) = left_neighbors {
                        let range = weighted_neighbor_codes[offsets + level].to_bits() as usize
                            ..weighted_neighbor_codes[offsets + level + 1].to_bits() as usize;
                        for (&code, &weight) in weighted_neighbor_codes[range.clone()].iter().zip(&edge_weights[range]) {
                            let right_level = code.to_bits() as usize;
                            let source = &direction[right_level * stride..right_level * stride + rhs_count];
                            for (value, &term) in level_work[..rhs_count].iter_mut().zip(source) {
                                *value += weight * term;
                            }
                        }
                    } else {
                        let members = left.level_index().checked_members(level)?;
                        for &row in members {
                            if row >= design.nobs() {
                                return Err(invalid_argument(
                                    "two-FE PCG membership exceeds the observation axis",
                                ));
                            }
                            let weight = weights.normalized(row);
                            let right_level = right_codes[row] as usize;
                            let source =
                                &direction[right_level * stride..right_level * stride + rhs_count];
                            for (value, &term) in level_work[..rhs_count].iter_mut().zip(source) {
                                *value += weight * term;
                            }
                        }
                    }
                    let denominator = left_weights[level];
                    for value in &mut level_work[..rhs_count] {
                        *value /= denominator;
                    }
                }
                Ok(())
            })?;

        let left_work = &self.left_work;
        let right_weights = &self.right_weights;
        let weighted_neighbor_codes = &self.left_rhs;
        let right_level_chunk = parallel_level_chunk_size(right.n_levels());
        let right_cell_chunk = right_level_chunk
            .checked_mul(stride)
            .ok_or_else(|| allocation("two-FE PCG right operator chunk dimensions overflow"))?;
        self.operator
            .par_chunks_mut(right_cell_chunk)
            .enumerate()
            .try_for_each(|(chunk, operator)| -> Result<()> {
                operator.fill(0.0);
                let first_level = chunk * right_level_chunk;
                for local_level in 0..operator.len() / stride {
                    let level = first_level + local_level;
                    let level_operator =
                        &mut operator[local_level * stride..(local_level + 1) * stride];
                    let source = &direction[level * stride..level * stride + rhs_count];
                    for (value, &term) in level_operator[..rhs_count].iter_mut().zip(source) {
                        *value = right_weights[level] * term;
                    }
                    if let Some(SchurRightNeighbors::Counted(right_neighbors)) = right_neighbors {
                        for &neighbor in &right_neighbors[right.level_index().checked_member_range(level)?] {
                            let packed = neighbor.to_bits();
                            let count = (packed >> 32) as f64;
                            if count == 0.0 { break; }
                            let left_level = packed as u32 as usize;
                            let source = &left_work[left_level * stride..left_level * stride + rhs_count];
                            for (value, &term) in level_operator[..rhs_count].iter_mut().zip(source) {
                                *value -= count * term;
                            }
                        }
                    } else if let Some(edge_weights) = match right_neighbors {
                        Some(SchurRightNeighbors::WeightedRhsScratch(values))
                        | Some(SchurRightNeighbors::WeightedCompactRhsScratch { weights: values, .. }) => Some(values),
                        _ => None,
                    } {
                        let range = match right_neighbors {
                            Some(SchurRightNeighbors::WeightedCompactRhsScratch { offsets, .. }) =>
                                weighted_neighbor_codes[offsets + level].to_bits() as usize
                                    ..weighted_neighbor_codes[offsets + level + 1].to_bits() as usize,
                            _ => right.level_index().checked_member_range(level)?,
                        };
                        for (&code, &weight) in weighted_neighbor_codes[range.clone()].iter().zip(&edge_weights[range]) {
                            if weight == 0.0 { break; }
                            let left_level = code.to_bits() as usize;
                            let source = &left_work[left_level * stride..left_level * stride + rhs_count];
                            for (value, &term) in level_operator[..rhs_count].iter_mut().zip(source) {
                                *value -= weight * term;
                            }
                        }
                    } else {
                        let members = right.level_index().checked_members(level)?;
                        for &row in members {
                            if row >= design.nobs() {
                                return Err(invalid_argument(
                                    "two-FE PCG membership exceeds the observation axis",
                                ));
                            }
                            let weight = weights.normalized(row);
                            let left_level = left_codes[row] as usize;
                            let source =
                                &left_work[left_level * stride..left_level * stride + rhs_count];
                            for (value, &term) in level_operator[..rhs_count].iter_mut().zip(source) {
                                *value -= weight * term;
                            }
                        }
                    }
                }
                Ok(())
            })?;
        if self.operator.iter().any(|value| !value.is_finite()) {
            Err(invalid_data(
                "two-FE categorical MAP PCG operator product is nonfinite",
            ))
        } else {
            Ok(())
        }
    }
}

fn two_fe_pcg_rhs_ready(
    graph: &TwoFePcgWorkspace,
    certificate_workspace: &BorrowedMapWorkspace,
    rhs: usize,
    stride: usize,
    tolerance: f64,
) -> Result<bool> {
    for level in 0..graph.right_weights.len() {
        let denominator = graph.right_weights[level];
        let mean = graph.residual[level * stride + rhs] / denominator;
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
        if !relative.is_finite() || relative > 0.5 * tolerance {
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
    restore_left_means: bool,
) -> Result<()> {
    let left = &design.effects()[graph.left_effect];
    let right = &design.effects()[graph.right_effect];
    let left_codes = left.codes();
    let right_codes = right.codes();
    let solution = &graph.solution;
    let left_weights = &graph.left_weights;
    if restore_left_means {
        // The edge cache is dead. Restore each original mean while traversing
        // the same immutable members for reconstruction. Both sums retain
        // their separate row-order arithmetic; original group totals remain
        // valid throughout PCG, so no weight-only restoration pass is needed.
        let data = values.as_slice();
        graph.left_work.par_chunks_mut(stride)
            .zip(graph.left_rhs.par_chunks_mut(stride))
            .enumerate()
            .try_for_each(|(level, (sum, mean))| -> Result<()> {
                sum.fill(0.0);
                mean.fill(0.0);
                for &row in left.level_index().checked_members(level)? {
                    let weight = weights.normalized(row);
                    let right_level = right_codes[row] as usize;
                    let original = &data[row * rhs_count..(row + 1) * rhs_count];
                    let source = &solution[right_level * stride..right_level * stride + rhs_count];
                    for (out, &value) in mean[..rhs_count].iter_mut().zip(original) {
                        *out += weight * value;
                    }
                    for (out, &value) in sum[..rhs_count].iter_mut().zip(source) {
                        *out += weight * value;
                    }
                }
                for (out, value) in sum[..rhs_count].iter_mut().zip(&mut mean[..rhs_count]) {
                    *value /= left_weights[level];
                    *out = *value - *out / left_weights[level];
                }
                Ok(())
            })?;
    } else {
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
    }
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

    // A direct one-partition projection has no iterative direction or update.
    let mut direction =
        fallible_matrix_zeros(target.len(), 1, "unable to reserve SD recovery direction")?;
    let mut increment = FeCoefficientBatch::intercept_zeros(design, 1)?;
    let mut dot = [0.0_f64; 1];
    let mut dot_correction = [0.0_f64; 1];
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
        None,
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

/// Prepared categorical fast lane selected from topology known before the
/// solve. It retains the automatic route's certified CG safety continuation,
/// but does not pay for a MAP pilot whose result cannot change this decision.
#[allow(clippy::too_many_arguments)]
pub(crate) fn absorb_borrowed_intercept_auto_direct_lsmr_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    lsmr: LsmrConfig,
    solver_memory_budget: Option<FeSolverMemoryBudget>,
    empty_design_is_structural_constant: bool,
    retain_coefficients: bool,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemWorkspaceReservation;
    absorb_auto_with_reservation(
        design,
        weights,
        values,
        config,
        AutoLsmrPlan::direct_lsmr(lsmr, solver_memory_budget),
        AutoRunOptions {
            empty_design_is_structural_constant,
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
    polish_borrowed_intercept_cg_with_products_certified(
        design, weights, values, config, reference_norms, false,
    )
}

pub(crate) fn polish_borrowed_intercept_cg_with_products_certified(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    reference_norms: &[f64],
    retain_coefficients: bool,
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
            retain_coefficients,
            reconstruct_projection: retain_coefficients,
            reconstruction_tolerance: None,
        },
        &mut reservation,
    )
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
    weighted_column_norms(
        values,
        weights,
        &mut workspace.reference_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
    let peak = workspace.payload_peak()?;
    let certificate = certify_initial(design, weights, values, config.tolerance, &mut workspace)?;
    Ok((certificate, peak))
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
    values: DenseMatrix,
    config: MapConfig,
    lsmr_plan: AutoLsmrPlan,
    empty_design_is_structural_constant: bool,
    retain_coefficients: bool,
    reservation: &mut R,
    direct_lsmr_cap: usize,
) -> Result<CertifiedBorrowedMap> {
    absorb_auto_with_prepared_lsmr(
        design,
        weights,
        values,
        config,
        lsmr_plan,
        empty_design_is_structural_constant,
        retain_coefficients,
        reservation,
        direct_lsmr_cap,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn absorb_auto_with_prepared_lsmr<'a, R: WorkspaceReservation>(
    design: &BorrowedInterceptFeDesign<'a>,
    weights: BorrowedMapWeights<'_>,
    mut values: DenseMatrix,
    config: MapConfig,
    lsmr_plan: AutoLsmrPlan,
    empty_design_is_structural_constant: bool,
    retain_coefficients: bool,
    reservation: &mut R,
    direct_lsmr_cap: usize,
    mut lsmr_prepared: Option<&mut BorrowedLsmrPrepared<'a>>,
) -> Result<CertifiedBorrowedMap> {
    let AutoLsmrPlan {
        config: lsmr_config,
        memory_budget: solver_memory_budget,
        skip_map_pilot,
        batch_policy,
    } = lsmr_plan;
    let solver_trace = std::env::var_os("FEREG_SOLVER_TRACE").is_some();
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
    let level_counts = design
        .effects()
        .iter()
        .map(|effect| effect.n_levels())
        .collect::<Vec<_>>();
    let solver_memory = estimate_fe_solver_memory_with_batch_policy(
        values.nrows(),
        values.ncols(),
        &level_counts,
        lsmr_config
            .map_or(rayon::current_num_threads(), |lsmr| lsmr.threads.resolved())
            .max(rayon::current_num_threads()),
        lsmr_config.and_then(|lsmr| lsmr.local_size),
        batch_policy,
    )?;
    let pilot_coefficients = if retain_coefficients {
        solver_memory.coefficient_batch_bytes
    } else {
        0
    };
    let pilot_workspace = BorrowedMapWorkspace::planned_payload_bytes(
        design,
        values.nrows(),
        values.ncols(),
        rayon::current_num_threads(),
    )?
    .checked_add(pilot_coefficients)
    .ok_or_else(|| allocation("automatic MAP pilot memory overflows"))?;
    let mut retained_operator = lsmr_prepared
        .as_ref()
        .map_or(0, |prepared| prepared.retained_bytes);
    if let Some(budget) = solver_memory_budget {
        let warm_peak = pilot_workspace
            .checked_add(retained_operator)
            .ok_or_else(|| allocation("retained LSMR pilot overlap overflows"))?;
        if !budget.fits(warm_peak) && retained_operator != 0 {
            // Release the reusable operator before a cold pilot when their
            // overlap no longer fits. The same certified solver remains available.
            if let Some(prepared) = lsmr_prepared.as_deref_mut() {
                prepared.solver = None;
                prepared.retained_bytes = 0;
            }
            retained_operator = 0;
        }
        if !budget.fits(pilot_workspace) {
            return Err(allocation(
                "automatic MAP pilot exceeds the joint execution/solver budget",
            ));
        }
    }
    let mut workspace =
        BorrowedMapWorkspace::new(design, values.nrows(), values.ncols(), reservation)?;
    let mut retained_coefficients = retain_coefficients
        .then(|| FeCoefficientBatch::intercept_zeros(design, values.ncols()))
        .transpose()?;
    let retained_coefficient_bytes = retained_coefficients
        .as_ref()
        .map_or(0, FeCoefficientBatch::retained_bytes);
    let mut attempted_peak = workspace
        .payload_peak()?
        .with_retained(retained_operator)?
        .with_retained(retained_coefficient_bytes)?;
    if solver_memory_budget
        .is_some_and(|budget| !budget.fits(attempted_peak.bytes().unwrap() as u128))
    {
        return Err(allocation(
            "automatic MAP reserved capacities exceed the joint budget",
        ));
    }
    weighted_column_norms(
        &values,
        weights,
        &mut workspace.reference_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
    )?;
    // The input is unchanged since its reference norms were computed above.
    // Copy those norms while freshly checking every FE moment; do not scan the
    // same matrix again solely to produce identical initial residual norms.
    let initial_certificate = certify_initial(design, weights, &values, config.tolerance, &mut workspace)?;
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
        let mut result = if let Some(coefficients) = retained_coefficients {
            let (residualized, report) = result.into_parts();
            CertifiedBorrowedMap::from_certified_parts_with_coefficients(
                residualized,
                report,
                coefficients,
            )?
        } else {
            result
        };
        result.set_solver_workspace_peak(attempted_peak);
        return mark_auto(result, 0, 0.0, 0.0, 0.0);
    }

    let initial_moment = initial_certificate.relative_two_norm;
    let pilot_budget = if skip_map_pilot {
        0
    } else {
        config.max_iterations.min(4)
    };
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
            let mut result = if let Some(coefficients) = retained_coefficients {
                let (residualized, report) = result.into_parts();
                CertifiedBorrowedMap::from_certified_parts_with_coefficients(
                    residualized,
                    report,
                    coefficients,
                )?
            } else {
                result
            };
            result.set_solver_workspace_peak(attempted_peak);
            return mark_auto(result, pilot_iteration, contraction, 0.0, 0.0);
        }
    }
    let pilot_seconds = pilot_started.elapsed().as_secs_f64();
    let contraction = if skip_map_pilot {
        0.0
    } else {
        pilot_contraction(initial_moment, final_pilot_certificate.relative_two_norm)?
    };
    if solver_trace {
        eprintln!(
            "FEREG_SOLVER_TRACE|phase=pilot|iterations={pilot_budget}|contraction={contraction:.17e}|initial_rel2={initial_moment:.17e}|pilot_rel2={:.17e}|setup={pilot_setup_seconds:.9}|solve={pilot_seconds:.9}|public_tolerance={:.17e}",
            final_pilot_certificate.relative_two_norm, config.tolerance,
        );
    }
    if pilot_budget == config.max_iterations {
        return Err(invalid_data(
            "borrowed auto exhausted its iteration budget during the MAP pilot",
        ));
    }

    // LSMR and any subsequent CG polish continue the original absorption
    // problem. Keep the raw RHS norms used by the pilot certificate:
    // recomputing the reference scale from the already-residualized pilot
    // state makes the same requested tolerance artificially stricter and can
    // erase the pilot's benefit on slow graphs.
    let original_reference_norms = std::mem::take(&mut workspace.reference_norms);
    drop(workspace);
    let original_norm_bytes = original_reference_norms.capacity() as u128 * 8;
    let mut remaining_iterations = config.max_iterations - pilot_budget;
    let mut candidate_failure = None;
    let mut attempted_lsmr = None;
    let total_fe_coefficients = design.effects().iter().try_fold(0_usize, |total, effect| {
        total
            .checked_add(effect.n_levels())
            .ok_or_else(|| invalid_data("borrowed auto FE coefficient count overflows usize"))
    })?;
    let coefficient_bytes = if retain_coefficients {
        solver_memory
            .coefficient_batch_bytes
            .checked_mul(5)
            .ok_or_else(|| allocation("automatic CG retained coefficient memory overflows"))?
    } else {
        0
    };
    let cg_workspace_bytes = solver_memory
        .cg_bytes
        .checked_add(coefficient_bytes)
        .and_then(|bytes| bytes.checked_add(original_norm_bytes))
        .ok_or_else(|| allocation("automatic CG workspace memory overflows"))?;
    let cg_fits = solver_memory_budget.is_none_or(|budget| budget.fits(cg_workspace_bytes));
    if let Some(lsmr_config) = lsmr_config {
        let resolved_preconditioner = resolve_borrowed_lsmr_preconditioner(
            design.effects().len(),
            total_fe_coefficients,
            lsmr_config.preconditioner,
        );
        let lsmr_workspace_bytes = solver_memory
            .lsmr_bytes(resolved_preconditioner)
            .checked_add(retained_coefficient_bytes)
            .and_then(|bytes| bytes.checked_add(original_norm_bytes))
            .ok_or_else(|| allocation("LSMR and pilot coefficient overlap overflows"))?;
        let lsmr_fits = solver_memory_budget.is_none_or(|budget| budget.fits(lsmr_workspace_bytes));
        let route = if skip_map_pilot {
            AutoLsmrRoute::Direct
        } else {
            auto_lsmr_route_at_cap(contraction, total_fe_coefficients, direct_lsmr_cap)
        };
        if route != AutoLsmrRoute::None && !lsmr_fits {
            candidate_failure = Some(
                "automatic LSMR route did not fit the joint execution/solver memory plan; selected CG"
                    .to_owned(),
            );
        } else {
            match route {
                AutoLsmrRoute::Direct => {
                    let mut memory = BorrowedLsmrCallMemory::with_batch_policy(
                        solver_memory,
                        resolved_preconditioner,
                        solver_memory_budget,
                        retained_coefficient_bytes + original_norm_bytes,
                        batch_policy,
                    )?;
                    let lsmr_tolerance = direct_lsmr_tolerance(config.tolerance);
                    let lsmr_started = solver_trace.then(Instant::now);
                    let solved = run_borrowed_lsmr_with_prepared(
                        design,
                        weights,
                        &values,
                        lsmr_tolerance,
                        remaining_iterations,
                        lsmr_config,
                        Some(&original_reference_norms),
                        retain_coefficients,
                        lsmr_prepared.as_deref_mut(),
                        &mut memory,
                    );
                    attempted_peak = attempted_peak.max(memory.peak);
                    match solved {
                        Ok((lsmr_values, mut report, owned_setup_seconds, lsmr_coefficients))
                            if report.certified() =>
                        {
                            if solver_trace {
                                eprintln!(
                                    "FEREG_SOLVER_TRACE|phase=lsmr|outcome=certified|tolerance={lsmr_tolerance:.17e}|iterations={}|stop={:?}|owned_setup={owned_setup_seconds:.9}|solver_setup={:.9}|solve={:.9}|elapsed={:.9}|absinf={:.17e}|relinf={:.17e}|rel2={:.17e}|normal={:.17e}",
                                    report.iterations,
                                    report.stop_reason,
                                    report.setup_seconds,
                                    report.solve_seconds,
                                    lsmr_started
                                        .map_or(0.0, |started| started.elapsed().as_secs_f64()),
                                    report.certificate.absolute_infinity_norm,
                                    report.certificate.relative_infinity_norm,
                                    report.certificate.relative_two_norm,
                                    report.normal_equation_residual,
                                );
                            }
                            report.solver_workspace_peak = attempted_peak;
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
                        Ok((lsmr_values, report, owned_setup_seconds, lsmr_coefficients)) => {
                            if solver_trace {
                                eprintln!(
                                    "FEREG_SOLVER_TRACE|phase=lsmr|outcome=uncertified|tolerance={lsmr_tolerance:.17e}|iterations={}|stop={:?}|owned_setup={owned_setup_seconds:.9}|solver_setup={:.9}|solve={:.9}|elapsed={:.9}|absinf={:.17e}|relinf={:.17e}|rel2={:.17e}|normal={:.17e}",
                                    report.iterations,
                                    report.stop_reason,
                                    report.setup_seconds,
                                    report.solve_seconds,
                                    lsmr_started
                                        .map_or(0.0, |started| started.elapsed().as_secs_f64()),
                                    report.certificate.absolute_infinity_norm,
                                    report.certificate.relative_infinity_norm,
                                    report.certificate.relative_two_norm,
                                    report.normal_equation_residual,
                                );
                            }
                            remaining_iterations = remaining_iterations
                                .checked_sub(report.iterations)
                                .filter(|remaining| *remaining > 0)
                                .ok_or_else(|| {
                                    invalid_data(
                                        "automatic LSMR continuation exhausted the shared iteration budget before CG polish",
                                    )
                                })?;
                            values = lsmr_values;
                            if let (Some(pilot), Some(lsmr)) =
                                (retained_coefficients.as_mut(), lsmr_coefficients.as_ref())
                            {
                                pilot.add_assign(lsmr, 1.0)?;
                            }
                            attempted_lsmr = Some(AutoLsmrAttemptAccounting {
                                iterations: report.iterations,
                                setup_seconds: owned_setup_seconds + report.setup_seconds,
                                solve_seconds: report.solve_seconds,
                                requested_preconditioner: report.requested_preconditioner,
                                used_preconditioner: report.used_preconditioner,
                            });
                            candidate_failure = Some(format!(
                                "automatic LSMR path did not certify after {} iterations; continued CG from the LSMR candidate",
                                report.iterations
                            ));
                        }
                        Err(error)
                            if matches!(
                                error.code,
                                ErrorCode::Allocation | ErrorCode::Interrupted
                            ) =>
                        {
                            return Err(error);
                        }
                        Err(error) => {
                            if solver_trace {
                                eprintln!(
                                    "FEREG_SOLVER_TRACE|phase=lsmr|outcome=error|tolerance={lsmr_tolerance:.17e}|elapsed={:.9}|error={error}",
                                    lsmr_started
                                        .map_or(0.0, |started| started.elapsed().as_secs_f64()),
                                );
                            }
                            candidate_failure =
                                Some(format!("automatic LSMR path failed ({error}); retried CG"));
                        }
                    }
                }
                AutoLsmrRoute::CgPolish => {
                    // This developer-only continuation keeps additional input
                    // banks; do not publish the ordinary-route envelope for it.
                    attempted_peak = SolverWorkspacePeak::UNMODELED;
                    let bulk_tolerance = lsmr_cg_bulk_tolerance(config.tolerance);
                    match run_borrowed_lsmr(
                        design,
                        weights,
                        &values,
                        bulk_tolerance,
                        remaining_iterations,
                        lsmr_config,
                        Some(&original_reference_norms),
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
                                    reconstruct_projection: false,
                                    reconstruction_tolerance: None,
                                },
                                reservation,
                            ) {
                                Ok(polished) => {
                                    let (residualized, mut report) = polished.into_parts();
                                    let polish_iterations = report.iterations;
                                    report.solver_workspace_peak = attempted_peak;
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
    if let Some(prepared) = lsmr_prepared {
        prepared.release_before_cg();
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
            reconstruct_projection: false,
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
    let (residualized, mut report, coefficients) = marked.into_full_parts();
    let cg_model = SolverWorkspacePeak::from_bytes(cg_workspace_bytes)?;
    let cg_peak = report
        .solver_workspace_peak
        .with_retained(original_norm_bytes + retained_coefficient_bytes)?;
    report.solver_workspace_peak = attempted_peak.max(cg_model).max(cg_peak);
    if let Some(attempt) = attempted_lsmr {
        report.iterations = report.iterations.saturating_add(attempt.iterations);
        report.setup_seconds += attempt.setup_seconds;
        report.solve_seconds += attempt.solve_seconds;
        report.requested_preconditioner = attempt.requested_preconditioner;
        report.used_preconditioner = attempt.used_preconditioner;
    }
    let had_candidate_failure = candidate_failure.is_some();
    if let Some(warning) = candidate_failure {
        report.fallback_used = true;
        report.warnings.insert(0, warning);
    }
    if solver_trace {
        eprintln!(
            "FEREG_SOLVER_TRACE|phase=cg|outcome=certified|iterations={}|stop={:?}|setup={:.9}|solve={:.9}|absinf={:.17e}|relinf={:.17e}|rel2={:.17e}|normal={:.17e}|after_lsmr_failure={}",
            report.iterations,
            report.stop_reason,
            report.setup_seconds,
            report.solve_seconds,
            report.certificate.absolute_infinity_norm,
            report.certificate.relative_infinity_norm,
            report.certificate.relative_two_norm,
            report.normal_equation_residual,
            had_candidate_failure,
        );
    }
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

fn borrowed_weights(weights: ValidatedWeights<'_>) -> BorrowedMapWeights<'_> {
    match weights.values {
        Some(values) => BorrowedMapWeights::Provided(values),
        None => BorrowedMapWeights::Unit,
    }
}

/// Own one fixed operator during low-memory OLS projection. The estimator drops
/// it before QR/VCE; a CG continuation releases it before allocating CG state.
pub(crate) struct BorrowedLsmrAbsorber<'a> {
    design: BorrowedInterceptFeDesign<'a>,
    weights: BorrowedMapWeights<'a>,
    config: super::AbsorbConfig,
    prepared: BorrowedLsmrPrepared<'a>,
}

impl<'a> BorrowedLsmrAbsorber<'a> {
    pub(crate) fn new(
        design: BorrowedInterceptFeDesign<'a>,
        weights: BorrowedMapWeights<'a>,
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
            weights,
            config,
            prepared: BorrowedLsmrPrepared::default(),
        })
    }

    pub(crate) fn absorb(&mut self, values: DenseMatrix) -> Result<CertifiedBorrowedMap> {
        if self.config.solver == FeSolverKind::Auto {
            let mut reservation = SystemWorkspaceReservation;
            return absorb_auto_with_prepared_lsmr(
                &self.design,
                self.weights,
                values,
                self.config.map,
                AutoLsmrPlan::direct_lsmr(self.config.lsmr, self.config.solver_memory_budget),
                false,
                self.config.retain_coefficients,
                &mut reservation,
                usize::MAX,
                Some(&mut self.prepared),
            );
        }
        let weights = validate_weights(self.weights, values.nrows())?;
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
            self.config
                .lsmr
                .threads
                .resolved()
                .max(rayon::current_num_threads()),
            self.config.lsmr.local_size,
        )?;
        let preconditioner = resolve_borrowed_lsmr_preconditioner(
            levels.len(),
            self.design.total_term_slots(),
            self.config.lsmr.preconditioner,
        );
        let mut memory = BorrowedLsmrCallMemory::new(
            estimate,
            preconditioner,
            self.config.solver_memory_budget,
            0,
        )?;
        let (residualized, report, _, coefficients) = run_borrowed_lsmr_with_prepared(
            &self.design,
            weights,
            &values,
            self.config.lsmr.tolerance,
            self.config.lsmr.max_iterations,
            self.config.lsmr,
            None,
            self.config.retain_coefficients,
            Some(&mut self.prepared),
            &mut memory,
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
}

// Keep the maximum even if postprocessing fails and auto continues to CG.
struct BorrowedLsmrCallMemory {
    batch_policy: LsmrBatchPolicy,
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
        Self::with_batch_policy(estimate, preconditioner, budget, parent_bytes, LsmrBatchPolicy::Default)
    }

    fn with_batch_policy(
        estimate: crate::memory::FeSolverMemoryEstimate,
        preconditioner: LsmrPreconditioner,
        budget: Option<FeSolverMemoryBudget>,
        parent_bytes: u128,
        batch_policy: LsmrBatchPolicy,
    ) -> Result<Self> {
        let mut memory = Self {
            batch_policy,
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
    fn release_before_cg(&mut self) {
        self.solver = None;
        self.retained_bytes = 0;
        self.disabled = true;
    }

    #[allow(clippy::too_many_arguments)]
    fn solve(
        &mut self,
        design: &BorrowedInterceptFeDesign<'a>,
        weights: ValidatedWeights<'_>,
        rhs: &[&[f64]],
        options: &LsmrOptions,
        requested: LsmrPreconditioner,
        preconditioner: PreconditionerConfig,
        batch_policy: LsmrBatchPolicy,
    ) -> Result<(within::BatchSolveResult, bool)> {
        let levels = design
            .effects()
            .iter()
            .map(|effect| effect.n_levels())
            .collect::<Vec<_>>();
        let memory = estimate_fe_solver_memory_with_batch_policy(
            design.nobs(),
            rhs.len(),
            &levels,
            rayon::current_num_threads(),
            options.local_size,
            batch_policy,
        )?;
        let resolved = resolve_borrowed_lsmr_preconditioner(
            levels.len(),
            design.total_term_slots(),
            requested,
        );
        let started = Instant::now();
        let mut setup_seconds = 0.0;
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
            batch_policy,
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
                    batch_policy,
                )
                .map_err(|error| {
                    error.into_fereg("borrowed modified LSMR failed after unpreconditioned fallback")
                })?
            }
            Err(error) => return Err(error.into_fereg("borrowed modified LSMR failed")),
        };
        self.batches += 1;
        solved.time_setup += setup_seconds;
        solved.time_total = started.elapsed().as_secs_f64();
        if std::env::var_os("FEREG_SOLVER_TRACE").is_some() {
            emit_solver_trace(&format!(
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
    within::Solver::new(effects, weights, preconditioner).map_err(|error| {
        FeregError::new(
            ErrorCode::Internal,
            format!("borrowed modified LSMR failed: {error}"),
        )
    })
}

fn lsmr_cg_bulk_tolerance(final_tolerance: f64) -> f64 {
    // The scale-free square-root target makes LSMR a reducer rather than a
    // second final solver. CG still receives the caller's exact tolerance and
    // the original RHS norms, so this cannot weaken publication criteria.
    final_tolerance.max(final_tolerance.sqrt())
}

#[allow(clippy::too_many_arguments)]
fn run_borrowed_lsmr(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    tolerance: f64,
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
        tolerance,
        max_iterations,
        lsmr_config,
        certificate_reference_norms,
        retain_coefficients,
        None,
        &mut memory,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_borrowed_lsmr_with_prepared<'a>(
    design: &BorrowedInterceptFeDesign<'a>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    tolerance: f64,
    max_iterations: usize,
    lsmr_config: LsmrConfig,
    certificate_reference_norms: Option<&[f64]>,
    retain_coefficients: bool,
    prepared: Option<&mut BorrowedLsmrPrepared<'a>>,
    memory: &mut BorrowedLsmrCallMemory,
) -> Result<(DenseMatrix, SolveReport, f64, Option<FeCoefficientBatch>)> {
    let total_started = Instant::now();
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
    let rhs = (0..values.ncols())
        .map(|column| values.column(column))
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
    let mut prepared = prepared.filter(|p| !p.disabled);
    let (mut solved, preconditioner_fallback) = if let Some(prepared) = prepared.as_deref_mut() {
        prepared.solve(
            design,
            weights,
            &rhs_views,
            &options,
            lsmr_config.preconditioner,
            preconditioner,
            memory.batch_policy,
        )?
    } else {
        let effects = borrowed_lsmr_effects(design)?;
        let initial = super::lsmr_batch::solve_batch_with_policy(
            effects.clone(),
            &rhs_views,
            weights.values,
            &options,
            preconditioner,
            memory.batch_policy,
        );
        match initial {
            Ok(solved) => (solved, false),
            Err(error)
                if lsmr_config.preconditioner == LsmrPreconditioner::Auto
                    && error.preconditioner_breakdown() =>
            {
                let solved = super::lsmr_batch::solve_batch_with_policy(
                    effects,
                    &rhs_views,
                    weights.values,
                    &options,
                    PreconditionerConfig::Off,
                    memory.batch_policy,
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
    for column in 0..lsmr_values.ncols() {
        for (row, value) in solved.demeaned(column).iter().copied().enumerate() {
            lsmr_values[(row, column)] = value;
        }
    }
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
    let certificate_bytes = certificate_workspace.payload_peak()?.bytes().unwrap() as u128;
    memory.include_payload(
        retained,
        (lsmr_values.storage_capacity() as u128 + solved.x.capacity() as u128) * 8
            + certificate_bytes,
    )?;
    weighted_column_norms(
        values,
        weights,
        &mut certificate_workspace.reference_norms,
        &mut certificate_workspace.value_scales,
        &mut certificate_workspace.group_sums,
    )?;
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
    let certificate = certify(
        design,
        weights,
        &lsmr_values,
        tolerance,
        &mut certificate_workspace,
    )?;
    let certification_seconds = certification_started.elapsed().as_secs_f64();
    if std::env::var_os("FEREG_SOLVER_TRACE").is_some() {
        for rhs in 0..lsmr_values.ncols() {
            emit_solver_trace(&format!(
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
        checkpoint_retries: 0,
        solver_workspace_peak: crate::memory::SolverWorkspacePeak::UNMODELED,
        requested_solver: FeSolverKind::Lsmr,
        used_solver: FeSolverKind::Lsmr,
        iterations: solved.iterations.iter().copied().max().unwrap_or(0),
        stop_reason,
        certificate,
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
        pilot_iterations: 0,
        pilot_contraction: 0.0,
        warnings,
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
        reconstruct_projection,
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
    borrowed_symmetric_projection_into_recording(
        design,
        weights,
        &values,
        &mut residual,
        &mut workspace,
        residual_coefficients.as_mut(),
    )?;
    if reconstruct_projection {
        reconstruct_intercept_product(design, residual_coefficients.as_ref()
            .ok_or_else(|| invalid_argument("stable CG projection requires coefficient products"))?, &mut residual)?;
    }
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
        if reconstruct_projection {
            reconstruct_intercept_product(design, projection_coefficients.as_ref()
                .ok_or_else(|| invalid_argument("stable CG projection requires coefficient products"))?, &mut projection)?;
        }
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
    // Keep validation ahead of weight checks and workspace admission. Wide,
    // fresh one-effect fits can scan on the already installed team without
    // changing the diagnostic or any numerical reduction order.
    let invalid_values = if skip_initial_one_effect_certificate
        && design.effects().len() == 1
        && values.nrows() >= 16_384
        && values.ncols() >= 8
        && rayon::current_thread_index().is_some()
        && rayon::current_num_threads() > 1
    {
        values.as_slice().par_iter().any(|value| !value.is_finite())
    } else {
        values.as_slice().iter().any(|value| !value.is_finite())
    };
    if invalid_values {
        return Err(invalid_data("borrowed MAP values must be finite"));
    }
    let weights = validate_weights(weights, values.nrows())?;

    let setup_started = Instant::now();
    let mut workspace =
        BorrowedMapWorkspace::new(design, values.nrows(), values.ncols(), reservation)?;
    workspace.ols_norm_reduction = skip_initial_one_effect_certificate;
    update_ols_workspace_norms(&values, weights, &mut workspace, true)?;
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

    if !skip_initial_one_effect_certificate {
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
    if rayon::current_num_threads() > 1
        && workspace.parallel_group_means.len() >= parallel_mean_count
        && (values.ncols() > 1 || values.nrows() >= PARALLEL_PROJECTION_MIN_ROWS)
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
            )?;
        } else {
            group_means(members, weights, values, workspace)?;
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
        )?;
    }

    let means = &workspace.parallel_group_means[..parallel_mean_count];
    if let Some((batch, effect_index)) = coefficients {
        record_parallel_group_means(batch, effect_index, means, levels, columns)?;
    }
    let codes = effect.codes();
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
    if FROM_INPUT || COMPLEMENT {
        let auxiliary = auxiliary.expect("validated auxiliary projection matrix");
        values
            .as_mut_slice()
            .par_chunks_mut(columns)
            .zip(auxiliary.as_slice().par_chunks(columns))
            .enumerate()
            .try_for_each(|(observation, (row, auxiliary))| {
                update_row(observation, row, Some(auxiliary))
            })?;
    } else {
        values
            .as_mut_slice()
            .par_chunks_mut(columns)
            .enumerate()
            .try_for_each(|(observation, row)| update_row(observation, row, None))?;
    }
    Ok(())
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
    let level_chunk = parallel_level_chunk_size(levels);
    let mean_chunk = level_chunk
        .checked_mul(columns)
        .ok_or_else(|| allocation("borrowed MAP parallel level chunk dimensions overflow"))?;
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
                .par_chunks_mut(mean_chunk)
                .zip(accumulated.par_chunks_mut(mean_chunk))
                .enumerate()
                .try_for_each(|(chunk, (means, accumulated))| -> Result<()> {
                    let first_level = chunk * level_chunk;
                    for local_level in 0..means.len() / columns {
                        let level = first_level + local_level;
                        let start = local_level * columns;
                        let level_means = &mut means[start..start + columns];
                        fused_group_means_for_level(
                            effect,
                            level,
                            weights,
                            values_read,
                            level_means,
                        )?;
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
                })?;
            recorded_inline = true;
        }
    }
    if !recorded_inline {
        workspace.parallel_group_means[..parallel_mean_count]
            .par_chunks_mut(mean_chunk)
            .enumerate()
            .try_for_each(|(chunk, means)| -> Result<()> {
                let first_level = chunk * level_chunk;
                for local_level in 0..means.len() / columns {
                    let start = local_level * columns;
                    fused_group_means_for_level(
                        effect,
                        first_level + local_level,
                        weights,
                        values_read,
                        &mut means[start..start + columns],
                    )?;
                }
                Ok(())
            })?;
    }

    let means = &workspace.parallel_group_means[..parallel_mean_count];
    if !recorded_inline {
        if let Some((batch, effect_index)) = coefficients {
            record_parallel_group_means(batch, effect_index, means, levels, columns)?;
        }
    }
    let codes = effect.codes();
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
    if FROM_INPUT || COMPLEMENT {
        let auxiliary = auxiliary.expect("validated auxiliary projection matrix");
        values
            .as_mut_slice()
            .par_chunks_mut(columns)
            .zip(auxiliary.as_slice().par_chunks(columns))
            .enumerate()
            .try_for_each(|(observation, (row, auxiliary))| {
                update_row(observation, row, Some(auxiliary))
            })?;
    } else {
        values
            .as_mut_slice()
            .par_chunks_mut(columns)
            .enumerate()
            .try_for_each(|(observation, row)| update_row(observation, row, None))?;
    }
    Ok(())
}

fn parallel_level_chunk_size(levels: usize) -> usize {
    let target_chunks = rayon::current_num_threads()
        .saturating_mul(PARALLEL_LEVEL_CHUNKS_PER_THREAD)
        .max(1);
    levels.div_ceil(target_chunks).max(1)
}

fn fill_parallel_group_statistics(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    group_weights: &mut [f64],
    means: &mut [f64],
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
    let level_chunk = parallel_level_chunk_size(levels);
    let mean_chunk = level_chunk
        .checked_mul(columns)
        .ok_or_else(|| allocation("borrowed MAP parallel level chunk dimensions overflow"))?;
    group_weights
        .par_chunks_mut(level_chunk)
        .zip(means.par_chunks_mut(mean_chunk))
        .enumerate()
        .try_for_each(|(chunk, (group_weights, means))| -> Result<()> {
            let first_level = chunk * level_chunk;
            for (local_level, group_weight) in group_weights.iter_mut().enumerate() {
                let level = first_level + local_level;
                let start = local_level * columns;
                let level_means = &mut means[start..start + columns];
                if columns <= PARALLEL_GROUP_MAX_RHS {
                    *group_weight =
                        fused_group_means_for_level(effect, level, weights, values, level_means)?;
                } else {
                    let members = effect.level_index().checked_members(level)?;
                    if members.is_empty() {
                        return Err(invalid_argument(
                            "borrowed MAP effect contains an empty final level",
                        ));
                    }
                    *group_weight = group_weight_sum(members, weights, values.nrows())?;
                    for (rhs, mean) in level_means.iter_mut().enumerate() {
                        *mean = group_mean_for_column_with_weight(
                            members,
                            rhs,
                            weights,
                            *group_weight,
                            values,
                        )?;
                    }
                }
            }
            Ok(())
        })
}

fn fused_group_means_for_level(
    effect: &crate::fe::BorrowedInterceptEffect<'_>,
    level: usize,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    means: &mut [f64],
) -> Result<f64> {
    fused_group_means_by_value(effect, level, weights, values.nrows(), means, |row, rhs| {
        values[(row, rhs)]
    })
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
    let mut weight_sum = 0.0_f64;
    let mut weight_correction = 0.0_f64;
    for &observation in members {
        if observation >= nrows {
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
        for (rhs, scale) in scales.iter_mut().enumerate().take(means.len()) {
            *scale = scale.max(value(observation, rhs).abs());
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
                        weight * (value(observation, rhs) / scales[rhs]),
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
                        value(observation, rhs) / scales[rhs],
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
    update_ols_workspace_norms(residuals, weights, workspace, false)?;
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

    let maximum_levels = design
        .effects()
        .iter()
        .map(crate::fe::BorrowedInterceptEffect::n_levels)
        .max()
        .unwrap_or(0);
    let maximum_mean_count = maximum_levels
        .checked_mul(residuals.ncols())
        .ok_or_else(|| allocation("borrowed MAP certificate dimensions overflow"))?;
    if !design.effects().is_empty()
        && residuals.nrows() >= PARALLEL_PROJECTION_MIN_ROWS
        && rayon::current_num_threads() > 1
        && workspace.parallel_group_weights.len() >= maximum_levels
        && workspace.parallel_group_means.len() >= maximum_mean_count
    {
        return certify_effects_parallel(design, weights, residuals, tolerance, workspace);
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

// Fixed group partitions retain canonical row/RHS arithmetic within each
// partial and a deterministic merge. Only the diagnostic two-norm changes
// summation order; every moment and the maximum used for acceptance is exact.
// Norm preparation has released this scratch, so there is no new allocation.
const OLS_CERTIFICATE_GROUP_CHUNK: usize = 1024;

fn ols_certificate_partials(
    group_weights: &[f64],
    means: &[f64],
    scales: &[f64],
    scratch: &mut [f64],
    raw_weight_scale: f64,
    rhs_maxima: &mut [f64],
) -> Result<(f64, f64)> {
    let columns = scales.len();
    let group_chunk = OLS_CERTIFICATE_GROUP_CHUNK;
    let partial_width = columns + 2;
    let partial_count = group_weights.len().div_ceil(group_chunk) * partial_width;
    let partials = &mut scratch[..partial_count];
    means
        .par_chunks(group_chunk * columns)
        .zip(group_weights.par_chunks(group_chunk))
        .zip(partials.par_chunks_mut(partial_width))
        .try_for_each(|((means, weights), partial)| -> Result<()> {
            partial.fill(0.0);
            for (row, &group_weight) in means.chunks(columns).zip(weights) {
                let root = group_weight.sqrt();
                if !root.is_finite() || root <= 0.0 {
                    return Err(invalid_data(
                        "borrowed MAP certificate group weight is invalid",
                    ));
                }
                for rhs in 0..columns {
                    let mean = row[rhs];
                    let denominator = scales[rhs];
                    let relative = if denominator == 0.0 {
                        if mean == 0.0 {
                            0.0
                        } else {
                            return Err(invalid_data(
                                "borrowed MAP zero-scale certificate has a nonzero moment",
                            ));
                        }
                    } else {
                        (mean.abs() / denominator) * root
                    };
                    if !relative.is_finite() {
                        return Err(invalid_data("borrowed MAP certificate ratio is not finite"));
                    }
                    let normalized = mean.abs() * group_weight;
                    let raw = normalized * raw_weight_scale;
                    partial[0] = partial[0].max(if raw.is_finite() {
                        raw
                    } else if normalized == 0.0 {
                        0.0
                    } else {
                        f64::INFINITY
                    });
                    partial[2 + rhs] = partial[2 + rhs].max(relative);
                    partial[1] = relative.mul_add(relative, partial[1]);
                }
            }
            if !partial[1].is_finite() {
                return Err(invalid_data(
                    "borrowed MAP certificate two-norm accumulation is not finite",
                ));
            }
            Ok(())
        })?;
    let mut absolute = 0.0_f64;
    let mut square_sum = 0.0_f64;
    for partial in partials.chunks_exact(partial_width) {
        absolute = absolute.max(partial[0]);
        square_sum += partial[1];
        for rhs in 0..columns {
            rhs_maxima[rhs] = rhs_maxima[rhs].max(partial[2 + rhs]);
        }
    }
    Ok((absolute, square_sum))
}

fn certify_effects_parallel(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    residuals: &DenseMatrix,
    tolerance: f64,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<FeCertificate> {
    let columns = residuals.ncols();
    let mut absolute_infinity_norm = 0.0_f64;
    let mut relative_infinity_norm = 0.0_f64;
    let mut relative_square_sum = 0.0_f64;
    let mut relative_count = 0_usize;
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
        )?;
        if workspace.ols_norm_reduction
            && ((16..=64).contains(&columns)
                || ((8..16).contains(&columns) && design.effects().len() <= 2
                    && mean_count >= 65_536))
            && workspace.parallel_dot_sums.len()
                >= levels.div_ceil(OLS_CERTIFICATE_GROUP_CHUNK) * (columns + 2)
        {
            let (absolute, square) = ols_certificate_partials(
                &workspace.parallel_group_weights[..levels],
                &workspace.parallel_group_means[..mean_count],
                &workspace.rhs_scales,
                &mut workspace.parallel_dot_sums,
                weights.raw_scale,
                &mut workspace.rhs_relative_infinity_norms,
            )?;
            absolute_infinity_norm = absolute_infinity_norm.max(absolute);
            relative_square_sum += square;
            if !relative_square_sum.is_finite() {
                return Err(invalid_data(
                    "borrowed MAP certificate two-norm accumulation is not finite",
                ));
            }
            relative_count = relative_count
                .checked_add(mean_count)
                .ok_or_else(|| allocation("borrowed MAP certificate moment count overflows"))?;
            for &relative in &workspace.rhs_relative_infinity_norms {
                relative_infinity_norm = relative_infinity_norm.max(relative);
            }
            continue;
        }
        let group_weights = &workspace.parallel_group_weights[..levels];
        let means = &workspace.parallel_group_means[..mean_count];
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

/// Form I-S from its FE contributions, not by subtracting two observation
/// vectors. The latter introduces off-model-space cancellation error that CG
/// can amplify along weak graph directions despite passing FE normal equations.
/// This PPML action reuses the admitted mean buffers and retains no coefficient
/// history; ordinary OLS projection arithmetic remains unchanged.
fn borrowed_symmetric_projection_from_group_means(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    input: &DenseMatrix,
    projection: &mut DenseMatrix,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<()> {
    let columns = input.ncols();
    if projection.nrows() != input.nrows()
        || projection.ncols() != columns
        || input.nrows() != design.nobs()
        || columns == 0
        || columns > PARALLEL_GROUP_MAX_RHS
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
        if rayon::current_num_threads() > 1
            && workspace.parallel_group_means.len() >= cells
            && (columns > 1 || input.nrows() >= PARALLEL_PROJECTION_MIN_ROWS)
        {
            let chunk = parallel_level_chunk_size(effect.n_levels()) * columns;
            workspace.parallel_group_means[..cells]
                .par_chunks_mut(chunk)
                .enumerate()
                .try_for_each(|(part, means)| -> Result<()> {
                    for (local, means) in means.chunks_mut(columns).enumerate() {
                        fused_group_means_by_value(
                            effect,
                            part * (chunk / columns) + local,
                            weights,
                            input.nrows(),
                            means,
                            |row, rhs| input[(row, rhs)] - projection[(row, rhs)],
                        )?;
                    }
                    Ok(())
                })?;
            let means = &workspace.parallel_group_means[..cells];
            projection
                .as_mut_slice()
                .par_chunks_mut(columns)
                .enumerate()
                .try_for_each(|(row, values)| -> Result<()> {
                    let start = effect.codes()[row] as usize * columns;
                    for (value, mean) in values.iter_mut().zip(&means[start..start + columns]) {
                        *value += mean;
                        if !value.is_finite() {
                            return Err(invalid_data("categorical projection action is nonfinite"));
                        }
                    }
                    Ok(())
                })?;
        } else {
            let mut means = [0.0; PARALLEL_GROUP_MAX_RHS];
            for level in 0..effect.n_levels() {
                fused_group_means_by_value(
                    effect,
                    level,
                    weights,
                    input.nrows(),
                    &mut means[..columns],
                    |row, rhs| input[(row, rhs)] - projection[(row, rhs)],
                )?;
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

fn reconstruct_intercept_product(
    design: &BorrowedInterceptFeDesign<'_>,
    coefficients: &FeCoefficientBatch,
    values: &mut DenseMatrix,
) -> Result<()> {
    if coefficients.rhs_count() != values.ncols() || values.nrows() != design.nobs()
        || coefficients.effect_channels().len() != design.effects().len()
        || design.effects().iter().enumerate().any(|(effect, source)|
            coefficients.effect_channels()[effect] != 1 || coefficients.level_counts()[effect] != source.n_levels()) {
        return Err(invalid_argument("CG projection product layout differs from its design"));
    }
    // Near the within space, input - projected_input can amplify subtraction
    // noise into a spurious CG direction. Reconstruct from the recorded group
    // means, so the operator output stays in the intended FE span.
    let width = values.ncols();
    values.as_mut_slice().par_chunks_mut(4096 * width).enumerate()
        .try_for_each(|(block, output)| -> Result<()> {
            for (local, row) in output.chunks_exact_mut(width).enumerate() {
                let observation = block * 4096 + local;
                for (rhs, value) in row.iter_mut().enumerate() {
                    let product = coefficients.rhs(rhs)?;
                    *value = 0.0;
                    for (effect, source) in design.effects().iter().enumerate() {
                        *value += product[coefficients.effect_offsets[effect] + source.codes()[observation] as usize];
                    }
                    if !value.is_finite() { return Err(invalid_data("CG projection product is nonfinite")); }
                }
            }
            Ok(())
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

fn borrowed_irons_tuck_candidate_in_place(
    x0: &mut DenseMatrix,
    x1: &DenseMatrix,
    x2: &DenseMatrix,
    d1d1: &mut [f64],
    d1d2: &mut [f64],
    d2d2: &mut [f64],
) -> Result<bool> {
    if x0.nrows() != x1.nrows()
        || x0.ncols() != x1.ncols()
        || x0.nrows() != x2.nrows()
        || x0.ncols() != x2.ncols()
    {
        return Err(invalid_argument(
            "reusable Irons-Tuck states have inconsistent dimensions",
        ));
    }
    if d1d1.len() != x0.ncols() || d1d2.len() != x0.ncols() || d2d2.len() != x0.ncols() {
        return Err(invalid_argument(
            "reusable Irons-Tuck scalar workspaces have inconsistent dimensions",
        ));
    }
    d1d1.fill(0.0);
    d1d2.fill(0.0);
    d2d2.fill(0.0);
    for (index, ((a, b), c)) in x0
        .as_slice()
        .iter()
        .zip(x1.as_slice())
        .zip(x2.as_slice())
        .enumerate()
    {
        let rhs = index % x0.ncols();
        let d1 = b - a;
        let d2 = c - 2.0 * b + a;
        d1d1[rhs] = d1.mul_add(d1, d1d1[rhs]);
        d1d2[rhs] = d1.mul_add(d2, d1d2[rhs]);
        d2d2[rhs] = d2.mul_add(d2, d2d2[rhs]);
    }
    let mut any = false;
    for rhs in 0..x0.ncols() {
        d1d1[rhs] = if d2d2[rhs].is_finite() && d2d2[rhs] > f64::EPSILON * d1d1[rhs].max(1.0) {
            let alpha = (-d1d2[rhs] / d2d2[rhs]).clamp(-64.0, 64.0);
            any |= alpha.is_finite();
            alpha
        } else {
            f64::NAN
        };
    }
    if !any {
        return Ok(false);
    }
    let columns = x0.ncols();
    for (index, ((output, b), c)) in x0
        .as_mut_slice()
        .iter_mut()
        .zip(x1.as_slice())
        .zip(x2.as_slice())
        .enumerate()
    {
        let alpha = d1d1[index % columns];
        if !alpha.is_finite() {
            *output = *c;
            continue;
        }
        let a = *output;
        let d1 = b - a;
        let d2 = c - 2.0 * b + a;
        *output = a + 2.0 * alpha * d1 + alpha * alpha * d2;
        if !output.is_finite() {
            return Ok(false);
        }
    }
    Ok(true)
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

#[allow(clippy::too_many_arguments)]
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
    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
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

#[allow(clippy::too_many_arguments)]
fn cg_update_solution_residual_and_weighted_norm(
    values: &mut DenseMatrix,
    residual: &mut DenseMatrix,
    direction: &DenseMatrix,
    projection: &DenseMatrix,
    alpha: &[f64],
    weights: ValidatedWeights<'_>,
    output: &mut [f64],
    corrections: &mut [f64],
    parallel_sums: &mut [f64],
) -> Result<()> {
    if alpha.len() != values.ncols()
        || residual.nrows() != values.nrows()
        || residual.ncols() != values.ncols()
        || direction.nrows() != values.nrows()
        || direction.ncols() != values.ncols()
        || projection.nrows() != values.nrows()
        || projection.ncols() != values.ncols()
        || output.len() != values.ncols()
        || corrections.len() != values.ncols()
    {
        return Err(invalid_argument("borrowed CG update dimensions differ"));
    }
    output.fill(0.0);
    corrections.fill(0.0);
    let columns = values.ncols();
    let chunks = values.nrows().div_ceil(PARALLEL_DOT_CHUNK_ROWS);
    let parallel_elements = chunks
        .checked_mul(columns)
        .ok_or_else(|| allocation("borrowed CG parallel dot dimensions overflow"))?;
    if values.nrows() >= PARALLEL_PROJECTION_MIN_ROWS
        && columns <= FUSED_PARALLEL_MAX_RHS
        && rayon::current_num_threads() > 1
        && parallel_sums.len() >= parallel_elements
    {
        let row_chunk = PARALLEL_DOT_CHUNK_ROWS
            .checked_mul(columns)
            .ok_or_else(|| allocation("borrowed CG parallel row chunk dimensions overflow"))?;
        parallel_sums[..parallel_elements]
            .par_chunks_mut(columns)
            .zip(values.as_mut_slice().par_chunks_mut(row_chunk))
            .zip(residual.as_mut_slice().par_chunks_mut(row_chunk))
            .zip(direction.as_slice().par_chunks(row_chunk))
            .zip(projection.as_slice().par_chunks(row_chunk))
            .enumerate()
            .try_for_each(
                |(chunk, ((((sums, values), residual), direction), projection))| -> Result<()> {
                    sums.fill(0.0);
                    let mut chunk_corrections = [0.0_f64; FUSED_PARALLEL_MAX_RHS];
                    let first_row = chunk * PARALLEL_DOT_CHUNK_ROWS;
                    for local_row in 0..values.len() / columns {
                        let weight = weights.normalized(first_row + local_row);
                        let offset = local_row * columns;
                        for rhs in 0..columns {
                            let index = offset + rhs;
                            values[index] -= alpha[rhs] * direction[index];
                            residual[index] -= alpha[rhs] * projection[index];
                            if !values[index].is_finite() || !residual[index].is_finite() {
                                return Err(invalid_data(
                                    "borrowed CG update produced a nonfinite value",
                                ));
                            }
                            let term = weight * residual[index] * residual[index];
                            if !term.is_finite() {
                                return Err(invalid_data(
                                    "borrowed CG weighted inner product produced a nonfinite term",
                                ));
                            }
                            compensated_add(term, &mut sums[rhs], &mut chunk_corrections[rhs])?;
                        }
                    }
                    for rhs in 0..columns {
                        sums[rhs] += chunk_corrections[rhs];
                        if !sums[rhs].is_finite() {
                            return Err(invalid_data(
                                "borrowed CG weighted inner product is nonfinite",
                            ));
                        }
                    }
                    Ok(())
                },
            )?;
        for chunk in 0..chunks {
            for rhs in 0..columns {
                compensated_add(
                    parallel_sums[chunk * columns + rhs],
                    &mut output[rhs],
                    &mut corrections[rhs],
                )?;
            }
        }
        for rhs in 0..columns {
            output[rhs] += corrections[rhs];
            if !output[rhs].is_finite() {
                return Err(invalid_data(
                    "borrowed CG weighted inner product is nonfinite",
                ));
            }
        }
        return Ok(());
    }
    if values.nrows() >= PARALLEL_PROJECTION_MIN_ROWS && rayon::current_num_threads() > 1 {
        cg_update_solution_and_residual(values, residual, direction, projection, alpha)?;
        return weighted_dot_columns_into(
            residual,
            residual,
            weights,
            output,
            corrections,
            parallel_sums,
        );
    }
    for row in 0..values.nrows() {
        let weight = weights.normalized(row);
        for rhs in 0..columns {
            values[(row, rhs)] -= alpha[rhs] * direction[(row, rhs)];
            residual[(row, rhs)] -= alpha[rhs] * projection[(row, rhs)];
            if !values[(row, rhs)].is_finite() || !residual[(row, rhs)].is_finite() {
                return Err(invalid_data(
                    "borrowed CG update produced a nonfinite value",
                ));
            }
            let term = weight * residual[(row, rhs)] * residual[(row, rhs)];
            if !term.is_finite() {
                return Err(invalid_data(
                    "borrowed CG weighted inner product produced a nonfinite term",
                ));
            }
            compensated_add(term, &mut output[rhs], &mut corrections[rhs])?;
        }
    }
    for rhs in 0..columns {
        output[rhs] += corrections[rhs];
        if !output[rhs].is_finite() {
            return Err(invalid_data(
                "borrowed CG weighted inner product is nonfinite",
            ));
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
    if values.ncols() == 1 && weights.values.is_none() {
        // Unit normalization is exactly one. Preserve the original scaled
        // recurrence and row order, with scalar state for the single column.
        let mut scale = 0.0_f64;
        let mut square_sum = 1.0_f64;
        for &value in values.as_slice() {
            if !value.is_finite() {
                return Err(invalid_data("borrowed MAP weighted value is not finite"));
            }
            update_scaled_sum_of_squares(value, &mut scale, &mut square_sum);
        }
        scales[0] = scale;
        scaled_square_sums[0] = square_sum;
    } else {
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
        checkpoint_retries: 0,
        solver_workspace_peak: crate::memory::SolverWorkspacePeak::UNMODELED,
        requested_solver: solver,
        used_solver: solver,
        iterations,
        stop_reason,
        normal_equation_residual,
        certificate,
        setup_seconds,
        solve_seconds,
        certification_seconds: 0.0,
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

pub(crate) const OLS_SCHUR_PROBE_ITERATIONS: usize = 8;
pub(crate) const OLS_SCHUR_MAX_ITERATIONS: usize = 64;
pub(crate) const OLS_SCHUR_MAX_CONTRACTION: f64 = 0.01;

#[derive(Default)]
struct CheckpointSchurCost {
    probe_iterations: usize,
    iterations: usize,
    contraction: f64,
    setup_seconds: f64,
    solve_seconds: f64,
    rejected: bool,
}

impl CheckpointSchurCost {
    fn add_to(&self, report: &mut SolveReport) {
        if self.probe_iterations == 0 {
            return;
        }
        report.iterations = report
            .iterations
            .saturating_add(self.probe_iterations + self.iterations);
        report.setup_seconds += self.setup_seconds;
        report.solve_seconds += self.solve_seconds;
        report.pilot_iterations = self.probe_iterations;
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
        if std::env::var_os("FEREG_SOLVER_TRACE").is_some() {
            emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=checkpoint_schur_attempt|outcome={outcome}|rows={}|rhs={}|probe_budget={}|contraction={:.17e}|iterations={}|setup_seconds={:.17e}|solve_seconds={:.17e}",
                values.nrows(), values.ncols(), self.probe_iterations, self.contraction,
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

// Values remain authoritative input until reconstruction. A failed final
// certificate can instead hand off its finite observation-space candidate.
enum SchurAttempt {
    Certified(CertifiedBorrowedMap),
    Continue {
        values: DenseMatrix,
        iterations: usize,
        candidate: bool,
    },
}


struct NarrowSchurState {
    rhs_count: usize,
    square: [f64; 8],
}

pub(crate) enum NarrowOlsSchurAttempt {
    Certified(CertifiedBorrowedMap),
    Rejected {
        values: DenseMatrix,
        contraction: f64,
        probe_seconds: f64,
        peak: SolverWorkspacePeak,
    },
}

/// Same-call narrow pilot and certified solve. Original values cannot leave
/// this owner as a candidate until observation-space reconstruction certifies.
pub(crate) fn try_absorb_ols_schur_narrow(
    design: BorrowedInterceptFeDesign<'_>, weights: BorrowedMapWeights<'_>,
    values: DenseMatrix, config: crate::fe::AbsorbConfig,
) -> Result<NarrowOlsSchurAttempt> {
    try_absorb_ols_schur_narrow_at_cap(design, weights, values, config, OLS_SCHUR_MAX_ITERATIONS)
}

fn try_absorb_ols_schur_narrow_at_cap(
    design: BorrowedInterceptFeDesign<'_>, weights: BorrowedMapWeights<'_>,
    values: DenseMatrix, mut config: crate::fe::AbsorbConfig, cap: usize,
) -> Result<NarrowOlsSchurAttempt> {
    validate_config(config.map)?;
    if cap > OLS_SCHUR_MAX_ITERATIONS || design.effects().len() != 2 || design.nobs() < 16_384
        || !(3..=8).contains(&values.ncols()) || design.nobs() != values.nrows()
        || rayon::current_num_threads() < 8
        || config.map.max_iterations <= OLS_SCHUR_PROBE_ITERATIONS + OLS_SCHUR_MAX_ITERATIONS {
        return Err(invalid_argument("narrow OLS Schur pilot dimensions are unsupported"));
    }
    let started = Instant::now();
    let planned = schur_ols_workspace_bytes(&design, values.ncols(), rayon::current_num_threads())?;
    if config.solver_memory_budget.is_some_and(|budget| !budget.fits(planned)) {
        return Err(allocation("narrow OLS Schur pilot exceeds the joint memory budget"));
    }
    let peak = SolverWorkspacePeak::from_bytes(planned)?;
    let validated = validate_weights(weights, values.nrows())?;
    let mut absorber = ReusableCategoricalMapAbsorber::new(design, values.ncols(), rayon::current_num_threads())?;
    absorber.workspace.set_rhs_count(values.ncols())?;
    absorber.workspace.ols_norm_reduction = true;
    update_ols_workspace_norms(&values, validated, &mut absorber.workspace, true)?;
    let mut graph = absorber.two_fe_pcg.take()
        .ok_or_else(|| invalid_argument("narrow OLS Schur workspace unavailable"))?;
    let mut map = config.map;
    map.tolerance = crate::fe::auto::direct_lsmr_tolerance(map.tolerance);
    let (contraction, state) = schur_probe_narrow_continuation(&absorber.design, validated,
        &values, &mut graph, absorber.max_rhs, &absorber.workspace, map.tolerance)?;
    let probe_seconds = started.elapsed().as_secs_f64();
    if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
        eprintln!("FEREG_PHASE|schur_route|contraction={contraction:.9}|probe={probe_seconds:.9}|selected={}", contraction <= OLS_SCHUR_MAX_CONTRACTION);
        eprintln!("FEREG_PHASE|schur_warm_pilot|width={}|retained={}", values.ncols(), state.is_some() && contraction <= OLS_SCHUR_MAX_CONTRACTION);
    }
    if contraction > OLS_SCHUR_MAX_CONTRACTION || !contraction.is_finite() {
        return Ok(NarrowOlsSchurAttempt::Rejected { values, contraction, probe_seconds, peak });
    }
    let retain_fallback_coefficients = config.retain_coefficients;
    config.retain_coefficients = false;
    let Some(state) = state else {
        let empty = BorrowedInterceptFeDesign::new(absorber.design.nobs(), Vec::new())?;
        let design = std::mem::replace(&mut absorber.design, empty);
        drop(graph);
        drop(absorber);
        config.retain_coefficients = retain_fallback_coefficients;
        return absorb_ols_schur_at_cap(design, weights, values, config, probe_seconds, contraction, cap)
            .map(NarrowOlsSchurAttempt::Certified);
    };
    let solve_started = Instant::now();
    let outcome = absorb_two_fe_pcg_reusing(&absorber.design, validated, values, map,
        OLS_SCHUR_PROBE_ITERATIONS, contraction, probe_seconds, solve_started,
        absorber.max_rhs, &mut graph, &mut absorber.workspace,
        &mut absorber.cg_residual_square, &mut absorber.cg_initial_residual_square,
        &mut absorber.cg_next_residual_square, &mut absorber.cg_direction_projection,
        &mut absorber.cg_ratio, Some(cap + OLS_SCHUR_PROBE_ITERATIONS),
        true, Some(&state))?;
    finish_ols_schur(absorber, graph, weights, config, map, peak, probe_seconds,
        solve_started, 0.0, contraction, OLS_SCHUR_PROBE_ITERATIONS, 0, outcome,
        retain_fallback_coefficients)
        .map(NarrowOlsSchurAttempt::Certified)
}

pub(crate) fn schur_ols_workspace_bytes(
    design: &BorrowedInterceptFeDesign<'_>,
    rhs: usize,
    threads: usize,
) -> Result<u128> {
    if design.effects().len() != 2 || !(1..=64).contains(&rhs) || threads == 0 {
        return Err(invalid_argument(
            "OLS Schur workspace dimensions are unsupported",
        ));
    }
    let left = design.effects().iter().map(|e| e.n_levels()).max().unwrap() as u128;
    let right = design.effects().iter().map(|e| e.n_levels()).min().unwrap() as u128;
    let width = rhs as u128;
    let parallel = design.nobs() >= PARALLEL_PROJECTION_MIN_ROWS && threads > 1;
    let mut elements = left + right + (2 * left + 4 * right) * width + 12 * width;
    if parallel {
        elements +=
            left * width + left + design.nobs().div_ceil(PARALLEL_DOT_CHUNK_ROWS) as u128 * width;
    }
    // The three-column probe is released before the full session is constructed.
    let probe_elements = left + right + (2 * left + 4 * right) * 3;
    Ok(8 * elements.max(probe_elements))
}

#[cfg(test)]
pub(crate) fn schur_ols_probe(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: &DenseMatrix,
) -> Result<f64> {
    schur_ols_probe_with_workspace(design, weights, values).map(|(contraction, _)| contraction)
}

pub(crate) fn schur_ols_probe_with_workspace(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: &DenseMatrix,
) -> Result<(f64, SolverWorkspacePeak)> {
    if design.effects().len() != 2 || design.nobs() != values.nrows() || values.ncols() < 3 {
        return Err(invalid_argument(
            "OLS Schur probe dimensions are unsupported",
        ));
    }
    // Admission must see the same normalized operator as the full solve:
    // weak weighted cuts can be hidden by well-connected unweighted topology.
    let weights = validate_weights(weights, values.nrows())?;
    let mut reservation = SystemWorkspaceReservation;
    let mut graph = TwoFePcgWorkspace::new(design, 3, &mut reservation)?;
    let peak = SolverWorkspacePeak::from_bytes(graph.capacity_elements() as u128 * 8)?;
    let contraction = schur_probe_reusing(design, weights, values, &mut graph, 3)?;
    Ok((contraction, peak))
}

fn schur_probe_reusing(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    graph: &mut TwoFePcgWorkspace,
    stride: usize,
) -> Result<f64> {
    if design.effects().len() != 2
        || design.nobs() != values.nrows()
        || values.ncols() < 3
        || stride < 3
        || graph.left_rhs.len() != graph.left_weights.len().saturating_mul(stride)
        || graph.residual.len() != graph.right_weights.len().saturating_mul(stride)
    {
        return Err(invalid_argument(
            "Schur probe workspace dimensions are inconsistent",
        ));
    }
    // A previous failed full-width attempt may leave nonfinite inactive lanes.
    // Serial application validates the whole operator bank, including its tail.
    graph.operator.fill(0.0);
    let left = &design.effects()[graph.left_effect];
    let right = &design.effects()[graph.right_effect];
    let columns = [0, 1, values.ncols() / 2];
    graph
        .left_rhs
        .par_chunks_mut(stride)
        .zip(graph.left_weights.par_iter_mut())
        .enumerate()
        .try_for_each(|(level, (sum, total))| -> Result<()> {
            sum[..3].fill(0.0);
            *total = 0.0;
            for &row in left.level_index().checked_members(level)? {
                let weight = weights.normalized(row);
                *total += weight;
                for (rhs, &column) in columns.iter().enumerate() {
                    sum[rhs] += weight * values[(row, column)];
                }
            }
            for value in &mut sum[..3] {
                *value /= *total;
            }
            Ok(())
        })?;
    let left_rhs = &graph.left_rhs;
    graph
        .residual
        .par_chunks_mut(stride)
        .zip(graph.right_weights.par_iter_mut())
        .enumerate()
        .try_for_each(|(level, (sum, total))| -> Result<()> {
            let members = right.level_index().checked_members(level)?;
            sum[..3].fill(0.0);
            *total = 0.0;
            for &row in members {
                let weight = weights.normalized(row);
                *total += weight;
                for (rhs, &column) in columns.iter().enumerate() {
                    sum[rhs] += weight * values[(row, column)];
                }
            }
            for &row in members {
                let weight = weights.normalized(row);
                let a = left.codes()[row] as usize;
                for rhs in 0..3 {
                    sum[rhs] -= weight * left_rhs[a * stride + rhs];
                }
            }
            Ok(())
        })?;
    let mut square = [0.0; 3];
    for level in 0..right.n_levels() {
        for rhs in 0..3 {
            let i = level * stride + rhs;
            graph.direction[i] = graph.residual[i] / graph.right_weights[level];
            square[rhs] = graph.residual[i].mul_add(graph.direction[i], square[rhs]);
        }
    }
    let initial = square;
    if initial.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return Ok(f64::INFINITY);
    }
    for _ in 0..OLS_SCHUR_PROBE_ITERATIONS {
        graph.apply_schur(design, weights, 3, stride)?;
        let mut projection = [0.0; 3];
        for level in 0..right.n_levels() {
            for rhs in 0..3 {
                let i = level * stride + rhs;
                projection[rhs] = graph.direction[i].mul_add(graph.operator[i], projection[rhs]);
            }
        }
        let mut step = [0.0; 3];
        for rhs in 0..3 {
            if square[rhs] <= initial[rhs] * 1e-24 || square[rhs] == 0.0 {
                continue;
            }
            if !projection[rhs].is_finite() || projection[rhs] <= 0.0 {
                return Ok(f64::INFINITY);
            }
            step[rhs] = square[rhs] / projection[rhs];
        }
        let mut next = [0.0; 3];
        for level in 0..right.n_levels() {
            for rhs in 0..3 {
                let i = level * stride + rhs;
                graph.residual[i] = (-step[rhs]).mul_add(graph.operator[i], graph.residual[i]);
                next[rhs] = graph.residual[i]
                    .mul_add(graph.residual[i] / graph.right_weights[level], next[rhs]);
            }
        }
        for rhs in 0..3 {
            if !next[rhs].is_finite() || next[rhs] < 0.0 {
                return Ok(f64::INFINITY);
            }
        }
        for level in 0..right.n_levels() {
            for rhs in 0..3 {
                let i = level * stride + rhs;
                graph.direction[i] = if step[rhs] == 0.0 {
                    0.0
                } else {
                    graph.residual[i] / graph.right_weights[level]
                        + next[rhs] / square[rhs] * graph.direction[i]
                };
            }
        }
        square = next;
    }
    Ok((0..3)
        .map(|rhs| {
            if initial[rhs] == 0.0 {
                0.0
            } else {
                (square[rhs] / initial[rhs]).sqrt()
            }
        })
        .fold(0.0, f64::max))
}

fn schur_probe_narrow_continuation(
    design: &BorrowedInterceptFeDesign<'_>,
    weights: ValidatedWeights<'_>,
    values: &DenseMatrix,
    graph: &mut TwoFePcgWorkspace,
    stride: usize,
    workspace: &BorrowedMapWorkspace,
    tolerance: f64,
) -> Result<(f64, Option<NarrowSchurState>)> {
    if design.effects().len() != 2
        || design.nobs() != values.nrows()
        || !(3..=8).contains(&values.ncols())
        || stride < values.ncols()
        || graph.left_rhs.len() != graph.left_weights.len().saturating_mul(stride)
        || graph.residual.len() != graph.right_weights.len().saturating_mul(stride)
    {
        return Err(invalid_argument(
            "Schur probe workspace dimensions are inconsistent",
        ));
    }
    let rhs_count = values.ncols();
    let selected = [0, 1, rhs_count / 2];
    let mut discarded = 0_u8;
    graph.solution.fill(0.0);
    // A previous failed full-width attempt may leave nonfinite inactive lanes.
    // Serial application validates the whole operator bank, including its tail.
    graph.operator.fill(0.0);
    let left = &design.effects()[graph.left_effect];
    let right = &design.effects()[graph.right_effect];
    let all_columns = [0, 1, 2, 3, 4, 5, 6, 7];
    let columns = &all_columns[..rhs_count];
    graph
        .left_rhs
        .par_chunks_mut(stride)
        .zip(graph.left_weights.par_iter_mut())
        .enumerate()
        .try_for_each(|(level, (sum, total))| -> Result<()> {
            sum[..rhs_count].fill(0.0);
            *total = 0.0;
            for &row in left.level_index().checked_members(level)? {
                let weight = weights.normalized(row);
                *total += weight;
                for (rhs, &column) in columns.iter().enumerate() {
                    sum[rhs] += weight * values[(row, column)];
                }
            }
            for value in &mut sum[..rhs_count] {
                *value /= *total;
            }
            Ok(())
        })?;
    let left_rhs = &graph.left_rhs;
    graph
        .residual
        .par_chunks_mut(stride)
        .zip(graph.right_weights.par_iter_mut())
        .enumerate()
        .try_for_each(|(level, (sum, total))| -> Result<()> {
            let members = right.level_index().checked_members(level)?;
            sum[..rhs_count].fill(0.0);
            *total = 0.0;
            for &row in members {
                let weight = weights.normalized(row);
                *total += weight;
                for (rhs, &column) in columns.iter().enumerate() {
                    sum[rhs] += weight * values[(row, column)];
                }
            }
            for &row in members {
                let weight = weights.normalized(row);
                let a = left.codes()[row] as usize;
                for rhs in 0..rhs_count {
                    sum[rhs] -= weight * left_rhs[a * stride + rhs];
                }
            }
            Ok(())
        })?;
    let mut square = [0.0; 8];
    for level in 0..right.n_levels() {
        for rhs in 0..rhs_count {
            let i = level * stride + rhs;
            graph.direction[i] = graph.residual[i] / graph.right_weights[level];
            square[rhs] = graph.residual[i].mul_add(graph.direction[i], square[rhs]);
        }
    }
    for rhs in 0..rhs_count {
        if !square[rhs].is_finite() || square[rhs] < 0.0 {
            if selected.contains(&rhs) { return Ok((f64::INFINITY, None)); }
            discarded |= 1 << rhs;
        }
        if !selected.contains(&rhs) && (discarded & (1 << rhs) != 0
            || two_fe_pcg_rhs_ready(graph, workspace, rhs, stride, tolerance)?) {
            square[rhs] = 0.0;
            for level in 0..right.n_levels() {
                graph.residual[level * stride + rhs] = 0.0;
                graph.direction[level * stride + rhs] = 0.0;
            }
        }
    }
    let initial = square;
    for _ in 0..OLS_SCHUR_PROBE_ITERATIONS {
        if let Err(error) = graph.apply_schur(design, weights, rhs_count, stride) {
            // Additional columns must not change the three-column admission
            // decision. A nonfinite operator confined to those lanes disables
            // warm continuation; structural errors and selected-lane failures
            // retain the original probe error.
            if error.code != ErrorCode::InvalidData || graph.operator.chunks(stride)
                .any(|row| selected.iter().any(|&rhs| !row[rhs].is_finite())) {
                return Err(error);
            }
            for rhs in 0..rhs_count {
                if graph.operator.chunks(stride).any(|row| !row[rhs].is_finite()) {
                    discarded |= 1 << rhs;
                    square[rhs] = 0.0;
                    for level in 0..right.n_levels() {
                        let i = level * stride + rhs;
                        graph.operator[i] = 0.0;
                        graph.residual[i] = 0.0;
                        graph.direction[i] = 0.0;
                        graph.solution[i] = 0.0;
                    }
                }
            }
        }
        let mut projection = [0.0; 8];
        for level in 0..right.n_levels() {
            for rhs in 0..rhs_count {
                let i = level * stride + rhs;
                projection[rhs] = graph.direction[i].mul_add(graph.operator[i], projection[rhs]);
            }
        }
        let mut step = [0.0; 8];
        for rhs in 0..rhs_count {
            if square[rhs] <= initial[rhs] * 1e-24 || square[rhs] == 0.0 {
                continue;
            }
            if !projection[rhs].is_finite() || projection[rhs] <= 0.0 {
                if selected.contains(&rhs) { return Ok((f64::INFINITY, None)); }
                discarded |= 1 << rhs;
                continue;
            }
            step[rhs] = square[rhs] / projection[rhs];
        }
        let mut next = [0.0; 8];
        for level in 0..right.n_levels() {
            for rhs in 0..rhs_count {
                let i = level * stride + rhs;
                graph.solution[i] = step[rhs].mul_add(graph.direction[i], graph.solution[i]);
                graph.residual[i] = (-step[rhs]).mul_add(graph.operator[i], graph.residual[i]);
                next[rhs] = graph.residual[i]
                    .mul_add(graph.residual[i] / graph.right_weights[level], next[rhs]);
            }
        }
        for rhs in 0..rhs_count {
            if !next[rhs].is_finite() || next[rhs] < 0.0 {
                if selected.contains(&rhs) { return Ok((f64::INFINITY, None)); }
                discarded |= 1 << rhs;
                next[rhs] = 0.0;
                step[rhs] = 0.0;
                for level in 0..right.n_levels() {
                    let i = level * stride + rhs;
                    graph.residual[i] = 0.0;
                    graph.direction[i] = 0.0;
                    graph.solution[i] = 0.0;
                }
            }
        }
        for level in 0..right.n_levels() {
            for rhs in 0..rhs_count {
                let i = level * stride + rhs;
                graph.direction[i] = if step[rhs] == 0.0 {
                    0.0
                } else {
                    graph.residual[i] / graph.right_weights[level]
                        + next[rhs] / square[rhs] * graph.direction[i]
                };
            }
        }
        square = next;
    }
    let contraction = selected.into_iter().map(|rhs| {
        if initial[rhs] == 0.0 { 0.0 } else { (square[rhs] / initial[rhs]).sqrt() }
    }).fold(0.0, f64::max);
    if discarded != 0 || graph.solution.iter().any(|v| !v.is_finite()) {
        return Ok((contraction, None));
    }
    for rhs in 0..rhs_count {
        if graph.direction.chunks(stride).all(|row| row[rhs] == 0.0)
            && !two_fe_pcg_rhs_ready(graph, workspace, rhs, stride, tolerance)? {
            // The probe's relative-energy cutoff can stop a selected lane
            // before the stricter full certificate. Restart that lane from its
            // current residual; keep its already-computed solution.
            square[rhs] = 0.0;
            for level in 0..right.n_levels() {
                let i = level * stride + rhs;
                graph.direction[i] = graph.residual[i] / graph.right_weights[level];
                square[rhs] = graph.residual[i].mul_add(graph.direction[i], square[rhs]);
            }
        }
    }
    Ok((contraction, Some(NarrowSchurState { rhs_count, square })))
}

pub(crate) fn absorb_ols_schur_certified(
    design: BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: crate::fe::AbsorbConfig,
    probe_seconds: f64,
    contraction: f64,
) -> Result<CertifiedBorrowedMap> {
    absorb_ols_schur_at_cap(
        design,
        weights,
        values,
        config,
        probe_seconds,
        contraction,
        OLS_SCHUR_MAX_ITERATIONS,
    )
}

#[allow(clippy::too_many_arguments)]
fn absorb_ols_schur_at_cap(
    design: BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: crate::fe::AbsorbConfig,
    probe_seconds: f64,
    contraction: f64,
    cap: usize,
) -> Result<CertifiedBorrowedMap> {
    absorb_ols_schur_with_probe_count(design, weights, values, config, probe_seconds,
        contraction, cap, OLS_SCHUR_PROBE_ITERATIONS)
}

/// Same immutable graph/weights as an accepted initial OLS probe. The final
/// projection has a fresh tolerance and certificate, and spends no pilot steps.
pub(crate) fn absorb_ols_schur_reusing_probe_certified(
    design: BorrowedInterceptFeDesign<'_>,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: crate::fe::AbsorbConfig,
    contraction: f64,
) -> Result<CertifiedBorrowedMap> {
    if config.retain_coefficients || values.ncols() != 1 || contraction > OLS_SCHUR_MAX_CONTRACTION {
        return Err(invalid_argument("final Schur projection requires an accepted probe"));
    }
    absorb_ols_schur_with_probe_count(design, weights, values, config, 0.0,
        contraction, OLS_SCHUR_MAX_ITERATIONS, 0)
}

#[allow(clippy::too_many_arguments)]
fn absorb_ols_schur_with_probe_count(
    design: BorrowedInterceptFeDesign<'_>, weights: BorrowedMapWeights<'_>,
    values: DenseMatrix, mut config: crate::fe::AbsorbConfig,
    probe_seconds: f64, contraction: f64, cap: usize, probe_iterations: usize,
) -> Result<CertifiedBorrowedMap> {
    validate_config(config.map)?;
    if !probe_seconds.is_finite()
        || probe_seconds < 0.0
        || !contraction.is_finite()
        || contraction < 0.0
        || config.map.max_iterations <= probe_iterations
    {
        return Err(invalid_argument("OLS Schur configuration is unsupported"));
    }
    // Schur's observation-space projection has no coefficient products. Its
    // original-input LSMR fallback can retain the requested products under its
    // existing memory admission, avoiding a second saved-effect solve.
    let retain_fallback_coefficients = config.retain_coefficients;
    config.retain_coefficients = false;
    let planned = schur_ols_workspace_bytes(&design, values.ncols(), rayon::current_num_threads())?;
    if config
        .solver_memory_budget
        .is_some_and(|budget| !budget.fits(planned))
    {
        return Err(allocation(
            "OLS Schur workspace exceeds the joint memory budget",
        ));
    }
    let attempted_peak = SolverWorkspacePeak::from_bytes(planned)?;
    let started = Instant::now();
    let mut absorber =
        ReusableCategoricalMapAbsorber::new(design, values.ncols(), rayon::current_num_threads())?;
    validate_config(config.map)?;
    if values.as_slice().iter().any(|v| !v.is_finite()) {
        return Err(invalid_data("OLS Schur values must be finite"));
    }
    let validated = validate_weights(weights, values.nrows())?;
    absorber.workspace.set_rhs_count(values.ncols())?;
    absorber.workspace.ols_norm_reduction = true;
    update_ols_workspace_norms(&values, validated, &mut absorber.workspace, true)?;
    let setup_seconds = started.elapsed().as_secs_f64();
    let solve_started = Instant::now();
    let mut map = config.map;
    map.tolerance = crate::fe::auto::direct_lsmr_tolerance(map.tolerance);
    map.max_iterations -= probe_iterations;
    let mut graph = absorber
        .two_fe_pcg
        .take()
        .ok_or_else(|| invalid_argument("OLS Schur workspace unavailable"))?;
    let outcome = absorb_two_fe_pcg_reusing(
        &absorber.design,
        validated,
        values,
        map,
        0,
        0.0,
        setup_seconds,
        solve_started,
        absorber.max_rhs,
        &mut graph,
        &mut absorber.workspace,
        &mut absorber.cg_residual_square,
        &mut absorber.cg_initial_residual_square,
        &mut absorber.cg_next_residual_square,
        &mut absorber.cg_direction_projection,
        &mut absorber.cg_ratio,
        Some(cap),
        true,
        None,
    )?;
    finish_ols_schur(absorber, graph, weights, config, map, attempted_peak,
        setup_seconds, solve_started, probe_seconds, contraction, probe_iterations, probe_iterations, outcome,
        retain_fallback_coefficients)
}

#[allow(clippy::too_many_arguments)]
fn finish_ols_schur(
    mut absorber: ReusableCategoricalMapAbsorber<'_>, graph: TwoFePcgWorkspace,
    weights: BorrowedMapWeights<'_>, config: crate::fe::AbsorbConfig, map: MapConfig,
    attempted_peak: SolverWorkspacePeak, setup_seconds: f64, solve_started: Instant,
    probe_seconds: f64, contraction: f64, probe_iterations: usize, iteration_offset: usize, outcome: SchurAttempt,
    retain_fallback_coefficients: bool,
) -> Result<CertifiedBorrowedMap> {
    match outcome {
        SchurAttempt::Certified(mut result) => {
            result.report.solver_workspace_peak = attempted_peak;
            result.report.requested_solver = FeSolverKind::Auto;
            result.report.used_solver = FeSolverKind::Cg;
            result.report.iterations += iteration_offset;
            result.report.setup_seconds += probe_seconds;
            result.report.pilot_iterations = probe_iterations;
            result.report.pilot_contraction = contraction;
            result.with_residual_norms(std::mem::take(&mut absorber.workspace.residual_norms))
        }
        SchurAttempt::Continue {
            values,
            iterations,
            candidate,
        } => {
            let solve_seconds = solve_started.elapsed().as_secs_f64();
            let reference_norms = std::mem::take(&mut absorber.workspace.reference_norms);
            let empty = BorrowedInterceptFeDesign::new(absorber.design.nobs(), Vec::new())?;
            let design = std::mem::replace(&mut absorber.design, empty);
            drop(graph);
            drop(absorber);
            let mut remaining = config.map;
            remaining.max_iterations = map
                .max_iterations
                .checked_sub(iterations)
                .filter(|n| *n > 0)
                .ok_or_else(|| {
                    invalid_data("OLS Schur continuation exhausted the shared iteration budget")
                })?;
            if let Some(budget) = config.solver_memory_budget.filter(|_| candidate) {
                let levels = [
                    design.effects()[0].n_levels(),
                    design.effects()[1].n_levels(),
                ];
                let estimate = estimate_fe_solver_memory(
                    design.nobs(),
                    values.ncols(),
                    &levels,
                    rayon::current_num_threads(),
                    Some(4),
                )?;
                if !budget.fits(estimate.cg_bytes) {
                    return Err(allocation(
                        "OLS Schur continuation exceeds the joint memory budget",
                    ));
                }
            }
            let mut result = if !candidate {
                // A capped attempt has not changed these observations. Use
                // coefficient-space LSMR before observation-space CG can
                // accumulate span drift on a weak graph. The existing direct
                // route admits its own workspace and retains certified CG as
                // a constrained-memory or numerical fallback.
                drop(reference_norms);
                let mut plan = AutoLsmrPlan::direct_lsmr(config.lsmr, config.solver_memory_budget);
                // Only public serial OLS asks this wide Schur fallback to
                // retain products. Shared calls clear that request at their
                // fit boundary; narrow, threaded and explicit routes keep
                // their original transport and memory admission.
                if retain_fallback_coefficients && design.effects().len() == 2
                    && (16..=64).contains(&values.ncols())
                    && config.lsmr.threads.resolved() == 1 && rayon::current_num_threads() == 1
                {
                    plan.batch_policy = LsmrBatchPolicy::RetainedSerialOls;
                }
                let mut reservation = SystemWorkspaceReservation;
                absorb_auto_with_reservation(
                    &design, weights, values, remaining, plan,
                    AutoRunOptions { empty_design_is_structural_constant: false,
                        retain_coefficients: retain_fallback_coefficients },
                    &mut reservation,
                )?
            } else {
                let mut reservation = SystemWorkspaceReservation;
                let mut result = absorb_cg_with_reservation(
                    &design,
                    weights,
                    values,
                    remaining,
                    false,
                    CgContinuation {
                        reference_norms: Some(&reference_norms),
                        ..CgContinuation::ORDINARY
                    },
                    &mut reservation,
                )?;
                result.report.solver_workspace_peak = result.report.solver_workspace_peak
                    .with_retained(reference_norms.capacity() as u128 * 8)?;
                result
            };
            result.report.solver_workspace_peak = attempted_peak.max(result.report.solver_workspace_peak);
            result.report.requested_solver = FeSolverKind::Auto;
            result.report.iterations += iterations + iteration_offset;
            result.report.setup_seconds += setup_seconds + probe_seconds;
            result.report.solve_seconds += solve_seconds;
            result.report.pilot_iterations = probe_iterations;
            result.report.pilot_contraction = contraction;
            result.report.fallback_used = true;
            result.report.warnings.insert(0, format!("automatic two-FE Schur CG continued with {:?} after {iterations} iterations (reconstructed candidate: {candidate})", result.report.used_solver));
            Ok(result)
        }
    }
}

// Fixed row partitions and a canonical LASSQ merge make this OLS-only
// reduction independent of the worker count, including the serial path.
// Reuse the existing dot workspace; never allocate another N-by-K matrix.
fn update_ols_workspace_norms(
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
    if !workspace.ols_norm_reduction || values.nrows() < 16_384 || values.ncols() > 64 {
        return weighted_column_norms(
            values,
            weights,
            output,
            &mut workspace.value_scales,
            &mut workspace.group_sums,
        );
    }
    let columns = values.ncols();
    if output.len() != columns {
        return Err(invalid_argument("OLS norm output is not aligned"));
    }
    let chunk_rows = 4096;
    let chunks = values.nrows().div_ceil(chunk_rows);
    let partial_width = 2 * columns;
    let partials = chunks * partial_width;
    let accumulate = |chunk: usize, partial: &mut [f64]| -> Result<()> {
        let (scales, squares) = partial.split_at_mut(columns);
        scales.fill(0.0);
        squares.fill(1.0);
        let start = chunk * chunk_rows;
        let end = (start + chunk_rows).min(values.nrows());
        for row in start..end {
            let root = weights.normalized(row).sqrt();
            if !root.is_finite() || root <= 0.0 {
                return Err(invalid_data("OLS norm weight root is invalid"));
            }
            let source = &values.as_slice()[row * columns..(row + 1) * columns];
            for ((scale, square), &value) in scales.iter_mut().zip(squares.iter_mut()).zip(source) {
                let value = root * value;
                if !value.is_finite() {
                    return Err(invalid_data("OLS norm weighted value is nonfinite"));
                }
                update_scaled_sum_of_squares(value, scale, square);
            }
        }
        Ok(())
    };
    let scales = &mut workspace.value_scales;
    let squares = &mut workspace.group_sums;
    scales.fill(0.0);
    squares.fill(1.0);
    let mut merge = |partial: &[f64]| {
        for rhs in 0..columns {
            let scale = partial[rhs];
            if scale == 0.0 {
                continue;
            }
            if scales[rhs] < scale {
                let ratio = scales[rhs] / scale;
                squares[rhs] = partial[columns + rhs] + squares[rhs] * ratio * ratio;
                scales[rhs] = scale;
            } else {
                let ratio = scale / scales[rhs];
                squares[rhs] += partial[columns + rhs] * ratio * ratio;
            }
        }
    };
    if rayon::current_num_threads() > 1 && workspace.parallel_dot_sums.len() >= partials {
        let scratch = &mut workspace.parallel_dot_sums[..partials];
        scratch
            .par_chunks_mut(partial_width)
            .enumerate()
            .try_for_each(|(chunk, partial)| accumulate(chunk, partial))?;
        for partial in scratch.chunks_exact(partial_width) {
            merge(partial);
        }
    } else {
        let mut partial = [0.0; 128];
        for chunk in 0..chunks {
            accumulate(chunk, &mut partial[..partial_width])?;
            merge(&partial[..partial_width]);
        }
    }
    for rhs in 0..columns {
        output[rhs] = if scales[rhs] == 0.0 {
            0.0
        } else {
            scales[rhs] * squares[rhs].sqrt()
        };
        if !output[rhs].is_finite() {
            return Err(invalid_data("OLS norm is nonfinite"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};

    #[test]
    fn automatic_initial_norm_handoff_preserves_fresh_certificate_and_workspace() {
        for threads in [1, 4] {
            rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
                for rows in [257, 16_385] {
                    let left: Vec<_> = (0..rows).map(|r| (r % 17) as u32).collect();
                    let right: Vec<_> = (0..rows).map(|r| ((r / 7) % 13) as u32).collect();
                    let li = LevelIndex::from_codes(&left, 17).unwrap();
                    let ri = LevelIndex::from_codes(&right, 13).unwrap();
                    let l = ValidatedBorrowedInterceptSource::new(&left, 17, &li).unwrap();
                    let r = ValidatedBorrowedInterceptSource::new(&right, 13, &ri).unwrap();
                    let design = BorrowedInterceptFeDesign::new(rows, vec![l.effect(), r.effect()]).unwrap();
                    for width in [1, 7, 52] {
                        let values = DenseMatrix::from_row_major(rows, width, (0..rows * width)
                            .map(|i| match (i % width) % 5 {
                                0 => if i % 2 == 0 { -0.0 } else { 0.0 },
                                1 => 0.25,
                                2 => (i as f64 * 0.17).sin() * 1e-200,
                                3 => (i as f64 * 0.17).sin() * 1e200,
                                _ => (i as f64 * 0.17).sin(),
                            }).collect()).unwrap();
                        for weight_scale in [None, Some(1e-100), Some(1.0), Some(1e100)] {
                            let raw = weight_scale.map(|scale| (0..rows)
                                .map(|r| scale * 10.0_f64.powi((r % 9) as i32 - 4)).collect::<Vec<_>>());
                            let weights = validate_weights(raw.as_deref().map_or(
                                BorrowedMapWeights::Unit, BorrowedMapWeights::Provided), rows).unwrap();
                            let mut full = BorrowedMapWorkspace::new(&design, rows, width, &mut SystemWorkspaceReservation).unwrap();
                            let mut reused = BorrowedMapWorkspace::new(&design, rows, width, &mut SystemWorkspaceReservation).unwrap();
                            for workspace in [&mut full, &mut reused] {
                                assert!(!workspace.ols_norm_reduction);
                                weighted_column_norms(&values, weights, &mut workspace.reference_norms,
                                    &mut workspace.value_scales, &mut workspace.group_sums).unwrap();
                            }
                            let expected = certify(&design, weights, &values, 1e-10, &mut full).unwrap();
                            let actual = certify_initial(&design, weights, &values, 1e-10, &mut reused).unwrap();
                            assert_eq!(actual, expected);
                            assert_eq!(reused.residual_norms, full.residual_norms);
                            assert_eq!(reused.value_scales, full.value_scales);
                            assert_eq!(reused.rhs_scales, full.rhs_scales);
                            assert_eq!(reused.rhs_relative_infinity_norms, full.rhs_relative_infinity_norms);
                            assert_eq!(reused.group_sums, full.group_sums);
                            assert_eq!(reused.group_corrections, full.group_corrections);
                            // A fresh moment check still rejects a non-orthogonal
                            // input; the norm handoff cannot grant a certificate.
                            assert_eq!(actual.passed, width == 1);
                        }
                    }
                }
            });
        }
    }

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
    fn checkpoint_schur_copy_respects_exact_transient_admission() {
        let values = DenseMatrix::from_columns(&[vec![1.0, -2.0], vec![3.0, 4.0]]).unwrap();
        let bytes = values.as_slice().len() as u128 * 8;
        assert_eq!(
            checkpoint_schur_copy(&values, bytes - 1).unwrap_err().code,
            ErrorCode::Allocation
        );
        let copy = checkpoint_schur_copy(&values, bytes).unwrap();
        assert_eq!(copy, values);
        assert_ne!(copy.as_slice().as_ptr(), values.as_slice().as_ptr());
        for rows in [16384, 122880, 983040] {
            for rhs in [16, 17, 51, 64] {
                for threads in [1, 4, 16] {
                    let estimate =
                        estimate_fe_solver_memory(rows, rhs, &[rows / 4, 128], threads, Some(4))
                            .unwrap();
                    assert!(estimate.lsmr_schwarz_bytes >= rows as u128 * rhs as u128 * 8);
                }
            }
        }
    }

    #[test]
    fn checkpoint_schur_reuses_probe_and_falls_back_on_untouched_inputs() {
        let rows = 16384;
        let first_codes = (0..rows).map(|r| (r % 257) as u32).collect::<Vec<_>>();
        let second_codes = (0..rows)
            .map(|r| ((r * 17 + r / 257) % 127) as u32)
            .collect::<Vec<_>>();
        let first_index = LevelIndex::from_codes(&first_codes, 257).unwrap();
        let second_index = LevelIndex::from_codes(&second_codes, 127).unwrap();
        let first = ValidatedBorrowedInterceptSource::new(&first_codes, 257, &first_index).unwrap();
        let second =
            ValidatedBorrowedInterceptSource::new(&second_codes, 127, &second_index).unwrap();
        let design =
            || BorrowedInterceptFeDesign::new(rows, vec![first.effect(), second.effect()]).unwrap();
        let columns = (0..17)
            .map(|c| {
                (0..rows)
                    .map(|r| {
                        ((r * (c + 17) % 997) as f64 / 17.0).sin()
                            + first_codes[r] as f64 * 0.001
                            + second_codes[r] as f64 * 0.01
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let values = DenseMatrix::from_columns(&columns).unwrap();
        let original = values.clone();
        let supplied = (0..rows).map(|r| 1.0 + (r % 7) as f64).collect::<Vec<_>>();
        let scaled = supplied.iter().map(|w| w * 1024.0).collect::<Vec<_>>();
        let config = MapConfig {
            tolerance: 1e-12,
            ..config()
        };
        for threads in [1, 4, 16] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    let mut reusable =
                        ReusableCategoricalMapAbsorber::new(design(), 64, threads).unwrap();
                    reusable.enable_checkpoint_lsmr();
                    let capacities = reusable.capacities();
                    for raw in [
                        BorrowedMapWeights::Unit,
                        BorrowedMapWeights::Provided(&supplied),
                        BorrowedMapWeights::Provided(&scaled),
                    ] {
                        let weights = validate_weights(raw, rows).unwrap();
                        let expected_probe = schur_ols_probe(&design(), raw, &values).unwrap();
                        let graph = reusable.two_fe_pcg.as_mut().unwrap();
                        for storage in [
                            &mut graph.left_rhs,
                            &mut graph.left_work,
                            &mut graph.residual,
                            &mut graph.direction,
                            &mut graph.operator,
                            &mut graph.solution,
                        ] {
                            storage.fill(f64::NAN);
                        }
                        for _ in 0..2 {
                            assert_eq!(
                                schur_probe_reusing(&design(), weights, &values, graph, 64)
                                    .unwrap(),
                                expected_probe
                            );
                        }
                        let expected = absorb_borrowed_intercept_map_certified(
                            &design(),
                            raw,
                            values.clone(),
                            MapConfig {
                                tolerance: 1e-13,
                                ..config
                            },
                        )
                        .unwrap();
                        let solved = reusable.absorb(raw, values.clone(), config).unwrap();
                        assert_eq!(solved.report.used_solver, FeSolverKind::Map);
                        assert_eq!(solved.report.requested_solver, FeSolverKind::Lsmr);
                        assert_eq!(solved.report.pilot_iterations, 8);
                        assert!(!solved.report.fallback_used);
                        assert_eq!(solved.report.checkpoint_retries, 0);
                        for (&a, &b) in solved
                            .residualized()
                            .as_slice()
                            .iter()
                            .zip(expected.residualized().as_slice())
                        {
                            assert!((a - b).abs() < 2e-10, "{a} != {b}");
                        }
                        // A width-65 owner has no Schur graph and supplies the unchanged
                        // checkpoint LSMR result for exactly these 17 authoritative RHSs.
                        let mut baseline =
                            ReusableCategoricalMapAbsorber::new(design(), 65, threads).unwrap();
                        baseline.enable_checkpoint_lsmr();
                        let original_lsmr = baseline.absorb(raw, values.clone(), config).unwrap();
                        for reject_certificate in [false, true] {
                            if reject_certificate {
                                reusable.checkpoint_schur_certificate_failure = true;
                            } else {
                                reusable.checkpoint_schur_iteration_cap = Some(1);
                            }
                            let fallback = reusable.absorb(raw, values.clone(), config).unwrap();
                            assert_eq!(fallback.report.used_solver, FeSolverKind::Lsmr);
                            assert_eq!(fallback.residualized(), original_lsmr.residualized());
                            assert_eq!(
                                fallback.report.checkpoint_retries,
                                original_lsmr.report.checkpoint_retries + 1
                            );
                            assert!(fallback.report.iterations > original_lsmr.report.iterations);
                            assert!(fallback
                                .report
                                .warnings
                                .iter()
                                .any(|w| w.contains("untouched authoritative inputs")));
                        }
                        if threads == 4 && raw == BorrowedMapWeights::Provided(&supplied) {
                            reusable.checkpoint_schur_iteration_cap = Some(1);
                            reusable.inject_checkpoint_lsmr_certificate_failure();
                            let final_map = reusable.absorb(raw, values.clone(), config).unwrap();
                            assert_eq!(final_map.report.used_solver, FeSolverKind::Map);
                            assert!(final_map.report.fallback_used);
                            assert_eq!(final_map.report.checkpoint_retries, 2);
                            for message in ["bounded current-weight Schur", "checkpoint LSMR"] {
                                assert!(final_map.report.warnings.iter().any(|w| w.contains(message)));
                            }
                            for (&a, &b) in final_map.residualized().as_slice().iter().zip(expected.residualized().as_slice()) {
                                assert!((a - b).abs() < 2e-10);
                            }
                        }
                        assert_eq!(values, original);
                        assert_eq!(reusable.capacities(), capacities);
                    }
                    for code in [ErrorCode::Allocation, ErrorCode::Interrupted] {
                        reusable.inject_checkpoint_lsmr_fatal_failure(code);
                        assert_eq!(
                            reusable
                                .absorb(BorrowedMapWeights::Unit, values.clone(), config)
                                .unwrap_err()
                                .code,
                            code
                        );
                        assert!(reusable
                            .absorb(BorrowedMapWeights::Unit, values.clone(), config)
                            .unwrap()
                            .report
                            .certified());
                        assert_eq!(reusable.capacities(), capacities);
                    }
                });
        }
    }

    #[test]
    fn checkpoint_schur_weighted_cut_rejects_before_copying() {
        let rows = 16384;
        let left_codes = (0..rows).map(|r| (r / 4) as u32).collect::<Vec<_>>();
        let right_codes = (0..rows)
            .map(|r| {
                (match r % 4 {
                    0 => r / 4 % 128,
                    1 => (r / 4 + 1) % 128,
                    _ => (r * 37 + r / 512 * 19) % 128,
                }) as u32
            })
            .collect::<Vec<_>>();
        let left_index = LevelIndex::from_codes(&left_codes, rows / 4).unwrap();
        let right_index = LevelIndex::from_codes(&right_codes, 128).unwrap();
        let left =
            ValidatedBorrowedInterceptSource::new(&left_codes, rows / 4, &left_index).unwrap();
        let right = ValidatedBorrowedInterceptSource::new(&right_codes, 128, &right_index).unwrap();
        let design =
            BorrowedInterceptFeDesign::new(rows, vec![left.effect(), right.effect()]).unwrap();
        let values = DenseMatrix::from_columns(
            &(0..17)
                .map(|c| {
                    (0..rows)
                        .map(|r| {
                            ((r * (c + 17) % 997) as f64 / 17.0).sin()
                                + (right_codes[r] as f64 * 0.049).sin()
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let weights = (0..rows)
            .map(|r| if r % 4 < 2 { 1.0 } else { 1e-10 })
            .collect::<Vec<_>>();
        let mut reusable = ReusableCategoricalMapAbsorber::new(design, 17, 4).unwrap();
        reusable.enable_checkpoint_lsmr();
        let capacities = reusable.capacities();
        let (result, cost) = reusable
            .try_checkpoint_schur(
                validate_weights(BorrowedMapWeights::Provided(&weights), rows).unwrap(),
                &values,
                config(),
            )
            .unwrap();
        assert!(result.is_none());
        assert!(cost.contraction > OLS_SCHUR_MAX_CONTRACTION);
        assert_eq!(cost.iterations, 0);
        assert_eq!(cost.solve_seconds, 0.0);
        assert!(!cost.rejected);
        assert_eq!(reusable.capacities(), capacities);
        for width in [1, 15] {
            let narrow = DenseMatrix::zeros(rows, width).unwrap();
            let (result, cost) = reusable
                .try_checkpoint_schur(
                    validate_weights(BorrowedMapWeights::Unit, rows).unwrap(),
                    &narrow,
                    config(),
                )
                .unwrap();
            assert!(result.is_none());
            assert_eq!(cost.probe_iterations, 0);
        }
    }

    #[test]
    fn ols_schur_probe_solve_budget_and_order_match_independent_map() {
        let rows = 16_384;
        let first_codes = (0..rows).map(|r| (r % 257) as u32).collect::<Vec<_>>();
        let second_codes = (0..rows)
            .map(|r| ((r * 17 + r / 257) % 127) as u32)
            .collect::<Vec<_>>();
        let first_index = LevelIndex::from_codes(&first_codes, 257).unwrap();
        let second_index = LevelIndex::from_codes(&second_codes, 127).unwrap();
        let first = ValidatedBorrowedInterceptSource::new(&first_codes, 257, &first_index).unwrap();
        let second =
            ValidatedBorrowedInterceptSource::new(&second_codes, 127, &second_index).unwrap();
        let design = |reverse| {
            BorrowedInterceptFeDesign::new(
                rows,
                if reverse {
                    vec![second.effect(), first.effect()]
                } else {
                    vec![first.effect(), second.effect()]
                },
            )
            .unwrap()
        };
        let columns = (0..17)
            .map(|c| {
                (0..rows)
                    .map(|r| {
                        ((r * (c + 17) % 997) as f64 / 17.0).sin()
                            + first_codes[r] as f64 * 0.001
                            + second_codes[r] as f64 * 0.01
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let values = || DenseMatrix::from_columns(&columns).unwrap();
        let mut config = crate::fe::AbsorbConfig::default();
        config.map.tolerance = 1e-10;
        let supplied = (0..rows).map(|r| 1.0 + (r % 7) as f64).collect::<Vec<_>>();
        let dispersed = (0..rows)
            .map(|r| 10_f64.powi((r % 13) as i32 - 6))
            .collect::<Vec<_>>();
        let constant = vec![128.0; rows];
        for weights in [
            BorrowedMapWeights::Unit,
            BorrowedMapWeights::Provided(&constant),
            BorrowedMapWeights::Provided(&supplied),
            BorrowedMapWeights::Provided(&dispersed),
        ] {
            let expected = absorb_borrowed_intercept_map_certified(
                &design(false),
                weights,
                values(),
                MapConfig {
                    tolerance: 1e-13,
                    max_iterations: 10_000,
                    ..config.map
                },
            )
            .unwrap();
            let mut serial_probe = None;
            let mut serial_values = None;
            for threads in [1, 4, 16] {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        let (contraction, probe_peak) =
                            schur_ols_probe_with_workspace(&design(false), weights, &values())
                                .unwrap();
                        assert!(probe_peak.bytes().unwrap() > 0);
                        assert!(contraction <= OLS_SCHUR_MAX_CONTRACTION, "{contraction}");
                        if let Some(expected) = serial_probe {
                            assert_eq!(contraction, expected);
                        } else {
                            serial_probe = Some(contraction);
                        }
                        assert_eq!(
                            contraction,
                            schur_ols_probe(&design(true), weights, &values()).unwrap()
                        );
                        let planned =
                            schur_ols_workspace_bytes(&design(false), 17, threads).unwrap();
                        let session =
                            ReusableCategoricalMapAbsorber::new(design(false), 17, threads)
                                .unwrap();
                        let w = &session.workspace;
                        let actual = session.two_fe_pcg.as_ref().unwrap().capacity_elements()
                            + session.cg_residual_square.capacity()
                            + session.cg_initial_residual_square.capacity()
                            + session.cg_next_residual_square.capacity()
                            + session.cg_direction_projection.capacity()
                            + session.cg_ratio.capacity()
                            + w.reference_norms.capacity()
                            + w.residual_norms.capacity()
                            + w.rhs_scales.capacity()
                            + w.rhs_relative_infinity_norms.capacity()
                            + w.value_scales.capacity()
                            + w.group_sums.capacity()
                            + w.group_corrections.capacity()
                            + w.parallel_group_means.capacity()
                            + w.parallel_group_weights.capacity()
                            + w.parallel_dot_sums.capacity();
                        assert!(planned >= actual as u128 * 8, "{planned} < {}", actual * 8);
                        drop(session);
                        let mut config = config;
                        config.lsmr.threads = crate::options::ThreadConfig::from_requested(threads, false).unwrap();
                        for (cap, fallback) in [(OLS_SCHUR_MAX_ITERATIONS, false), (1, true)] {
                            let mut result = absorb_ols_schur_at_cap(
                                design(false),
                                weights,
                                values(),
                                config,
                                0.125,
                                contraction,
                                cap,
                            )
                            .unwrap();
                            let peak = result.report.solver_workspace_peak.bytes().unwrap() as u128;
                            assert!(peak >= planned);
                            if !fallback {
                                assert_eq!(peak, planned);
                            }
                            assert_eq!(result.report.fallback_used, fallback);
                            if fallback && matches!(weights, BorrowedMapWeights::Unit) {
                                assert_eq!(result.report.used_solver, FeSolverKind::Lsmr);
                            }
                            assert_eq!(result.report.requested_solver, FeSolverKind::Auto);
                            assert!(result.report.iterations <= config.map.max_iterations);
                            assert!(result.report.setup_seconds >= 0.125);
                            assert_eq!(result.report.pilot_iterations, OLS_SCHUR_PROBE_ITERATIONS);
                            assert!(result.report.certified());
                            for (&a, &b) in result
                                .residualized
                                .as_slice()
                                .iter()
                                .zip(expected.residualized().as_slice())
                            {
                                assert!((a - b).abs() < 2e-9, "{a} != {b}");
                            }
                            if fallback {
                                let retained_config = crate::fe::AbsorbConfig {
                                    retain_coefficients: true, ..config
                                };
                                let retained = absorb_ols_schur_at_cap(design(false), weights,
                                    values(), retained_config, 0.125, contraction, cap).unwrap();
                                let (residuals, report, coefficients) = retained.into_full_parts();
                                assert!(report.certified() && report.fallback_used);
                                assert_eq!(report.iterations, result.report.iterations);
                                assert_eq!(report.certificate, result.report.certificate);
                                assert_eq!(residuals, result.residualized);
                                assert!(report.iterations <= config.map.max_iterations);
                                let coefficients = coefficients.expect("original-input fallback retains requested products");
                                for column in 0..17 {
                                    let coefficient = coefficients.rhs(column).unwrap();
                                    for row in 0..rows {
                                        let fitted = coefficient[coefficients.effect_offsets()[0] + first_codes[row] as usize]
                                            + coefficient[coefficients.effect_offsets()[1] + second_codes[row] as usize];
                                        assert!((fitted - (columns[column][row] - residuals[(row, column)])).abs() <= 2e-9);
                                        assert!((residuals[(row, column)] - expected.residualized()[(row, column)]).abs() <= 2e-9);
                                    }
                                }
                            }
                            if !fallback {
                                if let Some(expected) = &serial_values {
                                    assert_eq!(&result.residualized, expected);
                                } else {
                                    serial_values = Some(result.residualized.clone());
                                }
                                assert_eq!(result.take_residual_norms().len(), 17);
                            }
                        }
                        let mut limited = config;
                        limited.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
                            execution_bytes: 0,
                            usable_bytes: planned - 1,
                        });
                        assert_eq!(
                            absorb_ols_schur_certified(
                                design(false),
                                weights,
                                values(),
                                limited,
                                0.0,
                                contraction
                            )
                            .unwrap_err()
                            .code,
                            ErrorCode::Allocation
                        );
                        limited = config;
                        limited.map.max_iterations = OLS_SCHUR_PROBE_ITERATIONS + 1;
                        assert!(absorb_ols_schur_at_cap(
                            design(false),
                            weights,
                            values(),
                            limited,
                            0.0,
                            contraction,
                            1
                        )
                        .is_err());
                    });
            }
        }
    }

    #[test]
    fn narrow_schur_continuation_preserves_admission_projection_and_budget() {
        let rows = 20_000;
        for disconnected in [false, true] {
        let left_codes = (0..rows).map(|r| ((r * 8191 % rows) / 5) as u32).collect::<Vec<_>>();
        let right_codes = (0..rows).map(|r| {
            let row = r * 8191 % rows;
            let firm = if row % 5 < 3 { row / 5 * 17 } else { row / 5 * 389 + row % 5 * 97 };
            if disconnected { (firm % 499 + usize::from(row / 5 >= 2000) * 499) as u32 }
            else { (firm % 997) as u32 }
        }).collect::<Vec<_>>();
        let left_index = LevelIndex::from_codes(&left_codes, 4000).unwrap();
        let right_levels = if disconnected { 998 } else { 997 };
        let right_index = LevelIndex::from_codes(&right_codes, right_levels).unwrap();
        let left = ValidatedBorrowedInterceptSource::new(&left_codes, 4000, &left_index).unwrap();
        let right = ValidatedBorrowedInterceptSource::new(&right_codes, right_levels, &right_index).unwrap();
        let design = || BorrowedInterceptFeDesign::new(rows, vec![left.effect(), right.effect()]).unwrap();
        for scale in [None, Some(1e-100), Some(1.0), Some(1e100)] {
            let raw = scale.map(|s| (0..rows).map(|r| s * (0.125 + (r % 17) as f64 / 11.0)).collect::<Vec<_>>());
            let weights = raw.as_deref().map_or(BorrowedMapWeights::Unit, BorrowedMapWeights::Provided);
            for rhs in [3, 5, 7, 8] {
                let mut columns = (0..rhs).map(|c| (0..rows)
                    .map(|r| ((r * (c + 17) % 997) as f64 / 17.0).sin()
                        + left_codes[r] as f64 * 0.001 + right_codes[r] as f64 * 0.01)
                    .collect::<Vec<_>>()).collect::<Vec<_>>();
                columns[rhs - 1].fill(1.0);
                if rhs >= 5 {
                    columns[rhs - 2] = (0..rows).map(|r| left_codes[r] as f64 * 0.01
                        + right_codes[r] as f64 * 0.03).collect();
                }
                let values = DenseMatrix::from_columns(&columns).unwrap();
                let oracle = absorb_borrowed_intercept_map_certified(&design(), weights,
                    values.clone(), MapConfig { tolerance: 1e-13, ..config() }).unwrap();
                for threads in [8, 16] {
                    rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
                        let expected_probe = schur_ols_probe(&design(), weights, &values).unwrap();
                        assert!(expected_probe <= OLS_SCHUR_MAX_CONTRACTION);
                        let validated = validate_weights(weights, rows).unwrap();
                        {
                            let mut owner = ReusableCategoricalMapAbsorber::new(design(), rhs, threads).unwrap();
                            owner.workspace.set_rhs_count(rhs).unwrap();
                            owner.workspace.ols_norm_reduction = true;
                            update_ols_workspace_norms(&values, validated, &mut owner.workspace, true).unwrap();
                            let (probe, state) = schur_probe_narrow_continuation(&owner.design, validated,
                                &values, owner.two_fe_pcg.as_mut().unwrap(), rhs, &owner.workspace, 1e-14).unwrap();
                            assert_eq!(probe.to_bits(), expected_probe.to_bits());
                            assert!(state.is_some(), "ordinary and FE-collinear lanes must be resumable");
                        }
                        let config = crate::fe::AbsorbConfig { map: MapConfig { tolerance: 1e-14, ..config() },
                            ..crate::fe::AbsorbConfig::default() };
                        for cap in [OLS_SCHUR_MAX_ITERATIONS, 1] {
                            let result = match try_absorb_ols_schur_narrow_at_cap(design(), weights,
                                values.clone(), config, cap).unwrap() {
                                NarrowOlsSchurAttempt::Certified(result) => result,
                                NarrowOlsSchurAttempt::Rejected { .. } => panic!("accepted probe rejected"),
                            };
                            assert!(result.report.certified());
                            assert_eq!(result.report.pilot_iterations, OLS_SCHUR_PROBE_ITERATIONS);
                            assert_eq!(result.report.pilot_contraction.to_bits(), expected_probe.to_bits());
                            assert!(result.report.iterations >= OLS_SCHUR_PROBE_ITERATIONS);
                            assert!(result.report.iterations <= config.map.max_iterations);
                            assert_eq!(result.report.fallback_used, cap == 1);
                            if cap == 1 {
                                let baseline = absorb_ols_schur_at_cap(design(), weights, values.clone(),
                                    config, 0.0, expected_probe, cap).unwrap();
                                assert_eq!(result.residualized(), baseline.residualized());
                                assert_eq!(result.report.iterations, baseline.report.iterations);
                                if rhs == 7 && threads == 8 {
                                    let retained_config = crate::fe::AbsorbConfig { retain_coefficients: true, ..config };
                                    let retained = match try_absorb_ols_schur_narrow_at_cap(design(), weights,
                                        values.clone(), retained_config, cap).unwrap() {
                                        NarrowOlsSchurAttempt::Certified(result) => result,
                                        NarrowOlsSchurAttempt::Rejected { .. } => panic!("accepted retaining probe rejected"),
                                    };
                                    let (residuals, report, coefficients) = retained.into_full_parts();
                                    assert!(report.certified() && report.fallback_used);
                                    assert!(report.iterations <= config.map.max_iterations);
                                    let coefficients = coefficients.expect("capped original-input LSMR retains requested products");
                                    for column in 0..rhs {
                                        let coefficient = coefficients.rhs(column).unwrap();
                                        for row in 0..rows {
                                            let fitted = coefficient[coefficients.effect_offsets()[0] + left_codes[row] as usize]
                                                + coefficient[coefficients.effect_offsets()[1] + right_codes[row] as usize];
                                            assert!((fitted - (values[(row, column)] - residuals[(row, column)])).abs() <= 2e-9);
                                            assert!((residuals[(row, column)] - oracle.residualized()[(row, column)]).abs() <= 2e-9);
                                        }
                                    }
                                }
                            } else {
                                assert!(result.report.iterations <= OLS_SCHUR_PROBE_ITERATIONS + cap);
                            }
                            for (&a,&b) in result.residualized().as_slice().iter().zip(oracle.residualized().as_slice()) {
                                assert!((a-b).abs() <= 2e-9,
                                    "disconnected={disconnected} scale={scale:?} rhs={rhs} threads={threads} cap={cap}: {a} != {b}");
                            }
                        }
                        let mut limited = config;
                        limited.solver_memory_budget = Some(crate::memory::FeSolverMemoryBudget {
                            execution_bytes: 0, usable_bytes: schur_ols_workspace_bytes(&design(), rhs, threads).unwrap() - 1,
                        });
                        match try_absorb_ols_schur_narrow(design(), weights, values.clone(), limited) {
                            Err(error) => assert_eq!(error.code, ErrorCode::Allocation),
                            Ok(_) => panic!("under-budget pilot allocated its workspace"),
                        }
                    });
                }
            }
        }
        }
    }

    #[test]
    fn narrow_schur_unsafe_extra_column_preserves_probe_and_prior_failure() {
        let rows = 16_384;
        let first_codes = (0..rows).map(|r| (r % 257) as u32).collect::<Vec<_>>();
        let second_codes = (0..rows).map(|r| ((r * 17 + r / 257) % 127) as u32).collect::<Vec<_>>();
        let first_index = LevelIndex::from_codes(&first_codes, 257).unwrap();
        let second_index = LevelIndex::from_codes(&second_codes, 127).unwrap();
        let first = ValidatedBorrowedInterceptSource::new(&first_codes, 257, &first_index).unwrap();
        let second = ValidatedBorrowedInterceptSource::new(&second_codes, 127, &second_index).unwrap();
        let design = || BorrowedInterceptFeDesign::new(rows, vec![first.effect(), second.effect()]).unwrap();
        let mut columns = (0..5).map(|c| (0..rows)
            .map(|r| ((r * (c + 17) % 997) as f64 / 17.0).sin()).collect::<Vec<_>>()).collect::<Vec<_>>();
        for value in &mut columns[3] { *value *= 1e200; }
        columns[4].fill(1.0);
        let values = DenseMatrix::from_columns(&columns).unwrap();
        rayon::ThreadPoolBuilder::new().num_threads(8).build().unwrap().install(|| {
            let weights = BorrowedMapWeights::Unit;
            let expected = schur_ols_probe(&design(), weights, &values).unwrap();
            assert!(expected <= OLS_SCHUR_MAX_CONTRACTION);
            let mut owner = ReusableCategoricalMapAbsorber::new(design(), 5, 8).unwrap();
            owner.workspace.set_rhs_count(5).unwrap();
            owner.workspace.ols_norm_reduction = true;
            let validated = validate_weights(weights, rows).unwrap();
            update_ols_workspace_norms(&values, validated, &mut owner.workspace, true).unwrap();
            let (actual, state) = schur_probe_narrow_continuation(&owner.design, validated, &values,
                owner.two_fe_pcg.as_mut().unwrap(), 5, &owner.workspace, 1e-14).unwrap();
            assert_eq!(actual.to_bits(), expected.to_bits());
            assert!(state.is_none());
            drop(owner);
            let config = crate::fe::AbsorbConfig::default();
            let original_error = absorb_ols_schur_certified(design(), weights, values.clone(), config, 0.0, expected).unwrap_err();
            match try_absorb_ols_schur_narrow(design(), weights, values, config) {
                Err(error) => assert_eq!(error, original_error),
                Ok(_) => panic!("unsafe extra column bypassed the original failure"),
            }
        });
    }

    #[test]
    fn ols_schur_probe_uses_weighted_cuts_and_preserves_common_scale() {
        let rows = 16_384;
        let left_codes = (0..rows).map(|r| (r / 4) as u32).collect::<Vec<_>>();
        let right_codes = (0..rows)
            .map(|r| {
                let base = r / 4 % 128;
                (match r % 4 {
                    0 => base,
                    1 => (base + 1) % 128,
                    _ => (r * 37 + r / 512 * 19) % 128,
                }) as u32
            })
            .collect::<Vec<_>>();
        let left_index = LevelIndex::from_codes(&left_codes, rows / 4).unwrap();
        let right_index = LevelIndex::from_codes(&right_codes, 128).unwrap();
        let left =
            ValidatedBorrowedInterceptSource::new(&left_codes, rows / 4, &left_index).unwrap();
        let right = ValidatedBorrowedInterceptSource::new(&right_codes, 128, &right_index).unwrap();
        let design =
            BorrowedInterceptFeDesign::new(rows, vec![left.effect(), right.effect()]).unwrap();
        let values = DenseMatrix::from_columns(
            &(0..3)
                .map(|c| {
                    (0..rows)
                        .map(|r| {
                            ((r * (c + 17) % 997) as f64 / 17.0).sin()
                                + (right_codes[r] as f64 * 0.049).sin()
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let unit = schur_ols_probe(&design, BorrowedMapWeights::Unit, &values).unwrap();
        assert!(unit <= OLS_SCHUR_MAX_CONTRACTION, "unit {unit}");
        let constant = vec![128.0; rows];
        assert_eq!(
            unit,
            schur_ols_probe(&design, BorrowedMapWeights::Provided(&constant), &values).unwrap()
        );
        let weights = (0..rows)
            .map(|r| if r % 4 < 2 { 1.0 } else { 1e-10 })
            .collect::<Vec<_>>();
        let scaled = weights.iter().map(|w| w * 1024.0).collect::<Vec<_>>();
        let weighted =
            schur_ols_probe(&design, BorrowedMapWeights::Provided(&weights), &values).unwrap();
        assert!(
            weighted > OLS_SCHUR_MAX_CONTRACTION,
            "weighted cut {weighted}"
        );
        assert_eq!(
            weighted,
            schur_ols_probe(&design, BorrowedMapWeights::Provided(&scaled), &values).unwrap()
        );
        for threads in [8, 16] {
            rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
                let design = || BorrowedInterceptFeDesign::new(rows, vec![left.effect(), right.effect()]).unwrap();
                let expected = schur_ols_probe(&design(), BorrowedMapWeights::Provided(&weights), &values).unwrap();
                let owned = values.clone();
                let original_pointer = owned.as_slice().as_ptr();
                match try_absorb_ols_schur_narrow(design(), BorrowedMapWeights::Provided(&weights),
                    owned, crate::fe::AbsorbConfig::default()).unwrap() {
                    NarrowOlsSchurAttempt::Rejected { values: original, contraction, peak, probe_seconds } => {
                        assert_eq!(contraction.to_bits(), expected.to_bits());
                        assert_eq!(original, values);
                        assert_eq!(original.as_slice().as_ptr(), original_pointer);
                        assert_eq!(peak.bytes().unwrap() as u128, schur_ols_workspace_bytes(&design(), 3, threads).unwrap());
                        assert!(probe_seconds > 0.0);
                    }
                    NarrowOlsSchurAttempt::Certified(_) => panic!("weighted cut passed the unchanged admission threshold"),
                }
            });
        }
        for invalid in [0.0, -1.0, f64::INFINITY, f64::NAN] {
            let mut bad = weights.clone();
            bad[7] = invalid;
            assert_eq!(
                schur_ols_probe(&design, BorrowedMapWeights::Provided(&bad), &values)
                    .unwrap_err()
                    .code,
                ErrorCode::InvalidData
            );
        }
    }

    #[test]
    fn ols_parallel_certificate_matches_canonical_moments_and_errors() {
        let rows = 65_536;
        let codes = (0..rows).map(|r| (r % 8193) as u32).collect::<Vec<_>>();
        let index = LevelIndex::from_codes(&codes, 8193).unwrap();
        let source = ValidatedBorrowedInterceptSource::new(&codes, 8193, &index).unwrap();
        let design = BorrowedInterceptFeDesign::new(rows, vec![source.effect()]).unwrap();
        for width in [8, 12, 17] {
            let columns = (0..width)
                .map(|c| {
                    (0..rows)
                        .map(|r| {
                            if c == 0 {
                                0.0
                            } else {
                                ((r * (c + 17) % 997) as f64 / 17.0).sin()
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let values = DenseMatrix::from_columns(&columns).unwrap();
            let raw_weights = (0..rows)
                .map(|r| 0.01 + (r % 19) as f64 / 7.0)
                .collect::<Vec<_>>();
            for threads in [4, 16] {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        for weights in [
                            BorrowedMapWeights::Unit,
                            BorrowedMapWeights::Provided(&raw_weights),
                        ] {
                            let weights = validate_weights(weights, rows).unwrap();
                            let mut reservation = SystemWorkspaceReservation;
                            let mut work =
                                BorrowedMapWorkspace::new(&design, rows, width, &mut reservation).unwrap();
                            for scale in [1000.0, 0.0, 1e-200] {
                                work.reference_norms.fill(scale);
                                work.residual_norms.fill(scale);
                                work.ols_norm_reduction = false;
                                let canonical = certify_with_current_norms(
                                    &design, weights, &values, 1e-8, &mut work,
                                );
                                let maxima = work.rhs_relative_infinity_norms.clone();
                                work.ols_norm_reduction = true;
                                let parallel = certify_with_current_norms(
                                    &design, weights, &values, 1e-8, &mut work,
                                );
                                match (canonical, parallel) {
                                    (Ok(a), Ok(b)) => {
                                        assert_eq!(a.passed, b.passed);
                                        assert_eq!(a.absolute_infinity_norm, b.absolute_infinity_norm);
                                        assert_eq!(a.relative_infinity_norm, b.relative_infinity_norm);
                                        assert!(
                                            (a.relative_two_norm / b.relative_two_norm - 1.0).abs()
                                                < 1e-12
                                        );
                                        assert_eq!(maxima, work.rhs_relative_infinity_norms);
                                    }
                                    (Err(a), Err(b)) => {
                                        assert_eq!(a.code, b.code);
                                        assert_eq!(a.message, b.message);
                                    }
                                    other => panic!("parallel certificate changed outcome: {other:?}"),
                                }
                            }
                        }
                    });
            }
        }
    }

    #[test]
    fn ols_schur_readiness_includes_group_weight_certificate_scale() {
        let codes = [0_u32; 100];
        let index = LevelIndex::from_codes(&codes, 1).unwrap();
        let source = ValidatedBorrowedInterceptSource::new(&codes, 1, &index).unwrap();
        let design =
            BorrowedInterceptFeDesign::new(100, vec![source.effect(), source.effect()]).unwrap();
        let mut reservation = SystemWorkspaceReservation;
        let mut graph = TwoFePcgWorkspace::new(&design, 1, &mut reservation).unwrap();
        let mut workspace = BorrowedMapWorkspace::new(&design, 100, 1, &mut reservation).unwrap();
        graph.right_weights[0] = 100.0;
        graph.residual[0] = 2e-7;
        workspace.reference_norms[0] = 1.0;
        // The mean-only rule would accept 2e-9 at tolerance 1e-8.
        assert!(!two_fe_pcg_rhs_ready(&graph, &workspace, 0, 1, 1e-8).unwrap());
        graph.residual[0] = 2e-8;
        assert!(two_fe_pcg_rhs_ready(&graph, &workspace, 0, 1, 1e-8).unwrap());
    }

    #[test]
    fn ols_norms_are_scaled_and_bit_stable_across_worker_counts() {
        let rows = 16_391;
        let design = BorrowedInterceptFeDesign::new(rows, Vec::new()).unwrap();
        let columns = [
            vec![0.0; rows],
            (0..rows)
                .map(|r| if r % 2 == 0 { 1e200 } else { -1e-200 })
                .collect(),
            (0..rows).map(|r| (r as f64).sin() * 1e-150).collect(),
        ];
        let values = DenseMatrix::from_columns(&columns).unwrap();
        let raw_weights = (0..rows).map(|r| 1.0 + (r % 17) as f64).collect::<Vec<_>>();
        let weights = validate_weights(BorrowedMapWeights::Provided(&raw_weights), rows).unwrap();
        let run = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    let mut reservation = SystemWorkspaceReservation;
                    let mut workspace =
                        BorrowedMapWorkspace::new(&design, rows, 3, &mut reservation).unwrap();
                    workspace.ols_norm_reduction = true;
                    update_ols_workspace_norms(&values, weights, &mut workspace, true).unwrap();
                    workspace.reference_norms
                })
        };
        let serial = run(1);
        for threads in [4, 16] {
            assert_eq!(
                serial.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                run(threads).iter().map(|x| x.to_bits()).collect::<Vec<_>>()
            );
        }
        for c in 0..3 {
            let scale = columns[c].iter().map(|x| x.abs()).fold(0.0, f64::max);
            let expected = if scale == 0.0 {
                0.0
            } else {
                scale
                    * (0..rows)
                        .map(|r| weights.normalized(r) * (columns[c][r] / scale).powi(2))
                        .sum::<f64>()
                        .sqrt()
            };
            assert!((serial[c] - expected).abs() <= expected * 1e-12);
        }
        let mut reservation = SystemWorkspaceReservation;
        let mut workspace = BorrowedMapWorkspace::new(&design, rows, 3, &mut reservation).unwrap();
        workspace.ols_norm_reduction = true;
        let invalid =
            DenseMatrix::from_columns(&[vec![f64::MAX; rows], vec![0.0; rows], vec![0.0; rows]])
                .unwrap();
        assert!(update_ols_workspace_norms(&invalid, weights, &mut workspace, false).is_err());
    }

    #[test]
    fn retained_lsmr_reports_reused_operator_overlap_and_preserves_results() {
        let rows = 256;
        let first = (0..rows).map(|row| (row % 8) as u32).collect::<Vec<_>>();
        let second = (0..rows)
            .map(|row| ((row / 8) % 7) as u32)
            .collect::<Vec<_>>();
        let first_index = LevelIndex::from_codes(&first, 8).unwrap();
        let second_index = LevelIndex::from_codes(&second, 7).unwrap();
        let first_source = ValidatedBorrowedInterceptSource::new(&first, 8, &first_index).unwrap();
        let second_source =
            ValidatedBorrowedInterceptSource::new(&second, 7, &second_index).unwrap();
        for solver in [FeSolverKind::Lsmr, FeSolverKind::Auto] {
            let design = BorrowedInterceptFeDesign::new(
                rows,
                vec![first_source.effect(), second_source.effect()],
            )
            .unwrap();
            let config = crate::fe::AbsorbConfig {
                solver,
                solver_memory_budget: Some(FeSolverMemoryBudget {
                    execution_bytes: 137,
                    usable_bytes: 1 << 30,
                }),
                ..crate::fe::AbsorbConfig::default()
            };
            let input = DenseMatrix::from_columns(&[(0..rows)
                .map(|row| (row as f64 * 0.17).sin())
                .collect::<Vec<_>>()])
            .unwrap();
            let mut session =
                BorrowedLsmrAbsorber::new(design, BorrowedMapWeights::Unit, config).unwrap();
            if solver == FeSolverKind::Lsmr {
                let estimate = estimate_fe_solver_memory(
                    rows,
                    1,
                    &[8, 7],
                    session
                        .config
                        .lsmr
                        .threads
                        .resolved()
                        .max(rayon::current_num_threads()),
                    session.config.lsmr.local_size,
                )
                .unwrap();
                let required = estimate.lsmr_bytes(LsmrPreconditioner::Diagonal);
                session.config.solver_memory_budget = Some(FeSolverMemoryBudget {
                    execution_bytes: 137,
                    usable_bytes: 136 + required,
                });
                assert_eq!(
                    session.absorb(input.clone()).unwrap_err().code,
                    ErrorCode::Allocation
                );
                assert_eq!(session.prepared.builds, 0);
                session
                    .config
                    .solver_memory_budget
                    .as_mut()
                    .unwrap()
                    .usable_bytes += 1;
            }
            let cold = session.absorb(input.clone()).unwrap();
            assert_eq!(session.prepared.builds, 1);
            assert!(cold.report().solver_workspace_peak.bytes().is_some());
            let warm = session.absorb(input).unwrap();
            assert_eq!(session.prepared.builds, 1);
            assert!(warm.report().solver_workspace_peak.bytes().is_some());
            assert!(session.prepared.retained_bytes > 0);
            let pilot = BorrowedMapWorkspace::planned_payload_bytes(
                &session.design,
                rows,
                1,
                rayon::current_num_threads(),
            )
            .unwrap();
            assert!(
                warm.report().solver_workspace_peak.bytes().unwrap() as u128
                    >= pilot + session.prepared.retained_bytes
            );
            assert_eq!(cold.residualized(), warm.residualized());
            assert_eq!(cold.report().certificate, warm.report().certificate);
            if solver == FeSolverKind::Auto {
                session.config.solver_memory_budget = Some(FeSolverMemoryBudget {
                    execution_bytes: 137,
                    usable_bytes: 137 + pilot,
                });
                let already_projected = warm.residualized().clone();
                let bounded = session.absorb(already_projected.clone()).unwrap();
                assert!(session.prepared.solver.is_none());
                assert_eq!(session.prepared.retained_bytes, 0);
                assert_eq!(bounded.residualized(), &already_projected);
                assert_eq!(
                    bounded.report().solver_workspace_peak.bytes(),
                    Some(pilot as u64)
                );
            }
        }
    }

    #[test]
    fn lsmr_postprocess_admission_preserves_attempt_peak_and_checks_parent_overlap() {
        let estimate = estimate_fe_solver_memory(128, 3, &[16, 8], 1, Some(4)).unwrap();
        let required = estimate.lsmr_bytes(LsmrPreconditioner::Off);
        let parent = 137;
        let base = 211;
        let budget = FeSolverMemoryBudget {
            execution_bytes: base,
            usable_bytes: base + parent + required,
        };
        let mut memory =
            BorrowedLsmrCallMemory::new(estimate, LsmrPreconditioner::Off, Some(budget), parent)
                .unwrap();
        assert_eq!(memory.peak.bytes(), Some((parent + required) as u64));
        memory.include(required).unwrap();
        let prior = memory.peak;
        assert_eq!(
            memory.include(required + 1).unwrap_err().code,
            ErrorCode::Allocation
        );
        assert_eq!(memory.peak, prior);
        assert_eq!(
            memory.include(u128::MAX).unwrap_err().code,
            ErrorCode::Allocation
        );
        assert_eq!(memory.peak, prior);
        let short = FeSolverMemoryBudget {
            usable_bytes: budget.usable_bytes - 1,
            ..budget
        };
        assert!(BorrowedLsmrCallMemory::new(
            estimate,
            LsmrPreconditioner::Off,
            Some(short),
            parent
        )
        .is_err());
        memory.budget = None;
        memory.include_payload(123, required).unwrap();
        assert_eq!(
            memory.peak.bytes(),
            Some((parent + required + 123 + estimate.lsmr_result_metadata_bytes) as u64)
        );
    }

    #[test]
    fn memory_plan_includes_coefficient_layouts_and_all_map_banks() {
        let codes = [0, 0, 1, 1];
        let index = LevelIndex::from_codes(&codes, 2).unwrap();
        let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).unwrap();
        for effects in [1, 3, 512] {
            let design =
                BorrowedInterceptFeDesign::new(4, (0..effects).map(|_| source.effect()).collect())
                    .unwrap();
            for rhs in [1, 19] {
                let batch = FeCoefficientBatch::intercept_zeros(&design, rhs).unwrap();
                let estimate =
                    estimate_fe_solver_memory(4, rhs, &vec![2; effects], 1, Some(4)).unwrap();
                assert_eq!(estimate.coefficient_batch_bytes, batch.retained_bytes());
                assert!(estimate.coefficient_batch_bytes > (effects * 2 * rhs * 8) as u128);
            }
        }
        let rows = 16_384;
        let codes: Vec<u32> = (0..rows).map(|r| (r % 127) as u32).collect();
        let index = LevelIndex::from_codes(&codes, 127).unwrap();
        let source = ValidatedBorrowedInterceptSource::new(&codes, 127, &index).unwrap();
        let design = BorrowedInterceptFeDesign::new(rows, vec![source.effect()]).unwrap();
        for threads in [1, 4] {
            for rhs in [1, 19, 65] {
                let workspace = BorrowedMapWorkspace::new_with_parallelism(
                    &design,
                    rows,
                    rhs,
                    threads,
                    &mut SystemWorkspaceReservation,
                )
                .unwrap();
                assert_eq!(
                    workspace.payload_peak().unwrap().bytes().unwrap() as u128,
                    BorrowedMapWorkspace::planned_payload_bytes(&design, rows, rhs, threads)
                        .unwrap()
                );
                if rhs == 1 {
                    let matrix = DenseMatrix::zeros(rows, 1).unwrap();
                    let coefficient = FeCoefficientBatch::intercept_zeros(&design, 1).unwrap();
                    assert_eq!(
                        borrowed_cg_recovery_workspace_bytes(rows, 1, 127, 127, threads).unwrap(),
                        workspace.payload_peak().unwrap().bytes().unwrap() as u128
                            + 4 * matrix.storage_capacity() as u128 * 8
                            + 4 * coefficient.retained_bytes()
                            + 8 * 8,
                    );
                }
            }
        }
    }

    #[test]
    fn retained_lsmr_reuses_preparation_and_preserves_each_batch() {
        let rows = 2048;
        let codes = [97_usize, 89, 9].map(|levels| {
            (0..rows)
                .map(|r| ((r * 37 + r / 11) % levels) as u32)
                .collect::<Vec<_>>()
        });
        let indexes = codes
            .iter()
            .zip([97, 89, 9])
            .map(|(c, levels)| LevelIndex::from_codes(c, levels).unwrap())
            .collect::<Vec<_>>();
        let sources = codes
            .iter()
            .zip(&indexes)
            .zip([97, 89, 9])
            .map(|((c, index), levels)| {
                ValidatedBorrowedInterceptSource::new(c, levels, index).unwrap()
            })
            .collect::<Vec<_>>();
        let make_design = || {
            BorrowedInterceptFeDesign::new(rows, sources.iter().map(|s| s.effect()).collect())
                .unwrap()
        };
        let raw_weights = (0..rows).map(|r| 0.5 + (r % 7) as f64).collect::<Vec<_>>();
        for threads in [1, 7] {
            let team = crate::parallel::ThreadTeam::new(
                crate::options::ThreadConfig::from_requested(threads, true).unwrap(),
            );
            team.install(|| {
                for weights in [
                    BorrowedMapWeights::Unit,
                    BorrowedMapWeights::Provided(&raw_weights),
                ] {
                    for solver in [FeSolverKind::Auto, FeSolverKind::Lsmr] {
                        let config = super::super::AbsorbConfig {
                            solver,
                            map: config(),
                            lsmr: LsmrConfig {
                                tolerance: 1e-12,
                                preconditioner: LsmrPreconditioner::Schwarz,
                                ..LsmrConfig::default()
                            },
                            retain_coefficients: true,
                            ..super::super::AbsorbConfig::default()
                        };
                        let mut session =
                            BorrowedLsmrAbsorber::new(make_design(), weights, config).unwrap();
                        for (batch, columns) in [1, 3, 2].into_iter().enumerate() {
                            let rhs = DenseMatrix::from_columns(
                                &(0..columns)
                                    .map(|j| {
                                        (0..rows)
                                            .map(|r| {
                                                (r as f64 * (j + batch + 3) as f64 * 0.137).sin()
                                                    + codes[0][r] as f64 * 0.01
                                            })
                                            .collect::<Vec<_>>()
                                    })
                                    .collect::<Vec<_>>(),
                            )
                            .unwrap();
                            let expected = if solver == FeSolverKind::Auto {
                                absorb_borrowed_intercept_auto_direct_lsmr_certified(
                                    &make_design(),
                                    weights,
                                    rhs.clone(),
                                    config.map,
                                    config.lsmr,
                                    None,
                                    false,
                                    true,
                                )
                            } else {
                                absorb_borrowed_intercept_lsmr_certified(
                                    &make_design(),
                                    weights,
                                    rhs.clone(),
                                    config.lsmr,
                                    true,
                                )
                            }
                            .unwrap();
                            let actual = session.absorb(rhs).unwrap();
                            assert_eq!(actual.residualized(), expected.residualized());
                            assert_eq!(actual.report().certificate, expected.report().certificate);
                            assert_eq!(actual.report().iterations, expected.report().iterations);
                            assert_eq!(actual.into_full_parts().2, expected.into_full_parts().2);
                            assert_eq!(session.prepared.builds, 1);
                            assert_eq!(session.prepared.batches, batch + 1);
                        }
                        let mut constrained = config;
                        constrained.solver_memory_budget = Some(FeSolverMemoryBudget {
                            execution_bytes: 0,
                            usable_bytes: 0,
                        });
                        let mut rejected =
                            BorrowedLsmrAbsorber::new(make_design(), weights, constrained).unwrap();
                        let rhs = DenseMatrix::from_columns(&[(0..rows)
                            .map(|r| (r as f64 * 0.137).sin())
                            .collect::<Vec<_>>()])
                        .unwrap();
                        assert_eq!(
                            rejected.absorb(rhs).unwrap_err().code,
                            ErrorCode::Allocation
                        );
                        assert_eq!(rejected.prepared.builds, 0);
                    }
                }
            })
            .unwrap();
        }
    }

    #[test]
    fn reusable_one_fe_matches_weighted_group_means_and_reuses_warm_residual() {
        for threads in [1, 4, 16] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            pool.install(|| {
                let n = 16_384;
                let codes: Vec<u32> = (0..n).map(|row| (row / 8) as u32).collect();
                let index = LevelIndex::from_codes(&codes, n / 8).unwrap();
                let source = ValidatedBorrowedInterceptSource::new(&codes, n / 8, &index).unwrap();
                let design = BorrowedInterceptFeDesign::new(n, vec![source.effect()]).unwrap();
                let narrow_design =
                    BorrowedInterceptFeDesign::new(n, vec![source.effect()]).unwrap();
                let narrow =
                    ReusableCategoricalMapAbsorber::new(narrow_design, 3, threads).unwrap();
                assert!(narrow.two_fe_pcg.is_none());
                assert!(narrow.accelerator_x0.is_none());
                assert!(narrow.accelerator_x1.is_none());
                assert!(narrow.accelerator_x2.is_none());
                drop(narrow);
                let mut projector =
                    ReusableCategoricalMapAbsorber::new(design, 17, threads).unwrap();
                assert!(projector.two_fe_pcg.is_none());
                assert!(projector.accelerator_x0.is_none());
                assert!(projector.accelerator_x1.is_none());
                assert!(projector.accelerator_x2.is_none());
                let capacities = projector.capacities();
                for (frame, width) in [(0, 17), (1, 1), (2, 17)] {
                    let weights: Vec<f64> =
                        (0..n).map(|r| 1.0 + ((r + frame) % 7) as f64).collect();
                    let columns: Vec<Vec<f64>> = (0..width)
                        .map(|j| {
                            (0..n)
                                .map(|r| {
                                    if j == 0 {
                                        (r / 8) as f64
                                    } else {
                                        ((r * (j + 1) + frame) % 19) as f64
                                    }
                                })
                                .collect()
                        })
                        .collect();
                    let raw = DenseMatrix::from_columns(&columns).unwrap();
                    let actual = projector
                        .absorb(BorrowedMapWeights::Provided(&weights), raw, config())
                        .unwrap();
                    assert!(actual.report().certified());
                    assert_eq!(actual.report().iterations, 1);
                    for column in 0..width {
                        for group in 0..n / 8 {
                            let rows = group * 8..group * 8 + 8;
                            let denominator: f64 = rows.clone().map(|r| weights[r]).sum();
                            let mean = rows
                                .clone()
                                .map(|r| weights[r] * columns[column][r])
                                .sum::<f64>()
                                / denominator;
                            for row in rows {
                                assert!(
                                    (actual.residualized()[(row, column)]
                                        - (columns[column][row] - mean))
                                        .abs()
                                        < 1e-11
                                );
                            }
                        }
                    }
                    let within = actual.residualized().clone();
                    let warm = projector
                        .absorb(
                            BorrowedMapWeights::Provided(&weights),
                            within.clone(),
                            config(),
                        )
                        .unwrap();
                    assert_eq!(warm.report().iterations, 0);
                    assert_eq!(warm.residualized(), &within);
                    assert_eq!(projector.capacities(), capacities);
                }
            });
        }
    }

    #[test]
    fn reusable_selected_norms_preserve_mapping_and_reject_invalid_selections() {
        let codes = [0_u32, 0, 1, 1, 2, 2, 3, 3];
        let index = LevelIndex::from_codes(&codes, 4).unwrap();
        let source = ValidatedBorrowedInterceptSource::new(&codes, 4, &index).unwrap();
        let design = BorrowedInterceptFeDesign::new(codes.len(), vec![source.effect()]).unwrap();
        let mut projector = ReusableCategoricalMapAbsorber::new(design, 5, 1).unwrap();
        let capacities = projector.capacities();
        let weights = [1.0, 3.0, 2.0, 5.0, 4.0, 7.0, 6.0, 8.0];
        let matrix = || {
            DenseMatrix::from_columns(&[
                vec![1.0, 3.0, 2.0, -1.0, 5.0, 4.0, 6.0, 8.0],
                vec![1.0; 8],
                vec![1e140, 2e140, -3e140, 1e140, 2e140, 5e140, -1e140, 3e140],
                vec![0.0; 8],
                vec![1e-140, 2e-140, 5e-140, 3e-140, 0.0, -1e-140, 2e-140, 4e-140],
            ])
            .unwrap()
        };
        let mut norms = [-1.0; 3];
        for selection in [[4, 0, 2], [2, 2, 0]] {
            let (actual, reused) = projector
                .absorb_with_selected_norms(
                    BorrowedMapWeights::Provided(&weights),
                    matrix(),
                    config(),
                    8.0,
                    selection.into_iter(),
                    &mut norms,
                )
                .unwrap();
            assert!(reused);
            for (position, &column) in selection.iter().enumerate() {
                let expected = (0..codes.len()).fold(0.0_f64, |norm, row| {
                    norm.hypot((weights[row] / 8.0).sqrt() * actual.residualized()[(row, column)])
                });
                assert!((norms[position] / expected - 1.0).abs() < 2e-15);
            }
        }
        let saved = norms;
        assert!(projector
            .absorb_with_selected_norms(
                BorrowedMapWeights::Provided(&weights),
                matrix(),
                config(),
                8.0,
                [0, 5, 2].into_iter(),
                &mut norms,
            )
            .is_err());
        assert_eq!(norms, saved);
        assert!(projector
            .absorb_with_selected_norms(
                BorrowedMapWeights::Provided(&weights),
                matrix(),
                config(),
                8.0,
                0..2,
                &mut norms,
            )
            .is_err());
        assert_eq!(norms, saved);
        let (_, reused) = projector
            .absorb_with_selected_norms(
                BorrowedMapWeights::Provided(&weights),
                matrix(),
                config(),
                4.0,
                0..3,
                &mut norms,
            )
            .unwrap();
        assert!(!reused);
        assert_eq!(norms, saved);
        assert_eq!(projector.capacities(), capacities);
    }

    #[test]
    fn reusable_one_fe_trailing_norms_follow_current_values_weights_and_width() {
        let codes = [0_u32, 0, 1, 1, 2, 2, 3, 3];
        let index = LevelIndex::from_codes(&codes, 4).unwrap();
        let source = ValidatedBorrowedInterceptSource::new(&codes, 4, &index).unwrap();
        let design = BorrowedInterceptFeDesign::new(codes.len(), vec![source.effect()]).unwrap();
        let mut reusable = ReusableCategoricalMapAbsorber::new(design, 3, 1).unwrap();
        let capacities = reusable.capacities();
        let columns = [
            vec![1.0, 2.0, 3.0, 5.0, -1.0, 4.0, 6.0, 3.0],
            vec![1e140, 2e140, -3e140, 1e140, 2e140, 5e140, -1e140, 3e140],
            vec![1e-140, 2e-140, 5e-140, 3e-140, 0.0, -1e-140, 2e-140, 4e-140],
        ];
        let first = [1.0, 3.0, 2.0, 5.0, 4.0, 7.0, 6.0, 8.0];
        let next = [8e100, 1e100, 7e100, 2e100, 6e100, 3e100, 5e100, 4e100];
        let mut output = [0.0; 2];
        for (weights, scale) in [
            (BorrowedMapWeights::Unit, 1.0),
            (BorrowedMapWeights::Provided(&first), 8.0),
            (BorrowedMapWeights::Provided(&next), 8e100),
        ] {
            for width in [3, 1, 2, 3] {
                let norms = &mut output[..width - 1];
                let (actual, reused) = reusable
                    .absorb_with_trailing_norms(
                        weights,
                        DenseMatrix::from_columns(&columns[..width]).unwrap(),
                        config(),
                        scale,
                        norms,
                    )
                    .unwrap();
                assert!(reused);
                assert!(actual.report().iterations > 0);
                for (column, &norm) in norms.iter().enumerate() {
                    // Hypot is independent of the scaled-square accumulator
                    // and remains finite at both extreme column scales.
                    let expected = (0..codes.len()).fold(0.0_f64, |sum, row| {
                        let w = match weights {
                            BorrowedMapWeights::Unit => 1.0,
                            BorrowedMapWeights::Provided(w) => w[row] / scale,
                        };
                        sum.hypot(w.sqrt() * actual.residualized()[(row, column + 1)])
                    });
                    assert!((norm / expected - 1.0).abs() < 2e-15);
                }
                let (values, _) = actual.into_parts();
                let saved = norms.to_vec();
                norms.fill(-1.0);
                let (warm, reused) = reusable
                    .absorb_with_trailing_norms(weights, values, config(), scale, norms)
                    .unwrap();
                assert!(reused);
                assert_eq!(warm.report().iterations, 0);
                assert_eq!(norms, saved.as_slice());
                assert_eq!(reusable.capacities(), capacities);
            }
        }
    }

    #[test]
    fn reusable_trailing_norms_do_not_publish_on_mismatch_or_failure() {
        let codes = [0_u32, 0, 1, 1, 2, 2, 3, 3];
        let index = LevelIndex::from_codes(&codes, 4).unwrap();
        let source = ValidatedBorrowedInterceptSource::new(&codes, 4, &index).unwrap();
        let second_codes = [0_u32, 1, 0, 1, 0, 1, 0, 1];
        let second_index = LevelIndex::from_codes(&second_codes, 2).unwrap();
        let second = ValidatedBorrowedInterceptSource::new(&second_codes, 2, &second_index).unwrap();
        let weights = [1.0, 3.0, 2.0, 5.0, 4.0, 7.0, 6.0, 8.0];
        let values = || {
            DenseMatrix::from_columns(&[
                vec![1.0, 3.0, 2.0, -1.0, 5.0, 4.0, 6.0, 8.0],
                vec![2.0, -1.0, 5.0, 3.0, 4.0, 6.0, -2.0, 7.0],
            ])
            .unwrap()
        };
        for multi_fe in [false, true] {
            let effects = if multi_fe {
                vec![source.effect(), second.effect()]
            } else {
                vec![source.effect()]
            };
            let design = BorrowedInterceptFeDesign::new(codes.len(), effects).unwrap();
            let mut reusable = ReusableCategoricalMapAbsorber::new(design, 2, 1).unwrap();
            reusable.enable_checkpoint_lsmr();
            let mut norms = [-1.0];
            for scale in [8.0, 4.0] {
                let saved = norms;
                let (_, reused) = reusable
                    .absorb_with_trailing_norms(
                        BorrowedMapWeights::Provided(&weights),
                        values(),
                        config(),
                        scale,
                        &mut norms,
                    )
                    .unwrap();
                assert_eq!(reused, !multi_fe && scale == 8.0);
                if !reused {
                    assert_eq!(norms, saved);
                }
            }
            let saved = norms;
            let mut invalid = values();
            invalid[(0, 0)] = f64::NAN;
            assert!(reusable
                .absorb_with_trailing_norms(
                    BorrowedMapWeights::Unit,
                    invalid,
                    config(),
                    1.0,
                    &mut norms,
                )
                .is_err());
            assert_eq!(norms, saved);
            let mut oversized = [-1.0; 3];
            assert!(reusable
                .absorb_with_trailing_norms(
                    BorrowedMapWeights::Unit,
                    values(),
                    config(),
                    1.0,
                    &mut oversized,
                )
                .is_err());
            assert_eq!(oversized, [-1.0; 3]);
        }
    }

    #[test]
    fn reusable_categorical_map_matches_one_shot_across_weight_frames_and_rhs_widths() {
        let first_codes = [0_u32, 0, 1, 1, 2, 2, 0, 1, 2];
        let second_codes = [0_u32, 1, 0, 1, 0, 1, 2, 2, 2];
        let first_index = LevelIndex::from_codes(&first_codes, 3).expect("first index");
        let second_index = LevelIndex::from_codes(&second_codes, 3).expect("second index");
        let first = ValidatedBorrowedInterceptSource::new(&first_codes, 3, &first_index)
            .expect("first source");
        let second = ValidatedBorrowedInterceptSource::new(&second_codes, 3, &second_index)
            .expect("second source");
        let columns = [
            vec![1.0, 2.0, 4.0, 8.0, 3.0, 5.0, 9.0, 7.0, 6.0],
            vec![2.0, -1.0, 3.0, 0.5, 4.0, 1.0, -2.0, 8.0, 6.0],
            vec![0.0, 1.0, 0.5, 2.0, -1.0, 3.0, 4.0, -2.0, 5.0],
        ];
        let first_weights = [1.0, 3.0, 2.0, 5.0, 4.0, 7.0, 6.0, 8.0, 9.0];
        let second_weights = [9.0, 2.0, 8.0, 3.0, 7.0, 4.0, 6.0, 5.0, 1.0];

        let expected_design = BorrowedInterceptFeDesign::new(
            first_codes.len(),
            vec![first.effect(), second.effect()],
        )
        .expect("expected design");
        let oracle_config = MapConfig {
            tolerance: 1e-14,
            ..config()
        };
        let expected = absorb_borrowed_intercept_map_certified(
            &expected_design,
            BorrowedMapWeights::Provided(&first_weights),
            DenseMatrix::from_columns(&columns).expect("expected matrix"),
            oracle_config,
        )
        .expect("one-shot map");

        let reusable_design = BorrowedInterceptFeDesign::new(
            first_codes.len(),
            vec![first.effect(), second.effect()],
        )
        .expect("reusable design");
        let mut reusable = ReusableCategoricalMapAbsorber::new(reusable_design, columns.len(), 1)
            .expect("reusable map");
        let initial_capacities = reusable.capacities();
        let actual = reusable
            .absorb(
                BorrowedMapWeights::Provided(&first_weights),
                DenseMatrix::from_columns(&columns).expect("actual matrix"),
                config(),
            )
            .expect("reusable first frame");
        assert!(actual.report().certified());
        for (&left, &right) in actual
            .residualized()
            .as_slice()
            .iter()
            .zip(expected.residualized().as_slice())
        {
            assert!(
                (left - right).abs() <= 1e-12,
                "{left} != {right}; report={:?}",
                actual.report()
            );
        }

        let narrower = [columns[0].clone(), columns[2].clone()];
        let expected_design = BorrowedInterceptFeDesign::new(
            first_codes.len(),
            vec![first.effect(), second.effect()],
        )
        .expect("second expected design");
        let expected = absorb_borrowed_intercept_map_certified(
            &expected_design,
            BorrowedMapWeights::Provided(&second_weights),
            DenseMatrix::from_columns(&narrower).expect("second expected matrix"),
            oracle_config,
        )
        .expect("second one-shot map");
        let actual = reusable
            .absorb(
                BorrowedMapWeights::Provided(&second_weights),
                DenseMatrix::from_columns(&narrower).expect("second actual matrix"),
                config(),
            )
            .expect("reusable second frame");
        assert_eq!(reusable.capacities(), initial_capacities);
        for (&left, &right) in actual
            .residualized()
            .as_slice()
            .iter()
            .zip(expected.residualized().as_slice())
        {
            assert!((left - right).abs() <= 1e-12, "{left} != {right}");
        }
    }

    #[test]
    fn checkpoint_lsmr_recertifies_and_numerical_failure_falls_back_transactionally() {
        let first_codes = [0_u32, 0, 1, 1, 2, 2, 0, 1, 2];
        let second_codes = [0_u32, 1, 0, 1, 0, 1, 2, 2, 2];
        let first_index = LevelIndex::from_codes(&first_codes, 3).expect("first index");
        let second_index = LevelIndex::from_codes(&second_codes, 3).expect("second index");
        let first = ValidatedBorrowedInterceptSource::new(&first_codes, 3, &first_index)
            .expect("first source");
        let second = ValidatedBorrowedInterceptSource::new(&second_codes, 3, &second_index)
            .expect("second source");
        let design = BorrowedInterceptFeDesign::new(
            first_codes.len(),
            vec![first.effect(), second.effect()],
        )
        .expect("design");
        let columns = [
            vec![1.0, 2.0, 4.0, 8.0, 3.0, 5.0, 9.0, 7.0, 6.0],
            vec![2.0, -1.0, 3.0, 0.5, 4.0, 1.0, -2.0, 8.0, 6.0],
        ];
        let weights = [1.0, 3.0, 2.0, 5.0, 4.0, 7.0, 6.0, 8.0, 9.0];
        let mut reusable = ReusableCategoricalMapAbsorber::new(design, columns.len(), 1)
            .expect("reusable projector");
        reusable.enable_checkpoint_lsmr();

        reusable.inject_checkpoint_lsmr_certificate_failure();
        let fallback = reusable
            .absorb(
                BorrowedMapWeights::Provided(&weights),
                DenseMatrix::from_columns(&columns).expect("fallback values"),
                config(),
            )
            .expect("certified MAP fallback");
        assert_eq!(fallback.report().requested_solver, FeSolverKind::Lsmr);
        assert_eq!(fallback.report().used_solver, FeSolverKind::Map);
        assert!(fallback.report().fallback_used);
        assert_eq!(fallback.report().checkpoint_retries, 1);
        assert!(fallback.report().certified());

        let accelerated = reusable
            .absorb(
                BorrowedMapWeights::Provided(&weights),
                DenseMatrix::from_columns(&columns).expect("accelerated values"),
                config(),
            )
            .expect("recertified LSMR checkpoint");
        assert_eq!(accelerated.report().requested_solver, FeSolverKind::Lsmr);
        assert_eq!(accelerated.report().used_solver, FeSolverKind::Lsmr);
        assert!(!accelerated.report().fallback_used);
        assert_eq!(accelerated.report().checkpoint_retries, 0);
        assert!(accelerated.report().certification_seconds >= 0.0);
        for (&left, &right) in accelerated
            .residualized()
            .as_slice()
            .iter()
            .zip(fallback.residualized().as_slice())
        {
            assert!((left - right).abs() <= 5e-10, "{left} != {right}");
        }

        for code in [ErrorCode::Allocation, ErrorCode::Interrupted] {
            reusable.inject_checkpoint_lsmr_certificate_failure();
            reusable.checkpoint_lsmr_retry_error = Some(code);
            let error = reusable
                .absorb(
                    BorrowedMapWeights::Provided(&weights),
                    DenseMatrix::from_columns(&columns).expect("failure inputs"),
                    config(),
                )
                .expect_err("retry failures remain fatal");
            assert_eq!(error.code, code);
            let recovered = reusable
                .absorb(
                    BorrowedMapWeights::Provided(&weights),
                    DenseMatrix::from_columns(&columns).expect("recovery inputs"),
                    config(),
                )
                .expect("failed retry leaves reusable canonical state usable");
            assert_eq!(recovered.residualized(), accelerated.residualized());
            assert_eq!(recovered.report().checkpoint_retries, 0);
        }
    }

    #[test]
    fn sparse_two_fe_pcg_acceleration_matches_certified_map() {
        let rows = 256_usize;
        let first_levels = 80_usize;
        let second_levels = 64_usize;
        let first_codes = (0..rows)
            .map(|row| (row % first_levels) as u32)
            .collect::<Vec<_>>();
        let second_codes = (0..rows)
            .map(|row| ((row / first_levels + 7 * row) % second_levels) as u32)
            .collect::<Vec<_>>();
        let first_index =
            LevelIndex::from_codes(&first_codes, first_levels).expect("first sparse index");
        let second_index =
            LevelIndex::from_codes(&second_codes, second_levels).expect("second sparse index");
        let first = ValidatedBorrowedInterceptSource::new(&first_codes, first_levels, &first_index)
            .expect("first sparse source");
        let second =
            ValidatedBorrowedInterceptSource::new(&second_codes, second_levels, &second_index)
                .expect("second sparse source");
        let columns = [
            (0..rows)
                .map(|row| ((row * 13 % 71) as f64).sin() + row as f64 / rows as f64)
                .collect::<Vec<_>>(),
            (0..rows)
                .map(|row| ((row * 5 % 43) as f64).cos() - 0.25 * (row % 9) as f64)
                .collect::<Vec<_>>(),
            vec![1.0; rows],
        ];
        let weights = (0..rows)
            .map(|row| 1.0 + (row % 17) as f64 / 11.0)
            .collect::<Vec<_>>();
        let oracle_design =
            BorrowedInterceptFeDesign::new(rows, vec![first.effect(), second.effect()])
                .expect("sparse oracle design");
        let oracle = absorb_borrowed_intercept_map_certified(
            &oracle_design,
            BorrowedMapWeights::Provided(&weights),
            DenseMatrix::from_columns(&columns).expect("sparse oracle values"),
            MapConfig {
                tolerance: 1e-13,
                max_iterations: 20_000,
                check_every: 4,
                symmetric: true,
            },
        )
        .expect("sparse MAP oracle");
        let design = BorrowedInterceptFeDesign::new(rows, vec![first.effect(), second.effect()])
            .expect("sparse reusable design");
        let mut reusable = ReusableCategoricalMapAbsorber::new(design, columns.len(), 1)
            .expect("sparse reusable absorber");
        assert!(reusable.two_fe_pcg.is_some());
        let capacities = reusable.capacities();
        let actual = reusable
            .absorb(
                BorrowedMapWeights::Provided(&weights),
                DenseMatrix::from_columns(&columns).expect("sparse reusable values"),
                config(),
            )
            .expect("sparse reusable PCG absorption");
        assert!(actual.report().certified());
        assert_eq!(reusable.capacities(), capacities);
        for (&left, &right) in actual
            .residualized()
            .as_slice()
            .iter()
            .zip(oracle.residualized().as_slice())
        {
            assert!((left - right).abs() <= 5e-11, "{left} != {right}");
        }
    }

    #[test]
    fn schur_restoration_during_reconstruction_is_bit_exact() {
        let rows = 20_000;
        let left_codes = (0..rows).map(|r| ((r * 8191 % rows) / 5) as u32).collect::<Vec<_>>();
        let right_codes = (0..rows).map(|r| ((r * 37 + r / 5) % 997) as u32).collect::<Vec<_>>();
        let left_index = LevelIndex::from_codes(&left_codes, 4000).unwrap();
        let right_index = LevelIndex::from_codes(&right_codes, 997).unwrap();
        let left = ValidatedBorrowedInterceptSource::new(&left_codes, 4000, &left_index).unwrap();
        let right = ValidatedBorrowedInterceptSource::new(&right_codes, 997, &right_index).unwrap();
        let design = BorrowedInterceptFeDesign::new(rows, vec![left.effect(), right.effect()]).unwrap();
        for scale in [None, Some(1e-100), Some(1.0), Some(1e100)] {
            let raw = scale.map(|s| (0..rows).map(|r| s * (0.125 + (r % 17) as f64 / 11.0)).collect::<Vec<_>>());
            let weights = validate_weights(raw.as_deref().map_or(BorrowedMapWeights::Unit,
                BorrowedMapWeights::Provided), rows).unwrap();
            for rhs in [3, 5, 8, 52] {
                let stride = rhs + 1;
                let columns = (0..rhs).map(|c| (0..rows).map(|r| if c + 1 == rhs { 1.0 }
                    else { ((r * (c + 7) % 103) as f64 - 51.0) / 13.0 }).collect::<Vec<_>>()).collect::<Vec<_>>();
                for threads in [1, 8, 16] {
                    rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
                        let mut expected_values = DenseMatrix::from_columns(&columns).unwrap();
                        let mut actual_values = expected_values.clone();
                        let mut expected = TwoFePcgWorkspace::new(&design, stride, &mut SystemWorkspaceReservation).unwrap();
                        let mut actual = TwoFePcgWorkspace::new(&design, stride, &mut SystemWorkspaceReservation).unwrap();
                        for (i, v) in expected.solution.iter_mut().enumerate() {
                            *v = ((i * 17 % 101) as f64 - 50.0) / 37.0;
                        }
                        actual.solution.copy_from_slice(&expected.solution);
                        fill_two_fe_left_rhs(&design.effects()[expected.left_effect], weights,
                            &expected_values, stride, &mut expected.left_rhs, &mut expected.left_weights).unwrap();
                        actual.left_weights.copy_from_slice(&expected.left_weights);
                        expected.left_rhs.par_chunks_mut(stride).zip(&expected.left_weights)
                            .for_each(|(sum, &total)| { for value in &mut sum[..rhs] { *value /= total; } });
                        // Private packed codes can look nonfinite as f64. None
                        // may enter arithmetic after their cache lifetime ends.
                        actual.left_rhs.fill(f64::NAN);
                        reconstruct_two_fe_pcg_candidate(&design, weights, &mut expected,
                            &mut expected_values, rhs, stride, false).unwrap();
                        reconstruct_two_fe_pcg_candidate(&design, weights, &mut actual,
                            &mut actual_values, rhs, stride, true).unwrap();
                        for (a, b) in [(&actual.left_rhs, &expected.left_rhs),
                            (&actual.left_work, &expected.left_work), (&actual.left_weights, &expected.left_weights)] {
                            assert!(a.iter().zip(b).all(|(a,b)| a.to_bits() == b.to_bits()));
                        }
                        assert!(actual_values.as_slice().iter().zip(expected_values.as_slice())
                            .all(|(a,b)| a.to_bits() == b.to_bits()));
                    });
                }
            }
        }
    }

    #[test]
    fn counted_unit_schur_edges_match_observation_operator_and_budget() {
        let rows = 20_000;
        let left_codes = (0..rows).map(|r| ((r * 8191 % rows) / 5) as u32).collect::<Vec<_>>();
        let right_codes = (0..rows).map(|r| {
            let row = r * 8191 % rows;
            let firm = if row % 5 < 3 { row / 5 * 17 } else { row / 5 * 389 + row % 5 * 97 };
            (firm % 997) as u32
        }).collect::<Vec<_>>();
        let left_index = LevelIndex::from_codes(&left_codes, 4000).unwrap();
        let right_index = LevelIndex::from_codes(&right_codes, 997).unwrap();
        let left = ValidatedBorrowedInterceptSource::new(&left_codes, 4000, &left_index).unwrap();
        let right = ValidatedBorrowedInterceptSource::new(&right_codes, 997, &right_index).unwrap();
        let design = BorrowedInterceptFeDesign::new(rows, vec![left.effect(), right.effect()]).unwrap();
        let weights = validate_weights(BorrowedMapWeights::Unit, rows).unwrap();
        for rhs in [1, 3, 4, 7, 12, 52] {
            let stride = rhs + 1;
            let mut reservation = SystemWorkspaceReservation;
            let mut graph = TwoFePcgWorkspace::new(&design, stride, &mut reservation).unwrap();
            for row in 0..rows {
                graph.left_weights[left_codes[row] as usize] += 1.0;
                graph.right_weights[right_codes[row] as usize] += 1.0;
            }
            for (index, value) in graph.direction.iter_mut().enumerate() {
                *value = ((index * 17 % 101) as f64 - 50.0) / 37.0;
            }
            rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap().install(|| {
                graph.apply_schur(&design, weights, rhs, stride).unwrap();
            });
            let expected_left = graph.left_work.clone();
            let expected_operator = graph.operator.clone();
            let mut scratch = vec![0.0; rows * 2];
            assert!(has_repeated_schur_edges(&design.effects()[graph.left_effect], right.codes()).unwrap());
            let unique_neighbors = (0..rows as u32).collect::<Vec<_>>();
            assert!(!has_repeated_schur_edges(&design.effects()[graph.left_effect], &unique_neighbors).unwrap());
            assert_eq!(cache_unit_schur_edges(&design, &mut graph, &mut scratch[..rows - 1], true).unwrap(), UnitSchurEdges::None);
            for (cells, allow_rhs) in [(rows,false), (rows,true), (rows * 2 - 1,true), (rows * 2,true)] {
                let cache = cache_unit_schur_edges(&design, &mut graph, &mut scratch[..cells], allow_rhs).unwrap();
                assert_eq!(cache, if cells == rows * 2 { UnitSchurEdges::BothScratch }
                    else if allow_rhs && graph.left_rhs.len() >= rows { UnitSchurEdges::BothRhsScratch }
                    else { UnitSchurEdges::RightScratch });
                for threads in [8, 16] {
                    rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
                        let (left, right) = match cache {
                            UnitSchurEdges::BothScratch => (Some(SchurLeftNeighbors::Counted(&scratch[..rows])), &scratch[rows..]),
                            UnitSchurEdges::BothRhsScratch => (Some(SchurLeftNeighbors::RhsScratch), &scratch[..rows]),
                            _ => (None, &scratch[..rows]),
                        };
                        graph.apply_schur_parallel(&design, weights, rhs, stride, left, Some(SchurRightNeighbors::Counted(right))).unwrap();
                    });
                    for (actual, expected) in [(&graph.left_work, &expected_left), (&graph.operator, &expected_operator)] {
                        for (&actual, &expected) in actual.iter().zip(expected) {
                            assert!((actual - expected).abs() <= 2e-12, "{actual} != {expected}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn weighted_schur_edge_scratch_matches_operator_projection_and_continuation() {
        let rows = 20_000;
        for disconnected in [false, true] {
            let left_codes = (0..rows).map(|r| ((r * 8191 % rows) / 5) as u32).collect::<Vec<_>>();
            let right_codes = (0..rows).map(|r| {
                let row = r * 8191 % rows;
                let firm = if row % 5 < 3 { row / 5 * 17 } else { row / 5 * 389 + row % 5 * 97 };
                if disconnected { (firm % 499 + usize::from(row / 5 >= 2000) * 499) as u32 }
                else { (firm % 997) as u32 }
            }).collect::<Vec<_>>();
            let right_levels = if disconnected { 998 } else { 997 };
            let left_index = LevelIndex::from_codes(&left_codes, 4000).unwrap();
            let right_index = LevelIndex::from_codes(&right_codes, right_levels).unwrap();
            let left = ValidatedBorrowedInterceptSource::new(&left_codes, 4000, &left_index).unwrap();
            let right = ValidatedBorrowedInterceptSource::new(&right_codes, right_levels, &right_index).unwrap();
            let design = || BorrowedInterceptFeDesign::new(rows, vec![left.effect(), right.effect()]).unwrap();
            for scale in [1e-100, 1.0, 1e100] {
                let raw = (0..rows).map(|r| scale * (0.125 + (r % 17) as f64 / 11.0)).collect::<Vec<_>>();
                let weights = validate_weights(BorrowedMapWeights::Provided(&raw), rows).unwrap();
                for rhs_count in [3, 5, 8, 52] {
                    let stride = rhs_count + 1;
                    let mut graph = TwoFePcgWorkspace::new(&design(), stride, &mut SystemWorkspaceReservation).unwrap();
                    for row in 0..rows {
                        graph.left_weights[left_codes[row] as usize] += weights.normalized(row);
                        graph.right_weights[right_codes[row] as usize] += weights.normalized(row);
                    }
                    for (index, value) in graph.direction.iter_mut().enumerate() {
                        *value = ((index * 17 % 101) as f64 - 50.0) / 37.0;
                    }
                    rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap().install(|| {
                        graph.apply_schur(&design(), weights, rhs_count, stride).unwrap();
                    });
                    let expected_left = graph.left_work.clone();
                    let expected_operator = graph.operator.clone();
                    let capacity = graph.capacity_elements();
                    let mut scratch = vec![0.0; rows];
                    assert!(!cache_weighted_schur_right_edges(&design(), weights, &mut graph, &mut scratch[..rows-1]).unwrap());
                    let cached = cache_weighted_schur_right_edges(&design(), weights, &mut graph, &mut scratch).unwrap();
                    assert_eq!(cached, rhs_count >= 5);
                    if !cached { continue; }
                    for threads in [8, 16] {
                        rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
                            graph.apply_schur_parallel(&design(), weights, rhs_count, stride, None,
                                Some(SchurRightNeighbors::WeightedRhsScratch(&scratch))).unwrap();
                        });
                        for (actual, expected) in [(&graph.left_work, &expected_left), (&graph.operator, &expected_operator)] {
                            for (&actual, &expected) in actual.iter().zip(expected) {
                                assert!((actual-expected).abs() < 2e-12, "{actual} != {expected}");
                            }
                        }
                    }
                    let mut expected_edges = std::collections::BTreeMap::new();
                    for row in 0..rows {
                        *expected_edges.entry((left_codes[row], right_codes[row])).or_insert(0.0) += weights.normalized(row);
                    }
                    let offset_start = rows.max(2 * expected_edges.len());
                    let required_codes = offset_start + right_levels + 1 + 4000 + 1;
                    for cells in [rows, offset_start - 1, offset_start] {
                        let mut scratch = vec![0.0; cells];
                        assert!(cache_weighted_schur_right_edges(&design(), weights, &mut graph, &mut scratch).unwrap());
                        let old_codes = graph.left_rhs[..rows].iter().map(|v| v.to_bits()).collect::<Vec<_>>();
                        let old_weights = scratch[..rows].to_vec();
                        let offsets = compact_weighted_schur_edges(&design(), &mut graph, &mut scratch).unwrap();
                        assert_eq!(offsets.is_some(), graph.left_rhs.len() >= required_codes && cells >= offset_start);
                        if let Some(offsets) = offsets {
                            for (start, levels, is_left) in [(offsets.left, 4000, true), (offsets.right, right_levels, false)] {
                                let mut visited = 0;
                                for level in 0..levels {
                                    let range = graph.left_rhs[start + level].to_bits() as usize
                                        ..graph.left_rhs[start + level + 1].to_bits() as usize;
                                    let mut previous = None;
                                    for edge in range {
                                        let other = graph.left_rhs[edge].to_bits() as u32;
                                        assert!(previous.map_or(true, |value| value < other));
                                        previous = Some(other);
                                        let pair = if is_left { (level as u32, other) } else { (other, level as u32) };
                                        assert_eq!(scratch[edge].to_bits(), expected_edges[&pair].to_bits());
                                        visited += 1;
                                    }
                                }
                                assert_eq!(visited, expected_edges.len());
                            }
                        } else {
                            assert_eq!(old_codes, graph.left_rhs[..rows].iter().map(|v| v.to_bits()).collect::<Vec<_>>());
                            assert_eq!(old_weights, scratch[..rows]);
                        }
                        for threads in [8, 16] {
                            rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
                                let (left, right) = if let Some(offsets) = offsets {
                                    (Some(SchurLeftNeighbors::WeightedRhsScratch { weights: &scratch, offsets: offsets.left }),
                                        SchurRightNeighbors::WeightedCompactRhsScratch { weights: &scratch, offsets: offsets.right })
                                } else { (None, SchurRightNeighbors::WeightedRhsScratch(&scratch)) };
                                graph.apply_schur_parallel(&design(), weights, rhs_count, stride, left, Some(right)).unwrap();
                            });
                            for (actual, expected) in [(&graph.left_work, &expected_left), (&graph.operator, &expected_operator)] {
                                for (&actual, &expected) in actual.iter().zip(expected) {
                                    assert!((actual-expected).abs() < 2e-12, "compact {actual} != {expected}");
                                }
                            }
                        }
                    }
                    assert_eq!(graph.capacity_elements(), capacity);
                }
            }
            let raw = (0..rows).map(|r| 0.125 + (r % 17) as f64 / 11.0).collect::<Vec<_>>();
            let weights = BorrowedMapWeights::Provided(&raw);
            let values = DenseMatrix::from_columns(&(0..8).map(|column| (0..rows).map(|row| {
                ((row * (column + 13) % 1009) as f64).sin() + f64::from(left_codes[row]) * 0.01
            }).collect::<Vec<_>>()).collect::<Vec<_>>()).unwrap();
            let expected = absorb_borrowed_intercept_map_certified(&design(), weights,
                values.clone(), MapConfig { tolerance: 1e-13, ..config() }).unwrap();
            for (threads, cap) in [(4, OLS_SCHUR_MAX_ITERATIONS), (8, OLS_SCHUR_MAX_ITERATIONS), (4, 1), (8, 1)] {
                let actual = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
                    let probe = schur_ols_probe(&design(), weights, &values).unwrap();
                    assert!(probe <= OLS_SCHUR_MAX_CONTRACTION);
                    absorb_ols_schur_at_cap(design(), weights, values.clone(),
                        // Forced CG continuation uses the requested tolerance,
                        // while ordinary Schur internally tightens it. Request
                        // the same strict floor for this absolute oracle check.
                        crate::fe::AbsorbConfig { map: MapConfig { tolerance: 1e-14, ..config() },
                            ..crate::fe::AbsorbConfig::default() }, 0.0, probe, cap).unwrap()
                });
                assert!(actual.report().certified());
                let distance = actual.residualized().as_slice().iter().zip(expected.residualized().as_slice())
                    .map(|(actual, expected)| (actual-expected).abs()).fold(0.0_f64, f64::max);
                eprintln!("weighted edge projection: disconnected={disconnected}, threads={threads}, cap={cap}, distance={distance}");
                assert!(distance < 2e-9, "weighted edge projection distance {distance}");
            }
        }
    }

    #[test]
    fn counted_unit_schur_projection_matches_independent_map() {
        let rows = 20_000;
        for disconnected in [false, true] {
        let left_codes = (0..rows).map(|r| ((r * 8191 % rows) / 5) as u32).collect::<Vec<_>>();
        let right_codes = (0..rows).map(|r| {
            let row = r * 8191 % rows;
            let firm = if row % 5 < 3 { row / 5 * 17 } else { row / 5 * 389 + row % 5 * 97 };
            if disconnected { (firm % 499 + usize::from(row / 5 >= 2000) * 499) as u32 }
            else { (firm % 997) as u32 }
        }).collect::<Vec<_>>();
        let left_index = LevelIndex::from_codes(&left_codes, 4000).unwrap();
        let right_levels = if disconnected { 998 } else { 997 };
        let right_index = LevelIndex::from_codes(&right_codes, right_levels).unwrap();
        let left = ValidatedBorrowedInterceptSource::new(&left_codes, 4000, &left_index).unwrap();
        let right = ValidatedBorrowedInterceptSource::new(&right_codes, right_levels, &right_index).unwrap();
        let design = || BorrowedInterceptFeDesign::new(rows, vec![left.effect(), right.effect()]).unwrap();
        for rhs_count in [5, 7, 12] {
        let columns = (0..rhs_count).map(|column| (0..rows).map(|row| {
            ((row * (column + 13) % 1009) as f64).sin() + f64::from(left_codes[row]) * 0.01
        }).collect::<Vec<_>>()).collect::<Vec<_>>();
        let values = DenseMatrix::from_columns(&columns).unwrap();
        let expected = absorb_borrowed_intercept_map_certified(&design(), BorrowedMapWeights::Unit,
            values.clone(), MapConfig { tolerance: 1e-13, ..config() }).unwrap();
        let solve = |threads, tolerance, cap| {
            rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap().install(|| {
                let probe = schur_ols_probe(&design(), BorrowedMapWeights::Unit, &values).unwrap();
                assert!(probe <= OLS_SCHUR_MAX_CONTRACTION);
                absorb_ols_schur_at_cap(design(), BorrowedMapWeights::Unit, values.clone(),
                    crate::fe::AbsorbConfig { map: MapConfig { tolerance, ..config() },
                        ..crate::fe::AbsorbConfig::default() }, 0.0, probe, cap).unwrap()
            })
        };
        let uncached = solve(4, 1e-8, OLS_SCHUR_MAX_ITERATIONS);
        let cached = solve(8, 1e-8, OLS_SCHUR_MAX_ITERATIONS);
        let distance = |a: &CertifiedBorrowedMap, b: &CertifiedBorrowedMap| {
            a.residualized().as_slice().iter().zip(b.residualized().as_slice())
                .map(|(a,b)| (a-b).abs()).fold(0.0_f64, f64::max)
        };
        assert!(distance(&cached, &uncached) < 2e-9);
        eprintln!("counted Schur default tolerance: uncached/MAP={}, cached/MAP={}, cached/uncached={}",
            distance(&uncached, &expected), distance(&cached, &expected), distance(&cached, &uncached));
        // The absolute oracle bound here is tighter than the default norm-scaled
        // stopping rule on the deliberately large raw FE contribution.
        for threads in [4, 8] {
            let continued = solve(threads, 1e-14, 1);
            assert!(continued.report().certified());
            assert!(distance(&continued, &expected) < 2e-9);
        }
        let mut previous = None;
        for threads in [8, 16] {
            let actual = solve(threads, 1e-12, OLS_SCHUR_MAX_ITERATIONS);
            assert!(actual.report().certified());
            for (&actual, &expected) in actual.residualized().as_slice().iter().zip(expected.residualized().as_slice()) {
                assert!((actual - expected).abs() < 2e-9, "{actual} != {expected}");
            }
            if let Some(previous) = previous { assert_eq!(actual.residualized(), &previous); }
            previous = Some(actual.residualized().clone());
        }
        }
        }
    }

    #[test]
    fn large_two_fe_schur_operator_is_bit_stable_across_threads() {
        let rows = TWO_FE_PCG_PARALLEL_MIN_ROWS;
        let first_levels = 1_024;
        let second_levels = 997;
        let first_codes = (0..rows)
            .map(|row| (row % first_levels) as u32)
            .collect::<Vec<_>>();
        let second_codes = (0..rows)
            .map(|row| ((7 * row + row / first_levels) % second_levels) as u32)
            .collect::<Vec<_>>();
        let first_index = LevelIndex::from_codes(&first_codes, first_levels).expect("first index");
        let second_index =
            LevelIndex::from_codes(&second_codes, second_levels).expect("second index");
        let first = ValidatedBorrowedInterceptSource::new(&first_codes, first_levels, &first_index)
            .expect("first source");
        let second =
            ValidatedBorrowedInterceptSource::new(&second_codes, second_levels, &second_index)
                .expect("second source");
        let design = BorrowedInterceptFeDesign::new(rows, vec![first.effect(), second.effect()])
            .expect("design");
        let raw_weights = (0..rows)
            .map(|row| 1.0 + (row % 17) as f64 / 11.0)
            .collect::<Vec<_>>();
        let weights =
            validate_weights(BorrowedMapWeights::Provided(&raw_weights), rows).expect("weights");
        let build = |threads, rhs, stride| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("pool")
                .install(|| {
                    let mut reservation = SystemWorkspaceReservation;
                    let mut graph =
                        TwoFePcgWorkspace::new(&design, stride, &mut reservation).expect("graph");
                    let left_codes = design.effects()[graph.left_effect].codes();
                    let right_codes = design.effects()[graph.right_effect].codes();
                    for row in 0..rows {
                        let weight = weights.normalized(row);
                        graph.left_weights[left_codes[row] as usize] += weight;
                        graph.right_weights[right_codes[row] as usize] += weight;
                    }
                    for (index, direction) in graph.direction.iter_mut().enumerate() {
                        *direction = ((17 * index % 101) as f64 - 50.0) / 37.0;
                    }
                    graph
                        .apply_schur(&design, weights, rhs, stride)
                        .expect("Schur operator");
                    graph
                })
        };
        for (rhs, stride) in [(1, 1), (11, 11), (52, 52), (51, 64)] {
            let serial = build(1, rhs, stride);
            for threads in [2, 4, 8, 16] {
                let parallel = build(threads, rhs, stride);
                for (expected, actual) in [
                    (&serial.left_work, &parallel.left_work),
                    (&serial.operator, &parallel.operator),
                ] {
                    assert_eq!(
                        expected.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                        actual.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                    );
                }
            }
        }
    }

    #[test]
    fn large_two_fe_pcg_recurrence_is_bit_stable_across_threads() {
        let rows = TWO_FE_PCG_PARALLEL_MIN_ROWS;
        let first_levels = 1_024;
        let second_levels = 997;
        let first_codes = (0..rows)
            .map(|row| (row % first_levels) as u32)
            .collect::<Vec<_>>();
        let second_codes = (0..rows)
            .map(|row| ((7 * row + row / first_levels) % second_levels) as u32)
            .collect::<Vec<_>>();
        let first_index = LevelIndex::from_codes(&first_codes, first_levels).expect("first index");
        let second_index =
            LevelIndex::from_codes(&second_codes, second_levels).expect("second index");
        let first = ValidatedBorrowedInterceptSource::new(&first_codes, first_levels, &first_index)
            .expect("first source");
        let second =
            ValidatedBorrowedInterceptSource::new(&second_codes, second_levels, &second_index)
                .expect("second source");
        let columns = (0..3)
            .map(|column| {
                (0..rows)
                    .map(|row| {
                        ((row * (column + 3) % 103) as f64 - 51.0)
                            .mul_add(column as f64 + 0.5, (row % 29) as f64 / 31.0)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let weights = (0..rows)
            .map(|row| 1.0 + (row % 17) as f64 / 11.0)
            .collect::<Vec<_>>();
        let run = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("pool")
                .install(|| {
                    let design =
                        BorrowedInterceptFeDesign::new(rows, vec![first.effect(), second.effect()])
                            .expect("design");
                    let mut reusable =
                        ReusableCategoricalMapAbsorber::new(design, columns.len(), threads)
                            .expect("reusable map");
                    assert!(reusable.two_fe_pcg.is_some());
                    reusable
                        .absorb(
                            BorrowedMapWeights::Provided(&weights),
                            DenseMatrix::from_columns(&columns).expect("matrix"),
                            config(),
                        )
                        .expect("certified PCG")
                })
        };
        let serial = run(1);
        assert!(serial.report().iterations > CATEGORICAL_MAP_PILOT_SWEEPS);
        for threads in [7, 14] {
            let parallel = run(threads);
            assert_eq!(serial.residualized(), parallel.residualized());
            assert_eq!(serial.report().iterations, parallel.report().iterations);
            assert_eq!(serial.report().stop_reason, parallel.report().stop_reason);
            assert_eq!(serial.report().certificate, parallel.report().certificate);
        }
    }

    #[test]
    fn map_workspace_report_counts_reserved_capacity_before_norm_handoff() {
        struct RecordingReservation {
            bytes: u64,
        }
        impl WorkspaceReservation for RecordingReservation {
            fn reserve_f64(
                &mut self,
                values: &mut Vec<f64>,
                elements: usize,
                label: &'static str,
            ) -> Result<()> {
                SystemWorkspaceReservation.reserve_f64(values, elements + 7, label)?;
                self.bytes += (values.capacity() * std::mem::size_of::<f64>()) as u64;
                Ok(())
            }
        }
        for threads in [1, 4] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    for rows in [32, 16_384] {
                        let codes = (0..rows).map(|row| (row % 4) as u32).collect::<Vec<_>>();
                        let index = LevelIndex::from_codes(&codes, 4).unwrap();
                        let source =
                            ValidatedBorrowedInterceptSource::new(&codes, 4, &index).unwrap();
                        for effects in [vec![], vec![source.effect()]] {
                            let design = BorrowedInterceptFeDesign::new(rows, effects).unwrap();
                            let input = (0..rows)
                                .map(|row| {
                                    2.0 * (row % 4) as f64
                                        + if (row / 4) % 2 == 0 { -1.0 } else { 1.0 }
                                })
                                .collect::<Vec<_>>();
                            let mut reservation = RecordingReservation { bytes: 0 };
                            let result = absorb_with_reservation(
                                &design,
                                BorrowedMapWeights::Unit,
                                DenseMatrix::from_columns(&[input]).unwrap(),
                                config(),
                                false,
                                &mut reservation,
                            )
                            .unwrap();
                            assert_eq!(
                                result.report().solver_workspace_peak.bytes(),
                                Some(reservation.bytes)
                            );
                            assert!(reservation.bytes > 7 * 8);
                            assert!(result.report().certified());
                            if !design.effects().is_empty() {
                                for (row, residual) in
                                    result.residualized().as_slice().iter().enumerate()
                                {
                                    assert_eq!(
                                        *residual,
                                        if (row / 4) % 2 == 0 { -1.0 } else { 1.0 }
                                    );
                                }
                            }
                        }
                    }
                });
        }
    }

    #[test]
    fn cg_workspace_report_counts_capacity_early_exit_and_retained_coefficients() {
        struct RecordingReservation {
            bytes: u64,
        }
        impl WorkspaceReservation for RecordingReservation {
            fn reserve_f64(
                &mut self,
                values: &mut Vec<f64>,
                elements: usize,
                label: &'static str,
            ) -> Result<()> {
                SystemWorkspaceReservation.reserve_f64(values, elements + 7, label)?;
                self.bytes += (values.capacity() * 8) as u64;
                Ok(())
            }
        }
        for threads in [1, 4] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| {
                    let rows = 1024;
                    let codes = (0..rows).map(|row| (row % 4) as u32).collect::<Vec<_>>();
                    let index = LevelIndex::from_codes(&codes, 4).unwrap();
                    let source = ValidatedBorrowedInterceptSource::new(&codes, 4, &index).unwrap();
                    let design =
                        BorrowedInterceptFeDesign::new(rows, vec![source.effect()]).unwrap();
                    for already_within in [false, true] {
                        for retain_coefficients in [false, true] {
                            let columns = (0..2)
                                .map(|rhs| {
                                    (0..rows)
                                        .map(|row| {
                                            let noise = if (row / 4) % 2 == 0 { -1.0 } else { 1.0 };
                                            (rhs + 1) as f64
                                                * (noise
                                                    + if already_within {
                                                        0.0
                                                    } else {
                                                        2.0 * codes[row] as f64
                                                    })
                                        })
                                        .collect::<Vec<_>>()
                                })
                                .collect::<Vec<_>>();
                            let mut reservation = RecordingReservation { bytes: 0 };
                            let result = absorb_cg_with_reservation(
                                &design,
                                BorrowedMapWeights::Unit,
                                DenseMatrix::from_columns(&columns).unwrap(),
                                config(),
                                false,
                                CgContinuation {
                                    retain_coefficients,
                                    ..CgContinuation::ORDINARY
                                },
                                &mut reservation,
                            )
                            .unwrap();
                            assert!(result.report().certified());
                            assert_eq!(result.report().iterations == 0, already_within);
                            let matrices = if already_within {
                                0
                            } else {
                                3 * rows as u64 * 2 * 8
                            };
                            // Four levels times two RHSs, plus channel/count/offset metadata.
                            let one_coefficient_bank =
                                4 * 2 * 8 + (1 + 1 + 2) * std::mem::size_of::<usize>() as u64;
                            let coefficients = if !retain_coefficients {
                                0
                            } else if already_within {
                                one_coefficient_bank
                            } else {
                                4 * one_coefficient_bank
                            };
                            assert_eq!(
                                result.report().solver_workspace_peak.bytes(),
                                Some(reservation.bytes + matrices + coefficients)
                            );
                            for row in 0..rows {
                                let noise = if (row / 4) % 2 == 0 { -1.0 } else { 1.0 };
                                for rhs in 0..2 {
                                    assert!(
                                        (result.residualized()[(row, rhs)]
                                            - (rhs + 1) as f64 * noise)
                                            .abs()
                                            < 1e-12
                                    );
                                }
                            }
                        }
                    }
                });
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
    fn ppml_cg_action_preserves_fe_space_under_cancellation() {
        let observations = 16;
        let codes = (0..3)
            .map(|axis| {
                (0..observations)
                    .map(|row| (((row / 2) >> axis) & 1) as u32)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let indices = codes
            .iter()
            .map(|c| LevelIndex::from_codes(c, 2).unwrap())
            .collect::<Vec<_>>();
        let sources = codes
            .iter()
            .zip(&indices)
            .map(|(c, i)| ValidatedBorrowedInterceptSource::new(c, 2, i).unwrap())
            .collect::<Vec<_>>();
        let design = BorrowedInterceptFeDesign::new(
            observations,
            sources
                .iter()
                .map(ValidatedBorrowedInterceptSource::effect)
                .collect(),
        )
        .unwrap();
        let input = DenseMatrix::from_columns(&[
            (0..observations)
                .map(|row| {
                    if row % 2 == 0 {
                        1e10 * (row + 1) as f64
                    } else {
                        0.001 * row as f64
                    }
                })
                .collect(),
            (0..observations)
                .map(|row| (row as f64 * 0.71).sin())
                .collect(),
        ])
        .unwrap();
        let weights = (0..observations)
            .map(|row| 1.0 + (row % 5) as f64)
            .collect::<Vec<_>>();
        for raw_weights in [
            BorrowedMapWeights::Unit,
            BorrowedMapWeights::Provided(&weights),
        ] {
            let run = |threads| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| {
                        let mut workspace = BorrowedMapWorkspace::new_with_parallelism(
                            &design,
                            observations,
                            2,
                            threads,
                            &mut SystemWorkspaceReservation,
                        )
                        .unwrap();
                        let mut action = DenseMatrix::zeros(observations, 2).unwrap();
                        borrowed_symmetric_projection_from_group_means(
                            &design,
                            validate_weights(raw_weights, observations).unwrap(),
                            &input,
                            &mut action,
                            &mut workspace,
                        )
                        .unwrap();
                        // Identical FE tuples must receive identical contributions,
                        // independently of the magnitude of the input's off-space part.
                        for row in (0..observations).step_by(2) {
                            for rhs in 0..2 {
                                assert_eq!(action[(row, rhs)], action[(row + 1, rhs)]);
                            }
                        }
                        for rhs in 0..2 {
                            let scale = (0..observations)
                                .map(|row| action[(row, rhs)].abs())
                                .fold(1.0_f64, f64::max);
                            for tuple in 0..8 {
                                let additive = action[(0, rhs)]
                                    + (0..3)
                                        .filter(|axis| tuple & (1 << axis) != 0)
                                        .map(|axis| {
                                            action[((1 << axis) * 2, rhs)] - action[(0, rhs)]
                                        })
                                        .sum::<f64>();
                                assert!(
                                    (action[(tuple * 2, rhs)] - additive).abs() < 1e-14 * scale
                                );
                            }
                        }
                        let mut subtractive = DenseMatrix::zeros(observations, 2).unwrap();
                        borrowed_symmetric_projection_into_recording(
                            &design,
                            validate_weights(raw_weights, observations).unwrap(),
                            &input,
                            &mut subtractive,
                            &mut workspace,
                            None,
                        )
                        .unwrap();
                        for rhs in 0..2 {
                            let scale = (0..observations)
                                .map(|row| input[(row, rhs)].abs())
                                .fold(1.0_f64, f64::max);
                            for row in 0..observations {
                                assert!(
                                    (action[(row, rhs)] - subtractive[(row, rhs)]).abs()
                                        < 1e-14 * scale
                                );
                            }
                        }
                        action
                    })
            };
            assert_eq!(run(1), run(3));
        }
    }

    #[test]
    fn fused_symmetric_complement_and_cg_update_match_separate_passes() {
        let observations = 20_000;
        let level_counts = [257, 263, 269];
        let code_sets = level_counts
            .iter()
            .enumerate()
            .map(|(effect, &levels)| {
                (0..observations)
                    .map(|row| ((row * (2 * effect + 1) + row / 17) % levels) as u32)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let indexes = code_sets
            .iter()
            .zip(level_counts)
            .map(|(codes, levels)| LevelIndex::from_codes(codes, levels).expect("index"))
            .collect::<Vec<_>>();
        let sources = code_sets
            .iter()
            .zip(indexes.iter())
            .zip(level_counts)
            .map(|((codes, index), levels)| {
                ValidatedBorrowedInterceptSource::new(codes, levels, index).expect("source")
            })
            .collect::<Vec<_>>();
        let design = BorrowedInterceptFeDesign::new(
            observations,
            sources.iter().map(|source| source.effect()).collect(),
        )
        .expect("design");
        let columns = (0..11)
            .map(|column| {
                (0..observations)
                    .map(|row| {
                        ((row % 101) as f64 - 50.0).mul_add(
                            column as f64 + 0.25,
                            ((row * (column + 5)) % 4_103) as f64 / 19.0,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let input = DenseMatrix::from_columns(&columns).expect("matrix");
        let weight_values = (0..observations)
            .map(|row| 1.0 + (row % 13) as f64 / 17.0)
            .collect::<Vec<_>>();
        let weights = validate_weights(BorrowedMapWeights::Provided(&weight_values), observations)
            .expect("weights");

        for threads in [1, 7] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("pool")
                .install(|| {
                    let mut reservation = SystemWorkspaceReservation;
                    let mut workspace = BorrowedMapWorkspace::new_with_parallelism(
                        &design,
                        observations,
                        columns.len(),
                        threads,
                        &mut reservation,
                    )
                    .expect("workspace");
                    let mut expected_initial = input.clone();
                    let mut expected_initial_coefficients =
                        FeCoefficientBatch::intercept_zeros(&design, columns.len())
                            .expect("coefficients");
                    project_effect_recording(
                        &design.effects()[0],
                        weights,
                        &mut expected_initial,
                        &mut workspace,
                        Some((&mut expected_initial_coefficients, 0)),
                    )
                    .expect("separate initial effect projection");
                    let mut actual_initial =
                        DenseMatrix::zeros(observations, columns.len()).expect("initial matrix");
                    let mut actual_initial_coefficients =
                        FeCoefficientBatch::intercept_zeros(&design, columns.len())
                            .expect("coefficients");
                    project_effect_recording_from_input(
                        &design.effects()[0],
                        weights,
                        &input,
                        &mut actual_initial,
                        &mut workspace,
                        Some((&mut actual_initial_coefficients, 0)),
                    )
                    .expect("fused initial effect projection");
                    assert_eq!(expected_initial, actual_initial);
                    assert_eq!(expected_initial_coefficients, actual_initial_coefficients);

                    let mut expected = input.clone();
                    let mut expected_coefficients =
                        FeCoefficientBatch::intercept_zeros(&design, columns.len())
                            .expect("coefficients");
                    borrowed_residual_sweep_recording(
                        &design,
                        weights,
                        &mut expected,
                        &mut workspace,
                        Some(&mut expected_coefficients),
                        true,
                    )
                    .expect("separate symmetric projection");
                    for (projected, original) in
                        expected.as_mut_slice().iter_mut().zip(input.as_slice())
                    {
                        *projected = *original - *projected;
                    }

                    let mut actual =
                        DenseMatrix::zeros(observations, columns.len()).expect("actual matrix");
                    let mut actual_coefficients =
                        FeCoefficientBatch::intercept_zeros(&design, columns.len())
                            .expect("coefficients");
                    borrowed_symmetric_projection_into_recording(
                        &design,
                        weights,
                        &input,
                        &mut actual,
                        &mut workspace,
                        Some(&mut actual_coefficients),
                    )
                    .expect("fused symmetric projection");

                    assert_eq!(expected, actual);
                    assert_eq!(expected_coefficients, actual_coefficients);

                    let alpha = (0..columns.len())
                        .map(|rhs| 0.01 * (rhs + 1) as f64)
                        .collect::<Vec<_>>();
                    let direction = input.clone();
                    let projection = actual.clone();
                    let mut expected_values = input.clone();
                    let mut expected_residual = expected.clone();
                    cg_update_solution_and_residual(
                        &mut expected_values,
                        &mut expected_residual,
                        &direction,
                        &projection,
                        &alpha,
                    )
                    .expect("separate CG update");
                    let mut expected_norms = vec![0.0; columns.len()];
                    let mut expected_corrections = vec![0.0; columns.len()];
                    weighted_dot_columns_into(
                        &expected_residual,
                        &expected_residual,
                        weights,
                        &mut expected_norms,
                        &mut expected_corrections,
                        &mut workspace.parallel_dot_sums,
                    )
                    .expect("separate residual norm");

                    let mut actual_values = input.clone();
                    let mut actual_residual = expected.clone();
                    let mut actual_norms = vec![0.0; columns.len()];
                    let mut actual_corrections = vec![0.0; columns.len()];
                    cg_update_solution_residual_and_weighted_norm(
                        &mut actual_values,
                        &mut actual_residual,
                        &direction,
                        &projection,
                        &alpha,
                        weights,
                        &mut actual_norms,
                        &mut actual_corrections,
                        &mut workspace.parallel_dot_sums,
                    )
                    .expect("fused CG update and residual norm");

                    assert_eq!(expected_values, actual_values);
                    assert_eq!(expected_residual, actual_residual);
                    assert_eq!(expected_norms, actual_norms);
                });
        }
    }

    #[test]
    fn large_wide_three_fe_projection_and_certificate_are_bit_stable() {
        let observations = 50_000;
        assert!(observations >= PARALLEL_PROJECTION_MIN_ROWS);
        let level_counts = [4_096, 2_048, 1_024];
        let code_sets = level_counts
            .iter()
            .map(|&levels| {
                (0..observations)
                    .map(|row| (row % levels) as u32)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let indexes = code_sets
            .iter()
            .zip(level_counts)
            .map(|(codes, levels)| LevelIndex::from_codes(codes, levels).expect("index"))
            .collect::<Vec<_>>();
        let sources = code_sets
            .iter()
            .zip(indexes.iter())
            .zip(level_counts)
            .map(|((codes, index), levels)| {
                ValidatedBorrowedInterceptSource::new(codes, levels, index).expect("source")
            })
            .collect::<Vec<_>>();
        let design = BorrowedInterceptFeDesign::new(
            observations,
            sources.iter().map(|source| source.effect()).collect(),
        )
        .expect("design");
        let columns = (0..11)
            .map(|column| {
                (0..observations)
                    .map(|row| {
                        let centered = (row % 97) as f64 - 48.0;
                        centered.mul_add(
                            column as f64 + 0.5,
                            ((row * (column + 3)) % 4_099) as f64 / 17.0,
                        )
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
                    .expect("certified wide projection")
                })
        };
        let serial = run(1);
        for threads in [2, 4, 8, 16] {
            let parallel = run(threads);
            assert_eq!(serial.residualized(), parallel.residualized());
            assert_eq!(serial.report().iterations, parallel.report().iterations);
            assert_eq!(serial.report().stop_reason, parallel.report().stop_reason);
            assert_eq!(serial.report().certificate, parallel.report().certificate);
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
    fn fresh_one_effect_nonfinite_validation_precedes_weights_and_allocation() {
        for threads in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            pool.install(|| {
                for rows in [16_383, 16_384] {
                    let codes = vec![0_u32; rows];
                    let index = LevelIndex::from_codes(&codes, 1).unwrap();
                    let source = ValidatedBorrowedInterceptSource::new(&codes, 1, &index).unwrap();
                    let design =
                        BorrowedInterceptFeDesign::new(rows, vec![source.effect()]).unwrap();
                    for columns in [7, 8] {
                        for position in [0, rows * columns / 2, rows * columns - 1] {
                            for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                                let mut data = vec![1.0; rows * columns];
                                data[position] = bad;
                                let values =
                                    DenseMatrix::from_row_major(rows, columns, data).unwrap();
                                let mut reservation = InjectedReservation {
                                    fail_at: Some(0),
                                    ..InjectedReservation::default()
                                };
                                // Both a bad weight shape and an allocation failure
                                // would mask this diagnostic if validation moved later.
                                let error = absorb_initial_with_reservation(
                                    &design,
                                    BorrowedMapWeights::Provided(&[]),
                                    values,
                                    config(),
                                    true,
                                    &mut reservation,
                                )
                                .unwrap_err();
                                assert_eq!(error.code, ErrorCode::InvalidData);
                                assert_eq!(error.message, "borrowed MAP values must be finite");
                                assert_eq!(reservation.calls, 0);
                            }
                        }
                    }
                }
            });
        }
    }

    #[test]
    fn unit_single_column_norm_matches_generic_unit_weights_bitwise() {
        for n in [0, 1, 7, 127, 16_383, 16_384, 32_771] {
            for case in 0..7 {
                let mut values: Vec<_> = (0..n).map(|r| (r as f64).sin()).collect();
                if n > 0 {
                    match case {
                        1 => values.fill(-0.0),
                        2 => values[n / 2] = f64::NAN,
                        3 => values[n - 1] = f64::INFINITY,
                        4 => for value in &mut values { *value *= 1e-300; },
                        5 => for value in &mut values { *value *= 1e150; },
                        6 => { values[0] = f64::MAX; values[n / 2] = -f64::MAX; },
                        _ => (),
                    }
                }
                let matrix = DenseMatrix::from_row_major(n, 1, values).unwrap();
                let weights = vec![1.0; n];
                let run = |weights| {
                    let mut output = [0.0];let mut scale = [0.0];let mut sum = [0.0];
                    weighted_column_norms(&matrix, weights, &mut output, &mut scale, &mut sum)
                        .map(|()| (output[0].to_bits(), scale[0].to_bits(), sum[0].to_bits()))
                };
                assert_eq!(run(ValidatedWeights { values: None, raw_scale: 1.0 }),
                    run(ValidatedWeights { values: Some(&weights), raw_scale: 1.0 }), "N={n}, case={case}");
            }
        }
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
    fn borrowed_cg_constant_energy_roundoff_preserves_weighted_projection() {
        let n = 45;
        let codes = vec![0_u32; n];
        let index = LevelIndex::from_codes(&codes, 1).unwrap();
        let source = ValidatedBorrowedInterceptSource::new(&codes, 1, &index).unwrap();
        let design = BorrowedInterceptFeDesign::new(n, vec![source.effect()]).unwrap();
        let weights = (0..n).map(|r| (1 + r % 7) as f64).collect::<Vec<_>>();
        let x = (0..n).map(|r| (r % 5) as f64 - 2.0).collect::<Vec<_>>();
        let mean =
            x.iter().zip(&weights).map(|(x, w)| x * w).sum::<f64>() / weights.iter().sum::<f64>();
        let input = DenseMatrix::from_columns(&[vec![1.0; n], x.clone()]).unwrap();
        let ordinary = absorb_borrowed_intercept_cg_retaining_certified(
            &design,
            BorrowedMapWeights::Provided(&weights),
            DenseMatrix::from_columns(&[x.clone()]).unwrap(),
            config(),
        )
        .unwrap();
        let ordinary_error = (0..n)
            .map(|row| (ordinary.residualized()[(row, 0)] - (x[row] - mean)).abs())
            .fold(0.0_f64, f64::max);
        // The unaffected variable RHS already has cancellation error above
        // 1e-13; use the existing borrowed-CG oracle bound for that column.
        assert!(ordinary_error <= 2e-10);
        let recovered = absorb_borrowed_intercept_cg_retaining_certified(
            &design,
            BorrowedMapWeights::Provided(&weights),
            input,
            config(),
        )
        .expect("constant RHS energy must not fail from a two-ulp subtraction overshoot");
        let (residual, report, coefficients) = recovered.into_full_parts();
        assert!(report.certified());
        let coefficients = coefficients.unwrap();
        for row in 0..n {
            assert!(residual[(row, 0)].abs() <= 1e-13);
            assert!((residual[(row, 1)] - (x[row] - mean)).abs() <= 2e-10);
        }
        assert!((coefficients.rhs(0).unwrap()[0] - 1.0).abs() <= 1e-13);
        assert!((coefficients.rhs(1).unwrap()[0] - mean).abs() <= 2e-10);
    }

    #[test]
    fn hestenes_energy_audit_recomputes_remaining_energy_and_rejects_invalid_steps() {
        let unit = ValidatedWeights {
            values: None,
            raw_scale: 1.0,
        };
        let call = |values: &DenseMatrix, direction: &DenseMatrix, previous: f64, decrease: f64| {
            let mut potential = [previous];
            let error = cg_hestenes_error_and_update(
                &[1.0],
                &[decrease],
                &mut potential,
                values,
                direction,
                unit,
            )?;
            Ok::<_, FeregError>((potential[0], error))
        };
        let one = DenseMatrix::from_row_major(1, 1, vec![1.0]).unwrap();
        let two_ulps = f64::from_bits(1.0_f64.to_bits() + 2);
        let (energy, error) = call(&one, &one, 1.0, two_ulps).unwrap();
        assert_eq!(energy, f64::EPSILON);
        assert!(error > 1.0);

        let delta = 1e-7;
        let values = DenseMatrix::from_row_major(2, 1, vec![1.0 + delta, 1.0 - delta]).unwrap();
        let direction = DenseMatrix::from_row_major(2, 1, vec![1.0, 1.0]).unwrap();
        let previous = values[(0, 0)].powi(2) + values[(1, 0)].powi(2);
        let decrease = f64::from_bits(previous.to_bits() + 2);
        let expected = (values[(0, 0)] - 1.0).powi(2) + (values[(1, 0)] - 1.0).powi(2);
        let (energy, error) = call(&values, &direction, previous, decrease).unwrap();
        assert!((energy - expected).abs() <= 4.0 * f64::EPSILON * expected);
        assert!(energy > f64::EPSILON && error > 1.0);

        let large = DenseMatrix::from_row_major(1, 1, vec![1e150]).unwrap();
        let previous = 1e150_f64 * 1e150;
        let decrease = f64::from_bits(previous.to_bits() + 2);
        assert_eq!(
            cg_audit_remaining_energy(&large, &large, unit, 0, 1.0, previous, decrease)
                .unwrap(),
            0.0
        );
        // The energy is representable, but the unchanged Hestenes quotient
        // overflows at this scale and must still fail closed.
        assert_eq!(
            call(&large, &large, previous, decrease).unwrap_err().code,
            ErrorCode::InvalidData
        );
        for (value, step) in [
            (2.0, 2.0),
            (1.0, 0.5),
            (f64::NAN, 1.0),
            (1.0, f64::INFINITY),
        ] {
            let value = DenseMatrix::from_row_major(1, 1, vec![value]).unwrap();
            let step = DenseMatrix::from_row_major(1, 1, vec![step]).unwrap();
            assert_eq!(
                call(&value, &step, 1.0, two_ulps).unwrap_err().code,
                ErrorCode::InvalidData
            );
        }
        assert_eq!(
            call(&one, &one, 1.0, 2.0).unwrap_err().code,
            ErrorCode::InvalidData
        );
        assert_eq!(
            call(&one, &one, 1.0, f64::NAN).unwrap_err().code,
            ErrorCode::InvalidData
        );
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
    fn map_conversion_peak_preserves_overreservation_and_moved_layout() {
        let codes = [0_u32, 0, 1, 1];
        let index = LevelIndex::from_codes(&codes, 2).unwrap();
        let source = ValidatedBorrowedInterceptSource::new(&codes, 2, &index).unwrap();
        let design = BorrowedInterceptFeDesign::new(4, vec![source.effect()]).unwrap();
        let mut accumulator = MapCoefficientAccumulator::zeros(&design, 3).unwrap();
        accumulator.values.try_reserve_exact(17).unwrap();
        accumulator.effect_offsets.try_reserve_exact(11).unwrap();
        accumulator.level_counts.try_reserve_exact(7).unwrap();
        accumulator
            .values
            .copy_from_slice(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let source_bytes = accumulator.values.capacity() as u128 * 8
            + accumulator.effect_offsets.capacity() as u128 * std::mem::size_of::<usize>() as u128;
        let level_capacity = accumulator.level_counts.capacity();
        let (batch, conversion_peak) = accumulator.into_batch().unwrap();
        assert_eq!(batch.rhs(0).unwrap(), &[1.0, 4.0]);
        assert_eq!(batch.rhs(1).unwrap(), &[2.0, 5.0]);
        assert_eq!(batch.rhs(2).unwrap(), &[3.0, 6.0]);
        assert_eq!(batch.level_counts.capacity(), level_capacity);
        assert_eq!(conversion_peak, source_bytes + batch.retained_bytes());
        assert!(conversion_peak > 2 * 6 * 8);
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
        assert!(
            report.solver_workspace_peak.bytes().unwrap() as u128
                >= coefficients.retained_bytes() + 2 * 7 * 3 * 8
        );
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
        let values = DenseMatrix::from_row_major(1, 2, vec![1.0, 1.0]).unwrap();
        let error = cg_hestenes_error_and_update(
            &[0.5, 0.25],
            &[4.0, 8.0],
            &mut improvement,
            &values,
            &values,
            ValidatedWeights {
                values: None,
                raw_scale: 1.0,
            },
        )
        .expect("Hestenes update");
        assert!((error - 0.5).abs() <= f64::EPSILON);
        assert_eq!(improvement, [8.0, 18.0]);
        assert_eq!(
            cg_hestenes_error_and_update(
                &[2.0, 0.0],
                &[1.0, 0.0],
                &mut [1.0, 1.0],
                &values,
                &values,
                ValidatedWeights {
                    values: None,
                    raw_scale: 1.0
                }
            )
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
        let direct = absorb_borrowed_intercept_auto_direct_lsmr_certified(
            &design,
            BorrowedMapWeights::Unit,
            input.clone(),
            config(),
            LsmrConfig::default(),
            None,
            false,
            false,
        )
        .expect("direct LSMR auto");
        assert_eq!(direct.report().requested_solver, FeSolverKind::Auto);
        assert_eq!(direct.report().used_solver, FeSolverKind::Lsmr);
        assert_eq!(direct.report().pilot_iterations, 0);
        assert_eq!(direct.report().pilot_contraction, 0.0);
        assert!(direct.report().certified());
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
    fn lsmr_continuation_certificate_uses_the_supplied_original_rhs_scale() {
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
        let values = DenseMatrix::from_columns(&[outcome]).expect("matrix");
        let weights = validate_weights(BorrowedMapWeights::Unit, observations).expect("weights");
        let mut reference_norms = vec![0.0; values.ncols()];
        let mut scales = vec![0.0; values.ncols()];
        let mut square_sums = vec![0.0; values.ncols()];
        weighted_column_norms(
            &values,
            weights,
            &mut reference_norms,
            &mut scales,
            &mut square_sums,
        )
        .expect("reference norms");
        for norm in &mut reference_norms {
            *norm *= 1_000.0;
        }
        let lsmr = LsmrConfig {
            tolerance: 1e-14,
            max_iterations: 1,
            preconditioner: LsmrPreconditioner::Schwarz,
            ..LsmrConfig::default()
        };
        let (ordinary, ordinary_report, _, _) = run_borrowed_lsmr(
            &design,
            weights,
            &values,
            lsmr.tolerance,
            lsmr.max_iterations,
            lsmr,
            None,
            false,
        )
        .expect("ordinary LSMR certificate");
        let (continued, continued_report, _, _) = run_borrowed_lsmr(
            &design,
            weights,
            &values,
            lsmr.tolerance,
            lsmr.max_iterations,
            lsmr,
            Some(&reference_norms),
            false,
        )
        .expect("continuation LSMR certificate");
        assert_eq!(ordinary, continued);
        assert!(ordinary_report.certificate.relative_infinity_norm > 0.0);
        assert!(
            continued_report.certificate.relative_infinity_norm
                < ordinary_report.certificate.relative_infinity_norm / 100.0
        );
    }

    #[test]
    fn uncertified_lsmr_candidate_is_polished_and_fully_accounted() {
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
        let mut strict = config();
        strict.tolerance = 1e-14;
        let input = DenseMatrix::from_columns(&[outcome]).expect("matrix");
        let result = absorb_borrowed_intercept_auto_routed_certified(
            &design,
            BorrowedMapWeights::Unit,
            input.clone(),
            strict,
            LsmrConfig {
                tolerance: 1e-14,
                max_iterations: 1,
                preconditioner: LsmrPreconditioner::Off,
                ..LsmrConfig::default()
            },
            None,
            true,
        )
        .expect("polished auto result");
        let report = result.report();
        assert!(report.certified());
        assert_eq!(report.used_solver, FeSolverKind::Cg);
        assert!(report.fallback_used);
        assert_eq!(
            report.requested_preconditioner,
            Some(LsmrPreconditioner::Off)
        );
        assert_eq!(report.used_preconditioner, Some(LsmrPreconditioner::Off));
        assert!(report.iterations > report.pilot_iterations + 1);
        assert!(report
            .warnings
            .first()
            .is_some_and(|warning| { warning.contains("continued CG from the LSMR candidate") }));
        let (residualized, _, coefficients) = result.into_full_parts();
        let coefficients = coefficients.expect("combined pilot, LSMR, and CG coefficients");
        let rhs = coefficients.rhs(0).expect("coefficient RHS");
        for row in 0..observations {
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
        let memory = estimate_fe_solver_memory(
            observations,
            1,
            &[workers, workers + 1],
            rayon::current_num_threads(),
            lsmr.local_size,
        )
        .expect("solver memory");
        assert!(memory.cg_bytes < memory.lsmr_schwarz_bytes);
        let weights: Vec<f64> = (0..observations)
            .map(|row| 0.5 + (row % 19) as f64 / 19.0)
            .collect();
        let run = |usable_bytes| {
            absorb_borrowed_intercept_auto_routed_certified(
                &design,
                BorrowedMapWeights::Provided(&weights),
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
        // The original certificate norm remains live beside the CG continuation.
        let cg_required = memory.cg_bytes + std::mem::size_of::<f64>() as u128;
        let cg = run(cg_required).expect("memory-routed CG");
        assert_eq!(cg.report().used_solver, FeSolverKind::Cg);
        assert!(cg.report().fallback_used);
        assert_eq!(
            cg.report().solver_workspace_peak.bytes(),
            Some(cg_required as u64)
        );
        assert!(cg
            .report()
            .warnings
            .first()
            .is_some_and(|warning| { warning.contains("joint execution/solver memory plan") }));

        // The old model charged the compensation table but omitted the equally
        // sized primary table. Check rejection before any LSMR numerical work.
        let bounded = run(327_680).expect("small budget must use certified CG");
        assert_eq!(bounded.report().used_solver, FeSolverKind::Cg);
        assert!(bounded.report().fallback_used);
        assert!(bounded
            .report()
            .warnings
            .iter()
            .any(|warning| { warning.contains("joint execution/solver memory plan") }));
        assert_eq!(bounded.residualized(), cg.residualized());
        assert_eq!(
            bounded.report().solver_workspace_peak,
            cg.report().solver_workspace_peak
        );

        let error = run(cg_required - 1).expect_err("neither route may allocate");
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
                    None,
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
            None,
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
                    reconstruct_projection: false,
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
