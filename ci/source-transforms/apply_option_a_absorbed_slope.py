#!/usr/bin/env python3
# Apply Johannes's option-A absorbed-slope source correction.
#
# This transform is intentionally narrow:
# - preserve the guarded intercept-plus-one-slope preparation/MAP/topology work;
# - remove the superseded signed residual-DoF and negative-iid publication path;
# - return the frozen hidden projection to its last accepted 31/9 partition;
# - retain the old 24-row construction as an executable fail-closed transaction test;
# - leave the new 48-row positive-DoF fixture ready for the subsequent pinned
#   reference recapture/reseal gate.
#
# The script is idempotent only at the completed option-A preparation state and
# fails on unexpected source anchors.
from __future__ import annotations

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BASE = "f61fee923db2bd3544d457dfe50de8117d27adda"
OLD_SLOPE_FIXTURE = (
    "tests/stata/parity_reghdfe/estimate_ols_intercept_slope_parity.do"
)
FAILURE_FIXTURE = (
    "tests/stata/parity_reghdfe/"
    "estimate_ols_intercept_slope_nonpositive_dof_fail_closed.do"
)


def path(relative: str) -> Path:
    return ROOT / relative


def git_show(relative: str) -> str:
    completed = subprocess.run(
        ["git", "-C", str(ROOT), "show", f"{BASE}:{relative}"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"unable to read {relative} from {BASE}: {completed.stderr.strip()}"
        )
    return completed.stdout


def read(relative: str) -> str:
    return path(relative).read_text(encoding="utf-8")


def write(relative: str, content: str) -> None:
    destination = path(relative)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(content, encoding="utf-8")


def replace_once(relative: str, old: str, new: str) -> None:
    text = read(relative)
    count = text.count(old)
    if count != 1:
        raise RuntimeError(
            f"{relative}: expected one option-A anchor, observed {count}: {old[:100]!r}"
        )
    write(relative, text.replace(old, new, 1))


def append_section(relative: str, section: str) -> None:
    text = read(relative).rstrip() + "\n\n"
    write(relative, text + section.strip() + "\n")


# The signed inference object was the only post-f61 change in this file.
write(
    "crates/stfixest-core/src/ols/prepared_unadjusted.rs",
    git_show("crates/stfixest-core/src/ols/prepared_unadjusted.rs"),
)

# Remove signed-only public exports while retaining the slope topology API.
replace_once(
    "crates/stfixest-core/src/lib.rs",
    "    fit_prepared_categorical_ols_signed_unadjusted_map,\n",
    "",
)
replace_once(
    "crates/stfixest-core/src/lib.rs",
    "    CertifiedPreparedCategoricalOlsSignedUnadjusted, ",
    "",
)
replace_once(
    "crates/stfixest-core/src/lib.rs",
    (
        "    PreparedOlsRobustCovariance, PreparedOlsScoreCertificate, "
        "PreparedOlsSignedFitStatistics,\n"
        "    PreparedOlsSignedResidualDofReport, PreparedOlsSignedUnadjustedCertificate,\n"
        "    PreparedOlsSignedUnadjustedCovariance, PreparedOlsSingletons, "
        "PreparedOlsUnadjustedCertificate,\n"
    ),
    (
        "    PreparedOlsRobustCovariance, PreparedOlsScoreCertificate, "
        "PreparedOlsSingletons,\n"
        "    PreparedOlsUnadjustedCertificate,\n"
    ),
)
replace_once(
    "crates/stfixest-core/src/ols/mod.rs",
    (
        "pub use prepared_unadjusted::{\n"
        "    fit_prepared_categorical_ols_signed_unadjusted_map,\n"
        "    fit_prepared_categorical_ols_unadjusted_map, fit_prepared_ols_unadjusted_map,\n"
        "    CertifiedPreparedCategoricalOlsSignedUnadjusted, "
        "CertifiedPreparedCategoricalOlsUnadjusted,\n"
        "    CertifiedPreparedOlsUnadjusted, PreparedOlsFitStatistics, "
        "PreparedOlsReferenceWeightReport,\n"
        "    PreparedOlsResidualDofReport, PreparedOlsSignedFitStatistics,\n"
        "    PreparedOlsSignedResidualDofReport, PreparedOlsSignedUnadjustedCertificate,\n"
        "    PreparedOlsSignedUnadjustedCovariance, PreparedOlsUnadjustedCertificate,\n"
        "    PreparedOlsUnadjustedConfig, PreparedOlsUnadjustedCovariance,\n"
        "};\n"
    ),
    (
        "pub use prepared_unadjusted::{\n"
        "    fit_prepared_categorical_ols_unadjusted_map, "
        "fit_prepared_ols_unadjusted_map,\n"
        "    CertifiedPreparedCategoricalOlsUnadjusted, CertifiedPreparedOlsUnadjusted,\n"
        "    PreparedOlsFitStatistics, PreparedOlsReferenceWeightReport, "
        "PreparedOlsResidualDofReport,\n"
        "    PreparedOlsUnadjustedCertificate, PreparedOlsUnadjustedConfig, "
        "PreparedOlsUnadjustedCovariance,\n"
        "};\n"
    ),
)

# Route slope iid models through the ordinary positive-DoF composition.
replace_once(
    "crates/stfixest-plugin/src/estimate.rs",
    "    fit_prepared_categorical_ols_signed_unadjusted_map,\n",
    "",
)
replace_once(
    "crates/stfixest-plugin/src/estimate.rs",
    (
        "            if prepared.has_absorbed_slope() {\n"
        "                let inference = fit_prepared_categorical_ols_signed_unadjusted_map(\n"
        "                    &prepared,\n"
        "                    PreparedOlsUnadjustedConfig {\n"
        "                        absorbed_dof: action.absorbed_dof,\n"
        "                        ..PreparedOlsUnadjustedConfig::default()\n"
        "                    },\n"
        "                )?;\n"
        "                let shadow = build_shadow_result(\n"
        "                    PreparedOlsResultData::categorical(&prepared),\n"
        "                    None,\n"
        "                    CertifiedOlsInference::CategoricalSignedIid(&inference),\n"
        "                    action.outputs.residual.is_some(),\n"
        "                )?;\n"
        "                return precommit_and_store(&action, &shadow, spi);\n"
        "            }\n"
    ),
    "",
)
replace_once(
    "crates/stfixest-plugin/src/estimate.rs",
    "    validate_shadow_coefficients(shadow, action.vce == OlsVceKind::Iid)?;\n",
    "    validate_shadow_coefficients(shadow)?;\n",
)
replace_once(
    "crates/stfixest-plugin/src/estimate.rs",
    (
        "fn validate_shadow_coefficients(\n"
        "    shadow: &OlsShadowResult<'_>,\n"
        "    allow_signed_negative_dof: bool,\n"
        ") -> Result<()> {\n"
    ),
    "fn validate_shadow_coefficients(shadow: &OlsShadowResult<'_>) -> Result<()> {\n",
)
replace_once(
    "crates/stfixest-plugin/src/estimate.rs",
    (
        "        let signed_negative_dof =\n"
        "            allow_signed_negative_dof && shadow.dof[18] == 1.0 "
        "&& shadow.dof[19] < 0.0;\n"
        "        if shadow.covariance[row * dimension + row] < 0.0 "
        "&& !signed_negative_dof {\n"
        "            return Err(invalid(\"OLS covariance shadow has a negative diagonal\"));\n"
        "        }\n"
    ),
    (
        "        if shadow.covariance[row * dimension + row] < 0.0 {\n"
        "            return Err(invalid(\"OLS covariance shadow has a negative diagonal\"));\n"
        "        }\n"
    ),
)

# Restore the ordinary result adapter, then reapply only the slope-row identity.
result_path = "crates/stfixest-plugin/src/result.rs"
result = git_show(result_path)
old_identity = (
    "                    CategoricalTopologyDofEffectIdentity::Explicit { term_id, sources } => {\n"
    "                        match sources.as_slice() {\n"
    "                            [] => {\n"
    "                                return Err(invalid(\n"
    "                                    \"categorical absorbed effect has no source identity\",\n"
    "                                ));\n"
    "                            }\n"
    "                            [source] => (1.0, f64::from(term_id.get()), "
    "f64::from(source.get())),\n"
    "                            _ => (3.0, f64::from(term_id.get()), 0.0),\n"
    "                        }\n"
    "                    }\n"
    "                    CategoricalTopologyDofEffectIdentity::ImplicitNoAbsorb => "
    "(2.0, 0.0, 0.0),\n"
)
new_identity = (
    "                    CategoricalTopologyDofEffectIdentity::Explicit { term_id, sources } => {\n"
    "                        if let Some(slope_source) = effect.slope_source {\n"
    "                            if sources.len() != 1 {\n"
    "                                return Err(invalid(\n"
    "                                    \"categorical absorbed slope has a non-singleton "
    "source tuple\",\n"
    "                                ));\n"
    "                            }\n"
    "                            (4.0, f64::from(term_id.get()), "
    "f64::from(slope_source.get()))\n"
    "                        } else {\n"
    "                            match sources.as_slice() {\n"
    "                                [] => {\n"
    "                                    return Err(invalid(\n"
    "                                        \"categorical absorbed effect has no source "
    "identity\",\n"
    "                                    ));\n"
    "                                }\n"
    "                                [source] => {\n"
    "                                    (1.0, f64::from(term_id.get()), "
    "f64::from(source.get()))\n"
    "                                }\n"
    "                                _ => (3.0, f64::from(term_id.get()), 0.0),\n"
    "                            }\n"
    "                        }\n"
    "                    }\n"
    "                    CategoricalTopologyDofEffectIdentity::ImplicitNoAbsorb => {\n"
    "                        if effect.slope_source.is_some() {\n"
    "                            return Err(invalid(\n"
    "                                \"implicit categorical absorbed effect has a slope source\",\n"
    "                            ));\n"
    "                        }\n"
    "                        (2.0, 0.0, 0.0)\n"
    "                    }\n"
)
if result.count(old_identity) != 1:
    raise RuntimeError("result.rs: categorical identity anchor changed")
write(result_path, result.replace(old_identity, new_identity, 1))

# Positive df_r is already posted by ereturn post; remove the signed override.
replace_once(
    "stata/_stfixest_estimate_ols_test.ado",
    (
        "    // Stata's ereturn post deliberately leaves e(df_r) undefined for a\n"
        "    // negative dof() argument. The pinned absorbed-slope convention is signed,\n"
        "    // so publish the already native-certified scalar explicitly.\n"
        "    ereturn scalar df_r = `df_r'\n"
    ),
    "",
)

# Restore the last accepted 31/9 hidden projection and its external receipt binding.
for relative in (
    "benchmarks/manifests/ols_hidden_numeric_v1.json",
    "benchmarks/manifests/ols_hidden_numeric_candidate_receipt_v1.json",
    "scripts/ols_hidden_numeric.py",
    "tests/python/test_ols_hidden_numeric.py",
):
    write(relative, git_show(relative))

# The old 24-row design is now a failure/transaction oracle, not parity success.
old_path = path(OLD_SLOPE_FIXTURE)
if old_path.exists():
    old_path.unlink()
write(
    FAILURE_FIXTURE,
    r'''version 18.0
clear all
set more off

quietly findfile stfixest.ado
local ado_path = subinstr(`"`r(fn)'"', "\", "/", .)
local suffix "/stata/stfixest.ado"
assert substr(`"`ado_path'"', -strlen(`"`suffix'"'), .) == `"`suffix'"'
local root = substr(`"`ado_path'"', 1, strlen(`"`ado_path'"') - strlen(`"`suffix'"'))

* Option A keeps this original 24-row over-absorbed construction solely as a
* fail-closed oracle. The pinned comparator reports conservative df_a=26 with
* N=24; StFixest must reject nonpositive residual DoF before result publication.
import delimited using `"`root'/tests/fixtures/ols/ols_core.csv"', clear

quietly regress y
matrix __stfx_prior_b = e(b)
generate byte __stfx_prior_sample = e(sample)
capture confirm variable __stfx_option_a_residual
assert _rc == 111

capture noisily _stfixest_ols_hidden_numeric_v1 y c.x1, ///
    absorb(firm##c.time worker) residual(__stfx_option_a_residual)
assert _rc == 2001
assert `"`e(cmd)'"' == "regress"
matrix __stfx_after_b = e(b)
assert mreldif(__stfx_prior_b, __stfx_after_b) == 0
assert e(sample) == __stfx_prior_sample
capture confirm variable __stfx_option_a_residual
assert _rc == 111

capture noisily stfeols y c.x1, absorb(firm##c.time worker)
assert _rc == 498

di as result "END_FIXTURE|estimate_ols_intercept_slope_nonpositive_dof_fail_closed|rc=0"
''',
)

# Keep the fail-closed fixture in every qualification lane that formerly ran
# the signed success fixture.
profiles_path = path("ci/stata_profiles.json")
profiles = json.loads(profiles_path.read_text(encoding="utf-8"))
for profile in profiles["profiles"].values():
    fixtures = profile.get("fixtures", [])
    profile["fixtures"] = [
        FAILURE_FIXTURE if item == OLD_SLOPE_FIXTURE else item for item in fixtures
    ]
profiles_path.write_text(json.dumps(profiles, indent=2) + "\n", encoding="utf-8")

qualifier = "scripts/development/qualify_stfeols_full_baseline.py"
if path(qualifier).exists():
    text = read(qualifier)
    if OLD_SLOPE_FIXTURE in text:
        write(qualifier, text.replace(OLD_SLOPE_FIXTURE, FAILURE_FIXTURE))

# Independent positive-DoF and negative-DoF mathematical oracle.
write(
    "crates/stfixest-testkit/tests/intercept_slope_oracle.rs",
    r'''use stfixest_core::{
    fit_prepared_categorical_ols_unadjusted_map, prepare_categorical_ols_data, AbsorbTermId,
    AbsorbedTerm, CoefficientSlot, CompiledTermGraph, ComponentKind, ComponentOrder, ErrorCode,
    ExpandedTerm, LevelState, LoadedNumericColumn, LoadedOlsInput, MetadataFlag, OlsInputBinding,
    OlsInputRole, OlsRequestV1, PreparedOlsFitStatus, PreparedOlsUnadjustedConfig, RawTermKind,
    SourceEntry, SourceId, TermComponent, TermId, TermOrder, TimeSeriesState,
};
use stfixest_testkit::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights,
};

const TOLERANCE: f64 = 2e-8;

#[test]
fn positive_dof_intercept_slope_matches_explicit_projection_oracle() {
    let (base_y, base_x1, base_firm, base_time, base_worker) = base_case();
    let y = doubled(&base_y);
    let x1 = doubled(&base_x1);
    let firm = doubled(&base_firm);
    let time = doubled(&base_time);
    let worker = doubled(&base_worker);
    let prepared = prepare_case(&y, &x1, &firm, &time, &worker);
    let result = fit_prepared_categorical_ols_unadjusted_map(
        &prepared,
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect("certified positive-DoF intercept-slope fit");

    assert!(result.fit().absorption_report().certified());
    assert!(result.fit().residual_absorption_report().certified());
    assert!(result.fit().rank_certificate().passed);
    assert!(result.fit().score_certificate().passed);
    assert!(result.fit().bread().certificate().passed);
    assert!(result.certificate().passed);
    assert_eq!(result.fit().finite_rank(), 0);
    assert_eq!(
        result.fit().coefficients()[0].status,
        PreparedOlsFitStatus::CollinearWithFixedEffects
    );
    assert_eq!(
        result.fit().coefficients()[1].status,
        PreparedOlsFitStatus::NormalizedAbsorbedIntercept
    );
    assert_close(
        result.fit().coefficients()[1].value.expect("intercept"),
        6.479_166_666_666_667,
    );

    let absorbed = result.absorbed_dof();
    assert_eq!(absorbed.effects.len(), 3);
    assert_eq!(absorbed.initial_categories, 28);
    assert_eq!(absorbed.redundant_categories, 2);
    assert_eq!(absorbed.absorbed_rank, 26);
    assert_eq!(
        absorbed
            .effects
            .iter()
            .map(|row| (row.categories, row.redundant, row.coefficients))
            .collect::<Vec<_>>(),
        vec![(8, 0, 8), (8, 0, 8), (12, 2, 10)]
    );
    assert!(absorbed.effects[0].redundancy_exact);
    assert!(!absorbed.effects[1].redundancy_exact);
    assert!(absorbed.effects[2].redundancy_exact);
    assert!(absorbed.is_approximate);

    let dof = result.dof();
    assert_eq!(dof.model_rank, 26);
    assert_eq!(dof.residual_dof, 22);
    assert!(dof.absorbed_rank_is_approximate);

    let oracle = fit_explicit_ols(&ExplicitOlsProblem {
        outcome: y,
        regressors: vec![x1],
        effects: vec![
            DummyEffect::new(firm, 8, true, vec![time]).expect("firm intercept and slope"),
            DummyEffect::new(worker, 12, true, Vec::new()).expect("worker intercept"),
        ],
        weights: OracleWeights::Unit,
        clusters: Vec::new(),
    })
    .expect("independent explicit-dummy oracle");
    assert_eq!(oracle.explicit_dummy_columns, 28);
    assert_eq!(oracle.explicit_dummy_rank, 22);
    assert_eq!(oracle.finite_regressor_rank, 0);
    assert_eq!(oracle.model_rank, 22);
    assert_eq!(oracle.residual_degrees_of_freedom, 26);
    assert!(oracle.coefficients[0].is_none());
    assert_close(oracle.weighted_rss, 0.04);
    assert_close(result.statistics().rss, oracle.weighted_rss);
    assert_slice_close(result.fit().residuals(), &oracle.residuals);
    assert_eq!(result.covariance().values().len(), 1);
    assert_close(
        result.covariance().values()[0],
        oracle.weighted_rss / 22.0 / 48.0,
    );
    assert!(result.covariance().values()[0] >= 0.0);
}

#[test]
fn original_overabsorbed_case_fails_before_iid_publication() {
    let (y, x1, firm, time, worker) = base_case();
    let prepared = prepare_case(&y, &x1, &firm, &time, &worker);
    let error = fit_prepared_categorical_ols_unadjusted_map(
        &prepared,
        PreparedOlsUnadjustedConfig::default(),
    )
    .expect_err("nonpositive residual DoF must fail closed");
    assert_eq!(error.code, ErrorCode::InsufficientObservations);
    assert!(error.message.contains("positive residual degrees of freedom"));
}

fn base_case() -> (Vec<f64>, Vec<f64>, Vec<u32>, Vec<f64>, Vec<u32>) {
    (
        vec![
            4.20, 4.95, 5.10, 5.65, 5.90, 6.55, 6.70, 7.35, 7.60, 8.05, 8.50, 8.85, 3.80,
            4.45, 4.90, 5.25, 5.80, 6.15, 6.60, 6.95, 7.40, 7.85, 8.20, 8.75,
        ],
        vec![
            -1.50, -1.10, -0.80, -0.50, -0.20, 0.10, 0.40, 0.70, 1.00, 1.30, 1.60, 1.90,
            -1.40, -1.00, -0.70, -0.40, -0.10, 0.20, 0.50, 0.80, 1.10, 1.40, 1.70, 2.00,
        ],
        vec![
            0_u32, 1, 0, 1, 1, 2, 2, 3, 3, 0, 0, 1, 4, 5, 4, 5, 5, 6, 6, 7, 7, 4, 4, 5,
        ],
        vec![
            0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0,
            3.0, 0.0, 1.0, 2.0, 3.0, 0.0, 1.0, 2.0, 3.0,
        ],
        vec![
            0_u32, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11,
            11,
        ],
    )
}

fn doubled<T: Clone>(values: &[T]) -> Vec<T> {
    values.iter().cloned().chain(values.iter().cloned()).collect()
}

fn prepare_case(
    y: &[f64],
    x1: &[f64],
    firm: &[u32],
    time: &[f64],
    worker: &[u32],
) -> stfixest_core::PreparedCategoricalOlsData {
    let sources = ["x1", "firm", "time", "worker"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| SourceEntry {
            id: source(index),
            canonical_name: name.to_owned(),
        })
        .collect();
    let model = CompiledTermGraph {
        sources,
        terms: vec![ExpandedTerm {
            id: TermId::new(1).expect("term"),
            order: TermOrder::new(1).expect("order"),
            coefficient_slot: CoefficientSlot::new(1).expect("slot"),
            raw_kind: RawTermKind::Variable,
            whole_column_omitted: false,
            canonical_token: "c.x1".to_owned(),
            components: vec![TermComponent {
                order: ComponentOrder::new(1).expect("component"),
                source: source(0),
                kind: ComponentKind::Continuous,
                level_state: LevelState::Absent,
                level_value: 0.0,
                base: MetadataFlag::False,
                omitted: MetadataFlag::Unavailable,
                raw_operator: "c".to_owned(),
                time_series_state: TimeSeriesState::None,
                raw_time_series_operator: String::new(),
            }],
        }],
    };
    let bindings = vec![
        OlsInputBinding {
            stata_var_position: 1,
            role: OlsInputRole::Candidate,
        },
        OlsInputBinding {
            stata_var_position: 2,
            role: OlsInputRole::Outcome,
        },
        OlsInputBinding {
            stata_var_position: 3,
            role: OlsInputRole::NumericSource(source(0)),
        },
        OlsInputBinding {
            stata_var_position: 4,
            role: OlsInputRole::NumericSource(source(1)),
        },
        OlsInputBinding {
            stata_var_position: 5,
            role: OlsInputRole::NumericSource(source(2)),
        },
        OlsInputBinding {
            stata_var_position: 6,
            role: OlsInputRole::NumericSource(source(3)),
        },
    ];
    let request = OlsRequestV1 {
        model,
        absorbed_terms: vec![
            AbsorbedTerm {
                id: AbsorbTermId::new(1).expect("firm term"),
                intercept: true,
                categorical_sources: vec![source(1)],
                slope_sources: vec![source(2)],
                label: "firm##c.time".to_owned(),
                saved_name: String::new(),
            },
            AbsorbedTerm {
                id: AbsorbTermId::new(2).expect("worker term"),
                intercept: true,
                categorical_sources: vec![source(3)],
                slope_sources: Vec::new(),
                label: "worker".to_owned(),
                saved_name: String::new(),
            },
        ],
        bindings,
        include_intercept: true,
        keep_singletons: false,
    };
    let loaded = LoadedOlsInput {
        candidate: vec![true; y.len()],
        outcome: column(y.iter().copied()),
        numeric_sources: vec![
            (source(0), column(x1.iter().copied())),
            (source(1), column(firm.iter().copied().map(f64::from))),
            (source(2), column(time.iter().copied())),
            (source(3), column(worker.iter().copied().map(f64::from))),
        ],
        analytic_weight: None,
        frequency_weight: None,
        probability_weight: None,
    };
    prepare_categorical_ols_data(&request, &loaded).expect("prepared intercept-slope case")
}

fn source(index: usize) -> SourceId {
    SourceId::new(u32::try_from(index + 1).expect("small source")).expect("source")
}

fn column(values: impl Iterator<Item = f64>) -> LoadedNumericColumn {
    let values = values.collect::<Vec<_>>();
    LoadedNumericColumn {
        missing: vec![false; values.len()],
        values,
    }
}

fn assert_close(actual: f64, expected: f64) {
    let scale = actual.abs().max(expected.abs()).max(1.0);
    assert!(
        (actual - expected).abs() <= TOLERANCE * scale,
        "actual={actual:.17e}, expected={expected:.17e}"
    );
}

fn assert_slice_close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (&actual, &expected) in actual.iter().zip(expected) {
        assert_close(actual, expected);
    }
}
''',
)

# Update the Python contract to prohibit the signed path and register the
# nonpositive-DoF licensed transaction test.
write(
    "tests/python/test_categorical_hidden_transport_contract.py",
    r'''from __future__ import annotations

import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PLUGIN = ROOT / "crates/stfixest-plugin/src"
CORE = ROOT / "crates/stfixest-core/src"
STATA = ROOT / "stata/_stfixest_estimate_ols_test.ado"
PROFILES = ROOT / "ci/stata_profiles.json"
FAILURE_FIXTURE = (
    "tests/stata/parity_reghdfe/"
    "estimate_ols_intercept_slope_nonpositive_dof_fail_closed.do"
)


class CategoricalHiddenTransportContractTests(unittest.TestCase):
    def test_hidden_dispatch_uses_only_positive_dof_inference_families(self):
        estimate = (PLUGIN / "estimate.rs").read_text(encoding="utf-8")
        result = (PLUGIN / "result.rs").read_text(encoding="utf-8")
        core = "\n".join(
            item.read_text(encoding="utf-8")
            for item in CORE.rglob("*.rs")
        )
        for symbol in (
            "prepare_categorical_ols_data",
            "prepare_categorical_ols_clustered_data",
            "fit_prepared_categorical_ols_unadjusted_map",
            "fit_prepared_categorical_ols_robust_map",
            "fit_prepared_categorical_ols_clustered_map",
            "CategoricalIid",
            "CategoricalRobust",
            "CategoricalClustered",
        ):
            self.assertIn(symbol, estimate)
        for forbidden in (
            "fit_prepared_categorical_ols_signed_unadjusted_map",
            "CategoricalSignedIid",
            "PreparedOlsSignedResidualDofReport",
            "PreparedOlsSignedUnadjustedCertificate",
        ):
            self.assertNotIn(forbidden, estimate + result + core)
        self.assertIn("categorical_sources.len() > 1", estimate)
        self.assertIn("plan_categorical_absorbed_request", estimate)
        self.assertNotIn('ACTION_ESTIMATE_OLS: &str', estimate)

    def test_result_wire_retains_slope_identity_without_signed_dof(self):
        result = (PLUGIN / "result.rs").read_text(encoding="utf-8")
        view = (PLUGIN / "result_view.rs").read_text(encoding="utf-8")
        self.assertIn("const FE_COLUMNS: usize = 8", result)
        self.assertIn("const RECEIPT_COLUMNS: usize = 16", result)
        self.assertIn("(4.0, f64::from(term_id.get())", result)
        self.assertIn("effect.slope_source", result)
        self.assertIn("Categorical(&'a PreparedCategoricalOlsData)", view)
        self.assertNotIn("unsafe {", result + view)

    def test_native_fe_shadow_validator_accepts_only_structured_slopes(self):
        estimate = (PLUGIN / "estimate.rs").read_text(encoding="utf-8")
        self.assertIn("validate_shadow_fixed_effect_row(row)?", estimate)
        self.assertIn("3 => term > 0 && source == 0", estimate)
        self.assertIn("4 => term > 0 && source > 0", estimate)
        self.assertIn("coefficients != categories - redundant", estimate)
        self.assertNotIn("allow_signed_negative_dof", estimate)

    def test_stata_gate_allows_one_guarded_intercept_slope_channel(self):
        stata = STATA.read_text(encoding="utf-8")
        self.assertIn("`absorb_terms'[`row',2] < 1", stata)
        self.assertIn("`absorb_terms'[`row',3] != 1", stata)
        self.assertIn(
            "hidden prepared OLS supports categorical intercept effects and one guarded slope",
            stata,
        )
        self.assertIn("if `n_absorb_slopes' > 1", stata)
        self.assertIn("absorbed slope is qualified only for iid inference", stata)
        self.assertIn("absorbed slope is not yet qualified for weights", stata)
        self.assertIn("absorbed slope currently requires pairwise absorbed DoF", stata)
        self.assertNotIn("ereturn scalar df_r = `df_r'", stata)

    def test_nonpositive_dof_fixture_is_in_every_qualification_profile(self):
        profiles = json.loads(PROFILES.read_text(encoding="utf-8"))["profiles"]
        for name in ("quick", "differential", "full"):
            with self.subTest(profile=name):
                self.assertIn(FAILURE_FIXTURE, profiles[name]["fixtures"])
                self.assertNotIn(
                    "tests/stata/parity_reghdfe/estimate_ols_intercept_slope_parity.do",
                    profiles[name]["fixtures"],
                )

    def test_positive_dof_fixture_is_bound_independently(self):
        fixture_test = (
            ROOT / "tests/python/test_intercept_slope_positive_dof_fixture.py"
        ).read_text(encoding="utf-8")
        oracle = (
            ROOT / "crates/stfixest-testkit/tests/intercept_slope_oracle.rs"
        ).read_text(encoding="utf-8")
        self.assertIn("ols_intercept_slope_positive_dof.csv", fixture_test)
        self.assertIn("residual_dof, 22", oracle)
        self.assertIn("ErrorCode::InsufficientObservations", oracle)


if __name__ == "__main__":
    unittest.main()
''',
)

# Restore pre-signed documentation and add concise option-A slope notes.
for relative in (
    "docs/compatibility_matrix.md",
    "docs/data_plane.md",
    "docs/memory_model.md",
    "docs/ols_core.md",
    "docs/plugin_protocol.md",
    "docs/scientific_contracts.md",
    "tests/stata/parity_reghdfe/README.md",
):
    write(relative, git_show(relative))

append_section(
    "docs/ols_core.md",
    '''
## Guarded intercept-plus-one-slope preparation under option A

The hidden categorical preparation path may bind one continuous slope source to
one single-source categorical intercept effect and residualize it through the
allocation-bounded certified slope MAP implementation. This engineering does
not create a signed residual-DoF inference family. Ordinary iid composition
continues to require `df_r > 0` and nonnegative covariance. The original 24-row
over-absorbed construction is a fail-closed oracle; the dedicated 48-row
positive-DoF fixture awaits pinned reference recapture before the frozen
`intercept_and_slope` case can return to the executable projection.
''',
)
append_section(
    "docs/compatibility_matrix.md",
    '''
## Option-A absorbed-slope checkpoint

Guarded intercept-plus-one-slope preparation and certified MAP are present, but
the frozen semantic case remains blocked while its dedicated 48-row
positive-residual-DoF fixture is recaptured and resealed. The old 24-row design
is retained only as an insufficient-observations/transaction-atomicity test.
No signed residual-DoF or negative-iid-covariance success path is supported.
''',
)
append_section(
    "docs/plugin_protocol.md",
    '''
## Option-A slope inference boundary

The hidden wire may carry one guarded absorbed slope, but it may publish a
result only through the ordinary positive-residual-DoF inference object.
Nonpositive residual DoF aborts before the receipt's `COMPLETE` cell and before
any logical Stata transaction commit.
''',
)
append_section(
    "docs/scientific_contracts.md",
    '''
## Absorbed-slope residual DoF

Johannes selected option A: covariance and completed OLS results require
strictly positive residual DoF. The 24-row over-absorbed slope construction is
a failure oracle. A separate 48-row fixture preserves the command shape and FE
support for the pending pinned-reference recapture.
''',
)
append_section(
    "docs/memory_model.md",
    '''
## Guarded slope MAP memory

The intercept-plus-one-slope MAP path borrows final-sample codes and the
continuous slope source, reserves all workspaces before mutation, and adds no
signed-inference storage. The subsequent ordinary iid composition retains the
existing fallible O(K) and O(K^2) allocation bounds.
''',
)
append_section(
    "docs/data_plane.md",
    '''
## Option-A slope transport

One guarded intercept-plus-slope request can reach the hidden prepared path.
Only an ordinary positive-DoF certified result may reach private shadows.
Nonpositive residual DoF leaves the completion receipt false and exposes no
requested output.
''',
)
append_section(
    "tests/stata/parity_reghdfe/README.md",
    f'''
`{FAILURE_FIXTURE}` preserves the original 24-row absorbed-slope construction
only as an option-A fail-closed transaction test. The new 48-row fixture is not
a parity fixture until the exact pinned `reghdfe` reference is recaptured and
the case is resealed.
''',
)

# Final static guards.
for relative in (
    "crates/stfixest-core/src/lib.rs",
    "crates/stfixest-core/src/ols/mod.rs",
    "crates/stfixest-core/src/ols/prepared_unadjusted.rs",
    "crates/stfixest-plugin/src/estimate.rs",
    "crates/stfixest-plugin/src/result.rs",
    "crates/stfixest-testkit/tests/intercept_slope_oracle.rs",
):
    text = read(relative)
    for forbidden in (
        "fit_prepared_categorical_ols_signed_unadjusted_map",
        "CategoricalSignedIid",
        "PreparedOlsSignedResidualDofReport",
        "PreparedOlsSignedUnadjustedCertificate",
    ):
        if forbidden in text:
            raise RuntimeError(f"{relative}: superseded signed symbol remains: {forbidden}")

projection = json.loads(read("benchmarks/manifests/ols_hidden_numeric_v1.json"))
if "intercept_and_slope" in projection["executable_case_ids"]:
    raise RuntimeError("intercept_and_slope remains executable before positive-DoF reseal")
if projection["blockers"][0] != {
    "case_id": "intercept_and_slope",
    "reason": "absorbed_intercept_slope",
}:
    raise RuntimeError("last accepted 31/9 projection was not restored")

print("option-A source correction prepared")
