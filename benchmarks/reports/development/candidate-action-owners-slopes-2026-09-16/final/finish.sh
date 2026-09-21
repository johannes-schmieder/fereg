#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_action_pairs
#$ -cwd
#$ -j y
set -euo pipefail
module load python3/3.12.4 stata-mp/19
export STATA_MP_PATH="$(command -v stata-mp)"
export CARGO_HOME=$TOOLCHAINS/cargo
export RUSTUP_HOME=$TOOLCHAINS/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$PWD/target-release"
export CARGO_BUILD_JOBS=4 RAYON_NUM_THREADS=4 CARGO_NET_OFFLINE=true
export FEREG_GIT_COMMIT=5240e58-action-coverage3 FEREG_BUILD_DATE=2026-09-16
export OMP_NUM_THREADS=4 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export PYTHONDONTWRITEBYTECODE=1
grep -qx ACTION_COVERAGE_NATIVE_PASS final-checks.log
cpus=$(python3 harness/select-cpus.py)
(cd source && taskset -c "$cpus" bash scripts/build_linux.sh) > build-recheck.txt 2>&1
python3 - <<'HASH'
from pathlib import Path
import hashlib,json
paths={'baseline':Path('baseline/dist/fereg_linux.plugin'),'candidate':Path('source/dist/fereg_linux.plugin')}
artifacts={a:hashlib.sha256(p.read_bytes()).hexdigest() for a,p in paths.items()}
assert artifacts['baseline']=='a1c53228f37a23d02ea1ac06fbb2a7a0c68aedb21a1b072f9b6bf8d2adb7bd19'
Path('artifacts.json').write_text(json.dumps(artifacts,indent=2)+'\n')
Path('build-success.txt').write_text('OLS_PARENT_BUILD_PASS\n')
HASH
taskset -c "$cpus" python3 harness/run-screen.py
