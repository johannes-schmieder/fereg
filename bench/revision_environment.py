#!/usr/bin/env python3
"""Reuse newly qualified comparator environments after checking every declared pin."""
import hashlib,json,sys
from pathlib import Path
root,origin=map(Path,sys.argv[1:]);source=root/'source/bench';env=root/'environment';old=origin/'environment'
assert (origin/'COMPLETE').read_text().strip()=='COMPARATOR_SETUP_COMPLETE'
pins=json.loads((source/'reghdfejl/pins.json').read_text())
actual=json.loads((old/'reghdfejl/versions.json').read_text())
for key,value in pins.items():assert actual[key]==value,key
for name in ['Project.toml','Manifest.toml']:
    assert (source/'julia-ppml'/name).read_bytes()==(old/'julia-ppml'/name).read_bytes(),name
assert json.loads((old/'julia-ppml-versions.json').read_text())=={'julia':'1.11.7','GLFixedEffectModels':'0.5.6'}
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(old/'reghdfejl/references/jl.plugin')==actual['bridge_plugin_sha256']
for name,expected in actual['manifests'].items():assert sha(old/'reghdfejl'/name)==expected,name
for name in ['julia-ppml','julia-ppml-depot','julia-ppml-versions.json','reghdfejl']:
    (env/name).symlink_to(old/name,target_is_directory=(old/name).is_dir())
print('EXPANDED_COMPARATOR_REUSE_PINS_PASS')
