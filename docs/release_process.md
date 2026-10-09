# Packaging

Development and local qualification do not authorize native publication. See
[licensing](licensing.md), [validation](VALIDATION.md) and
[development checks](development_workflow.md). GitHub tests remain paused.

Build the platform plugin with `scripts/build_macos.sh`, `build_linux.sh` or
`build_windows.ps1`. Each build runs its native audit/load checks and writes a
checksum. Run scientific, transaction and installed-package checks for the actual
artifact before distributing a development package. Retain explicit platform gaps.

`scripts/stage_release.py` stages a deterministic archive with one plugin per
included platform, production Stata files, checksums, source identity, dependency
audit (receipt written by `scripts/run_dependency_audit.py`), SBOM and licenses. Default production staging requires all three platforms.
`--review-platforms macos` creates a visibly incomplete review archive; passing its
local checks does not confer publication eligibility. `run_staged_stata_tests.py
--review-only` admits only such explicitly requested review packages.

Source archives use `scripts/package_release.py`. Generated packages belong under
ignored `target/` or `release-artifacts/`; no raw captures, credentials, personal
paths, build caches or reference datasets belong in a commit. Never force-push or
rewrite history to reduce checkout size.
