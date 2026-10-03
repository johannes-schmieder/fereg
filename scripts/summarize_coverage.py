#!/usr/bin/env python3
"""Map LLVM function instances to the versioned runtime source inventory."""
from __future__ import annotations
import argparse
from collections import Counter
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]


def summarize(export, inventory):
    # Rust generic monomorphizations and closures share declaration regions.
    # A source function is reached if at least one of its instances was called.
    entries = {}
    for unit in export['data']:
        for function in unit['functions']:
            if not function.get('regions'):
                continue
            region = function['regions'][0]
            filename = function['filenames'][region[5]]
            # Exported source paths are absolute on the build host; match the
            # repository suffix so retained CI artifacts can be read locally.
            match = re.search(r'(crates/(?:fereg-core|fereg-plugin)/src/.*)$', filename)
            if not match:
                continue
            key = match[1]
            entries.setdefault(key, []).append((region[0], function['count']))
    modules, totals, hotspots = {}, Counter(), []
    for path, item in inventory['files'].items():
        states = {'reached':[], 'unreached':[], 'unmapped':[]}
        for name, function in item['functions'].items():
            start = function['line']
            end = start + function['lines'] - 1
            counts = [count for line,count in entries.get(path,[]) if start <= line <= end]
            state = 'unmapped' if not counts else ('reached' if any(counts) else 'unreached')
            states[state].append(name)
            totals[state] += 1
            if function['lines'] > 150:
                hotspots.append({'file':path,'function':name,'line':start,'lines':function['lines'],'coverage':state})
        modules[path] = {k:v for k,v in states.items() if v}
    return {'definition':'Source functions from complexity definition v1; LLVM instance entry counts; generic/closure instances folded into containing source function.',
            'limits':'Unmapped means no exported instance (including never instantiated code); it is not coverage. Branch coverage and licensed Stata coverage are not claimed.',
            'functions':dict(totals),'modules':modules,'largest_functions':sorted(hotspots,key=lambda x:-x['lines'])[:20]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory',type=Path,default=ROOT/'target/coverage-baseline')
    parser.add_argument('--output',type=Path,required=True)
    args = parser.parse_args()
    inv = json.loads((ROOT/'docs/cleanup/complexity.json').read_text())['inventory']
    result = summarize(json.loads((args.directory/'export.json').read_text()), inv)
    for module, entry in [('estimate','estimate_ols_test#1'), ('ppml_estimate','estimate_ppml_test#1')]:
        path = f'crates/fereg-plugin/src/{module}.rs'
        if entry not in result['modules'][path].get('reached',[]):
            raise SystemExit(f'native coverage is missing its required entry point: {entry}')
    result['identity'] = json.loads((args.directory/'identity.json').read_text())
    result['campaign'] = 'Long independent oracle suite, existing actual-plugin ABI assertions and 73 complete OLS/PPML replays, separate instrumented next build. Full Rust/allocation/transaction suite qualified separately without instrumentation.'
    args.output.write_text(json.dumps(result,indent=2,sort_keys=True)+'\n')
    print(json.dumps(result['functions'],sort_keys=True))


if __name__ == '__main__': main()
