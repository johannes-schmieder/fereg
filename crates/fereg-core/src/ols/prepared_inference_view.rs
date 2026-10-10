//! Shared absorbed-DoF boundary for certified prepared OLS inference.
//!
//! The legacy single-source prepared path and the categorical-interaction path
//! retain different public report identities.  Numerical inference consumes a
//! validated common summary, while each result continues to retain its exact
//! source-bound absorbed-DoF report.

use crate::error::{allocation, invalid_argument as invalid, ErrorCode, FeregError, Result};
use crate::prepared_categorical_data::PreparedOlsData;
use crate::prepared_fit_view::{OrdinaryPrepared, PreparedOlsFitView};
use crate::topology_dof::{
    CategoricalTopologyDofEffectIdentity, CategoricalTopologyDofPolicy,
    CategoricalTopologyDofReport,
};

use super::PreparedOlsAbsorbedDofPolicy;

/// Topology-validated summary consumed by common inference calculations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PreparedOlsAbsorbedDofSummary {
    pub initial_categories: usize,
    pub redundant_categories: usize,
    pub absorbed_rank: usize,
    pub nested_categories: usize,
    pub is_approximate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NestingState {
    Iid,
    ClusterAware,
}

pub(crate) trait PreparedOlsInferenceView: PreparedOlsFitView {
    fn inference_compute_absorbed_dof(
        &self,
        policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<CategoricalTopologyDofReport>;

    fn inference_absorbed_dof_summary(
        &self,
        report: &CategoricalTopologyDofReport,
        expected_policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<PreparedOlsAbsorbedDofSummary>;

    fn inference_compute_clustered_absorbed_dof(
        &self,
        policy: PreparedOlsAbsorbedDofPolicy,
        cluster_dimensions: &[&[u32]],
    ) -> Result<CategoricalTopologyDofReport>;

    fn inference_clustered_absorbed_dof_summary(
        &self,
        report: &CategoricalTopologyDofReport,
        expected_policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<PreparedOlsAbsorbedDofSummary>;

    fn inference_has_explicit_absorbed_effects(&self) -> bool;
}

impl PreparedOlsInferenceView for OrdinaryPrepared<'_> {
    fn inference_compute_absorbed_dof(
        &self,
        policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<CategoricalTopologyDofReport> {
        if !self.structural_intercept() {
            return Err(FeregError::new(ErrorCode::InvalidArgument,
                "prepared absorbed-DoF intercept metadata is inconsistent with the supported surface"));
        }
        self.validate_fe_bindings()?;
        self.absorbed_dof(categorical_policy(policy), &[])
    }

    fn inference_absorbed_dof_summary(
        &self,
        report: &CategoricalTopologyDofReport,
        expected_policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<PreparedOlsAbsorbedDofSummary> {
        validate_categorical_report(self, report, expected_policy, NestingState::Iid)
    }

    fn inference_compute_clustered_absorbed_dof(
        &self,
        policy: PreparedOlsAbsorbedDofPolicy,
        cluster_dimensions: &[&[u32]],
    ) -> Result<CategoricalTopologyDofReport> {
        if !self.structural_intercept() {
            return Err(FeregError::new(ErrorCode::InvalidArgument,
                "prepared absorbed-DoF intercept metadata is inconsistent with the supported surface"));
        }
        self.validate_fe_bindings()?;
        self.absorbed_dof(categorical_policy(policy), cluster_dimensions)
    }

    fn inference_clustered_absorbed_dof_summary(
        &self,
        report: &CategoricalTopologyDofReport,
        expected_policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<PreparedOlsAbsorbedDofSummary> {
        validate_categorical_report(self, report, expected_policy, NestingState::ClusterAware)
    }

    fn inference_has_explicit_absorbed_effects(&self) -> bool {
        !self.fe_effects().is_empty()
    }
}

impl PreparedOlsInferenceView for PreparedOlsData {
    fn inference_compute_absorbed_dof(
        &self,
        policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<CategoricalTopologyDofReport> {
        self.absorbed_dof(categorical_policy(policy), &[])
    }

    fn inference_absorbed_dof_summary(
        &self,
        report: &CategoricalTopologyDofReport,
        expected_policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<PreparedOlsAbsorbedDofSummary> {
        validate_categorical_report(self, report, expected_policy, NestingState::Iid)
    }

    fn inference_compute_clustered_absorbed_dof(
        &self,
        policy: PreparedOlsAbsorbedDofPolicy,
        cluster_dimensions: &[&[u32]],
    ) -> Result<CategoricalTopologyDofReport> {
        self.absorbed_dof(categorical_policy(policy), cluster_dimensions)
    }

    fn inference_clustered_absorbed_dof_summary(
        &self,
        report: &CategoricalTopologyDofReport,
        expected_policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<PreparedOlsAbsorbedDofSummary> {
        validate_categorical_report(self, report, expected_policy, NestingState::ClusterAware)
    }

    fn inference_has_explicit_absorbed_effects(&self) -> bool {
        !self.effect_specs().is_empty()
    }
}

impl PreparedOlsAbsorbedDofSummary {
    // Each caller validates row identity and estimator-specific rank semantics
    // before accumulation. Checked sums preserve overflow and error order.
    fn include(
        &mut self,
        categories: usize,
        redundant: usize,
        coefficients: usize,
        nested: bool,
        exact: bool,
    ) -> Result<()> {
        self.initial_categories = self
            .initial_categories
            .checked_add(categories)
            .ok_or_else(|| allocation("absorbed category total overflows"))?;
        self.redundant_categories = self
            .redundant_categories
            .checked_add(redundant)
            .ok_or_else(|| allocation("absorbed redundancy total overflows"))?;
        self.absorbed_rank = self
            .absorbed_rank
            .checked_add(coefficients)
            .ok_or_else(|| allocation("absorbed rank total overflows"))?;
        if nested {
            self.nested_categories = self
                .nested_categories
                .checked_add(categories)
                .ok_or_else(|| allocation("absorbed nested category total overflows"))?;
        }
        self.is_approximate |= !exact;
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn compute_canonical_prepared_ols_absorbed_dof(
    prepared: &PreparedOlsData,
    policy: PreparedOlsAbsorbedDofPolicy,
) -> Result<CategoricalTopologyDofReport> {
    OrdinaryPrepared(prepared).inference_compute_absorbed_dof(policy)
}

fn validate_categorical_report(
    prepared: &PreparedOlsData,
    report: &CategoricalTopologyDofReport,
    expected_policy: PreparedOlsAbsorbedDofPolicy,
    nesting_state: NestingState,
) -> Result<PreparedOlsAbsorbedDofSummary> {
    if report.policy != categorical_policy(expected_policy) {
        return Err(invalid(
            "categorical absorbed-DoF report policy differs from the requested policy",
        ));
    }
    let topology = prepared.fe_topology();
    let specs = prepared.effect_specs();
    let implicit_no_absorb = prepared.fit_structural_intercept()
        && specs.is_empty()
        && prepared.fit_sample().n_active() > 0;
    let explicit_rows = specs.iter().try_fold(0_usize, |total, spec| {
        total
            .checked_add(usize::from(spec.intercept) + spec.slopes.len())
            .ok_or_else(|| allocation("categorical absorbed-DoF extended row count overflows"))
    })?;
    let expected_rows = explicit_rows
        .checked_add(usize::from(implicit_no_absorb))
        .ok_or_else(|| allocation("categorical absorbed-DoF row count overflows"))?;
    if report.effects.len() != expected_rows
        || topology.effect_to_topology().len() != specs.len()
        || (nesting_state == NestingState::Iid && report.nested_categories != 0)
    {
        return Err(invalid(
            "categorical absorbed-DoF report dimensions or iid nesting state are inconsistent",
        ));
    }

    let mut computed = summary(0, 0, 0, 0, false);
    let mut rows = report.effects.as_slice();
    if implicit_no_absorb {
        let (implicit, remaining) = rows
            .split_first()
            .ok_or_else(|| invalid("categorical implicit no-absorb DoF row is absent"))?;
        if implicit.identity != CategoricalTopologyDofEffectIdentity::ImplicitNoAbsorb
            || implicit.label != "_cons"
            || implicit.categories != 1
            || implicit.redundant != 0
            || implicit.coefficients != 1
            || !implicit.redundancy_exact
            || implicit.nested
            || implicit.slope_source.is_some()
        {
            return Err(invalid(
                "categorical implicit no-absorb DoF row is inconsistent",
            ));
        }
        computed.initial_categories = 1;
        computed.absorbed_rank = 1;
        rows = remaining;
    }

    let mut row_index = 0_usize;
    for (effect_index, spec) in specs.iter().enumerate() {
        let topology_source_index = *topology
            .effect_to_topology()
            .get(effect_index)
            .ok_or_else(|| invalid("categorical absorbed-DoF topology map is out of range"))?;
        let source = topology.registry().source(topology_source_index)?;
        if spec.intercept {
            let row = rows
                .get(row_index)
                .ok_or_else(|| invalid("categorical absorbed-DoF intercept row is absent"))?;
            row_index = row_index
                .checked_add(1)
                .ok_or_else(|| allocation("categorical absorbed-DoF row index overflows"))?;
            let identity_matches = matches!(
                &row.identity,
                CategoricalTopologyDofEffectIdentity::Explicit { term_id, sources }
                    if *term_id == spec.term_id && sources == &spec.sources
            );
            if !identity_matches
                || row.label != spec.label
                || row.categories != source.n_levels()
                || row.redundant > row.categories
                || row.coefficients != row.categories - row.redundant
                || row.slope_source.is_some()
                || !categorical_nesting_row_is_valid(nesting_state, row)
            {
                return Err(invalid(
                    "categorical absorbed-DoF explicit row is inconsistent",
                ));
            }
            computed.include(
                row.categories,
                row.redundant,
                row.coefficients,
                row.nested,
                row.redundancy_exact,
            )?;
        }

        for slope in &spec.slopes {
            let slope_row = rows
                .get(row_index)
                .ok_or_else(|| invalid("categorical absorbed-DoF slope row is absent"))?;
            row_index = row_index
                .checked_add(1)
                .ok_or_else(|| allocation("categorical absorbed-DoF row index overflows"))?;
            let slope_identity_matches = matches!(
                &slope_row.identity,
                CategoricalTopologyDofEffectIdentity::Explicit { term_id, sources }
                    if *term_id == spec.term_id && sources == &spec.sources
            );
            if !slope_identity_matches
                || slope_row.label != slope.label
                || slope_row.categories != source.n_levels()
                || slope_row.redundant > slope_row.categories
                || (expected_policy != PreparedOlsAbsorbedDofPolicy::Continuous
                    && slope_row.redundant != 0)
                || slope_row.coefficients != slope_row.categories - slope_row.redundant
                || slope_row.redundancy_exact
                || slope_row.nested
                || slope_row.slope_source != Some(slope.source)
            {
                return Err(invalid(
                    "categorical absorbed-DoF slope row is inconsistent",
                ));
            }
            computed.include(
                slope_row.categories,
                slope_row.redundant,
                slope_row.coefficients,
                slope_row.nested,
                slope_row.redundancy_exact,
            )?;
        }
    }
    if row_index != rows.len() {
        return Err(invalid(
            "categorical absorbed-DoF report contains trailing rows",
        ));
    }
    if computed.nested_categories != report.nested_categories {
        return Err(invalid(
            "categorical absorbed-DoF nested category total is inconsistent",
        ));
    }
    let reported = summary(
        report.initial_categories,
        report.redundant_categories,
        report.absorbed_rank,
        report.nested_categories,
        report.is_approximate,
    );
    validate_totals(reported, computed)?;
    Ok(reported)
}

fn categorical_nesting_row_is_valid(
    nesting_state: NestingState,
    row: &crate::topology_dof::CategoricalTopologyDofEffect,
) -> bool {
    match (nesting_state, row.nested) {
        (NestingState::Iid, false) | (NestingState::ClusterAware, false) => true,
        (NestingState::Iid, true) => false,
        (NestingState::ClusterAware, true) => {
            row.redundant == row.categories && row.coefficients == 0 && row.redundancy_exact
        }
    }
}

fn validate_totals(
    reported: PreparedOlsAbsorbedDofSummary,
    computed: PreparedOlsAbsorbedDofSummary,
) -> Result<()> {
    if reported.initial_categories != computed.initial_categories
        || reported.redundant_categories != computed.redundant_categories
        || reported.absorbed_rank != computed.absorbed_rank
        || reported.is_approximate != computed.is_approximate
        || reported.redundant_categories > reported.initial_categories
        || reported.absorbed_rank != reported.initial_categories - reported.redundant_categories
    {
        return Err(invalid("absorbed-DoF report totals are inconsistent"));
    }
    Ok(())
}

const fn summary(
    initial_categories: usize,
    redundant_categories: usize,
    absorbed_rank: usize,
    nested_categories: usize,
    is_approximate: bool,
) -> PreparedOlsAbsorbedDofSummary {
    PreparedOlsAbsorbedDofSummary {
        initial_categories,
        redundant_categories,
        absorbed_rank,
        nested_categories,
        is_approximate,
    }
}

pub(super) const fn categorical_policy(
    policy: PreparedOlsAbsorbedDofPolicy,
) -> CategoricalTopologyDofPolicy {
    policy
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AbsorbTermId, AbsorbedTerm, CompiledTermGraph, LoadedNumericColumn, LoadedOlsInput,
        OlsInputBinding, OlsInputRole, OlsRequestV1, SourceEntry, SourceId,
    };

    #[test]
    fn continuous_slope_report_accepts_redundancy_but_rejects_malformed_reports() {
        let g = SourceId::new(1).unwrap();
        let z = SourceId::new(2).unwrap();
        let request = OlsRequestV1 {
            model: CompiledTermGraph {
                sources: vec![
                    SourceEntry {
                        id: g,
                        canonical_name: "g".into(),
                    },
                    SourceEntry {
                        id: z,
                        canonical_name: "z".into(),
                    },
                ],
                terms: vec![],
            },
            absorbed_terms: vec![AbsorbedTerm {
                id: AbsorbTermId::new(1).unwrap(),
                intercept: true,
                categorical_sources: vec![g],
                slope_sources: vec![z],
                label: "g##c.z".into(),
                saved_name: String::new(),
            }],
            bindings: [
                OlsInputRole::Candidate,
                OlsInputRole::Outcome,
                OlsInputRole::NumericSource(g),
                OlsInputRole::NumericSource(z),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, role)| OlsInputBinding {
                stata_var_position: i as u32 + 1,
                role,
            })
            .collect(),
            include_intercept: true,
            keep_singletons: true,
        };
        let column = |values: &[f64]| LoadedNumericColumn {
            values: values.to_vec(),
            missing: vec![false; values.len()],
        };
        let loaded = LoadedOlsInput {
            candidate: vec![true; 6],
            outcome: column(&[1., 2., 3., 4., 5., 6.]),
            numeric_sources: vec![
                (g, column(&[0., 0., 0., 1., 1., 1.])),
                (z, column(&[1., 1., 1., 1., 2., 3.])),
            ],
            analytic_weight: None,
            frequency_weight: None,
            probability_weight: None,
        };
        let prepared = crate::prepare_model(&request, &[], &loaded, true).unwrap();
        let policy = PreparedOlsAbsorbedDofPolicy::Continuous;
        let report = prepared.inference_compute_absorbed_dof(policy).unwrap();
        // Group zero's constant slope duplicates its intercept: four declared
        // dummy/slope columns span exactly three independent directions.
        assert_eq!(report.effects[1].redundant, 1);
        let summary = prepared
            .inference_absorbed_dof_summary(&report, policy)
            .unwrap();
        assert_eq!(
            (
                summary.initial_categories,
                summary.redundant_categories,
                summary.absorbed_rank
            ),
            (4, 1, 3)
        );
        for corruption in 0..7 {
            let mut bad = report.clone();
            let mut expected_policy = policy;
            match corruption {
                0 => bad.effects[1].redundant = 3,
                1 => bad.effects[1].coefficients = 2,
                2 => bad.redundant_categories = 0,
                3 => {
                    bad.policy = CategoricalTopologyDofPolicy::Pairwise;
                    expected_policy = PreparedOlsAbsorbedDofPolicy::Pairwise;
                }
                4 => bad.effects[1].slope_source = Some(g),
                5 => bad.effects[1].nested = true,
                _ => bad.effects[1].redundancy_exact = true,
            }
            assert!(prepared
                .inference_absorbed_dof_summary(&bad, expected_policy)
                .is_err());
        }
    }
}
