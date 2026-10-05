//! Certified MAP residualization for finalized categorical topology.
//!
//! This is the numerical bridge between interaction-aware final-sample topology
//! and the existing strict borrowed-intercept MAP solver. It constructs only a
//! small compact-source view table and O(U + G) borrowed design tables before
//! handing ownership of the dense RHS to MAP. No observation-scale codes,
//! weights, or membership are copied.

use crate::data::DenseMatrix;
use crate::error::{ErrorCode, FeregError, Result};
use crate::memory::FeSolverMemoryBudget;

use super::{
    absorb_borrowed_intercept_auto_certified, absorb_borrowed_intercept_auto_direct_lsmr_certified,
    absorb_borrowed_intercept_auto_routed_certified, absorb_borrowed_intercept_cg_certified,
    absorb_borrowed_intercept_cg_retaining_certified,
    absorb_borrowed_intercept_irons_tuck_certified, absorb_borrowed_intercept_lsmr_certified,
    absorb_borrowed_intercept_map_certified, absorb_borrowed_intercept_map_retaining_certified,
    borrow_categorical_topology_design, BorrowedMapWeights, CategoricalCodeView,
    CertifiedBorrowedMap, FinalizedCategoricalTopology, LsmrConfig, MapConfig,
};

pub(crate) fn absorb_finalized_categorical_topology_irons_tuck_certified(
    topology: &FinalizedCategoricalTopology,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    retain_coefficients: bool,
) -> Result<CertifiedBorrowedMap> {
    topology.validate()?;
    let nobs = topology.nobs();
    if values.nrows() != nobs {
        return Err(invalid(
            "finalized categorical topology and Irons-Tuck values have different observation counts",
        ));
    }
    let mut source_views = Vec::new();
    source_views
        .try_reserve_exact(topology.compacted_sources().len())
        .map_err(|_| {
            allocation("unable to reserve finalized categorical Irons-Tuck source views")
        })?;
    for source in topology.compacted_sources() {
        source_views.push(source.view());
    }
    let design = borrow_categorical_topology_design(
        nobs,
        topology.registry(),
        &source_views,
        topology.effect_to_topology(),
    )?;
    absorb_borrowed_intercept_irons_tuck_certified(
        &design,
        weights,
        values,
        config,
        retain_coefficients,
    )
}

/// Apply the topology-selected direct LSMR automatic fast lane without an
/// observation-scale code copy or a MAP pilot.
pub(crate) fn absorb_finalized_categorical_topology_auto_direct_lsmr_certified(
    topology: &FinalizedCategoricalTopology,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    lsmr: LsmrConfig,
    solver_memory_budget: Option<FeSolverMemoryBudget>,
    retain_coefficients: bool,
) -> Result<CertifiedBorrowedMap> {
    topology.validate()?;
    let nobs = topology.nobs();
    if values.nrows() != nobs {
        return Err(invalid(
            "finalized categorical topology and direct auto LSMR values have different observation counts",
        ));
    }

    let mut source_views: Vec<CategoricalCodeView<'_>> = Vec::new();
    source_views
        .try_reserve_exact(topology.compacted_sources().len())
        .map_err(|_| {
            allocation("unable to reserve finalized categorical direct auto LSMR source views")
        })?;
    for source in topology.compacted_sources() {
        push_reserved(
            &mut source_views,
            source.view(),
            "finalized categorical direct auto LSMR source-view capacity exhausted",
        )?;
    }
    let design = borrow_categorical_topology_design(
        nobs,
        topology.registry(),
        &source_views,
        topology.effect_to_topology(),
    )?;
    absorb_borrowed_intercept_auto_direct_lsmr_certified(
        &design,
        weights,
        values,
        config,
        lsmr,
        solver_memory_budget,
        false,
        retain_coefficients,
    )
}

