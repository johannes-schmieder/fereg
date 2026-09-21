#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_ppml_mem
#$ -cwd
#$ -j y
set -euo pipefail
export CARGO_HOME=$TOOLCHAINS/cargo
export RUSTUP_HOME=$TOOLCHAINS/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$PWD/target"
export CARGO_BUILD_JOBS=4 RAYON_NUM_THREADS=4 RUST_TEST_THREADS=1 CARGO_NET_OFFLINE=true
export FEREG_GIT_COMMIT=5240e58-publication-lifetime1 FEREG_BUILD_DATE=2026-09-16
cd source
set +e
cargo test --offline --locked -p fereg-plugin --lib -- --test-threads=1 > ../plugin.txt 2>&1
r1=$?
cargo test --offline --locked -p fereg-core --lib ppml::result::tests -- --test-threads=1 > ../core-result.txt 2>&1
r2=$?
cargo test --offline --locked -p fereg-core --lib ppml::memory::tests -- --test-threads=1 > ../core-memory.txt 2>&1
r3=$?
printf '%s %s %s\n' "$r1" "$r2" "$r3" > ../return-codes.txt
if ((r1 || r2 || r3)); then exit 1; fi
printf 'PPML_PUBLICATION_NATIVE_PASS\n' > ../PASS
