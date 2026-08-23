#!/usr/bin/env python3
"""Repair categorical iid inference imports after the shared-view refactor."""

from pathlib import Path

root = Path(__file__).resolve().parents[2]
path = root / "crates/stfixest-core/src/ols/prepared_unadjusted.rs"
text = path.read_text(encoding="utf-8")

unused = "use crate::prepared_fit_view::PreparedOlsFitView;\n"
if text.count(unused) != 1:
    raise SystemExit(
        f"expected one obsolete PreparedOlsFitView import, found {text.count(unused)}"
    )
text = text.replace(unused, "", 1)

anchor = (
    "use super::prepared_inference_view::{PreparedOlsAbsorbedDofSummary, "
    "PreparedOlsInferenceView};\n"
)
if text.count(anchor) != 1:
    raise SystemExit(
        f"expected one prepared inference-view import, found {text.count(anchor)}"
    )
text = text.replace(
    anchor,
    anchor + "\n#[cfg(test)]\nuse super::compute_prepared_ols_absorbed_dof;\n",
    1,
)

path.write_text(text, encoding="utf-8")
print("repaired categorical iid inference imports")
