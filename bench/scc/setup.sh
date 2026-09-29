#!/bin/bash
# Invoked inside the setup job, after module initialization.
set -euo pipefail
run_root=${1:?run root}
[[ "$run_root" =~ ^/projectnb/welfgr/fereg/(dev|runs)/[A-Za-z0-9][A-Za-z0-9._-]{0,95}$ ]]
cd "$run_root"
sha256sum -c input/SHA256SUMS
[[ ! -e source ]]
mkdir source environment
# This is an allowlisted Git archive, not a checkout of a floating remote branch.
tar -xf input/source.tar -C source
cp input/source.json source/.fereg-source.json
cmp controller/setup.sh source/bench/scc/setup.sh
cmp controller/job.sh source/bench/scc/job.sh
cp input/fixest.tar.gz input/ppmlhdfe.tar.gz environment/
mkdir environment/archives
mv environment/fixest.tar.gz environment/ppmlhdfe.tar.gz environment/archives/
export CARGO_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo
export RUSTUP_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/rustup
export RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
export CARGO_NET_OFFLINE=true
export CARGO_TARGET_DIR="$run_root/build"
export PATH="$CARGO_HOME/bin:$PATH"
export FEREG_GIT_COMMIT
FEREG_GIT_COMMIT=$(python3 -c 'import json; print(json.load(open("input/source.json"))["commit"])')
python3 -c 'import numpy,pandas; print("numpy",numpy.__version__,"pandas",pandas.__version__)'
bash source/scripts/build_linux.sh > logs/build.log 2>&1
python3 source/bench/setup.py --root "$run_root/environment" > logs/comparators.log 2>&1
python3 - "$run_root" <<'PY'
import hashlib,json,sys
from pathlib import Path
root=Path(sys.argv[1])
plugin=root/'source/dist/fereg_linux.plugin'
(root/'environment/build.json').write_text(json.dumps({'source':json.loads((root/'input/source.json').read_text()),
    'plugin_sha256':hashlib.sha256(plugin.read_bytes()).hexdigest()},indent=2)+'\n')
PY
printf 'FEREG_BENCH_SETUP_COMPLETE job=%s\n' "${JOB_ID:-local}" > environment/SETUP.COMPLETE
