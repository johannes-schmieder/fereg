#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_sys_setup
#$ -pe omp 28
#$ -l num_proc=28
#$ -l cpu_type=E5-2680v4
#$ -l mem_per_core=8G
#$ -l h_rt=04:00:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}"
: "${FEREG_SCC_RUNS_ROOT:?Set the allowed SCC run-root parent}"
[[ "$FEREG_SCC_RUNS_ROOT" == /* && "$FEREG_SCC_RUNS_ROOT" != / ]]
[[ "$RUN_ROOT" == "${FEREG_SCC_RUNS_ROOT%/}"/* && "$NSLOTS" == 28 ]]
module purge
module load python3/3.12.4
module load stata-mp/19
module load R/4.5.2
mkdir -p "$RUN_ROOT/env" "$RUN_ROOT/logs"
python3 "$RUN_ROOT/controller/setup.py" "$RUN_ROOT"
: "${FEREG_SCC_RUST_ROOT:?Set the isolated Rust installation root}"
[[ "$FEREG_SCC_RUST_ROOT" == /* ]]
export CARGO_HOME="$FEREG_SCC_RUST_ROOT/cargo"
export RUSTUP_HOME="$FEREG_SCC_RUST_ROOT/rustup"
export RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$RUN_ROOT/env/candidate-target"
export FEREG_GIT_COMMIT=d44e36bfa5ad7e41a06752d1008f0b9683f8c8dd
export FEREG_BUILD_DATE=2026-09-10
export OMP_NUM_THREADS=28
export OPENBLAS_NUM_THREADS=1
export MKL_NUM_THREADS=1
cd "$RUN_ROOT/candidate"
CARGO_NET_OFFLINE=false cargo fetch --locked
export CARGO_NET_OFFLINE=true
bash scripts/build_linux.sh > "$RUN_ROOT/logs/candidate-build.txt" 2>&1
python3 -m venv "$RUN_ROOT/env/python"
"$RUN_ROOT/env/python/bin/pip" install --disable-pip-version-check maturin numpy pandas scipy psutil matplotlib pytest pyreadstat > "$RUN_ROOT/logs/python-install.txt" 2>&1
pyfixest_source=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["pyfixest"])' "$RUN_ROOT/env/source-roots.json")
# The comparator's Rust compiler is isolated from the candidate's pinned toolchain.
export RUSTUP_TOOLCHAIN=stable
rustup toolchain install stable --profile minimal > "$RUN_ROOT/logs/comparator-rust.txt" 2>&1
CARGO_NET_OFFLINE=false "$RUN_ROOT/env/python/bin/pip" install --disable-pip-version-check "$pyfixest_source" >> "$RUN_ROOT/logs/python-install.txt" 2>&1
"$RUN_ROOT/env/python/bin/pip" freeze --all > "$RUN_ROOT/env/python-freeze.txt"
rustc --version >> "$RUN_ROOT/logs/comparator-rust.txt"
mkdir -p "$RUN_ROOT/env/R"
export R_LIBS_USER="$RUN_ROOT/env/R"
Rscript -e 'lib <- Sys.getenv("R_LIBS_USER"); install.packages(c("haven","jsonlite","digest","dreamerr","stringmagic"), lib=lib, repos="https://cloud.r-project.org", Ncpus=8)' > "$RUN_ROOT/logs/R-install.txt" 2>&1
R CMD INSTALL --library="$R_LIBS_USER" "$RUN_ROOT/input/archives/fixest.tar.gz" >> "$RUN_ROOT/logs/R-install.txt" 2>&1
Rscript -e 'library(fixest); library(haven); library(jsonlite); library(digest); print(sessionInfo()); write.csv(installed.packages()[,c("Package","Version","Built")], file.path(Sys.getenv("RUN_ROOT"), "env/R-packages.csv"), row.names=FALSE)' > "$RUN_ROOT/env/R-session.txt"
python3 - "$RUN_ROOT" <<'PY'
import hashlib,json,pathlib,sys
root=pathlib.Path(sys.argv[1])
paths=[root/'candidate/dist/fereg_linux.plugin', root/'env/python-freeze.txt',root/'env/R-packages.csv',root/'env/reference-files.json']
receipt={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}
(root/'env/setup.json').write_text(json.dumps(receipt,indent=2)+'\n')
PY
printf 'SYSTEMATIC_SETUP_SUCCESS job=%s\n' "$JOB_ID" > "$RUN_ROOT/env/SETUP.SUCCESS"
