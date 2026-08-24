//! Shared absorbed-DoF boundary for certified prepared OLS inference.
//!
//! The legacy single-source prepared path and the categorical-interaction path
//! retain different public report identities.  Numerical inference consumes a
//! validated common summary, while each result continues to retain its exact
//! source-bound absorbed-DoF report.

use crate::error::{ErrorCode, Result, StFixestError};
use crate::prepared_categorical_data::PreparedCategoricalOlsData;
use crate::prepared_fit_view::PreparedOlsFitView;
use crate::topology_dof::{
    CategoricalTopologyDofEffectIdentity, CategoricalTopologyDofPolicy,
    CategoricalTopologyDofReport,
};

use super::prepared_dof::compute_prepared_ols_absorbed_dof_with_nesting;
use super::{
    compute_prepared_ols_absorbed_dof, PreparedOlsAbsorbedDofEffect,
    PreparedOlsAbsorbedDofEffectIdentity, PreparedOlsAbsorbedDofPolicy,
    PreparedOlsAbsorbedDofReport, PreparedOlsData,
};

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
enum LegacyNestingState {
    Iid,
    ClusterAware,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CategoricalNestingState {
    Iid,
    ClusterAware,
}

pub(crate) trait PreparedOlsInferenceView: PreparedOlsFitView {
    type AbsorbedDofReport;