pub(crate) fn absorb_finalized_categorical_topology_lsmr_certified(
    topology: &FinalizedCategoricalTopology,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: LsmrConfig,
    retain_coefficients: bool,
) -> Result<CertifiedBorrowedMap> {
    topology.validate()?;
    let nobs = topology.nobs();
    if values.nrows() != nobs {
        return Err(invalid(
            "finalized categorical topology and LSMR values have different observation counts",
        ));
    }
    let mut source_views = Vec::new();
    source_views
        .try_reserve_exact(topology.compacted_sources().len())
        .map_err(|_| allocation("unable to reserve finalized categorical LSMR source views"))?;
    for source in topology.compacted_sources() {
        source_views.push(source.view());
    }
    let design = borrow_categorical_topology_design(
        nobs,
        topology.registry(),
        &source_views,
        topology.effect_to_topology(),
    )?;
    absorb_borrowed_intercept_lsmr_certified(&design, weights, values, config, retain_coefficients)
}

/// Residualize `values` against all finalized categorical effects in requested
/// term order and independently certify the returned residuals.
pub fn absorb_finalized_categorical_topology_map_certified(
    topology: &FinalizedCategoricalTopology,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemTopologyMapReservation;
    absorb_finalized_categorical_topology_map_with_reservation(
        topology,
        weights,
        values,
        config,
        false,
        &mut reservation,
    )
}

pub(crate) fn absorb_finalized_categorical_topology_map_retaining_certified(
    topology: &FinalizedCategoricalTopology,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    let mut reservation = SystemTopologyMapReservation;
    absorb_finalized_categorical_topology_map_with_reservation(
        topology,
        weights,
        values,
        config,
        true,
        &mut reservation,
    )
}

/// Residualize against finalized categorical effects with genuine CG while
/// retaining the same borrowed topology and independent final certificate.
pub fn absorb_finalized_categorical_topology_cg_certified(
    topology: &FinalizedCategoricalTopology,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    absorb_finalized_categorical_topology_cg_with_retention(
        topology, weights, values, config, false,
    )
}

pub(crate) fn absorb_finalized_categorical_topology_cg_retaining_certified(
    topology: &FinalizedCategoricalTopology,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    absorb_finalized_categorical_topology_cg_with_retention(topology, weights, values, config, true)
}

fn absorb_finalized_categorical_topology_cg_with_retention(
    topology: &FinalizedCategoricalTopology,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    retain_coefficients: bool,
) -> Result<CertifiedBorrowedMap> {
    topology.validate()?;
    let nobs = topology.nobs();
    if values.nrows() != nobs {
        return Err(invalid(
            "finalized categorical topology and CG values have different observation counts",
        ));
    }

    let mut source_views: Vec<CategoricalCodeView<'_>> = Vec::new();
    source_views
        .try_reserve_exact(topology.compacted_sources().len())
        .map_err(|_| allocation("unable to reserve finalized categorical CG source views"))?;
    for source in topology.compacted_sources() {
        push_reserved(
            &mut source_views,
            source.view(),
            "finalized categorical CG source-view capacity exhausted",
        )?;
    }
    let design = borrow_categorical_topology_design(
        nobs,
        topology.registry(),
        &source_views,
        topology.effect_to_topology(),
    )?;
    if retain_coefficients {
        absorb_borrowed_intercept_cg_retaining_certified(&design, weights, values, config)
    } else {
        absorb_borrowed_intercept_cg_certified(&design, weights, values, config)
    }
}

/// Apply the four-sweep MAP/CG automatic policy without copying finalized
/// categorical observation codes.
pub fn absorb_finalized_categorical_topology_auto_certified(
    topology: &FinalizedCategoricalTopology,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    topology.validate()?;
    let nobs = topology.nobs();
    if values.nrows() != nobs {
        return Err(invalid(
            "finalized categorical topology and auto values have different observation counts",
        ));
    }

    let mut source_views: Vec<CategoricalCodeView<'_>> = Vec::new();
    source_views
        .try_reserve_exact(topology.compacted_sources().len())
        .map_err(|_| allocation("unable to reserve finalized categorical auto source views"))?;
    for source in topology.compacted_sources() {
        push_reserved(
            &mut source_views,
            source.view(),
            "finalized categorical auto source-view capacity exhausted",
        )?;
    }
    let design = borrow_categorical_topology_design(
        nobs,
        topology.registry(),
        &source_views,
        topology.effect_to_topology(),
    )?;
    absorb_borrowed_intercept_auto_certified(&design, weights, values, config)
}

