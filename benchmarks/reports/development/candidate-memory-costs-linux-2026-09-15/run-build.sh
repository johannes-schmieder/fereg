#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_memory_build
#$ -cwd
#$ -j y
set -euo pipefail
module load python3/3.12.4
export CARGO_HOME=/projectnb/welfgr/fereg/toolchains/cargo
export RUSTUP_HOME=/projectnb/welfgr/fereg/toolchains/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$PWD/target"
export FEREG_GIT_COMMIT=4396bf0-memory-screen1 FEREG_BUILD_DATE=2026-09-15
export CARGO_BUILD_JOBS=4 RAYON_NUM_THREADS=4 CARGO_NET_OFFLINE=true
cd source
bash scripts/build_linux.sh
printf 'MEMORY_SCREEN_LINUX_BUILD_PASS\n'
