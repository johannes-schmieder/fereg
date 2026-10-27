#!/bin/bash
# Install the pinned FixedEffectModels.jl arm into ENVIRONMENT/julia with its
# own depot, from bench/julia/{Project,Manifest}.toml. Runs inside a setup job
# with the julia module loaded (never a measured host); precompiled packages
# keep compilation out of production warm-up.
# Usage: bash bench/scc/julia_environment.sh ENVIRONMENT
set -euo pipefail
ENVIRONMENT=${1:?environment directory}
SOURCE=$(cd "$(dirname "$0")/../.." && pwd -P)
export JULIA_DEPOT_PATH="$ENVIRONMENT/julia-depot"
# Multi-target package images load on any campaign node; jobs export the same target.
export JULIA_CPU_TARGET='generic;sandybridge,-xsaveopt,clone_all;haswell,-rdrnd,base(1)'
mkdir -p "$ENVIRONMENT/julia"
cp "$SOURCE/bench/julia/Project.toml" "$SOURCE/bench/julia/Manifest.toml" "$ENVIRONMENT/julia/"
julia --startup-file=no --project="$ENVIRONMENT/julia" -e 'using Pkg; Pkg.instantiate(); Pkg.precompile(); Pkg.status()'
cmp "$SOURCE/bench/julia/Manifest.toml" "$ENVIRONMENT/julia/Manifest.toml"
julia --startup-file=no --project="$ENVIRONMENT/julia" -e '
using FixedEffectModels, DataFrames, ReadStatTables, JSON
open(ARGS[1], "w") do io
    JSON.print(io, Dict("julia" => string(VERSION), "FixedEffectModels" => string(pkgversion(FixedEffectModels))))
end' "$ENVIRONMENT/julia-versions.json"
python3 - "$SOURCE/bench/campaign.json" "$ENVIRONMENT/julia-versions.json" <<'PY'
import json, sys
pin = json.load(open(sys.argv[1]))['julia']
assert json.load(open(sys.argv[2])) == pin, (json.load(open(sys.argv[2])), pin)
PY
echo JULIA_ENVIRONMENT_COMPLETE