    fn inference_compute_absorbed_dof(
        &self,
        policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<Self::AbsorbedDofReport>;

    fn inference_absorbed_dof_summary(
        &self,
        report: &Self::AbsorbedDofReport,
        expected_policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<PreparedOlsAbsorbedDofSummary>;

    fn inference_compute_clustered_absorbed_dof(
        &self,
        policy: PreparedOlsAbsorbedDofPolicy,
        cluster_dimensions: &[&[u32]],
    ) -> Result<Self::AbsorbedDofReport>;

    fn inference_clustered_absorbed_dof_summary(
        &self,
        report: &Self::AbsorbedDofReport,
        expected_policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<PreparedOlsAbsorbedDofSummary>;

    fn inference_has_explicit_absorbed_effects(&self) -> bool;
}

impl PreparedOlsInferenceView for PreparedOlsData {
    type AbsorbedDofReport = PreparedOlsAbsorbedDofReport;

    fn inference_compute_absorbed_dof(
        &self,
        policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<Self::AbsorbedDofReport> {
        compute_prepared_ols_absorbed_dof(self, policy)
    }

    fn inference_absorbed_dof_summary(
        &self,
        report: &Self::AbsorbedDofReport,
        expected_policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<PreparedOlsAbsorbedDofSummary> {
        validate_legacy_report(self, report, expected_policy, LegacyNestingState::Iid)
    }

    fn inference_compute_clustered_absorbed_dof(
        &self,
        policy: PreparedOlsAbsorbedDofPolicy,
        cluster_dimensions: &[&[u32]],
    ) -> Result<Self::AbsorbedDofReport> {
        compute_prepared_ols_absorbed_dof_with_nesting(self, policy, cluster_dimensions)
    }

    fn inference_clustered_absorbed_dof_summary(
        &self,
        report: &Self::AbsorbedDofReport,
        expected_policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<PreparedOlsAbsorbedDofSummary> {
        validate_legacy_report(
            self,
            report,
            expected_policy,
            LegacyNestingState::ClusterAware,
        )
    }

    fn inference_has_explicit_absorbed_effects(&self) -> bool {
        !self.fe_topology.effects().is_empty()
    }
}

impl PreparedOlsInferenceView for PreparedCategoricalOlsData {
    type AbsorbedDofReport = CategoricalTopologyDofReport;

    fn inference_compute_absorbed_dof(
        &self,
        policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<Self::AbsorbedDofReport> {
        self.absorbed_dof(categorical_policy(policy), &[])
    }

    fn inference_absorbed_dof_summary(
        &self,
        report: &Self::AbsorbedDofReport,
        expected_policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<PreparedOlsAbsorbedDofSummary> {
        validate_categorical_report(self, report, expected_policy, CategoricalNestingState::Iid)
    }

    fn inference_compute_clustered_absorbed_dof(
        &self,
        policy: PreparedOlsAbsorbedDofPolicy,
        cluster_dimensions: &[&[u32]],
    ) -> Result<Self::AbsorbedDofReport> {
        self.absorbed_dof(categorical_policy(policy), cluster_dimensions)
    }

    fn inference_clustered_absorbed_dof_summary(
        &self,
        report: &Self::AbsorbedDofReport,
        expected_policy: PreparedOlsAbsorbedDofPolicy,
    ) -> Result<PreparedOlsAbsorbedDofSummary> {
        validate_categorical_report(
            self,
            report,
            expected_policy,
            CategoricalNestingState::ClusterAware,
        )
    }

    fn inference_has_explicit_absorbed_effects(&self) -> bool {
        !self.effect_specs().is_empty()
    }
}

fn validate_legacy_report(
    prepared: &PreparedOlsData,
    report: &PreparedOlsAbsorbedDofReport,
    expected_policy: PreparedOlsAbsorbedDofPolicy,
    nesting_state: LegacyNestingState,
) -> Result<PreparedOlsAbsorbedDofSummary> {
    if report.policy != expected_policy {
        return Err(invalid(
            "prepared absorbed-DoF report policy differs from the requested policy",
        ));
    }
    let implicit_no_absorb = prepared.fit_structural_intercept()
        && prepared.fe_topology.effects().is_empty()
        && prepared.fit_sample().n_active() > 0;
    let expected_rows = prepared
        .fe_topology
        .effects()
        .len()
        .checked_add(usize::from(implicit_no_absorb))
        .ok_or_else(|| allocation("prepared absorbed-DoF row count overflows"))?;
    if report.effects.len() != expected_rows
        || (nesting_state == LegacyNestingState::Iid && report.nested_categories != 0)
    {
        return Err(invalid(
            "prepared absorbed-DoF report dimensions or iid nesting state are inconsistent",
        ));
    }

    let mut initial = 0_usize;
    let mut redundant = 0_usize;
    let mut rank = 0_usize;
    let mut nested_categories = 0_usize;
    let mut approximate = false;
    let mut rows = report.effects.as_slice();
    if implicit_no_absorb {
        let (implicit, remaining) = rows
            .split_first()
            .ok_or_else(|| invalid("prepared implicit no-absorb DoF row is absent"))?;
        if implicit.identity != PreparedOlsAbsorbedDofEffectIdentity::ImplicitNoAbsorb
            || implicit.label != "_cons"
            || implicit.categories != 1
            || implicit.redundant != 0
            || implicit.coefficients != 1
            || !implicit.redundancy_exact
            || implicit.nested
        {
            return Err(invalid(
                "prepared implicit no-absorb DoF row is inconsistent",
            ));
        }
        initial = 1;
        rank = 1;
        rows = remaining;
    }

    for (effect, row) in prepared.fe_topology.effects().iter().zip(rows) {
        let source = prepared
            .fe_topology
            .sources()
            .get(effect.topology_source_index)
            .ok_or_else(|| invalid("prepared absorbed-DoF FE source is out of range"))?;
        if row.identity
            != (PreparedOlsAbsorbedDofEffectIdentity::Explicit {
                term_id: effect.term_id,
                source: effect.source,
            })
            || row.label != effect.label
            || row.categories != source.n_levels()
            || row.redundant > row.categories
            || row.coefficients != row.categories - row.redundant
            || !legacy_nesting_row_is_valid(nesting_state, row)
        {
            return Err(invalid(
                "prepared absorbed-DoF explicit row is inconsistent",
            ));
        }
        initial = initial
            .checked_add(row.categories)
            .ok_or_else(|| allocation("prepared absorbed category total overflows"))?;
        redundant = redundant
            .checked_add(row.redundant)
            .ok_or_else(|| allocation("prepared absorbed redundancy total overflows"))?;
        rank = rank
            .checked_add(row.coefficients)
            .ok_or_else(|| allocation("prepared absorbed rank total overflows"))?;
        if row.nested {
            nested_categories = nested_categories
                .checked_add(row.categories)
                .ok_or_else(|| allocation("prepared absorbed nested category total overflows"))?;
        }
        approximate |= !row.redundancy_exact;
    }
    if nested_categories != report.nested_categories {
        return Err(invalid(
            "prepared absorbed-DoF nested category total is inconsistent",
        ));
    }
    validate_totals(
        report.initial_categories,
        report.redundant_categories,
        report.absorbed_rank,
        report.is_approximate,
        initial,
        redundant,
        rank,
        approximate,
    )?;
    Ok(summary(
        report.initial_categories,
        report.redundant_categories,
        report.absorbed_rank,
        report.nested_categories,
        report.is_approximate,
    ))
}

fn legacy_nesting_row_is_valid(
    nesting_state: LegacyNestingState,
    row: &PreparedOlsAbsorbedDofEffect,
) -> bool {
    match (nesting_state, row.nested) {
        (LegacyNestingState::Iid, false) | (LegacyNestingState::ClusterAware, false) => true,
        (LegacyNestingState::Iid, true) => false,
        (LegacyNestingState::ClusterAware, true) => {
            row.redundant == row.categories && row.coefficients == 0 && row.redundancy_exact
        }
    }
}

fn validate_categorical_report(
    prepared: &PreparedCategoricalOlsData,
    report: &CategoricalTopologyDofReport,
    expected_policy: PreparedOlsAbsorbedDofPolicy,
    nesting_state: CategoricalNestingState,
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
            .checked_add(1 + usize::from(spec.slope.is_some()))
            .ok_or_else(|| allocation("categorical absorbed-DoF extended row count overflows"))
    })?;
    let expected_rows = explicit_rows
        .checked_add(usize::from(implicit_no_absorb))
        .ok_or_else(|| allocation("categorical absorbed-DoF row count overflows"))?;
    if report.effects.len() != expected_rows
        || topology.effect_to_topology().len() != specs.len()
        || (nesting_state == CategoricalNestingState::Iid && report.nested_categories != 0)
    {
        return Err(invalid(
            "categorical absorbed-DoF report dimensions or iid nesting state are inconsistent",
        ));
    }

    let mut initial = 0_usize;
    let mut redundant = 0_usize;
    let mut rank = 0_usize;
    let mut nested_categories = 0_usize;
    let mut approximate = false;
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
        initial = 1;
        rank = 1;
        rows = remaining;
    }

    let mut row_index = 0_usize;
    for (effect_index, spec) in specs.iter().enumerate() {
        let row = rows
            .get(row_index)
            .ok_or_else(|| invalid("categorical absorbed-DoF intercept row is absent"))?;
        row_index = row_index
            .checked_add(1)
            .ok_or_else(|| allocation("categorical absorbed-DoF row index overflows"))?;
        let topology_source_index = *topology
            .effect_to_topology()
            .get(effect_index)
            .ok_or_else(|| invalid("categorical absorbed-DoF topology map is out of range"))?;
        let source = topology.registry().source(topology_source_index)?;
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
        initial = initial
            .checked_add(row.categories)
            .ok_or_else(|| allocation("categorical absorbed category total overflows"))?;
        redundant = redundant
            .checked_add(row.redundant)
            .ok_or_else(|| allocation("categorical absorbed redundancy total overflows"))?;
        rank = rank
            .checked_add(row.coefficients)
            .ok_or_else(|| allocation("categorical absorbed rank total overflows"))?;
        if row.nested {
            nested_categories = nested_categories
                .checked_add(row.categories)
                .ok_or_else(|| {
                    allocation("categorical absorbed nested category total overflows")
                })?;
        }
        approximate |= !row.redundancy_exact;

        if let Some(slope) = &spec.slope {
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
                || slope_row.redundant != 0
                || slope_row.coefficients != slope_row.categories
                || slope_row.redundancy_exact
                || slope_row.nested
                || slope_row.slope_source != Some(slope.source)
            {
                return Err(invalid(
                    "categorical absorbed-DoF slope row is inconsistent",
                ));
            }
            initial = initial
                .checked_add(slope_row.categories)
                .ok_or_else(|| allocation("categorical absorbed-slope category total overflows"))?;
            rank = rank
                .checked_add(slope_row.coefficients)
                .ok_or_else(|| allocation("categorical absorbed-slope rank total overflows"))?;
            approximate |= !slope_row.redundancy_exact;
        }
    }
    if row_index != rows.len() {
        return Err(invalid(
            "categorical absorbed-DoF report contains trailing rows",
        ));
    }
    if nested_categories != report.nested_categories {
        return Err(invalid(
            "categorical absorbed-DoF nested category total is inconsistent",
        ));
    }
    validate_totals(
        report.initial_categories,
        report.redundant_categories,
        report.absorbed_rank,
        report.is_approximate,
        initial,
        redundant,
        rank,
        approximate,
    )?;
    Ok(summary(
        report.initial_categories,
        report.redundant_categories,
        report.absorbed_rank,
        report.nested_categories,
        report.is_approximate,
    ))
}

fn categorical_nesting_row_is_valid(
    nesting_state: CategoricalNestingState,
    row: &crate::topology_dof::CategoricalTopologyDofEffect,
) -> bool {
    match (nesting_state, row.nested) {
        (CategoricalNestingState::Iid, false) | (CategoricalNestingState::ClusterAware, false) => {
            true
        }
        (CategoricalNestingState::Iid, true) => false,
        (CategoricalNestingState::ClusterAware, true) => {
            row.redundant == row.categories && row.coefficients == 0 && row.redundancy_exact
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_totals(
    reported_initial: usize,
    reported_redundant: usize,
    reported_rank: usize,
    reported_approximate: bool,
    computed_initial: usize,
    computed_redundant: usize,
    computed_rank: usize,
    computed_approximate: bool,
) -> Result<()> {
    if reported_initial != computed_initial
        || reported_redundant != computed_redundant
        || reported_rank != computed_rank
        || reported_approximate != computed_approximate
        || reported_redundant > reported_initial
        || reported_rank != reported_initial - reported_redundant
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

const fn categorical_policy(policy: PreparedOlsAbsorbedDofPolicy) -> CategoricalTopologyDofPolicy {
    match policy {
        PreparedOlsAbsorbedDofPolicy::None => CategoricalTopologyDofPolicy::None,
        PreparedOlsAbsorbedDofPolicy::FirstPair => CategoricalTopologyDofPolicy::FirstPair,
        PreparedOlsAbsorbedDofPolicy::Pairwise => CategoricalTopologyDofPolicy::Pairwise,
    }
}

fn invalid(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::InvalidArgument, message)
}

fn allocation(message: impl Into<String>) -> StFixestError {
    StFixestError::new(ErrorCode::Allocation, message)
}
