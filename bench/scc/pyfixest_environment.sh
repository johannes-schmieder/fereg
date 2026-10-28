#!/bin/bash
# Install the PyFixest arms pinned in campaign.json into ENVIRONMENT/<arm> from
# bench/pyfixest/<arm>.txt.
# Runs inside a setup job (never a measured host). The main revision builds its
# Rust extension with the pinned fereg toolchain; the release installs wheels.
# Usage: bash bench/scc/pyfixest_environment.sh ENVIRONMENT
set -euo pipefail
ENVIRONMENT=${1:?environment directory}
SOURCE=$(cd "$(dirname "$0")/../.." && pwd -P)
export CARGO_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/cargo
export RUSTUP_HOME=/projectnb/welfgr/johannes/fereg-rust-1.85.1/rustup
export RUSTUP_TOOLCHAIN=1.85.1-x86_64-unknown-linux-gnu
export PATH="$CARGO_HOME/bin:$PATH" CARGO_NET_OFFLINE=false CARGO_BUILD_JOBS=4
export PIP_DISABLE_PIP_VERSION_CHECK=1 PIP_NO_CACHE_DIR=1
for arm in $(python3 -c 'import json,sys; print(*json.load(open(sys.argv[1]))["pyfixest"])' "$SOURCE/bench/campaign.json"); do
    python3 -m venv "$ENVIRONMENT/$arm"
    "$ENVIRONMENT/$arm/bin/python" -I -m pip install --no-deps -r "$SOURCE/bench/pyfixest/$arm.txt"
    "$ENVIRONMENT/$arm/bin/python" -I -m pip check
    "$ENVIRONMENT/$arm/bin/python" -I -m pip freeze --all > "$ENVIRONMENT/$arm-freeze.txt"
    "$ENVIRONMENT/$arm/bin/python" -I -c 'import sys; sys.path.insert(0, sys.argv[1]); from campaign_pyfixest import check_pin; print(*check_pin(sys.argv[2]))' "$SOURCE/bench" "$arm"
done
echo PYFIXEST_ENVIRONMENT_COMPLETE
