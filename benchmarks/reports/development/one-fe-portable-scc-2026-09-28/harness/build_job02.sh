#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_center_validate
#$ -pe omp 4
#$ -l mem_per_core=3G,h_rt=00:30:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}" "${FEREG_DEV_SNAPSHOT:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260928-centered-inline-02 && "$NSLOTS" == 4 ]]
base=/projectnb/welfgr/fereg/dev/20260928-publication-target-01/source
mkdir "$RUN_ROOT/source"
rsync -rlt --exclude target --exclude dist --exclude .git "$base/" "$RUN_ROOT/source/"
python3 - "$RUN_ROOT" <<'PY'
from pathlib import Path
import json,hashlib,shutil,sys
r=Path(sys.argv[1]);m=json.loads((r/'staged/candidate-source.json').read_text())
for f,h in json.loads((r/'staged/deployment.json').read_text()).items():assert hashlib.sha256((r/'staged'/f).read_bytes()).hexdigest()==h,f
for f in m['overlay_files']:
 d=r/'source'/f;d.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(r/'staged/overlay'/f,d)
for f,h in m['files'].items():assert hashlib.sha256((r/'source'/f).read_bytes()).hexdigest()==h,f
print('VERIFIED_SOURCE_FILES',len(m['files']))
PY
module purge
module load stata-mp/19 python3/3.12.4
export CARGO_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo
export RUSTUP_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/rustup
export RUSTUP_BIN="$CARGO_HOME/bin/rustup" RUST_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
export RUSTUP_TOOLCHAIN="$RUST_TOOLCHAIN" CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=4
export CARGO_TARGET_DIR="$RUN_ROOT/work/target" PATH="$CARGO_HOME/bin:$PATH"
export RAYON_NUM_THREADS=4 RUSTFLAGS="" PYTHONDONTWRITEBYTECODE=1 OMP_NUM_THREADS=4 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
unset CARGO_ENCODED_RUSTFLAGS
mkdir -p "$RUN_ROOT/output" "$CARGO_TARGET_DIR"
cd "$RUN_ROOT/source"
python3 ci/run_fixture_profile.py --profile-config ci/stata_profiles.json \
 --profile scc-dev-ppml --repo-root "$RUN_ROOT/source" --run-root "$RUN_ROOT/output" \
 --summary-json "$RUN_ROOT/output/summary.json" --stata "$(command -v stata-mp)" --tested-sha "$FEREG_DEV_SNAPSHOT"
cargo test --locked -p fereg-core ppml::prepared_irls::tests -- --test-threads=4 > "$RUN_ROOT/output/irls-tests.log" 2>&1
cargo test --locked -p fereg-core ols::linear_session::tests -- --test-threads=4 > "$RUN_ROOT/output/linear-tests.log" 2>&1
cargo test --locked -p fereg-core implicit_centered -- --test-threads=4 > "$RUN_ROOT/output/centered-tests.log" 2>&1
cargo test --locked -p fereg-core one_fe_profile -- --test-threads=4 > "$RUN_ROOT/output/profile-tests.log" 2>&1
plugin_sha=$(sha256sum dist/fereg_linux.plugin | cut -d' ' -f1)
python3 scripts/run_stata_tests.py \
 --stata "$(command -v stata-mp)" --plugin "$RUN_ROOT/source/dist/fereg_linux.plugin" --plugin-sha256 "$plugin_sha" \
 --log-dir "$RUN_ROOT/output/oracles" --summary-json "$RUN_ROOT/output/oracles.json" \
 tests/stata/parity_ppmlhdfe/feppml_one_fe_passes_oracle.do \
 tests/stata/parity_ppmlhdfe/feppml_one_fe_profile_audit.do \
 tests/stata/development/feppml_one_fe_low_profile_audit.do \
 tests/stata/development/feppml_categorical_final_oracle.do \
 tests/stata/development/feppml_three_partition_slope_oracle.do \
 tests/stata/development/ols_multipartition_schur_oracle.do
python3 - "$RUN_ROOT" <<'PY'
from pathlib import Path
import json,hashlib,sys
r=Path(sys.argv[1])
for name,count in [('summary.json',1),('oracles.json',6)]:
 d=json.loads((r/'output'/name).read_text());assert d['status']=='success' and d['tests_failed']==0 and d['tests_passed']==count
m=json.loads((r/'staged/candidate-source.json').read_text())
for f,h in m['files'].items():assert hashlib.sha256((r/'source'/f).read_bytes()).hexdigest()==h,f
PY
printf 'FEREG_CENTERED_INLINE_VALIDATION_SUCCESS job=%s snapshot=%s\n' "$JOB_ID" "$FEREG_DEV_SNAPSHOT"
