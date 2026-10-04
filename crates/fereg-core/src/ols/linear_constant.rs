//! A dropped structural constant represented in the accepted finite basis.
//! Capture the existing QR recipe; independently certify current within rows
//! before omitted columns disappear. No outcome solve or FE projection occurs.

use super::super::finite_wls_kernel::{FIXED_BLOCK_MOMENT_ROWS, RANK_REVEALING_GUARD_EPSILONS};
use super::*;

#[derive(Debug)]
pub(crate) struct LinearConstantRepresentation {
    constant: usize,
    kept: Vec<usize>,
    pub(crate) loadings: Vec<f64>,
    geometry: (u64, u64, u64),
    reference_norm: f64,
    initial_norm: f64,
    pub(crate) residual_relative_norm: f64,
    pub(crate) relative_tolerance: f64,
}

impl LinearConstantRepresentation {
    pub(super) fn capacity_elements(&self) -> usize {
        self.kept.capacity() + self.loadings.capacity()
    }

    pub(super) fn capture(
        model: &super::super::PreparedOlsModel,
        retained: &RetainedWithinDesign,
        workspace: &FrozenLinearWorkspace,
        rank: usize,
        initial_norm: f64,
        relative_threshold: f64,
        generations: LinearSessionGenerations,
    ) -> Result<Option<Self>> {
        if initial_norm == 0.0 {
            return Ok(None);
        }
        let Some(constant) = model
            .columns
            .iter()
            .position(|column| matches!(column.kind, PreparedOlsColumnKind::Intercept))
        else {
            return Ok(None);
        };
        let candidates = workspace.candidate_ids.len();
        let Some(dropped) = workspace.permutation[rank..]
            .iter()
            .position(|&position| workspace.candidate_ids[position] == constant)
        else {
            return Ok(None);
        };
        let storage_position = retained
            .stored_generated_columns
            .iter()
            .position(|&generated| generated == constant)
            .ok_or_else(|| invalid_argument("constant recipe source is absent"))?;
        let reference_norm = retained.reference_norms[storage_position];
        let constant_norm = retained.within_norms[storage_position] / reference_norm;
        let boundary = initial_norm * relative_threshold;
        let guard = RANK_REVEALING_GUARD_EPSILONS * f64::EPSILON;
        if constant_norm >= boundary * (1.0 - guard) && constant_norm <= boundary * (1.0 + guard) {
            return Err(invalid_data(
                "constant nuisance rank is numerically ambiguous",
            ));
        }
        if constant_norm < boundary * (1.0 - guard) {
            return Ok(None);
        }
        let kept = fallible_copy(
            &workspace.kept,
            "unable to reserve constant basis identities",
        )?;
        let mut loadings = fallible_empty(rank, "unable to reserve constant basis recipe")?;
        for &generated in &kept {
            let pivot = workspace.permutation[..rank]
                .iter()
                .position(|&position| workspace.candidate_ids[position] == generated)
                .ok_or_else(|| invalid_argument("constant recipe pivot is absent"))?;
            let position = retained
                .stored_generated_columns
                .iter()
                .position(|&column| column == generated)
                .ok_or_else(|| invalid_argument("constant recipe basis source is absent"))?;
            let gamma = workspace.inverse_lower[pivot * (candidates - rank) + dropped];
            let loading = (gamma * reference_norm) / retained.reference_norms[position];
            if !loading.is_finite() {
                return Err(invalid_data("constant basis recipe is not finite"));
            }
            loadings.push(loading);
        }
        Ok(Some(Self {
            constant,
            kept,
            loadings,
            geometry: (
                generations.sample,
                generations.topology,
                generations.weights,
            ),
            reference_norm,
            initial_norm,
            residual_relative_norm: f64::INFINITY,
            relative_tolerance: relative_threshold * (1.0 + guard),
        }))
    }

    pub(super) fn certify(
        &mut self,
        retained: &RetainedWithinDesign,
        basis: &FrozenLinearBasis,
        weight_frame: (BorrowedMapWeights<'_>, f64),
        generations: LinearSessionGenerations,
        row: &mut Vec<f64>,
        interrupt: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        let (weights, weight_divisor) = weight_frame;
        // Every successful final-Hessian route commits a current canonical
        // rank solve. A restored recipe may serve rollback, but its old-weight
        // normalization cannot authorize final inference after a frame refresh.
        if self.geometry
            != (
                generations.sample,
                generations.topology,
                generations.weights,
            )
            || self.kept != basis.kept_generated_columns()
            || self.loadings.len() != self.kept.len()
            || !retained.generations.same_design(generations)
        {
            return Err(invalid_argument(
                "constant recipe has stale geometry, weights or basis identities",
            ));
        }
        let constant_position = retained
            .stored_generated_columns
            .iter()
            .position(|&column| column == self.constant)
            .ok_or_else(|| invalid_argument("current projected constant is absent"))?;
        let mut positions =
            fallible_empty(self.kept.len(), "unable to reserve constant span positions")?;
        for &generated in &self.kept {
            positions.push(
                retained
                    .stored_generated_columns
                    .iter()
                    .position(|&column| column == generated)
                    .ok_or_else(|| invalid_argument("current constant span column is absent"))?,
            );
        }
        resize_zeroed(
            row,
            retained.storage.columns(),
            "unable to reserve constant span row",
        )?;
        let mut reader = retained.storage.reader();
        let mut residual_norm = 0.0_f64;
        for observation in 0..retained.storage.rows() {
            if observation % FIXED_BLOCK_MOMENT_ROWS == 0 {
                interrupt()?;
            }
            reader.fill_row(observation, row)?;
            let mut fitted = 0.0;
            let mut correction = 0.0;
            for (&position, &loading) in positions.iter().zip(&self.loadings) {
                compensated_add(
                    loading * row[position] / self.reference_norm,
                    &mut fitted,
                    &mut correction,
                )?;
            }
            let residual = row[constant_position] / self.reference_norm - (fitted + correction);
            let weight = match weights {
                BorrowedMapWeights::Unit => 1.0,
                BorrowedMapWeights::Provided(values) => values[observation],
            };
            residual_norm = residual_norm.hypot((weight / weight_divisor).sqrt() * residual);
        }
        self.residual_relative_norm = residual_norm / self.initial_norm;
        if !self.residual_relative_norm.is_finite()
            || self.residual_relative_norm > self.relative_tolerance
            || self.loadings.iter().all(|&value| value == 0.0)
        {
            return Err(invalid_data(
                "current constant basis representation did not certify",
            ));
        }
        Ok(())
    }
}
