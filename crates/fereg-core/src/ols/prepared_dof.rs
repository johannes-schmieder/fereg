use crate::error::{ErrorCode, FeregError, Result};
#[cfg(test)]
use crate::metadata::AbsorbTermId;

use super::PreparedOlsData;

/// Ordinary and categorical consumers retain the same canonical report.
pub type PreparedOlsAbsorbedDofPolicy = crate::topology_dof::CategoricalTopologyDofPolicy;
pub type PreparedOlsAbsorbedDofEffectIdentity =
    crate::topology_dof::CategoricalTopologyDofEffectIdentity;
pub type PreparedOlsAbsorbedDofEffect = crate::topology_dof::CategoricalTopologyDofEffect;
pub type PreparedOlsAbsorbedDofReport = crate::topology_dof::CategoricalTopologyDofReport;

use crate::topology_dof::{
    SystemTopologyDofReservation as SystemPreparedDofReservation,
    TopologyDofReservation as PreparedDofReservation,
};

/// Compute the reference-compatible absorbed rank for the current prepared
/// single-source, intercept-only numeric FE surface.
///
/// Effect order, level counts, and memberships come exclusively from
/// [`PreparedOlsData::fe_topology`]. `PreparedOlsData` is accepted because it
/// retains the one authoritative categorical-code slice per unique source;
/// those slices are borrowed for linear-time pair edges and never copied.
pub fn compute_prepared_ols_absorbed_dof(
    prepared: &PreparedOlsData,
    policy: PreparedOlsAbsorbedDofPolicy,
) -> Result<PreparedOlsAbsorbedDofReport> {
    let mut reservation = SystemPreparedDofReservation;
    compute_with_reservation(prepared, policy, &mut reservation)
}

fn compute_with_reservation<R: PreparedDofReservation>(
    prepared: &PreparedOlsData,
    policy: PreparedOlsAbsorbedDofPolicy,
    reservation: &mut R,
) -> Result<PreparedOlsAbsorbedDofReport> {
    compute_with_nesting_reservation_validated(prepared, policy, &[], reservation)
}

fn compute_with_nesting_reservation_validated<R: PreparedDofReservation>(
    prepared: &PreparedOlsData,
    policy: PreparedOlsAbsorbedDofPolicy,
    cluster_dimensions: &[&[u32]],
    reservation: &mut R,
) -> Result<PreparedOlsAbsorbedDofReport> {
    // Ordinary entrypoints retain their narrower supported grammar while
    // report ownership and computation remain canonical.
    if !prepared.structural_intercept {
        return Err(invalid_argument(
            "prepared absorbed-DoF intercept metadata is inconsistent with the supported surface",
        ));
    }
    prepared.validate_fe_bindings()?;
    if cluster_dimensions.len() > 10 {
        return Err(invalid_argument(
            "prepared absorbed-DoF nesting view exceeds ten cluster dimensions",
        ));
    }
    if cluster_dimensions
        .iter()
        .any(|codes| codes.len() != prepared.sample.n_active())
    {
        return Err(invalid_argument(
            "prepared absorbed-DoF cluster nesting view is not sample-aligned",
        ));
    }
    crate::topology_dof::compute_with_reservation(
        prepared.fe_topology(),
        prepared.effect_specs(),
        prepared.structural_intercept,
        policy,
        cluster_dimensions,
        &[],
        None,
        reservation,
    )
}

