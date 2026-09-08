#!/usr/bin/env bash
#$ -P welfgr
#$ -N feppml_actual_auto
#$ -pe omp28 28
#$ -l h_rt=02:00:00
#$ -l mem_per_core=3G
#$ -l m_core=28
#$ -l num_proc=28
#$ -j y
#$ -m a
source /etc/bashrc
set -euo pipefail
: "${RUN_ROOT:?RUN_ROOT is required}"
[[ "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$ ]]
[[ "${NSLOTS:-}" == 28 && "${JOB_ID:-}" =~ ^[1-9][0-9]*$ ]]
module --force purge
module load stata-mp/19 python3/3.12.4
export CARGO_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo
export RUSTUP_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/rustup
export RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
export CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS="$NSLOTS" RAYON_NUM_THREADS="$NSLOTS"
export CARGO_TARGET_DIR="$RUN_ROOT/work/target"
export PATH="$CARGO_HOME/bin:$PATH"
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
test ! -e "$RUN_ROOT/work/smoke"
qhost -h "$HOSTNAME" -F m_core,cpu_arch,cpu_type > "$RUN_ROOT/logs/qhost.txt"
python3 - "$RUN_ROOT" <<'PY'
import sys, re
from pathlib import Path
root = Path(sys.argv[1])
cores = re.findall(r'(?:^|[:\s])m_core=([^\s]+)', (root/'logs/qhost.txt').read_text(), re.MULTILINE)
if len(set(cores)) != 1 or float(cores[0]) != 28:
    raise SystemExit('unexpected physical core count')
PY
export FEREG_GIT_COMMIT="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["snapshot_sha256"][:12])' "$RUN_ROOT/input/snapshot.json")"
(cd "$RUN_ROOT/source" && ./scripts/build_linux.sh)
dataset=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["dataset"])' "$RUN_ROOT/input/smoke.json")
dataset_sha256=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["dataset_sha256"])' "$RUN_ROOT/input/smoke.json")
python3 "$RUN_ROOT/source/scripts/ppml_auto_acceptance.py" \
    --task 17 --candidate "$RUN_ROOT/source" \
    --source-identity "$RUN_ROOT/input/source-files.json" \
    --plugin "$RUN_ROOT/source/dist/fereg_linux.plugin" \
    --dataset "$dataset" --dataset-sha256 "$dataset_sha256" \
    --stata "$(command -v stata-mp)" --ring-multiplier 10227 \
    --output "$RUN_ROOT/work/smoke"
printf 'ACTUAL_AUTO_LINUX_SMOKE_PASS job=%s\n' "$JOB_ID"
