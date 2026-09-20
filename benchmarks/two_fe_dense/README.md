# Local two-FE dense development comparisons

`run.py` reuses the systematic generator and the audited one-FE application
workers for two-effect OLS. `acceptance-20260920.json` freezes the primary
twenty-cell matrix, fresh-seed/order holdouts, regression controls and repeated
10% runtime-margin target. These are local Mac development checks.

Supply isolated source artifacts and matching `identity-VARIANT.json` files.
Set `STATA_MP_PATH`, `PATH` for R, and `R_LIBS_USER` to the pinned stock fixest
library plus its dependencies. For example:

```bash
python benchmarks/two_fe_dense/run.py ROOT --label screen \
  --variants baseline candidate fixest --cases iid --widths 10 50 --repeats 3
```

Generation and hashing precede timing. Arms rotate within each repetition and
each starts in a fresh process with a small warm-up. The timer includes requested
FE recovery, observation vectors and public predictions. Saved effects are
normalized separately within each connected component. Small `--oracle` runs
use explicit dummy matrices; all comparisons preserve the repository tolerances.

Focused weighted screens can use `--weight-kind aw|pw|fw` and
`--weight-design constant|moderate|dispersed`. Constant weights equal four;
moderate weights are independently seeded integers from one to five; dispersed
weights are independently seeded powers of ten from 1e-4 to 1e4. PW requires
robust or clustered inference. AW/PW reference fitting uses fixest's supplied
weights, and RSS and saved-effect normalization use the same reference scale.
FW comparisons are native-only: fixest's ordinary weights do not implement
literal frequency inference. The small oracle grounds one right-hand dummy per
component, checks weighted coefficients, covariance, RSS and requested outputs,
and separately checks FW coefficients against literal row expansion.

Smaller-width and weighted development screens are separate from the frozen
2026-09-20 acceptance matrix. Archive their labels and plans explicitly; do not
apply the old matrix's acceptance statement to these additional cases.

Default timings use stock reference tolerances. `--reference-precision` and
`--reference-tolerance` run separate accuracy diagnostics, and
`--reference-recovery-precision` changes only recovery. Their timings cannot
replace the headline default reference. After an observed reference-only output
disagreement, `--record-reference-output-disagreement` permits collection of
remaining calls while marking the series `reference_output_mismatch`; it never
labels that disagreement a scientific pass. Native comparisons still fail closed.

`archive.py ROOT OUTPUT LABEL...` collects sanitized timings, scientific hashes,
comparison results, phase traces and artifact identities, including failed calls.
It excludes licensed logs and machine-specific paths. Run it after timing ends.

`audit_outputs.py ROOT LABEL VARIANT` independently checks native FE constancy,
component normalization, reconstruction and normal equations. `accept.py ROOT
--candidate VARIANT` verifies both frozen final blocks, all regression cells,
the tight-reference and independent output receipts, fixtures, source identity
and reference fingerprints. A predeclared regression confirmation must retain
its initial failing measurement; the final campaign required none. The audit
refuses to overwrite an existing acceptance receipt.

`broad_milestone.json` defines the separate broad two-way milestone: 77 primary
cells and 111 native controls. `broad.py ROOT --artifacts ARTIFACT_ROOT
--candidate VARIANT --label LABEL --phase screen|block-a|block-b|controls`
expands the inventory and records an immutable execution plan. Screens use
one pair; each primary acceptance block uses five; controls use three.
A resumed label skips every recorded attempt, including failures. Rechecks
require a new label. Completing attempts does not establish acceptance.
`--dry-run` prints the exact plan without running applications.

Graph inputs include the systematic strong/weak designs plus disconnected
blocks, a bottleneck ring, a cycle, an unbalanced graph and a weighted bridge
control. Additional `group_concentrated` and `bridge_cut` weights are determined
from input identities; estimator output never selects graph memberships or
weights. `--memory low` and `--effects g1` provide explicit native regression
controls. One-effect saved-output comparisons export exactly one FE channel.

`--record-reference-science-disagreement` preserves a reference coefficient or
covariance disagreement while completing diagnostic calls; it is never a
scientific pass. `graph_oracle.py OUTPUT RESULT_JSON...` independently audits
full-sample IID graph results using grounded sparse dummy cross-products,
SuperLU, residual refinement and fresh weighted normal equations. It checks
all shared input column hashes and keeps each mismatch explicit. Its result
receipt records the implementation hash, numerical checks and direct-factor
size. The small dense-SVD oracle remains independently implemented.

For disconnected clustered cases with neither FE nested in a declared cluster,
`--reference-full-dof` explicitly requests fixest's exact full FE rank correction.
The harness checks nonnesting from the shared data before admitting this option.
This avoids the pinned reference's distinct nonnested/exact rank calculation;
the fitted objective and native correction are unchanged. Keep its source
adjudication with the relevant receipt. Zero-regressor CSVs retain their empty
schemas. FW small-oracle runs separately check literal replicated-row IID,
robust and clustered covariance, coefficients, RSS, rank and effective N.