/// Apply the frozen MAP-pilot/CG/LSMR policy while keeping finalized
/// categorical codes borrowed unless LSMR is actually selected.
pub(crate) fn absorb_finalized_categorical_topology_auto_routed_certified(
    topology: &FinalizedCategoricalTopology,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    lsmr: LsmrConfig,
    solver_memory_budget: Option<FeSolverMemoryBudget>,
    retain_coefficients: bool,
) -> Result<CertifiedBorrowedMap> {
    topology.validate()?;
    let nobs = topology.nobs();
    if values.nrows() != nobs {
        return Err(invalid(
            "finalized categorical topology and auto values have different observation counts",
        ));
    }

    let mut source_views: Vec<CategoricalCodeView<'_>> = Vec::new();
    source_views
        .try_reserve_exact(topology.compacted_sources().len())
        .map_err(|_| allocation("unable to reserve finalized categorical auto source views"))?;
    for source in topology.compacted_sources() {
        push_reserved(
            &mut source_views,
            source.view(),
            "finalized categorical auto source-view capacity exhausted",
        )?;
    }
    let design = borrow_categorical_topology_design(
        nobs,
        topology.registry(),
        &source_views,
        topology.effect_to_topology(),
    )?;
    absorb_borrowed_intercept_auto_routed_certified(
        &design,
        weights,
        values,
        config,
        lsmr,
        solver_memory_budget,
        retain_coefficients,
    )
}

trait TopologyMapReservation {
    fn reserve_vec<T>(
        &mut self,
        target: &mut Vec<T>,
        additional: usize,
        label: &'static str,
    ) -> Result<()>;
}

struct SystemTopologyMapReservation;

