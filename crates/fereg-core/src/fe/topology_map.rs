//! Compatibility entry points for one finalized FE binding.
use super::{
    AbsorbConfig, BorrowedMapWeights, CertifiedBorrowedMap, FeSolverKind,
    FinalizedCategoricalTopology, MapConfig,
};
use crate::data::DenseMatrix;
use crate::error::{allocation, Result};
use crate::memory::SystemVecReservation as SystemTopologyMapReservation;
use crate::memory::VecReservation as TopologyMapReservation;

fn bind_with_reservation<'a, R: TopologyMapReservation>(
    topology: &'a FinalizedCategoricalTopology,
    reservation: &mut R,
) -> Result<super::BorrowedInterceptFeDesign<'a>> {
    topology.validate()?;
    let mut sources = Vec::new();
    reservation.reserve_vec(
        &mut sources,
        topology.compacted_sources().len(),
        "unable to reserve finalized FE source views",
    )?;
    for source in topology.compacted_sources() {
        if sources.len() == sources.capacity() {
            return Err(allocation("finalized FE source-view capacity exhausted"));
        }
        sources.push(source.view());
    }
    super::borrow_categorical_topology_design(
        topology.nobs(),
        topology.registry(),
        &sources,
        topology.effect_to_topology(),
    )
}
fn absorb_finalized_categorical_topology_map_with_reservation<R: TopologyMapReservation>(
    topology: &FinalizedCategoricalTopology,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    map: MapConfig,
    retain: bool,
    reservation: &mut R,
) -> Result<CertifiedBorrowedMap> {
    let design = bind_with_reservation(topology, reservation)?;
    super::session::project_intercepts(
        &design,
        weights,
        values,
        AbsorbConfig {
            solver: FeSolverKind::Map,
            map,
            retain_coefficients: retain,
            ..AbsorbConfig::default()
        },
        false,
        false,
    )
}
pub fn absorb_finalized_categorical_topology_map_certified(
    topology: &FinalizedCategoricalTopology,
    weights: BorrowedMapWeights<'_>,
    values: DenseMatrix,
    config: MapConfig,
) -> Result<CertifiedBorrowedMap> {
    absorb_finalized_categorical_topology_map_with_reservation(
        topology,
        weights,
        values,
        config,
        false,
        &mut SystemTopologyMapReservation,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::fe::finalize_categorical_topology;
    use crate::fe::CategoricalCodeView;

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
        let result = absorb_finalized_categorical_topology_map_with_reservation(
            &topology,
            BorrowedMapWeights::Unit,
            matrix(&columns),
            MapConfig {
                tolerance: 1e-12,
                max_iterations: 20_000,
                check_every: 1,
                symmetric: true,
            },
            true,
            &mut SystemTopologyMapReservation,
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
