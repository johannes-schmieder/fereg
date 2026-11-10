#!/bin/bash
# Separate GLFixedEffectModels environment; run only in setup, never a timed job.
set -euo pipefail
ENVIRONMENT=${1:?environment directory}
SOURCE=$(cd "$(dirname "$0")/../.." && pwd -P)
export JULIA_DEPOT_PATH="$ENVIRONMENT/julia-ppml-depot"
export JULIA_CPU_TARGET='generic;sandybridge,-xsaveopt,clone_all;haswell,-rdrnd,base(1)'
mkdir -p "$ENVIRONMENT/julia-ppml"
cp "$SOURCE/bench/julia-ppml/Project.toml" "$SOURCE/bench/julia-ppml/Manifest.toml" "$ENVIRONMENT/julia-ppml/"
julia --startup-file=no --project="$ENVIRONMENT/julia-ppml" -e 'using Pkg; @assert VERSION == v"1.11.7"; Pkg.instantiate(); Pkg.precompile()'
cmp "$SOURCE/bench/julia-ppml/Manifest.toml" "$ENVIRONMENT/julia-ppml/Manifest.toml"
julia --startup-file=no --project="$ENVIRONMENT/julia-ppml" -e '
using GLFixedEffectModels, GLM, Distributions, DataFrames, ReadStatTables, JSON
@assert pkgversion(GLFixedEffectModels) == v"0.5.6"
open(ARGS[1], "w") do io
    JSON.print(io, Dict("julia"=>string(VERSION), "GLFixedEffectModels"=>string(pkgversion(GLFixedEffectModels))))
end' "$ENVIRONMENT/julia-ppml-versions.json"
echo JULIA_PPML_ENVIRONMENT_COMPLETE
