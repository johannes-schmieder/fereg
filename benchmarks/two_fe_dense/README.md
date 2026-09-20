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
