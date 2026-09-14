#!/usr/bin/env python3
"""Replay bounded singleton scale checks from two explicit source trees."""
from pathlib import Path
import argparse, json, subprocess
parser=argparse.ArgumentParser()
for name in ["baseline-source", "candidate-source", "rustc", "output"]:
    parser.add_argument("--"+name, required=True)
a=parser.parse_args();out=Path(a.output).resolve();out.mkdir(exist_ok=False)
body=(Path(__file__).parent/"scale_driver.rs").read_text()
for arm, source in [("baseline",a.baseline_source),("candidate",a.candidate_source)]:
    source=Path(source).resolve()
    prefix=""
    for module, relative in [("error","crates/fereg-core/src/error.rs"),("singleton","crates/fereg-core/src/fe/singleton.rs")]:
        prefix += "#[allow(dead_code)]\n#[path = "+json.dumps(str(source/relative))+"]\nmod "+module+";\n"
    rust=out/("scale-"+arm+".rs");rust.write_text(prefix+body)
    subprocess.run([a.rustc,"--edition=2021","-O",str(rust),"-o",str(out/("scale-"+arm))],check=True)

from pathlib import Path
import json,subprocess,statistics
r=out;out=[]
with (r/'scale.jsonl').open('w') as f:
 for shape in ['chain','half','balanced']:
  for n in [8192,16384,32768,65536,262144,524288,1048576]:
   for rep in range(3):
    arms=['baseline','candidate'] if n<=65536 or shape=='balanced' else ['candidate']
    if rep%2: arms.reverse()
    for arm in arms:
     row=json.loads(subprocess.check_output([str(r/f'scale-{arm}'),shape,str(n)],text=True));row.update(arm=arm,repeat=rep);out.append(row)
     f.write(json.dumps(row)+'\n');f.flush()
summary=[]
for shape in ['chain','half','balanced']:
 for n in sorted({v['n'] for v in out}):
  row={'shape':shape,'n':n}
  for arm in ['baseline','candidate']:
   runs=[v for v in out if v['shape']==shape and v['n']==n and v['arm']==arm]
   if runs:row[arm]={'seconds':statistics.median(v['seconds']for v in runs),'peak_bytes':max(v['peak_bytes']for v in runs)}
  if 'baseline'in row:row['speedup']=row['baseline']['seconds']/row['candidate']['seconds']
  summary.append(row)
(r/'scale-summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary,indent=2))