impl TopologyMapReservation for SystemTopologyMapReservation {
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

fn absorb_finalized_categorical_topology_map_with_reservation<R: TopologyMapReservation>(
    topology: &FinalizedCategoricalTopology,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
    retain_coefficients: bool,
    reservation: &mut R,
) -> Result<CertifiedBorrowedMap> {
    topology.validate()?;
    let nobs = topology.nobs();
    if values.nrows() != nobs {
        return Err(invalid(
            "finalized categorical topology and MAP values have different observation counts",
        ));
    }

    let mut source_views: Vec<CategoricalCodeView<'_>> = Vec::new();
    reservation.reserve_vec(
        &mut source_views,
        topology.compacted_sources().len(),
        "unable to reserve finalized categorical MAP source views",
    )?;
    for source in topology.compacted_sources() {
        push_reserved(
            &mut source_views,
            source.view(),
            "finalized categorical MAP source-view capacity exhausted",
        )?;
    }
    let design = borrow_categorical_topology_design(
        nobs,
        topology.registry(),
        &source_views,
        topology.effect_to_topology(),
    )?;
    if retain_coefficients {
        absorb_borrowed_intercept_map_retaining_certified(&design, weights, values, config)
    } else {
        absorb_borrowed_intercept_map_certified(&design, weights, values, config)
    }
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

fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::finalize_categorical_topology;

    #[derive(Default)]
    struct InjectedReservation {
        fail: bool,
        skip: bool,
        calls: usize,
    }

    impl TopologyMapReservation for InjectedReservation {
        fn reserve_vec<T>(
            &mut self,
            target: &mut Vec<T>,
            additional: usize,
            _label: &'static str,
        ) -> Result<()> {
            self.calls += 1;
            if self.fail {
                return Err(allocation(
                    "injected finalized categorical MAP allocation failure",
                ));
            }
            if self.skip {
                return Ok(());
            }
            target
                .try_reserve_exact(additional)
                .map_err(|_| allocation("test finalized categorical MAP reservation failed"))
        }
    }

    fn matrix(columns: &[Vec<f64>]) -> DenseMatrix {
        DenseMatrix::from_columns(columns).expect("matrix")
    }

    #[test]
    fn one_interaction_matches_direct_weighted_group_demeaning() {
        let firm = [0, 0, 0, 0, 1, 1, 1, 1];
        let year = [0, 0, 1, 1, 0, 0, 1, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let topology = finalize_categorical_topology(&sources, &[&[0, 1]], &[true; 8])
            .expect("final topology");
        let weights = [1.0, 3.0, 2.0, 2.0, 4.0, 1.0, 1.0, 5.0];
        let first = vec![1.0, 5.0, 2.0, 6.0, 3.0, 8.0, 4.0, 10.0];
        let second = vec![2.0, 4.0, 1.0, 9.0, 7.0, 2.0, 3.0, 11.0];
        let result = absorb_finalized_categorical_topology_map_certified(
            &topology,
            BorrowedMapWeights::Provided(&weights),
            matrix(&[first.clone(), second.clone()]),
            MapConfig::default(),
        )
        .expect("MAP");
        assert!(result.report().certified());

        let compacted_views = [
            topology.compacted_sources()[0].view(),
            topology.compacted_sources()[1].view(),
        ];
        let interaction_codes = topology
            .registry()
            .source(0)
            .expect("interaction source")
            .codes(&compacted_views)
            .expect("interaction codes");
        for column in 0..2 {
            let original = if column == 0 { &first } else { &second };
            for level in 0..4_u32 {
                let mut sum_w = 0.0;
                let mut sum_wx = 0.0;
                for row in 0..8 {
                    if interaction_codes[row] == level {
                        sum_w += weights[row];
                        sum_wx += weights[row] * original[row];
                    }
                }
                let mean = sum_wx / sum_w;
                for row in 0..8 {
                    if interaction_codes[row] == level {
                        let expected = original[row] - mean;
                        assert!((result.residualized()[(row, column)] - expected).abs() < 1e-12);
                    }
                }
            }
        }
    }

    #[test]
    fn repeated_interaction_terms_do_not_change_the_projection() {
        let firm = [0, 0, 0, 0, 1, 1, 1, 1];
        let year = [0, 0, 1, 1, 0, 0, 1, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let single = finalize_categorical_topology(&sources, &[&[0, 1]], &[true; 8])
            .expect("single topology");
        let repeated =
            finalize_categorical_topology(&sources, &[&[0, 1], &[1, 0], &[0, 1, 0]], &[true; 8])
                .expect("repeated topology");
        let values = matrix(&[vec![1.0, 5.0, 2.0, 6.0, 3.0, 8.0, 4.0, 10.0]]);
        let left = absorb_finalized_categorical_topology_map_certified(
            &single,
            BorrowedMapWeights::Unit,
            values.clone(),
            MapConfig::default(),
        )
        .expect("single MAP");
        let right = absorb_finalized_categorical_topology_map_certified(
            &repeated,
            BorrowedMapWeights::Unit,
            values,
            MapConfig::default(),
        )
        .expect("repeated MAP");
        assert_eq!(left.residualized(), right.residualized());
        assert_eq!(repeated.registry().sources().len(), 1);
    }

    #[test]
    fn mixed_single_and_interaction_effects_certify() {
        let firm = [0, 0, 0, 0, 1, 1, 1, 1];
        let year = [0, 0, 1, 1, 0, 0, 1, 1];
        let region = [0, 1, 0, 1, 0, 1, 0, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
            CategoricalCodeView::new(&region, 2),
        ];
        let topology = finalize_categorical_topology(&sources, &[&[0, 1], &[2]], &[true; 8])
            .expect("mixed topology");
        let values = matrix(&[vec![1.0, 3.0, 2.0, 6.0, 4.0, 8.0, 5.0, 9.0]]);
        let result = absorb_finalized_categorical_topology_map_certified(
            &topology,
            BorrowedMapWeights::Unit,
            values,
            MapConfig::default(),
        )
        .expect("mixed MAP");
        assert!(result.report().certified());
        assert!(result
            .residualized()
            .as_slice()
            .iter()
            .all(|value| value.is_finite()));
    }

    #[test]
    fn retaining_map_coefficients_reconstruct_every_rhs() {
        let worker = [0, 0, 1, 1, 2, 2, 3, 3];
        let firm = [0, 1, 1, 2, 2, 3, 3, 0];
        let sources = [
            CategoricalCodeView::new(&worker, 4),
            CategoricalCodeView::new(&firm, 4),
        ];
        let topology =
            finalize_categorical_topology(&sources, &[&[0_usize][..], &[1_usize][..]], &[true; 8])
                .expect("crossed topology");
        let columns = [
            vec![1.0, 3.0, 2.0, 6.0, 4.0, 8.0, 5.0, 9.0],
            vec![2.0, 1.0, 4.0, 3.0, 8.0, 5.0, 7.0, 6.0],
        ];
        let result = absorb_finalized_categorical_topology_map_retaining_certified(
            &topology,
            BorrowedMapWeights::Unit,
            matrix(&columns),
            MapConfig {
                tolerance: 1e-12,
                max_iterations: 20_000,
                check_every: 1,
                symmetric: true,
            },
        )
        .expect("retaining MAP");
        let (residualized, report, coefficients) = result.into_full_parts();
        assert!(report.certified());
        let coefficients = coefficients.expect("retained coefficient batch");
        for (rhs, original) in columns.iter().enumerate() {
            let alpha = coefficients.rhs(rhs).expect("coefficient RHS");
            for row in 0..original.len() {
                let reconstructed = residualized[(row, rhs)]
                    + alpha[worker[row] as usize]
                    + alpha[4 + firm[row] as usize];
                assert!((reconstructed - original[row]).abs() <= 1e-10);
            }
        }
    }

    #[test]
    fn empty_effect_table_preserves_values_under_generic_map_contract() {
        let raw = [0, 1, 0];
        let sources = [CategoricalCodeView::new(&raw, 2)];
        let topology =
            finalize_categorical_topology(&sources, &[], &[true; 3]).expect("empty topology");
        let values = matrix(&[vec![1.0, 2.0, 3.0]]);
        let result = absorb_finalized_categorical_topology_map_certified(
            &topology,
            BorrowedMapWeights::Unit,
            values.clone(),
            MapConfig::default(),
        )
        .expect("empty MAP");
        assert_eq!(result.residualized(), &values);
    }

    #[test]
    fn observation_mismatch_fails_before_map() {
        let raw = [0, 1, 0];
        let sources = [CategoricalCodeView::new(&raw, 2)];
        let topology =
            finalize_categorical_topology(&sources, &[&[0]], &[true; 3]).expect("topology");
        let error = absorb_finalized_categorical_topology_map_certified(
            &topology,
            BorrowedMapWeights::Unit,
            matrix(&[vec![1.0, 2.0]]),
            MapConfig::default(),
        )
        .expect_err("row mismatch");
        assert_eq!(error.code, ErrorCode::InvalidArgument);
    }

    #[test]
    fn source_view_reservation_and_capacity_guard_fail_before_map() {
        let firm = [0, 0, 1, 1];
        let year = [0, 1, 0, 1];
        let sources = [
            CategoricalCodeView::new(&firm, 2),
            CategoricalCodeView::new(&year, 2),
        ];
        let topology =
            finalize_categorical_topology(&sources, &[&[0, 1]], &[true; 4]).expect("topology");
        let values = matrix(&[vec![1.0, 2.0, 3.0, 4.0]]);

        let mut failed = InjectedReservation {
            fail: true,
            ..InjectedReservation::default()
        };
        let error = absorb_finalized_categorical_topology_map_with_reservation(
            &topology,
            BorrowedMapWeights::Unit,
            values.clone(),
            MapConfig::default(),
            false,
            &mut failed,
        )
        .expect_err("injected allocation failure");
        assert_eq!(error.code, ErrorCode::Allocation);
        assert_eq!(failed.calls, 1);

        let mut undersized = InjectedReservation {
            skip: true,
            ..InjectedReservation::default()
        };
        let error = absorb_finalized_categorical_topology_map_with_reservation(
            &topology,
            BorrowedMapWeights::Unit,
            values,
            MapConfig::default(),
            false,
            &mut undersized,
        )
        .expect_err("capacity guard");
        assert_eq!(error.code, ErrorCode::Allocation);
        assert_eq!(undersized.calls, 1);
    }
}
