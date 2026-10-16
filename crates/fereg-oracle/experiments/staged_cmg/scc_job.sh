#!/bin/bash -l
set -euo pipefail
mode=${1:?smoke or measure}
root=${2:?absolute campaign root}
cd "$root/source"
sha256sum -c SHA256SUMS > "$root/logs/source-check-$mode.stdout"
verify_exit() {
    result=$?
    trap - EXIT
    sha256sum -c SHA256SUMS > "$root/logs/source-check-$mode-final.stdout" || result=1
    if [[ -f "$root/logs/binary.sha256" ]]; then
        sha256sum -c "$root/logs/binary.sha256" >> "$root/logs/source-check-$mode-final.stdout" || result=1
    fi
    exit "$result"
}
trap verify_exit EXIT
module load miniconda/25.3.1
staged_python="${SCC_MINICONDA_BIN:?}/python3"
export CARGO_HOME=/projectnb/welfgr/fereg/toolchains/cargo
export RUSTUP_HOME=/projectnb/welfgr/fereg/toolchains/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$root/build"
export CARGO_BUILD_JOBS="${NSLOTS:?SGE slot allocation required}"
export RUST_TEST_THREADS="$CARGO_BUILD_JOBS"
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1
export LC_ALL=C TZ=UTC
if [[ "${STAGED_PROTOCOL:-original}" == persistent ]]; then
    export STAGED_PREVIOUS_BINARY="$root/previous/staged-cmg-prototype"
fi
"$staged_python" --version
if [[ "$mode" == smoke ]]; then
    cargo +1.85.1 test --locked --release
    cargo +1.85.1 build --locked --release
    sha256sum "$root/build/release/staged-cmg-prototype" > "$root/logs/binary.sha256"
fi
sha256sum -c "$root/logs/binary.sha256"
if [[ "$mode" == forecast ]]; then
    "$staged_python" diagnose_admitted.py "$root" forecast
elif [[ "$mode" == admitted-diagnostic ]]; then
    "$staged_python" diagnose_admitted.py "$root"
elif [[ "$mode" == gate ]]; then
    "$staged_python" run_scc.py measure --binary "$root/build/release/staged-cmg-prototype" --output "$root/gate" --protocol guarded-gate
    "$staged_python" evaluate_router.py gate "$root/gate" > "$root/gate/verdict.json"
    "$staged_python" run_scc.py pilot --binary "$root/build/release/staged-cmg-prototype" --output "$root/pilot" --protocol router --budget-seconds 300
else
    extra=()
    if [[ -n "${STAGED_BUDGET_SECONDS:-}" ]]; then extra=(--budget-seconds "$STAGED_BUDGET_SECONDS"); fi
    "$staged_python" run_scc.py "$mode" --binary "$root/build/release/staged-cmg-prototype" --output "$root/$mode" --protocol "${STAGED_PROTOCOL:-original}" "${extra[@]}"
fi
