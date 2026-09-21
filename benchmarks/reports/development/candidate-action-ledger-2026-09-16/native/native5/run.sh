#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_shared_mem
#$ -cwd
#$ -j y
set -euo pipefail
export CARGO_HOME=$TOOLCHAINS/cargo
export RUSTUP_HOME=$TOOLCHAINS/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$PWD/target"
export CARGO_BUILD_JOBS=4 RAYON_NUM_THREADS=4 RUST_TEST_THREADS=1 CARGO_NET_OFFLINE=true
export FEREG_GIT_COMMIT=5240e58-lifetime-ledger5 FEREG_BUILD_DATE=2026-09-16
cd source
set +e
cargo test --offline --locked -p fereg-core --lib retained_ -- --test-threads=1 > ../core-retained.txt 2>&1
r1=$?
cargo test --offline --locked -p fereg-core --lib score_reconstruction -- --test-threads=1 > ../core-inference.txt 2>&1
r2=$?
printf '%s %s\n' "$r1" "$r2" > ../return-codes.txt
if ((r1 || r2)); then exit 1; fi
printf 'CORE_LIFETIME_NATIVE_PASS\n' > ../PASS
