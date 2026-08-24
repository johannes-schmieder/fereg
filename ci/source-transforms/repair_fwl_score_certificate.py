#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PATH = ROOT / "crates/stfixest-core/src/ols/prepared_fit.rs"
WORKFLOW = ROOT / ".github/workflows/stata-ci.yml"
SENTINEL = ROOT / ".github/workflows/repair-finite-score-fwl.yml"
SELF = Path(__file__).resolve()

text = PATH.read_text(encoding="utf-8")


def replace_once(old: str, new: str, label: str) -> None:
    global text
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    text = text.replace(old, new, 1)


replace_once(
    "#[derive(Debug)]\nstruct FiniteBreadDraft {\n",
    "#[derive(Debug, Clone, Copy, Default)]\n"
    "struct FiniteScoreDraft {\n"
    "    /// Largest finite-coordinate score after dividing out the within-X norm.\n"
    "    /// The final response scale is applied only after the returned residual is\n"
    "    /// independently certified against the FE span.\n"
    "    infinity_scale: f64,\n"
    "    square_sum_scale: f64,\n"
    "    moments: usize,\n"
    "}\n\n"
    "#[derive(Debug)]\nstruct FiniteBreadDraft {\n",
    "finite score struct",
)
replace_once(
    "    certificate: PreparedOlsRankCertificate,\n    bread: Option<FiniteBreadDraft>,\n}\n",
    "    certificate: PreparedOlsRankCertificate,\n"
    "    finite_score: FiniteScoreDraft,\n"
    "    bread: Option<FiniteBreadDraft>,\n}\n",
    "QrFit finite score field",
)
replace_once(
    "        weights,\n"
    "        &reference_norms,\n"
    "        config,\n"
    "        reservation,\n"
    "        perturbations.finite_bread,\n"
    "    )?;\n"
    "    if perturbations.first_coefficient {\n"
    "        let coefficient = qr\n"
    "            .generated_coefficients\n"
    "            .iter_mut()\n"
    "            .flatten()\n"
    "            .next()\n"
    "            .ok_or_else(|| invalid_argument(\"injected perturbation requires a kept coefficient\"))?;\n"
    "        *coefficient += 0.25;\n"
    "    }\n",
    "        weights,\n"
    "        &reference_norms,\n"
    "        &score_norms,\n"
    "        config,\n"
    "        reservation,\n"
    "        perturbations.first_coefficient,\n"
    "        perturbations.finite_bread,\n"
    "    )?;\n",
    "QR call and perturbation",
)
replace_once(
    "        &score_norms,\n        &qr.generated_statuses,\n        within_outcome_norm,\n",
    "        &score_norms,\n        qr.finite_score,\n        &qr.generated_statuses,\n        within_outcome_norm,\n",
    "score call finite draft",
)
replace_once(
    "    weights: NormalizedWeights<'_>,\n"
    "    reference_norms: &[f64],\n"
    "    config: PreparedOlsFitConfig,\n"
    "    reservation: &mut R,\n"
    "    perturb_bread: bool,\n"
    ") -> Result<QrFit> {\n",
    "    weights: NormalizedWeights<'_>,\n"
    "    reference_norms: &[f64],\n"
    "    score_norms: &[f64],\n"
    "    config: PreparedOlsFitConfig,\n"
    "    reservation: &mut R,\n"
    "    perturb_first_coefficient: bool,\n"
    "    perturb_bread: bool,\n"
    ") -> Result<QrFit> {\n",
    "QR signature",
)
replace_once(
    "    if within.ncols() != expected_columns || reference_norms.len() != generated_columns {\n",
    "    if within.ncols() != expected_columns\n"
    "        || reference_norms.len() != generated_columns\n"
    "        || score_norms.len() != generated_columns\n"
    "    {\n",
    "QR dimension validation",
)
replace_once(
    "    let standardized_gram_elements = checked_square(\n"
    "        candidate_count,\n"
    "        \"prepared OLS standardized Gram dimensions overflow usize\",\n"
    "    )?;\n",
    "    let mut standardized_rhs = reserved_filled(\n"
    "        reservation,\n"
    "        candidate_count,\n"
    "        0.0_f64,\n"
    "        \"unable to reserve prepared OLS standardized RHS cross-products\",\n"
    "    )?;\n"
    "    for (position, generated) in canonical_candidates.iter().copied().enumerate() {\n"
    "        let matrix_column = generated\n"
    "            .checked_add(1)\n"
    "            .ok_or_else(|| allocation(\"prepared OLS standardized RHS column overflows\"))?;\n"
    "        let mut sum = 0.0_f64;\n"
    "        let mut correction = 0.0_f64;\n"
    "        for row in 0..nrows {\n"
    "            compensated_add(\n"
    "                within[(row, matrix_column)] * within[(row, 0)],\n"
    "                &mut sum,\n"
    "                &mut correction,\n"
    "            )?;\n"
    "        }\n"
    "        standardized_rhs[position] = sum + correction;\n"
    "        if !standardized_rhs[position].is_finite() {\n"
    "            return Err(invalid_data(\n"
    "                \"prepared OLS standardized RHS cross-product is not finite\",\n"
    "            ));\n"
    "        }\n"
    "    }\n\n"
    "    let standardized_gram_elements = checked_square(\n"
    "        candidate_count,\n"
    "        \"prepared OLS standardized Gram dimensions overflow usize\",\n"
    "    )?;\n",
    "standardized RHS cross-products",
)
replace_once(
    "    for &column in &candidate_columns[rank..] {\n"
    "        generated_statuses[column] = PreparedOlsFitStatus::NumericallyRankDeficient;\n"
    "    }\n"
    "    for (column, status) in generated_statuses.iter().copied().enumerate() {\n",
    "    for &column in &candidate_columns[rank..] {\n"
    "        generated_statuses[column] = PreparedOlsFitStatus::NumericallyRankDeficient;\n"
    "    }\n"
    "    if perturb_first_coefficient {\n"
    "        let coefficient = generated_coefficients\n"
    "            .iter_mut()\n"
    "            .flatten()\n"
    "            .next()\n"
    "            .ok_or_else(|| invalid_argument(\"injected perturbation requires a kept coefficient\"))?;\n"
    "        *coefficient += 0.25;\n"
    "    }\n"
    "    for (column, status) in generated_statuses.iter().copied().enumerate() {\n",
    "move coefficient perturbation",
)
replace_once(
    "    let bread = build_finite_bread_draft(\n",
    "    let finite_score = build_finite_score_draft(\n"
    "        &canonical_candidates,\n"
    "        &standardized_gram,\n"
    "        &standardized_rhs,\n"
    "        &generated_coefficients,\n"
    "        &generated_statuses,\n"
    "        reference_norms,\n"
    "        score_norms,\n"
    "    )?;\n"
    "    let bread = build_finite_bread_draft(\n",
    "finite score draft build",
)
replace_once(
    "        certificate: PreparedOlsRankCertificate {\n"
    "            relative_threshold,\n"
    "            maximum_dropped_trailing_norm,\n"
    "            condition_proxy,\n"
    "            passed: true,\n"
    "        },\n"
    "        bread: Some(bread),\n"
    "    })\n"
    "}\n\n"
    "fn build_finite_bread_draft",
    "        certificate: PreparedOlsRankCertificate {\n"
    "            relative_threshold,\n"
    "            maximum_dropped_trailing_norm,\n"
    "            condition_proxy,\n"
    "            passed: true,\n"
    "        },\n"
    "        finite_score,\n"
    "        bread: Some(bread),\n"
    "    })\n"
    "}\n\n"
    "fn build_finite_score_draft(\n"
    "    canonical_candidates: &[usize],\n"
    "    standardized_gram: &[f64],\n"
    "    standardized_rhs: &[f64],\n"
    "    generated_coefficients: &[Option<f64>],\n"
    "    generated_statuses: &[PreparedOlsFitStatus],\n"
    "    reference_norms: &[f64],\n"
    "    score_norms: &[f64],\n"
    ") -> Result<FiniteScoreDraft> {\n"
    "    let candidate_count = canonical_candidates.len();\n"
    "    let candidate_square = checked_square(\n"
    "        candidate_count,\n"
    "        \"prepared OLS finite-score Gram dimensions overflow usize\",\n"
    "    )?;\n"
    "    if standardized_gram.len() != candidate_square\n"
    "        || standardized_rhs.len() != candidate_count\n"
    "        || generated_coefficients.len() != generated_statuses.len()\n"
    "        || reference_norms.len() != generated_statuses.len()\n"
    "        || score_norms.len() != generated_statuses.len()\n"
    "    {\n"
    "        return Err(invalid_argument(\n"
    "            \"prepared OLS finite-score dimensions are inconsistent\",\n"
    "        ));\n"
    "    }\n\n"
    "    let mut draft = FiniteScoreDraft::default();\n"
    "    for (left, generated) in canonical_candidates.iter().copied().enumerate() {\n"
    "        let status = generated_statuses[generated];\n"
    "        if !matches!(\n"
    "            status,\n"
    "            PreparedOlsFitStatus::Kept | PreparedOlsFitStatus::NumericallyRankDeficient\n"
    "        ) {\n"
    "            return Err(invalid_argument(\n"
    "                \"prepared OLS finite-score candidate has a non-finite status\",\n"
    "            ));\n"
    "        }\n"
    "        let reference_norm = reference_norms[generated];\n"
    "        let score_norm = score_norms[generated];\n"
    "        if !reference_norm.is_finite()\n"
    "            || reference_norm <= 0.0\n"
    "            || !score_norm.is_finite()\n"
    "            || score_norm <= 0.0\n"
    "        {\n"
    "            return Err(invalid_data(\n"
    "                \"prepared OLS finite-score regressor scale is not finite and positive\",\n"
    "            ));\n"
    "        }\n\n"
    "        let mut fitted = 0.0_f64;\n"
    "        let mut correction = 0.0_f64;\n"
    "        for (right, other_generated) in canonical_candidates.iter().copied().enumerate() {\n"
    "            if let Some(coefficient) = generated_coefficients[other_generated] {\n"
    "                let standardized_coefficient = coefficient * reference_norms[other_generated];\n"
    "                if !standardized_coefficient.is_finite() {\n"
    "                    return Err(invalid_data(\n"
    "                        \"prepared OLS standardized coefficient is not finite\",\n"
    "                    ));\n"
    "                }\n"
    "                compensated_add(\n"
    "                    standardized_gram[left * candidate_count + right]\n"
    "                        * standardized_coefficient,\n"
    "                    &mut fitted,\n"
    "                    &mut correction,\n"
    "                )?;\n"
    "            }\n"
    "        }\n"
    "        let standardized_score = (standardized_rhs[left] - (fitted + correction)).abs();\n"
    "        let scale = standardized_score * reference_norm / score_norm;\n"
    "        if !scale.is_finite() {\n"
    "            return Err(invalid_data(\n"
    "                \"prepared OLS finite-score normal-equation scale is not finite\",\n"
    "            ));\n"
    "        }\n"
    "        draft.infinity_scale = draft.infinity_scale.max(scale);\n"
    "        draft.square_sum_scale = scale.mul_add(scale, draft.square_sum_scale);\n"
    "        if !draft.square_sum_scale.is_finite() {\n"
    "            return Err(invalid_data(\n"
    "                \"prepared OLS finite-score two-norm scale is not finite\",\n"
    "            ));\n"
    "        }\n"
    "        draft.moments = draft\n"
    "            .moments\n"
    "            .checked_add(1)\n"
    "            .ok_or_else(|| allocation(\"prepared OLS finite-score moment count overflows\"))?;\n"
    "    }\n"
    "    Ok(draft)\n"
    "}\n\n"
    "fn build_finite_bread_draft",
    "finite score function",
)
replace_once(
    "    score_norms: &[f64],\n    statuses: &[PreparedOlsFitStatus],\n",
    "    score_norms: &[f64],\n    finite_score: FiniteScoreDraft,\n    statuses: &[PreparedOlsFitStatus],\n",
    "certify_scores signature",
)
replace_once(
    "    let mut relative_infinity_norm = 0.0_f64;\n"
    "    let mut relative_square_sum = 0.0_f64;\n"
    "    let mut moments = 0_usize;\n",
    "    let mut relative_infinity_norm = finite_score.infinity_scale / response_scale;\n"
    "    let response_square = response_scale * response_scale;\n"
    "    let mut relative_square_sum = finite_score.square_sum_scale / response_square;\n"
    "    let mut moments = finite_score.moments;\n"
    "    if !relative_infinity_norm.is_finite() || !relative_square_sum.is_finite() {\n"
    "        return Err(invalid_data(\n"
    "            \"prepared OLS finite-score response normalization is not finite\",\n"
    "        ));\n"
    "    }\n",
    "certify_scores initialization",
)
replace_once(
    "        if status == PreparedOlsFitStatus::CollinearWithFixedEffects {\n"
    "            continue;\n"
    "        }\n"
    "        let denominator = if status == PreparedOlsFitStatus::NormalizedAbsorbedIntercept {\n",
    "        if matches!(\n"
    "            status,\n"
    "            PreparedOlsFitStatus::Kept\n"
    "                | PreparedOlsFitStatus::NumericallyRankDeficient\n"
    "                | PreparedOlsFitStatus::CollinearWithFixedEffects\n"
    "        ) {\n"
    "            continue;\n"
    "        }\n"
    "        let denominator = if status == PreparedOlsFitStatus::NormalizedAbsorbedIntercept {\n",
    "skip finite scores already certified",
)
replace_once(
    "            &[reference_norm * 1e-14],\n"
    "            &[PreparedOlsFitStatus::NormalizedAbsorbedIntercept],\n",
    "            &[reference_norm * 1e-14],\n"
    "            FiniteScoreDraft::default(),\n"
    "            &[PreparedOlsFitStatus::NormalizedAbsorbedIntercept],\n",
    "direct score test finite draft",
)

PATH.write_text(text, encoding="utf-8")

workflow_text = WORKFLOW.read_text(encoding="utf-8")
marker = "\n  # BEGIN ONE-SHOT FWL SCORE REPAIR\n"
if workflow_text.count(marker) != 1:
    raise SystemExit("one-shot FWL workflow marker is not unique")
WORKFLOW.write_text(workflow_text.split(marker, 1)[0] + "\n", encoding="utf-8")
if SENTINEL.exists():
    SENTINEL.unlink()
SELF.unlink()
print("applied FWL-space finite score certificate repair")
