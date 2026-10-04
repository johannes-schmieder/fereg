#!/bin/bash
# Invoked inside the setup job, after module initialization.
set -euo pipefail
umask 077
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
(
    cd source
    bash scripts/build_linux.sh
) > logs/build.log 2>&1
python3 source/bench/setup.py --root "$run_root/environment" > logs/comparators.log 2>&1
(
    set +u
    source /etc/bashrc
    module load llvm/16.0.6
    set -u
    python3 source/bench/build_clock.py --output "$run_root/environment/command-clock.plugin"
) > logs/clock-build.log 2>&1
python3 - "$run_root" <<'PY'
import hashlib,json,sys
from pathlib import Path
root=Path(sys.argv[1])
plugin=root/'source/dist/fereg_linux.plugin'
sys.path.insert(0,str(root/'source/bench'))
from run import native_identity, tooling_identity
policy=json.loads((root/'input/policy.json').read_text()) if (root/'input/policy.json').exists() else {'engine':'legacy'}
manifest=root/'input/suite.json' if (root/'input/suite.json').exists() else None
(root/'environment/build.json').write_text(json.dumps({'source':json.loads((root/'input/source.json').read_text()),
    'plugin_sha256':hashlib.sha256(plugin.read_bytes()).hexdigest(),
    'native_identity':native_identity(policy['engine'],root/'source'),
    'timing_identity':{'method':'monotonic-command-v1','plugin_sha256':hashlib.sha256((root/'environment/command-clock.plugin').read_bytes()).hexdigest()},
    'tooling_identity':tooling_identity(manifest)},indent=2)+'\n')
PY
python3 source/bench/run.py --tier smoke --cells ols-1fe --engine next \
    --environment "$run_root/environment" --root "$run_root/output/clock-smoke" \
    --summary "$run_root/output/clock-smoke.csv" --stata "$(command -v stata-mp)" \
    --stata-processors 4 --fereg-only --clock-plugin "$run_root/environment/command-clock.plugin" \
    > logs/clock-smoke.log 2>&1
python3 - "$run_root/output" <<'PY'
import csv,json,sys
from pathlib import Path
root=Path(sys.argv[1])
campaign=json.loads((root/'clock-smoke/results.json').read_text())
rows=list(csv.DictReader((root/'clock-smoke.csv').open()))
assert campaign['complete'] and campaign['identities_verified']
assert len(rows)==1 and all(row['status']=='ok' and row['accuracy']=='pass' and row['repetitions']=='5' for row in rows)
assert campaign['timing_identity']['method']=='monotonic-command-v1'
print('FEREG_CLOCK_SMOKE_COMPLETE')
PY
printf 'FEREG_BENCH_SETUP_COMPLETE job=%s\n' "${JOB_ID:-local}" > environment/SETUP.COMPLETE
