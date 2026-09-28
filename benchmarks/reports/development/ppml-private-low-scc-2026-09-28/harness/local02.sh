#!/bin/bash
set -euo pipefail
# Portable reproduction; the executed local driver digest is in mac-validation01.json.
export PATH="${FEREG_TOOLCHAIN_BIN:?Rust 1.85.1 toolchain bin directory}:$PATH"
export CARGO_TARGET_DIR="$PWD/target/implicit-private02-20260928" CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=4 PYTHONDONTWRITEBYTECODE=1
export FEREG_GIT_COMMIT=c09eeab353a8
export FEREG_TEST_AUDIT_THREADS=4
mkdir -p "$CARGO_TARGET_DIR/evidence"
for suite in ppml::prepared_irls::tests ols::linear_session:: finite_wls_kernel::tests implicit_centered one_fe_profile; do
 cargo test --locked -p fereg-core "$suite" -- --test-threads=1 > "$CARGO_TARGET_DIR/evidence/$suite.log" 2>&1
done
cargo test --locked -p fereg-plugin ppml_execution::tests -- --test-threads=1 > "$CARGO_TARGET_DIR/evidence/execution-tests.log" 2>&1
./scripts/build_macos.sh > "$CARGO_TARGET_DIR/evidence/build.log" 2>&1
plugin_sha=$(shasum -a 256 dist/fereg_macos.plugin | cut -d' ' -f1)
python3 scripts/run_stata_tests.py --stata "${STATA_BIN:?licensed Stata executable}" \
 --plugin "$PWD/dist/fereg_macos.plugin" --plugin-sha256 "$plugin_sha" \
 --log-dir "$CARGO_TARGET_DIR/evidence/oracles" --summary-json "$CARGO_TARGET_DIR/evidence/oracles.json" \
 tests/stata/parity_ppmlhdfe/feppml_one_fe_passes_oracle.do \
 tests/stata/parity_ppmlhdfe/feppml_one_fe_profile_audit.do \
 tests/stata/development/feppml_one_fe_low_profile_audit.do \
 tests/stata/development/feppml_categorical_final_oracle.do \
 tests/stata/development/feppml_three_partition_slope_oracle.do \
 tests/stata/development/ols_multipartition_schur_oracle.do > "$CARGO_TARGET_DIR/evidence/oracles.log" 2>&1
printf 'FEREG_PRIVATE_LOW_MAC_SUCCESS\n'
