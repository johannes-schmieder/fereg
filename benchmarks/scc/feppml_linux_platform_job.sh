#!/bin/bash -l
#$ -P welfgr
#$ -N feppml_platform
#$ -pe omp 4
#$ -l h_rt=01:00:00
#$ -l mem_per_core=4G
#$ -j y
#$ -m a

set -eo pipefail
source /etc/bashrc
set -u

if [ "$#" -ne 1 ]; then
    echo "usage: $0 /projectnb/welfgr/fereg/runs/RUN_ID" >&2
    exit 2
fi
RUN_ROOT=$1
if [[ ! "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/runs/[0-9]{8}T[0-9]{6}Z-[a-z0-9][a-z0-9-]{0,63}$ ]]; then
    echo "invalid exact SCC run root" >&2
    exit 2
fi
test "${USER:-}" = johannes
test "${JOB_NAME:-}" = "feppml_platform_$(basename "$RUN_ROOT")"
test "${SGE_O_WORKDIR:-}" = "$RUN_ROOT"

module --force purge
module load stata-mp/19
module load python3/3.12.4
SHARED_CARGO_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo
SHARED_RUSTUP_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/rustup
SHARED_PYTHON=/share/pkg.8/python3/3.12.4/install/bin/python3
RUN_CARGO_HOME="$RUN_ROOT/work/cargo-home"
RUN_TOOL_BIN="$RUN_ROOT/work/sealed-tool-bin"
test -d "$SHARED_CARGO_HOME/registry"
test ! -L "$SHARED_CARGO_HOME/registry"
test -d "$SHARED_CARGO_HOME/git"
test ! -L "$SHARED_CARGO_HOME/git"
test -x "$SHARED_CARGO_HOME/bin/cargo"
test -d "$SHARED_RUSTUP_HOME"
test ! -L "$SHARED_RUSTUP_HOME"
test "$(command -v python3)" = "$SHARED_PYTHON"
test ! -e "$RUN_CARGO_HOME"
test ! -L "$RUN_CARGO_HOME"
test ! -e "$RUN_TOOL_BIN"
test ! -L "$RUN_TOOL_BIN"
mkdir "$RUN_CARGO_HOME" "$RUN_TOOL_BIN"
cp -a "$SHARED_CARGO_HOME/registry" "$RUN_CARGO_HOME/registry"
cp -a "$SHARED_CARGO_HOME/git" "$RUN_CARGO_HOME/git"
chmod -R u+rwX "$RUN_CARGO_HOME"
printf '%s\n' \
    '#!/bin/bash' \
    'exec /share/pkg.8/python3/3.12.4/install/bin/python3 -I -B -S "$@"' \
    > "$RUN_TOOL_BIN/python3"
printf '%s\n' \
    '#!/bin/bash' \
    'set -euo pipefail' \
    'if [ "$#" -eq 1 ] && [ "$1" = "--version" ]; then' \
    '    exec /projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo/bin/cargo "$@"' \
    'fi' \
    'if [ "$#" -ne 7 ] || [ "$1" != "build" ] || [ "$2" != "--locked" ] || [ "$3" != "--release" ] || [ "$4" != "-p" ] || [ "$5" != "fereg-plugin" ] || [ "$6" != "--target" ] || [ "$7" != "x86_64-unknown-linux-gnu" ]; then' \
    '    echo "sealed cargo wrapper rejected invocation" >&2' \
    '    exit 97' \
    'fi' \
    'exec /projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo/bin/cargo "$@"' \
    > "$RUN_TOOL_BIN/cargo"
chmod 0555 "$RUN_TOOL_BIN/python3" "$RUN_TOOL_BIN/cargo"
export CARGO_HOME="$RUN_CARGO_HOME"
export RUSTUP_HOME="$SHARED_RUSTUP_HOME"
export RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
export CARGO_NET_OFFLINE=true
unset CARGO_TARGET_DIR
export PATH="$RUN_TOOL_BIN:$SHARED_CARGO_HOME/bin:$PATH"
test "$(rustc --version)" = "rustc 1.85.1 (4eb161250 2025-03-15)"
test "$(cargo --version)" = "cargo 1.85.1 (d73d2caf9 2024-12-31)"
export OMP_NUM_THREADS="${NSLOTS:-1}"
export OPENBLAS_NUM_THREADS=1
export MKL_NUM_THREADS=1
export NUMEXPR_NUM_THREADS=1

python3 -I -B -S "$RUN_ROOT/input/harness/benchmarks/scc/feppml_linux_platform_scc.py" \
    run --run-root "$RUN_ROOT"
