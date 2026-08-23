#!/usr/bin/env python3
"""Repair the staged clustered-inference transform for the current DoF layout."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TARGET = ROOT / "ci/source-transforms/activate_categorical_clustered_inference.py"
text = TARGET.read_text(encoding="utf-8")
start = text.index(
    'clustered = regex_once(\n    clustered,\n    r"fn build_clustered_dof'
)
end = text.index(
    '\nclustered = replace_once(\n    clustered,\n    "fn apply_psd_policy',
    start,
)
replacement = r'''clustered = regex_once(
    clustered,
    r"fn build_clustered_dof\(.*?\n}\n\n(?=fn cluster_correction)",
    r'''fn build_clustered_dof<P: PreparedOlsClusteredView + ?Sized>(
    prepared: &P,
    weights: PreparedOlsReferenceWeightReport,
    base: PreparedOlsResidualDofReport,
    absorbed: PreparedOlsAbsorbedDofSummary,
) -> Result<PreparedOlsClusteredDofReport> {
    let minimum_cluster_count = prepared
        .clustered_design()
        .dimensions()
        .iter()
        .map(|dimension| dimension.groups())
        .min()
        .ok_or_else(|| invalid_argument("prepared clustered design contains no dimensions"))?;
    if minimum_cluster_count < 2 {
        return Err(invalid_data(
            "prepared clustered inference requires at least two groups per dimension",
        ));
    }
    let unclustered_residual_dof = weights
        .effective_observations
        .checked_sub(absorbed.absorbed_rank)
        .and_then(|value| value.checked_sub(base.finite_model_dof))
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            invalid_data("prepared clustered unclustered residual DoF is not positive")
        })?;
    if unclustered_residual_dof != base.residual_dof {
        return Err(invalid_data(
            "prepared clustered base residual-DoF identity did not pass",
        ));
    }
    let fit_residual_dof = unclustered_residual_dof
        .checked_sub(absorbed.nested_categories)
        .filter(|value| *value > 0)
        .ok_or_else(|| invalid_data("prepared clustered fit-statistic DoF is not positive"))?;
    let reported_residual_dof = unclustered_residual_dof.min(minimum_cluster_count - 1);
    let nested_adjustment = usize::from(absorbed.nested_categories > 0);
    let correction_denominator_dof = weights
        .effective_observations
        .checked_sub(nested_adjustment)
        .and_then(|value| value.checked_sub(base.finite_model_dof))
        .and_then(|value| value.checked_sub(absorbed.absorbed_rank))
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            invalid_data("prepared clustered small-sample correction denominator is not positive")
        })?;
    Ok(PreparedOlsClusteredDofReport {
        finite_rank: base.finite_rank,
        finite_model_dof: base.finite_model_dof,
        absorbed_rank: absorbed.absorbed_rank,
        nested_absorbed_categories: absorbed.nested_categories,
        unclustered_residual_dof,
        reported_residual_dof,
        fit_residual_dof,
        correction_denominator_dof,
        minimum_cluster_count,
        has_structural_intercept: base.has_structural_intercept,
        has_normalized_absorbed_intercept: base.has_normalized_absorbed_intercept,
        absorbed_rank_is_approximate: absorbed.is_approximate,
    })
}

''',
    "generic clustered DoF builder",
)
'''
TARGET.write_text(text[:start] + replacement + text[end:], encoding="utf-8")
print("repaired staged categorical clustered-inference transform")
