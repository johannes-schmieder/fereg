#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_mem_input
#$ -cwd
#$ -j y
set -euo pipefail
export CARGO_HOME=$TOOLCHAINS/cargo RUSTUP_HOME=$TOOLCHAINS/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$PWD/target"
export CARGO_BUILD_JOBS=4 RAYON_NUM_THREADS=4 RUST_TEST_THREADS=1 CARGO_NET_OFFLINE=true
export FEREG_GIT_COMMIT=5240e58-input-memory3 FEREG_BUILD_DATE=2026-09-16
qstat -j "$JOB_ID" > allocation.txt
(cd source && cargo test --locked --offline -p fereg-plugin --lib -- --test-threads=1) > plugin.txt 2>&1
(cd source && cargo test --locked --offline -p fereg-core --lib ppml::memory::tests -- --test-threads=1) > core-memory.txt 2>&1
printf 'FRONTEND_MEMORY_PASS\n' > PASS
