#!/usr/bin/env bash
#$ -P welfgr
#$ -N fereg_dev_smoke
#$ -pe omp 4
#$ -l h_rt=00:30:00
#$ -l mem_per_core=4G
#$ -j y
#$ -m a

set -eo pipefail
# A caller may source this entry point under nounset. SCC's system startup
# deliberately probes unset variables, so disable it until initialization ends.
set +u
source /etc/bashrc
set -u

: "${RUN_ROOT:?RUN_ROOT is required}"
: "${FEREG_DEV_PROFILE:?FEREG_DEV_PROFILE must be ols, ppml, or shared}"
: "${FEREG_DEV_SNAPSHOT:?FEREG_DEV_SNAPSHOT is required}"

if [[ ! "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$ ]]; then
    echo "RUN_ROOT is not an exact fereg development run directory" >&2
    exit 2
fi
if [[ ! "$FEREG_DEV_SNAPSHOT" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ ]]; then
    echo "FEREG_DEV_SNAPSHOT must be a 40- or 64-character source identity" >&2
    exit 2
fi
if [[ "${NSLOTS:-}" != "4" ]]; then
    echo "development smoke requires exactly four SCC slots" >&2
    exit 2
fi

case "$FEREG_DEV_PROFILE" in
    ols) PROFILE=scc-dev-ols ;;
    ppml) PROFILE=scc-dev-ppml ;;
    shared) PROFILE=scc-dev-shared ;;
    *)
        echo "FEREG_DEV_PROFILE must be ols, ppml, or shared" >&2
        exit 2
        ;;
esac

SOURCE_ROOT="$RUN_ROOT/source"
OUTPUT_ROOT="$RUN_ROOT/output"
SUMMARY="$OUTPUT_ROOT/summary.json"
SUCCESS="$OUTPUT_ROOT/DEV_SMOKE.SUCCESS"
test -f "$SOURCE_ROOT/ci/stata_profiles.json"
test -f "$SOURCE_ROOT/ci/run_fixture_profile.py"
test ! -e "$SUMMARY"
test ! -e "$SUCCESS"
mkdir -p "$OUTPUT_ROOT" "$RUN_ROOT/work/target"

module --force purge
module load stata-mp/19
module load python3/3.12.4

export CARGO_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo
export RUSTUP_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/rustup
export RUSTUP_BIN="$CARGO_HOME/bin/rustup"
export RUST_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
export RUSTUP_TOOLCHAIN="$RUST_TOOLCHAIN"
export CARGO_NET_OFFLINE=true
export CARGO_TARGET_DIR="$RUN_ROOT/work/target"
export PATH="$CARGO_HOME/bin:$PATH"
export OMP_NUM_THREADS=4
export OPENBLAS_NUM_THREADS=1
export MKL_NUM_THREADS=1
export NUMEXPR_NUM_THREADS=1

python3 "$SOURCE_ROOT/ci/run_fixture_profile.py" \
    --profile-config "$SOURCE_ROOT/ci/stata_profiles.json" \
    --profile "$PROFILE" \
    --repo-root "$SOURCE_ROOT" \
    --run-root "$OUTPUT_ROOT" \
    --summary-json "$SUMMARY" \
    --stata "$(command -v stata-mp)" \
    --tested-sha "$FEREG_DEV_SNAPSHOT"

python3 - "$SUMMARY" "$FEREG_DEV_PROFILE" <<'PY'
import json
import sys
from pathlib import Path

summary = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
expected = {"ols": 1, "ppml": 1, "shared": 2}[sys.argv[2]]
if (
    summary.get("status") != "success"
    or summary.get("tests_failed") != 0
    or summary.get("tests_passed") != expected
):
    raise SystemExit("development smoke summary is not successful")
PY

printf 'profile=%s\nsnapshot=%s\njob_id=%s\n' \
    "$FEREG_DEV_PROFILE" "$FEREG_DEV_SNAPSHOT" "${JOB_ID:-unknown}" > "$SUCCESS"
printf 'FEREG_DEV_SMOKE_SUCCESS profile=%s snapshot=%s\n' \
    "$FEREG_DEV_PROFILE" "$FEREG_DEV_SNAPSHOT"
