#!/bin/bash
# Exact Linux Stata/Julia bridge installation, isolated from other Julia arms.
# Run inside the allocated setup job. Installation and precompilation are untimed.
set -euo pipefail
ENVIRONMENT=${1:?environment directory}/reghdfejl
mkdir -p "$ENVIRONMENT"
SOURCE=$(cd "$(dirname "$0")/../.." && pwd -P)
export JULIA_DEPOT_PATH="$ENVIRONMENT/depot"
export JULIA_CPU_TARGET='generic;sandybridge,-xsaveopt,clone_all;haswell,-rdrnd,base(1)'
export JULIA_NUM_PRECOMPILE_TASKS=${NSLOTS:-2}
python3 - "$SOURCE/bench/reghdfejl/pins.json" "$ENVIRONMENT" <<'PY'
import hashlib,json,pathlib,tarfile,urllib.request,sys
pins=json.load(open(sys.argv[1]));root=pathlib.Path(sys.argv[2]);refs=root/'references';refs.mkdir(parents=True,exist_ok=True)
def fetch(item,path):
    path.parent.mkdir(parents=True,exist_ok=True)
    if path.exists() and hashlib.sha256(path.read_bytes()).hexdigest()==item['sha256']:return
    with urllib.request.urlopen(item['url'],timeout=120) as response, path.open('wb') as stream:
        while chunk:=response.read(1<<20):stream.write(chunk)
    if hashlib.sha256(path.read_bytes()).hexdigest()!=item['sha256']:raise ValueError('download hash differs: '+path.name)
for name,item in pins['files'].items():fetch(item,refs/name)
archive=root/'runtime.tar.gz';fetch(pins['julia_archive'],archive)
with tarfile.open(archive) as source:source.extractall(root,filter='data')
archive.unlink()
runtime=root/('julia-'+pins['julia_version']);bindir=root/'bin';bindir.mkdir(exist_ok=True)
# jl asks `julia +CHANNEL` to locate libjulia. This shim only accepts the frozen
# runtime and prevents a measured process from using/installing a Juliaup channel.
launcher=bindir/'julia'
launcher.write_text('#!/bin/bash\nset -euo pipefail\nif [[ "${1:-}" == +* ]]; then\n  [[ "$1" == "+'+pins['julia_version']+'" ]] || exit 64\n  shift\nfi\nexec "'+str(runtime/'bin/julia')+'" "$@"\n')
launcher.chmod(0o755)
PY
export PATH="$ENVIRONMENT/bin:$PATH"
# Compile the unchanged bridge sources against SCC's C library. The published
# Linux binary requires GLIBC_2.34; keep it alongside this locally built binary.
cp "$ENVIRONMENT/references/jl.plugin" "$ENVIRONMENT/references/jl.plugin.upstream"
module load gcc/12.2.0
(cd "$ENVIRONMENT/references/cpp" && g++ -shared -fPIC -DSYSTEM=OPUNIX -std=c++11 \
    -static-libstdc++ -static-libgcc stplugin.c julia.ado.cpp \
    -I"$ENVIRONMENT/julia-1.12.7/include/julia" \
    -L"$ENVIRONMENT/julia-1.12.7/lib" -Wl,-rpath,"$ENVIRONMENT/julia-1.12.7/lib" \
    -ljulia -o ../jl.plugin)
g++ --version > "$ENVIRONMENT/bridge-compiler.txt"
ldd "$ENVIRONMENT/references/jl.plugin" > "$ENVIRONMENT/bridge-ldd.txt"
# Verify the shim and record the resolved shared library used by jl.
julia +1.12.7 --startup-file=no -e 'using Libdl; @assert VERSION==v"1.12.7"; println(dlpath("libjulia"))' > "$ENVIRONMENT/lib.txt"
# Both packages load DataFrames/CategoricalArrays into one embedded Julia process.
# The bridge's bundled @Stata manifest is older than reghdfejl's; use the exact
# reghdfejl dependency graph for both namespaces to prevent runtime upgrades and
# mixed versions. Both bridge dependencies are direct reghdfejl dependencies.
mkdir -p "$JULIA_DEPOT_PATH/environments/reghdfejl" "$JULIA_DEPOT_PATH/environments/Stata"
cp "$ENVIRONMENT/references/reghdfejl_project.toml" "$JULIA_DEPOT_PATH/environments/reghdfejl/Project.toml"
cp "$ENVIRONMENT/references/reghdfejl_manifest.toml" "$JULIA_DEPOT_PATH/environments/reghdfejl/Manifest.toml"
export JULIA_PKG_PRECOMPILE_AUTO=0
julia +1.12.7 --startup-file=no --project="$JULIA_DEPOT_PATH/environments/reghdfejl" -e '
using Pkg, TOML
manifest=joinpath(dirname(Base.active_project()), "Manifest.toml")
declared=TOML.parsefile(manifest)
# Pkg.pin resolves registry metadata even when every version is unchanged. That
# can add dependencies absent from the published manifest. Set only the pin
# flags directly, then require instantiate to preserve the entire declared graph.
for name in keys(Pkg.project().dependencies)
    entries=declared["deps"][name]
    @assert length(entries)==1
    entries[1]["pinned"]=true
