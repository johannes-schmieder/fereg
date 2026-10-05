//! Candidate-to-final RHS compaction for categorical fixed-effect topology.
//!
//! The finalized topology owns the authoritative retained-row map. This module
//! uses that map to copy a candidate observation-major `[Y, X]` batch and an
//! optional weight column exactly once onto the final sample, then hands the
//! compact matrix directly to the certified topology MAP bridge. It copies no
//! categorical codes or membership and retains no candidate-scale scratch.

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};

use super::{
    absorb_finalized_categorical_topology_map_certified, BorrowedMapWeights, CertifiedBorrowedMap,
    FinalizedCategoricalTopology, MapConfig,
};

/// Candidate-axis objective weights for categorical RHS compaction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CandidateCategoricalWeights<'a> {
    Unit,
    Provided(&'a [f64]),
}

/// One compact final-sample RHS plus optional owned objective weights.
#[derive(Debug, PartialEq)]
pub struct CompactedCategoricalRhs {
    values: DenseMatrix,
    weights: Option<Vec<f64>>,
    candidate_nrows: usize,
}

impl CompactedCategoricalRhs {
    #[must_use]
    pub const fn values(&self) -> &DenseMatrix {
        &self.values
    }

    #[must_use]
    pub fn weights(&self) -> Option<&[f64]> {
        self.weights.as_deref()
    }

    #[must_use]
    pub const fn candidate_nrows(&self) -> usize {
        self.candidate_nrows
    }

    /// Consume the compact batch and residualize it without another RHS or
    /// weight copy.
    pub fn absorb(
        self,
        topology: &FinalizedCategoricalTopology,
        config: MapConfig,
    ) -> Result<CertifiedBorrowedMap> {
        let Self {
            values,
            weights,
            candidate_nrows: _,
        } = self;
        match weights {
            None => absorb_finalized_categorical_topology_map_certified(
                topology,
                BorrowedMapWeights::Unit,
                values,
                config,
            ),
            Some(weights) => absorb_finalized_categorical_topology_map_certified(
                topology,
                BorrowedMapWeights::Provided(&weights),
                values,
                config,
            ),
        }
    }
}

/// Compact candidate-axis values and weights using the topology's exact
/// retained-row map.
pub fn compact_categorical_rhs(
    topology: &FinalizedCategoricalTopology,
    candidate_values: &DenseMatrix,
    candidate_weights: CandidateCategoricalWeights<'_>,
) -> Result<CompactedCategoricalRhs> {
    let mut reservation = SystemTopologyRhsReservation;
    compact_categorical_rhs_with_reservation(
        topology,
        candidate_values,
        candidate_weights,
        &mut reservation,
    )
}

trait TopologyRhsReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemTopologyRhsReservation;

impl TopologyRhsReservation for SystemTopologyRhsReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()> {
        target
            .try_reserve_exact(additional)
            .map_err(|_| allocation(label))
    }
}

fn compact_categorical_rhs_with_reservation<R: TopologyRhsReservation>(
    topology: &FinalizedCategoricalTopology,
    candidate_values: &DenseMatrix,
    candidate_weights: CandidateCategoricalWeights<'_>,
    reservation: &mut R,
) -> Result<CompactedCategoricalRhs> {
    topology.validate()?;
    if topology.nobs() == 0 {
        return Err(invalid_data(
            "categorical topology retains no observations for RHS compaction",
        ));
    }
    if candidate_values.ncols() == 0 {
        return Err(invalid(
            "categorical RHS compaction requires at least one value column",
        ));
    }
    match topology.retained_rows() {
        Some(rows) => {
            validate_retained_rows(rows, candidate_values.nrows())?;
            compact_selected_rhs(
                rows.iter().copied(),
                candidate_values,
                candidate_weights,
                reservation,
            )
        }
        None => {
            if topology.nobs() > candidate_values.nrows() {
                return Err(invalid("retained categorical row is out of range"));
            }
            compact_selected_rhs(
                0..topology.nobs(),
                candidate_values,
                candidate_weights,
                reservation,
            )
        }
    }
}

