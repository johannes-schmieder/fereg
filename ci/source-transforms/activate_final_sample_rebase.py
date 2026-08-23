#!/usr/bin/env python3
"""Activate final-sample categorical model rebasing and its report plan."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LIB = ROOT / "crates/stfixest-core/src/lib.rs"
DATA = ROOT / "crates/stfixest-core/src/ols/prepared_categorical_data.rs"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


lib = LIB.read_text(encoding="utf-8")
lib = replace_once(
    lib,
    '#[path = "ols/prepared_categorical_effect.rs"]\nmod prepared_categorical_effect;\n',
    '#[path = "ols/prepared_categorical_effect.rs"]\nmod prepared_categorical_effect;\n'
    '#[path = "ols/prepared_categorical_rebase.rs"]\nmod prepared_categorical_rebase;\n',
    "categorical rebase module declaration",
)
lib = replace_once(
    lib,
    'pub use prepared_categorical_data::{\n'
    '    prepare_categorical_ols_data, PreparedCategoricalModelSource, PreparedCategoricalOlsData,\n'
    '    PreparedCategoricalOlsRhs,\n'
    '};\n',
    'pub use prepared_categorical_data::{\n'
    '    prepare_categorical_ols_data, PreparedCategoricalModelSource, PreparedCategoricalOlsData,\n'
    '    PreparedCategoricalOlsRhs,\n'
    '};\n'
    'pub use prepared_categorical_rebase::PreparedCategoricalOlsReportPlan;\n',
    "categorical report-plan export",
)
LIB.write_text(lib, encoding="utf-8")

data = DATA.read_text(encoding="utf-8")
data = replace_once(
    data,
    'use crate::metadata::SourceId;\n',
    'use crate::metadata::SourceId;\n'
    'use crate::prepared_categorical_rebase::{\n'
    '    rebase_categorical_model, PreparedCategoricalOlsReportPlan,\n'
    '};\n',
    "categorical rebase imports",
)
data = replace_once(
    data,
    '    singletons: PreparedOlsSingletons,\n}\n',
    '    singletons: PreparedOlsSingletons,\n'
    '    report_plan: PreparedCategoricalOlsReportPlan,\n'
    '}\n',
    "categorical report-plan field",
)
data = replace_once(
    data,
    '    pub const fn singletons(&self) -> PreparedOlsSingletons {\n'
    '        self.singletons\n'
    '    }\n\n'
    '    /// Evaluate one generated ordinary model column on the final compact row\n',
    '    pub const fn singletons(&self) -> PreparedOlsSingletons {\n'
    '        self.singletons\n'
    '    }\n\n'
    '    #[must_use]\n'
    '    pub const fn report_plan(&self) -> &PreparedCategoricalOlsReportPlan {\n'
    '        &self.report_plan\n'
    '    }\n\n'
    '    /// Evaluate one generated ordinary model column on the final compact row\n',
    "categorical report-plan getter",
)
data = replace_once(
    data,
    '        validate_model_recipes(&self.model, &self.categorical_sources, &self.fe_topology)?;\n'
    '        Ok(())\n',
    '        validate_model_recipes(&self.model, &self.categorical_sources, &self.fe_topology)?;\n'
    '        validate_report_plan(&self.model, &self.report_plan)?;\n'
    '        Ok(())\n',
    "categorical report-plan validation call",
)
data = replace_once(
    data,
    '    fn resolve_level(\n',
    '    pub(crate) fn resolve_level(\n',
    "categorical level resolver visibility",
)
data = replace_once(
    data,
    '    let PreparedOlsData {\n'
    '        mut model,\n'
    '        sample,\n'
    '        categorical_sources: candidate_sources,\n'
    '        ..\n'
    '    } = availability;\n'
    '    let final_sources = final_categorical_sources(&model, cluster_sources)?;\n',
    '    let PreparedOlsData {\n'
    '        model: availability_model,\n'
    '        sample,\n'
    '        categorical_sources: candidate_sources,\n'
    '        ..\n'
    '    } = availability;\n'
    '    let final_sources = final_categorical_sources(&availability_model, cluster_sources)?;\n',
    "categorical availability model binding",
)
data = replace_once(
    data,
    '    let categorical_sources = build_final_model_sources(model_source_plans, &sample, &fe_topology)?;\n'
    '    remap_and_resolve_model(&mut model, &categorical_sources, &fe_topology)?;\n'
    '    let effect_specs = effect_specs(&bound_effects)?;\n'
    '    let prepared = PreparedCategoricalOlsData {\n'
    '        model,\n',
    '    let categorical_sources = build_final_model_sources(model_source_plans, &sample, &fe_topology)?;\n'
    '    let rebased = rebase_categorical_model(\n'
    '        &request.model,\n'
    '        &availability_model,\n'
    '        &categorical_sources,\n'
    '        &fe_topology,\n'
    '        request.include_intercept,\n'
    '    )?;\n'
    '    let effect_specs = effect_specs(&bound_effects)?;\n'
    '    let prepared = PreparedCategoricalOlsData {\n'
    '        model: rebased.model,\n',
    "categorical final model rebasing",
)
data = replace_once(
    data,
    '        singletons: PreparedOlsSingletons {\n'
    '            kept_by_request: plan.requested_keep_singletons(),\n'
    '            num_singletons,\n'
    '            cascade_events,\n'
    '        },\n'
    '    };\n',
    '        singletons: PreparedOlsSingletons {\n'
    '            kept_by_request: plan.requested_keep_singletons(),\n'
    '            num_singletons,\n'
    '            cascade_events,\n'
    '        },\n'
    '        report_plan: rebased.report_plan,\n'
    '    };\n',
    "categorical report-plan construction",
)
pattern = r"\nfn remap_and_resolve_model\(.*?\nfn effect_specs\("
replacement = "\nfn effect_specs("
data, count = re.subn(pattern, replacement, data, count=1, flags=re.S)
if count != 1:
    raise SystemExit(f"obsolete categorical model remap removal: expected one match, found {count}")
report_validation = '''fn validate_report_plan(
    model: &PreparedOlsModel,
    report_plan: &PreparedCategoricalOlsReportPlan,
) -> Result<()> {
    if report_plan.public_coefficient_count() != model.coefficients.len()
        || report_plan.original_slots().windows(2).any(|pair| {
            pair[0].get() >= pair[1].get()
        })
        || report_plan.original_slots().last().is_some_and(|slot| {
            slot.get() as usize > report_plan.original_coefficient_count()
        })
    {
        return Err(invalid(
            "categorical OLS final-sample report plan is inconsistent",
        ));
    }
    for (index, coefficient) in model.coefficients.iter().enumerate() {
        if coefficient.coefficient_slot.get() as usize != index + 1 {
            return Err(invalid(
                "categorical OLS rebased coefficient slots are not contiguous",
            ));
        }
    }
    Ok(())
}

'''
data = replace_once(
    data,
    'fn validate_numeric_sample(sample: &PreparedNumericSample) -> Result<()> {\n',
    report_validation + 'fn validate_numeric_sample(sample: &PreparedNumericSample) -> Result<()> {\n',
    "categorical report-plan validator",
)
DATA.write_text(data, encoding="utf-8")
print("activated final-sample categorical model rebasing")