fn invalid_argument(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

#[cfg(test)]
fn allocation(message: impl Into<String>) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{AbsorbedTerm, CompiledTermGraph, SourceEntry, SourceId};
    use crate::ols::{
        prepare_ols_data, LoadedNumericColumn, LoadedOlsInput, OlsInputBinding, OlsInputRole,
        OlsRequestV1,
    };

    #[derive(Clone, Copy)]
    enum TestWeights<'a> {
        Unit,
        Analytic(&'a [f64]),
        Frequency(&'a [u32]),
        Probability(&'a [f64]),
    }

    #[derive(Default)]
    struct InjectedReservation {
        calls: usize,
        max_additional: usize,
        fail_at: Option<usize>,
        undersize_at: Option<usize>,
        records: Vec<(usize, &'static str)>,
    }

    impl PreparedDofReservation for InjectedReservation {
        fn reserve_vec<T>(
            &mut self,
            target: &mut Vec<T>,
            additional: usize,
            label: &'static str,
        ) -> Result<()> {
            self.max_additional = self.max_additional.max(additional);
            self.records.push((additional, label));
            self.reserve(target, additional)
        }

        fn reserve_string(
            &mut self,
            target: &mut String,
            additional: usize,
            label: &'static str,
        ) -> Result<()> {
            self.max_additional = self.max_additional.max(additional);
            self.records.push((additional, label));
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation(
                    "injected prepared absorbed-DoF allocation failure",
                ));
            }
            if self.undersize_at != Some(call) {
                target
                    .try_reserve_exact(additional)
                    .map_err(|_| allocation("test prepared absorbed-DoF reservation failed"))?;
            }
            Ok(())
        }
    }

    impl InjectedReservation {
        fn reserve<T>(&mut self, target: &mut Vec<T>, additional: usize) -> Result<()> {
            let call = self.calls;
            self.calls += 1;
            if self.fail_at == Some(call) {
                return Err(allocation(
                    "injected prepared absorbed-DoF allocation failure",
                ));
            }
            if self.undersize_at != Some(call) {
                target
                    .try_reserve_exact(additional)
                    .map_err(|_| allocation("test prepared absorbed-DoF reservation failed"))?;
            }
            Ok(())
        }
    }

    #[test]
    fn sealed_reference_dof_anchors_match() {
        // Frozen licensed-Stata receipt `ols/20260821T030000Z-a5181f9`
        // records one-FE df_a=8, dof(none) df_a=19, and
        // dof(firstpair) df_a=21 on `ols_core.csv`.
        let one_fe = prepared_case(&[("firm", &[0, 1, 2, 3, 4, 5, 6, 7])], TestWeights::Unit);
        let report =
            compute_prepared_ols_absorbed_dof(&one_fe, PreparedOlsAbsorbedDofPolicy::Pairwise)
                .expect("one-FE DoF");
        assert_eq!(report.absorbed_rank, 8);
        assert_eq!(report.initial_categories, 8);
        assert_eq!(report.redundant_categories, 0);
        assert_eq!(report.effects[0].categories, 8);
        assert_eq!(report.effects[0].coefficients, 8);
        assert!(report.effects[0].redundancy_exact);

        let worker = (0..24).map(|row| (row % 12) as u32).collect::<Vec<_>>();
        let firm = (0..24).map(|row| (row % 8) as u32).collect::<Vec<_>>();
        let none = prepared_case(&[("worker", &worker), ("firm", &firm)], TestWeights::Unit);
        let report = compute_prepared_ols_absorbed_dof(&none, PreparedOlsAbsorbedDofPolicy::None)
            .expect("dof(none)");
        assert_eq!(report.absorbed_rank, 19);
        assert_eq!(report.initial_categories, 20);
        assert_eq!(report.redundant_categories, 1);
        assert_eq!(report.effects[0].categories, 12);
        assert_eq!(report.effects[0].redundant, 0);
        assert_eq!(report.effects[0].coefficients, 12);
        assert!(report.effects[0].redundancy_exact);
        assert_eq!(report.effects[1].categories, 8);
        assert_eq!(report.effects[1].redundant, 1);
        assert_eq!(report.effects[1].coefficients, 7);
        assert!(!report.effects[1].redundancy_exact);
        assert!(report.is_approximate);

        let worker = (0..24).map(|row| (row / 2) as u32).collect::<Vec<_>>();
        let firm = (0..24)
            .map(|row| {
                let worker = row / 2;
                let local = worker % 6;
                let component_offset = if worker < 6 { 0 } else { 4 };
                (component_offset + (local + row % 2) % 4) as u32
            })
            .collect::<Vec<_>>();
        let year = (0..24).map(|row| (row % 4) as u32).collect::<Vec<_>>();
        let first_pair = prepared_case(
            &[("worker", &worker), ("firm", &firm), ("year", &year)],
            TestWeights::Unit,
        );
        let report =
            compute_prepared_ols_absorbed_dof(&first_pair, PreparedOlsAbsorbedDofPolicy::FirstPair)
                .expect("dof(firstpair)");
        assert_eq!(report.absorbed_rank, 21);
        assert_eq!(report.initial_categories, 24);
        assert_eq!(report.redundant_categories, 3);
        assert_eq!(report.effects[0].categories, 12);
        assert_eq!(report.effects[0].redundant, 0);
        assert_eq!(report.effects[0].coefficients, 12);
        assert_eq!(report.effects[1].categories, 8);
        assert_eq!(report.effects[1].redundant, 2);
        assert_eq!(report.effects[1].coefficients, 6);
        assert!(report.effects[1].redundancy_exact);
        assert_eq!(report.effects[2].categories, 4);
        assert_eq!(report.effects[2].redundant, 1);
        assert_eq!(report.effects[2].coefficients, 3);
        assert!(!report.effects[2].redundancy_exact);
        assert!(report.is_approximate);
    }

    #[test]
    fn pairwise_policy_differs_from_first_pair_and_global_multipartite_formula() {
        assert_eq!(
            PreparedOlsAbsorbedDofPolicy::default(),
            PreparedOlsAbsorbedDofPolicy::Pairwise
        );
        let a = [0, 1];
        let b = [0, 0];
        let c = [0, 1];
        let prepared = prepared_case(&[("a", &a), ("b", &b), ("c", &c)], TestWeights::Unit);
        let none = compute_prepared_ols_absorbed_dof(&prepared, PreparedOlsAbsorbedDofPolicy::None)
            .expect("none");
        let first =
            compute_prepared_ols_absorbed_dof(&prepared, PreparedOlsAbsorbedDofPolicy::FirstPair)
                .expect("first pair");
        let pairwise =
            compute_prepared_ols_absorbed_dof(&prepared, PreparedOlsAbsorbedDofPolicy::Pairwise)
                .expect("pairwise");
        assert_eq!(none.absorbed_rank, 3);
        assert_eq!(first.absorbed_rank, 3);
        assert_eq!(pairwise.absorbed_rank, 2);
        assert_eq!(pairwise.effects[2].redundant, 2);

        let a = [0, 1];
        let b = [0, 1];
        let c = [0, 0];
        let prepared = prepared_case(&[("a", &a), ("b", &b), ("c", &c)], TestWeights::Unit);
        let none = compute_prepared_ols_absorbed_dof(&prepared, PreparedOlsAbsorbedDofPolicy::None)
            .expect("none");
        let first =
            compute_prepared_ols_absorbed_dof(&prepared, PreparedOlsAbsorbedDofPolicy::FirstPair)
                .expect("first pair");
        assert_eq!(none.absorbed_rank, 3);
        assert_eq!(first.absorbed_rank, 2);
    }

    #[test]
    fn repeated_sources_are_not_counted_twice_under_pairwise_policy() {
        let g = [0, 1, 0, 1];
        let h = [0, 0, 0, 0];
        let prepared = prepared_case_ordered(
            &[("g", &g), ("h", &h)],
            &[("g_first", 0), ("h", 1), ("g_again", 0)],
            TestWeights::Unit,
        );
        let pairwise =
            compute_prepared_ols_absorbed_dof(&prepared, PreparedOlsAbsorbedDofPolicy::Pairwise)
                .expect("pairwise repeated source");
        assert_eq!(prepared.fe_topology().registry().sources().len(), 2);
        assert_eq!(pairwise.absorbed_rank, 2);
        assert_eq!(pairwise.effects[2].redundant, 2);
        assert_eq!(pairwise.effects[2].coefficients, 0);
        assert!(pairwise.effects.iter().all(|effect| !effect.nested));
        assert_eq!(
            pairwise
                .effects
                .iter()
                .map(|effect| match effect.identity {
                    PreparedOlsAbsorbedDofEffectIdentity::Explicit { term_id, .. } => term_id.get(),
                    PreparedOlsAbsorbedDofEffectIdentity::ImplicitNoAbsorb => {
                        panic!("unexpected implicit row")
                    }
                })
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(
            pairwise
                .effects
                .iter()
                .map(|effect| effect.label.as_str())
                .collect::<Vec<_>>(),
            ["g_first", "h", "g_again"]
        );
    }

    #[test]
    fn topology_rank_is_weight_kind_and_row_order_invariant() {
        let g = [0, 0, 1, 1, 2, 2];
        let h = [0, 1, 0, 1, 0, 1];
        let analytic = [0.5, 2.0, 1.0, 4.0, 3.0, 1.5];
        let probability = [5.0, 1.0, 3.0, 2.0, 8.0, 4.0];
        let frequency = [2_u32, 1, 3, 1, 4, 2];
        let reports = [
            prepared_case(&[("g", &g), ("h", &h)], TestWeights::Unit),
            prepared_case(&[("g", &g), ("h", &h)], TestWeights::Analytic(&analytic)),
            prepared_case(
                &[("g", &g), ("h", &h)],
                TestWeights::Probability(&probability),
            ),
            prepared_case(&[("g", &g), ("h", &h)], TestWeights::Frequency(&frequency)),
        ]
        .iter()
        .map(|prepared| {
            compute_prepared_ols_absorbed_dof(prepared, PreparedOlsAbsorbedDofPolicy::Pairwise)
                .expect("weighted topology DoF")
                .absorbed_rank
        })
        .collect::<Vec<_>>();
        assert!(reports.iter().all(|rank| *rank == reports[0]));

        let permutation = [5_usize, 2, 0, 4, 1, 3];
        let permuted_g = permutation.map(|row| g[row]);
        let permuted_h = permutation.map(|row| h[row]);
        let permuted = prepared_case(&[("g", &permuted_g), ("h", &permuted_h)], TestWeights::Unit);
        assert_eq!(
            compute_prepared_ols_absorbed_dof(&permuted, PreparedOlsAbsorbedDofPolicy::Pairwise,)
                .expect("permuted DoF")
                .absorbed_rank,
            reports[0]
        );
    }

    #[test]
    fn report_does_not_depend_on_outcome_or_unrelated_prepared_columns() {
        let g = [0, 0, 1, 1];
        let h = [0, 1, 0, 1];
        let mut prepared = prepared_case(&[("g", &g), ("h", &h)], TestWeights::Unit);
        let expected =
            compute_prepared_ols_absorbed_dof(&prepared, PreparedOlsAbsorbedDofPolicy::Pairwise)
                .expect("baseline structural DoF");
        prepared
            .sample
            .outcome
            .copy_from_slice(&[9.0, -4.0, 2.5, 1.0]);
        prepared.sample.regressors.push(vec![3.0, 8.0, -1.0, 0.5]);
        let actual =
            compute_prepared_ols_absorbed_dof(&prepared, PreparedOlsAbsorbedDofPolicy::Pairwise)
                .expect("structural DoF with unrelated numeric changes");
        assert_eq!(actual, expected);
    }

    #[test]
    fn computation_does_not_reserve_observation_scale_workspaces() {
        let g = (0..4096).map(|row| (row % 2) as u32).collect::<Vec<_>>();
        let h = (0..4096)
            .map(|row| ((row / 2) % 2) as u32)
            .collect::<Vec<_>>();
        let prepared = prepared_case(&[("g", &g), ("h", &h)], TestWeights::Unit);
        let mut reservation = InjectedReservation::default();
        compute_with_reservation(
            &prepared,
            PreparedOlsAbsorbedDofPolicy::Pairwise,
            &mut reservation,
        )
        .expect("level-scale absorbed-DoF workspaces");
        assert_eq!(prepared.fe_topology().nobs(), 4096);
        assert_eq!(reservation.max_additional, 4);
    }

    #[test]
    fn no_absorb_has_one_implicit_rank_while_empty_explicit_topology_is_zero_rank() {
        let no_effects = prepared_case_n(4, &[], TestWeights::Unit);
        let report =
            compute_prepared_ols_absorbed_dof(&no_effects, PreparedOlsAbsorbedDofPolicy::Pairwise)
                .expect("no-effect DoF");
        assert_eq!(report.absorbed_rank, 1);
        assert_eq!(report.initial_categories, 1);
        assert_eq!(report.effects.len(), 1);
        assert_eq!(
            report.effects[0].identity,
            PreparedOlsAbsorbedDofEffectIdentity::ImplicitNoAbsorb
        );
        assert_eq!(report.effects[0].label, "_cons");
        assert!(!report.is_approximate);

        let empty: [u32; 0] = [];
        let empty_effect = prepared_case(&[("empty", &empty)], TestWeights::Unit);
        let report = compute_prepared_ols_absorbed_dof(
            &empty_effect,
            PreparedOlsAbsorbedDofPolicy::Pairwise,
        )
        .expect("empty structural DoF");
        assert_eq!(report.absorbed_rank, 0);
        assert_eq!(report.effects[0].categories, 0);
    }

    #[test]
    fn changed_outer_axes_fail_closed_with_immutable_topology() {
        let g = [0, 0, 1, 1];
        // Code and membership corruption is rejected by the topology constructor.
        // Its proof must never certify the still-mutable surrounding sample.
        for corruption in 0..3 {
            let mut prepared = prepared_case(&[("g", &g)], TestWeights::Unit);
            match corruption {
                0 => {
                    prepared.sample.original_row_by_active.pop();
                }
                1 => {
                    prepared.sample.regressors[0].pop();
                }
                _ => prepared.structural_intercept = false,
            }
            assert_eq!(
                compute_prepared_ols_absorbed_dof(
                    &prepared,
                    PreparedOlsAbsorbedDofPolicy::Pairwise
                )
                .expect_err("changed outer axes")
                .code,
                ErrorCode::InvalidArgument,
            );
        }
    }

    #[test]
    fn canonical_cluster_inputs_reject_malformed_axes_and_sentinels() {
        let codes = [0, 0, 1, 1];
        let prepared = prepared_case(&[("g", &codes)], TestWeights::Unit);
        for (dimensions, expected) in [
            (vec![&codes[..3]], ErrorCode::InvalidArgument),
            (vec![&[0, 0, u32::MAX, 1][..]], ErrorCode::InvalidData),
            (vec![&codes[..]; 11], ErrorCode::InvalidArgument),
        ] {
            let error = compute_with_nesting_reservation_validated(
                &prepared,
                PreparedOlsAbsorbedDofPolicy::Pairwise,
                &dimensions,
                &mut InjectedReservation::default(),
            )
            .unwrap_err();
            assert_eq!(error.code, expected);
        }
    }

    #[test]
    fn pair_union_find_matches_independent_graph_search() {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        for _case in 0..256 {
            let left_levels = 1 + (next_test_u64(&mut state) % 5) as usize;
            let right_levels = 1 + (next_test_u64(&mut state) % 5) as usize;
            let n = 32;
            let mut left = (0..n)
                .map(|_| (next_test_u64(&mut state) as usize % left_levels) as u32)
                .collect::<Vec<_>>();
            let mut right = (0..n)
                .map(|_| (next_test_u64(&mut state) as usize % right_levels) as u32)
                .collect::<Vec<_>>();
            for (level, code) in left.iter_mut().take(left_levels).enumerate() {
                *code = u32::try_from(level).expect("small left level");
            }
            for (level, code) in right.iter_mut().take(right_levels).enumerate() {
                *code = u32::try_from(level).expect("small right level");
            }

            let expected = graph_search_components(&left, left_levels, &right, right_levels);
            let prepared = prepared_case(&[("left", &left), ("right", &right)], TestWeights::Unit);
            let report = compute_prepared_ols_absorbed_dof(
                &prepared,
                PreparedOlsAbsorbedDofPolicy::Pairwise,
            )
            .expect("canonical union-find components");
            assert_eq!(report.effects[1].redundant, expected);
        }
    }

    #[test]
    fn every_reservation_failure_and_undersized_capacity_fails_closed() {
        let a = [0, 1, 0, 1];
        let b = [0, 0, 1, 1];
        let c = [0, 1, 1, 0];
        let prepared = prepared_case(&[("a", &a), ("b", &b), ("c", &c)], TestWeights::Unit);
        let mut successful = InjectedReservation::default();
        compute_with_reservation(
            &prepared,
            PreparedOlsAbsorbedDofPolicy::Pairwise,
            &mut successful,
        )
        .expect("successful injected reservation");
        assert!(successful.calls > 0);

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            assert_eq!(
                compute_with_reservation(
                    &prepared,
                    PreparedOlsAbsorbedDofPolicy::Pairwise,
                    &mut injected,
                )
                .expect_err("injected allocation failure")
                .code,
                ErrorCode::Allocation,
                "reservation call {fail_at}"
            );
        }
        for undersize_at in 0..successful.calls {
            if successful.records[undersize_at].0 == 0 {
                continue;
            }
            let mut injected = InjectedReservation {
                undersize_at: Some(undersize_at),
                ..InjectedReservation::default()
            };
            assert_eq!(
                compute_with_reservation(
                    &prepared,
                    PreparedOlsAbsorbedDofPolicy::Pairwise,
                    &mut injected,
                )
                .expect_err("undersized reservation")
                .code,
                ErrorCode::Allocation,
                "reservation call {undersize_at}"
            );
        }
    }

    #[test]
    fn implicit_no_absorb_row_and_label_reservations_fail_closed() {
        let prepared = prepared_case_n(4, &[], TestWeights::Unit);
        let mut successful = InjectedReservation::default();
        let report = compute_with_reservation(
            &prepared,
            PreparedOlsAbsorbedDofPolicy::Pairwise,
            &mut successful,
        )
        .expect("successful implicit no-absorb DoF reservation");
        assert!(successful
            .records
            .contains(&(1, "unable to reserve categorical absorbed-DoF rows")));
        assert!(successful
            .records
            .contains(&(5, "unable to reserve implicit no-absorb DoF label")));
        assert_eq!(report.effects.len(), 1);
        assert_eq!(
            report.effects[0].identity,
            PreparedOlsAbsorbedDofEffectIdentity::ImplicitNoAbsorb
        );
        assert_eq!(report.effects[0].label, "_cons");

        for fail_at in 0..successful.calls {
            let mut injected = InjectedReservation {
                fail_at: Some(fail_at),
                ..InjectedReservation::default()
            };
            assert_eq!(
                compute_with_reservation(
                    &prepared,
                    PreparedOlsAbsorbedDofPolicy::Pairwise,
                    &mut injected,
                )
                .expect_err("implicit no-absorb allocation failure")
                .code,
                ErrorCode::Allocation,
                "reservation call {fail_at}"
            );
        }
        for undersize_at in 0..successful.calls {
            if successful.records[undersize_at].0 == 0 {
                continue;
            }
            let mut injected = InjectedReservation {
                undersize_at: Some(undersize_at),
                ..InjectedReservation::default()
            };
            assert_eq!(
                compute_with_reservation(
                    &prepared,
                    PreparedOlsAbsorbedDofPolicy::Pairwise,
                    &mut injected,
                )
                .expect_err("undersized implicit no-absorb reservation")
                .code,
                ErrorCode::Allocation,
                "reservation call {undersize_at}"
            );
        }
    }

    fn prepared_case(effects: &[(&str, &[u32])], weights: TestWeights<'_>) -> PreparedOlsData {
        let n = effects.first().map_or(0, |(_, codes)| codes.len());
        prepared_case_n(n, effects, weights)
    }

    fn prepared_case_n(
        n: usize,
        effects: &[(&str, &[u32])],
        weights: TestWeights<'_>,
    ) -> PreparedOlsData {
        assert!(effects.iter().all(|(_, codes)| codes.len() == n));
        let effect_order = effects
            .iter()
            .enumerate()
            .map(|(index, (label, _))| (*label, index))
            .collect::<Vec<_>>();
        prepared_case_ordered_n(n, effects, &effect_order, weights)
    }

    fn prepared_case_ordered(
        sources_with_codes: &[(&str, &[u32])],
        effect_order: &[(&str, usize)],
        weights: TestWeights<'_>,
    ) -> PreparedOlsData {
        let n = sources_with_codes
            .first()
            .map_or(0, |(_, codes)| codes.len());
        prepared_case_ordered_n(n, sources_with_codes, effect_order, weights)
    }

    fn prepared_case_ordered_n(
        n: usize,
        sources_with_codes: &[(&str, &[u32])],
        effect_order: &[(&str, usize)],
        weights: TestWeights<'_>,
    ) -> PreparedOlsData {
        assert!(sources_with_codes.iter().all(|(_, codes)| codes.len() == n));
        assert!(effect_order
            .iter()
            .all(|(_, source_index)| *source_index < sources_with_codes.len()));
        let sources = sources_with_codes
            .iter()
            .enumerate()
            .map(|(index, (label, _))| SourceEntry {
                id: source_id(index),
                canonical_name: (*label).to_owned(),
            })
            .collect::<Vec<_>>();
        let mut bindings = vec![
            OlsInputBinding {
                stata_var_position: 1,
                role: OlsInputRole::Candidate,
            },
            OlsInputBinding {
                stata_var_position: 2,
                role: OlsInputRole::Outcome,
            },
        ];
        bindings.extend((0..sources_with_codes.len()).map(|index| OlsInputBinding {
            stata_var_position: u32::try_from(index + 3).expect("small source position"),
            role: OlsInputRole::NumericSource(source_id(index)),
        }));
        if let Some(role) = weight_role(weights) {
            bindings.push(OlsInputBinding {
                stata_var_position: u32::try_from(sources_with_codes.len() + 3)
                    .expect("small weight position"),
                role,
            });
        }
        let request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources,
                terms: Vec::new(),
            },
            absorbed_terms: effect_order
                .iter()
                .enumerate()
                .map(|(index, (label, source_index))| AbsorbedTerm {
                    id: AbsorbTermId::new(u32::try_from(index + 1).expect("small term"))
                        .expect("term id"),
                    intercept: true,
                    categorical_sources: vec![source_id(*source_index)],
                    slope_sources: Vec::new(),
                    label: (*label).to_owned(),
                    saved_name: String::new(),
                })
                .collect(),
            bindings,
            include_intercept: false,
            keep_singletons: true,
        };
        let raw_weights = weight_values(weights);
        let loaded = LoadedOlsInput {
            candidate: vec![true; n],
            outcome: loaded_column(vec![1.0; n]),
            numeric_sources: sources_with_codes
                .iter()
                .enumerate()
                .map(|(index, (_, codes))| {
                    (
                        source_id(index),
                        loaded_column(codes.iter().map(|code| f64::from(*code)).collect()),
                    )
                })
                .collect(),
            analytic_weight: matches!(weights, TestWeights::Analytic(_))
                .then(|| loaded_column(raw_weights.clone())),
            frequency_weight: matches!(weights, TestWeights::Frequency(_))
                .then(|| loaded_column(raw_weights.clone())),
            probability_weight: matches!(weights, TestWeights::Probability(_))
                .then(|| loaded_column(raw_weights)),
        };
        prepare_ols_data(&request, &loaded).expect("prepare absorbed-DoF case")
    }

    fn source_id(index: usize) -> SourceId {
        SourceId::new(u32::try_from(index + 1).expect("small source")).expect("source id")
    }

    fn loaded_column(values: Vec<f64>) -> LoadedNumericColumn {
        LoadedNumericColumn {
            missing: vec![false; values.len()],
            values,
        }
    }

    fn weight_role(weights: TestWeights<'_>) -> Option<OlsInputRole> {
        match weights {
            TestWeights::Unit => None,
            TestWeights::Analytic(_) => Some(OlsInputRole::AnalyticWeight),
            TestWeights::Frequency(_) => Some(OlsInputRole::FrequencyWeight),
            TestWeights::Probability(_) => Some(OlsInputRole::ProbabilityWeight),
        }
    }

    fn weight_values(weights: TestWeights<'_>) -> Vec<f64> {
        match weights {
            TestWeights::Unit => Vec::new(),
            TestWeights::Analytic(values) | TestWeights::Probability(values) => values.to_vec(),
            TestWeights::Frequency(values) => {
                values.iter().map(|value| f64::from(*value)).collect()
            }
        }
    }

    fn next_test_u64(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state
    }

    fn graph_search_components(
        left: &[u32],
        left_levels: usize,
        right: &[u32],
        right_levels: usize,
    ) -> usize {
        let nodes = left_levels + right_levels;
        let mut adjacency = vec![Vec::new(); nodes];
        for (&left_code, &right_code) in left.iter().zip(right) {
            let left_node = left_code as usize;
            let right_node = left_levels + right_code as usize;
            adjacency[left_node].push(right_node);
            adjacency[right_node].push(left_node);
        }
        let mut seen = vec![false; nodes];
        let mut components = 0;
        for start in 0..nodes {
            if seen[start] {
                continue;
            }
            components += 1;
            seen[start] = true;
            let mut stack = vec![start];
            while let Some(node) = stack.pop() {
                for neighbor in &adjacency[node] {
                    if !seen[*neighbor] {
                        seen[*neighbor] = true;
                        stack.push(*neighbor);
                    }
                }
            }
        }
        components
    }
}