fn compact_selected_rhs<R: TopologyRhsReservation>(
    retained_rows: impl ExactSizeIterator<Item = usize> + Clone,
    candidate_values: &DenseMatrix,
    candidate_weights: CandidateCategoricalWeights<'_>,
    reservation: &mut R,
) -> Result<CompactedCategoricalRhs> {
    let compact_elements = retained_rows
        .len()
        .checked_mul(candidate_values.ncols())
        .ok_or_else(|| allocation("compacted categorical RHS dimensions overflow"))?;
    let mut values = Vec::new();
    reservation.reserve_vec(
        &mut values,
        compact_elements,
        "unable to reserve compacted categorical RHS",
    )?;
    for row in retained_rows.clone() {
        let start = row
            .checked_mul(candidate_values.ncols())
            .ok_or_else(|| allocation("candidate categorical RHS row offset overflows"))?;
        let end = start
            .checked_add(candidate_values.ncols())
            .ok_or_else(|| allocation("candidate categorical RHS row end overflows"))?;
        let row_values = candidate_values
            .as_slice()
            .get(start..end)
            .ok_or_else(|| invalid("candidate categorical RHS row is out of range"))?;
        extend_reserved(
            &mut values,
            row_values,
            "compacted categorical RHS capacity exhausted",
        )?;
    }
    if values.len() != compact_elements || values.iter().any(|value| !value.is_finite()) {
        return Err(invalid_data(
            "compacted categorical RHS is incomplete or nonfinite",
        ));
    }

    let weights = match candidate_weights {
        CandidateCategoricalWeights::Unit => None,
        CandidateCategoricalWeights::Provided(candidate) => {
            if candidate.len() != candidate_values.nrows() {
                return Err(invalid(
                    "candidate categorical weights and RHS have different row counts",
                ));
            }
            if candidate
                .iter()
                .any(|weight| !weight.is_finite() || *weight <= 0.0)
            {
                return Err(invalid_data(
                    "candidate categorical weights must be positive and finite",
                ));
            }
            let mut compact = Vec::new();
            reservation.reserve_vec(
                &mut compact,
                retained_rows.len(),
                "unable to reserve compacted categorical weights",
            )?;
            for row in retained_rows.clone() {
                push_reserved(
                    &mut compact,
                    candidate[row],
                    "compacted categorical weight capacity exhausted",
                )?;
            }
            Some(compact)
        }
    };

    Ok(CompactedCategoricalRhs {
        values: DenseMatrix::from_row_major(retained_rows.len(), candidate_values.ncols(), values)?,
        weights,
        candidate_nrows: candidate_values.nrows(),
    })
}

fn validate_retained_rows(retained_rows: &[usize], candidate_nrows: usize) -> Result<()> {
    if retained_rows.iter().any(|row| *row >= candidate_nrows) {
        return Err(invalid(
            "categorical topology retained row exceeds the candidate RHS axis",
        ));
    }
    if retained_rows.windows(2).any(|rows| rows[0] >= rows[1]) {
        return Err(invalid(
            "categorical topology retained rows are not strictly increasing",
        ));
    }
    Ok(())
}

fn extend_reserved<T: Copy>(target: &mut Vec<T>, values: &[T], label: &'static str) -> Result<()> {
    if target.capacity().saturating_sub(target.len()) < values.len() {
        return Err(allocation(label));
    }
    target.extend_from_slice(values);
    Ok(())
}

fn push_reserved<T>(target: &mut Vec<T>, value: T, label: &'static str) -> Result<()> {
    if target.len() == target.capacity() {
        return Err(allocation(label));
    }
    target.push(value);
    Ok(())
}