end
open(manifest,"w") do io
    TOML.print(io,declared;sorted=true)
end
Pkg.instantiate()
@assert TOML.parsefile(manifest)==declared "instantiation changed the declared bridge dependency graph"
using DataFrames, CategoricalArrays, FixedEffectModels, Distributions, Vcov
@assert pkgversion(DataFrames)==v"1.8.2"
@assert pkgversion(CategoricalArrays)==v"1.1.1"
@assert pkgversion(FixedEffectModels)==v"1.13.3"
'
cp "$JULIA_DEPOT_PATH/environments/reghdfejl/"*.toml "$JULIA_DEPOT_PATH/environments/Stata/"
# jl's loader uses mtimes to decide whether to replace imported environments.
touch "$JULIA_DEPOT_PATH/environments/reghdfejl/"*.toml "$JULIA_DEPOT_PATH/environments/Stata/"*.toml
export JULIA_PKG_OFFLINE=true
mkdir -p "$ENVIRONMENT/setup"
python3 - "$ENVIRONMENT" <<'PY'
import pathlib,sys
r=pathlib.Path(sys.argv[1]);refs=r/'references'
(r/'setup/setup.do').write_text('''version 19
clear all
set more off
set processors 1
sysdir set PLUS "'''+str(refs)+'''"
adopath ++ "'''+str(refs)+'''"
capture adopath - SITE
capture adopath - "."
jl start, threads(1) channel(1.12.7)
set obs 120
generate long g1 = mod(_n,10)+1
generate long g2 = mod(_n,12)+1
generate double x = sin(_n)
generate double y = x + cos(_n) + g1/10 + g2/20
reghdfejl y x, absorb(a=g1 b=g2) vce(robust) threads(1)
drop a b
reghdfejl y x, absorb(a=g1 b=g2) vce(cluster g1) threads(1) tolerance(1e-12)
assert e(converged)==1
_jl: @assert string(VERSION)=="1.12.7";
_jl: @assert string(pkgversion(FixedEffectModels))=="1.13.3";
display "REGHDFEJL_SETUP_COMPLETE"
exit, clear
''')
PY
(cd "$ENVIRONMENT/setup" && "${STATA_BIN:-stata-mp}" -b do setup.do)
python3 - "$SOURCE/bench/reghdfejl/pins.json" "$ENVIRONMENT" <<'PY'
import hashlib,json,pathlib,sys
pins=json.load(open(sys.argv[1]));root=pathlib.Path(sys.argv[2]);log=(root/'setup/setup.log').read_text()
assert '\nREGHDFEJL_SETUP_COMPLETE\n' in log,'Stata bridge setup/smoke failed; inspect setup.log'
# Record the final manifests, containing the published dependency graph plus
# direct-dependency pin flags. Later runtime checks cover these exact files.
files=[*sorted((root/'depot/environments').rglob('*.toml'))]
assert len(files)>=4,'missing resolved bridge manifests'
metadata=dict(pins,bridge_environment='reghdfejl dependency graph shared with Stata namespace',lib=(root/'lib.txt').read_text().strip(),
    bridge_plugin_sha256=hashlib.sha256((root/'references/jl.plugin').read_bytes()).hexdigest(),
    bridge_compiler=(root/'bridge-compiler.txt').read_text().splitlines()[0],
    refs=str(root/'references'),bin=str(root/'bin'),depot=str(root/'depot'),channel=pins['julia_version'],
    manifests={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in files})
(root/'versions.json').write_text(json.dumps(metadata,indent=2)+'\n')
PY
echo REGHDFEJL_ENVIRONMENT_COMPLETE
