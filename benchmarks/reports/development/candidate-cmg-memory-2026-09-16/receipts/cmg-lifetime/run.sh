#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_cmg_join
#$ -cwd
#$ -j y
set -euo pipefail
export CARGO_HOME=$TOOLCHAINS/cargo RUSTUP_HOME=$TOOLCHAINS/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$PWD/target"
export CARGO_BUILD_JOBS=4 RAYON_NUM_THREADS=4 RUST_TEST_THREADS=1 CARGO_NET_OFFLINE=true
qstat -j "$JOB_ID" > allocation.txt
(cd source && cargo test --locked --offline --features parallel --lib execution::lifetime_tests -- --test-threads=1) > lifetime.txt 2>&1
(cd source && cargo test --locked --offline --features parallel --test parallel --test memory_accounting -- --test-threads=1) > regression.txt 2>&1
printf 'CMG_LIFETIME_DRAFT_PASS\n' > PASS