fn invalid(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn invalid_data(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidData, message)
}

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::{finalize_categorical_topology, CategoricalCodeView};

    #[derive(Default)]
    struct InjectedReservation {
        calls: usize,
        fail_at: Option<usize>,
        skip_at: Option<usize>,
    }

    impl TopologyRhsReservation for InjectedReservation {
        fn reserve_vec<T>(
            &mut self,
            target: &mut Vec<T>,
            additional: usize,
            _label: &'static str,
        ) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation("injected categorical RHS allocation failure"));
            }
            if self.skip_at == Some(call) {
                return Ok(());
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test categorical RHS reservation failed"))
        }
    }

    fn candidate_matrix() -> DenseMatrix {
        DenseMatrix::from_columns(&[
            vec![10.0, 11.0, 12.0, 13.0, 14.0, 15.0],
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        ])
        .expect("candidate matrix")
    }

    fn pruned_topology() -> FinalizedCategoricalTopology {
        let firm = [0, 0, 1, 1, 2, 2];
        let year = [0, 1, 0, 1, 0, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 3),
            CategoricalCodeView::new(&year, 2),
        ];
        finalize_categorical_topology(
            &sources,
            &[&[0, 1]],
            &[true, false, true, true, false, true],
        )
        .expect("topology")
    }

    #[test]
    fn compacts_values_and_weights_in_authoritative_row_order() {
        let topology = pruned_topology();
        assert_eq!(topology.retained_rows().unwrap(), &[0, 2, 3, 5]);
        let weights = [1.0, 9.0, 2.0, 3.0, 9.0, 4.0];
        let compact = compact_categorical_rhs(
            &topology,
            &candidate_matrix(),
            CandidateCategoricalWeights::Provided(&weights),
        )
        .expect("compact");
        assert_eq!(compact.candidate_nrows(), 6);
        assert_eq!(compact.values().nrows(), 4);
        assert_eq!(compact.values().ncols(), 2);
        assert_eq!(
            compact.values().as_slice(),
            &[10.0, 1.0, 12.0, 3.0, 13.0, 4.0, 15.0, 6.0]
        );
        assert_eq!(compact.weights(), Some(&[1.0, 2.0, 3.0, 4.0][..]));
    }

    #[test]
    fn compacted_interaction_absorbs_without_another_rhs_copy() {
        let topology = pruned_topology();
        let candidate = candidate_matrix();
        let compact =
            compact_categorical_rhs(&topology, &candidate, CandidateCategoricalWeights::Unit)
                .expect("compact");
        let result = compact
            .absorb(&topology, MapConfig::default())
            .expect("MAP");
        assert!(result.report().certified());
        // Every retained firm#year cell is a singleton in this constructed
        // final sample, so both columns residualize exactly to zero.
        assert!(result
            .residualized()
            .as_slice()
            .iter()
            .all(|value| value.abs() < 1e-12));
    }

    #[test]
    fn invalid_candidate_dimensions_and_weights_fail_closed() {
        let topology = pruned_topology();
        let candidate = candidate_matrix();
        assert_eq!(
            compact_categorical_rhs(
                &topology,
                &candidate,
                CandidateCategoricalWeights::Provided(&[1.0; 5]),
            )
            .expect_err("weight length")
            .code,
            ErrorCode::InvalidArgument
        );
        let mut bad_weights = [1.0; 6];
        bad_weights[3] = 0.0;
        assert_eq!(
            compact_categorical_rhs(
                &topology,
                &candidate,
                CandidateCategoricalWeights::Provided(&bad_weights),
            )
            .expect_err("bad weight")
            .code,
            ErrorCode::InvalidData
        );
        let too_short = DenseMatrix::from_columns(&[
            vec![1.0, 2.0, 3.0, 4.0, 5.0],
            vec![1.0, 2.0, 3.0, 4.0, 5.0],
        ])
        .expect("short matrix");
        assert_eq!(
            compact_categorical_rhs(&topology, &too_short, CandidateCategoricalWeights::Unit,)
                .expect_err("short candidate")
                .code,
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn every_reservation_and_capacity_guard_fails_closed() {
        let topology = pruned_topology();
        let candidate = candidate_matrix();
        let weights = [1.0; 6];
        let mut successful = InjectedReservation::default();
        compact_categorical_rhs_with_reservation(
            &topology,
            &candidate,
            CandidateCategoricalWeights::Provided(&weights),
            &mut successful,
        )
        .expect("successful audit");
        assert_eq!(successful.calls, 2);

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            let error = compact_categorical_rhs_with_reservation(
                &topology,
                &candidate,
                CandidateCategoricalWeights::Provided(&weights),
                &mut injected,
            )
            .expect_err("allocation failure");
            assert_eq!(error.code, ErrorCode::Allocation, "call {fail_at}");
        }

        for skip_at in 0..successful.calls {
            let mut undersized = InjectedReservation {
                skip_at: Some(skip_at),
                ..InjectedReservation::default()
            };
            let error = compact_categorical_rhs_with_reservation(
                &topology,
                &candidate,
                CandidateCategoricalWeights::Provided(&weights),
                &mut undersized,
            )
            .expect_err("capacity guard");
            assert_eq!(error.code, ErrorCode::Allocation, "call {skip_at}");
        }
    }
}
