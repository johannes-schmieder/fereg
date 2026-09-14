import os
from pathlib import Path
import subprocess,json,hashlib,shutil,textwrap
repo=Path(os.environ['REPOSITORY_ROOT']);r=Path(__file__).resolve().parent
fmt=os.path.join(os.environ['RUST_TOOLCHAIN_BIN'], 'rustfmt')
spans=[('crates/fereg-core/src/ols/prepared_fit_view.rs','fn fill_generated_column_tile<','/// Common scientific input boundary',0),('crates/fereg-core/src/ols/prepared_rhs.rs','    #[test]\n    fn generated_predictor_tiles','    #[test]\n    fn empty_rhs',4)]
for name,start,end,indent in spans:
 p=repo/name;s=p.read_text();a=s.index(start);b=s.index(end,a);formatted=subprocess.run([fmt,'--edition','2021','--emit','stdout'],input=textwrap.dedent(s[a:b]),text=True,capture_output=True,check=True).stdout;p.write_text(s[:a]+textwrap.indent(formatted,' '*indent)+'\n'+s[b:])
p=repo/'crates/fereg-core/src/ols/prepared_fit_view.rs';s=p.read_text();start=0
while True:
 a=s.find('    fn fit_subtract_raw_columns(',start)
 if a<0:break
 b=s.index('\n    }',a)+6;formatted=subprocess.run([fmt,'--edition','2021','--emit','stdout'],input=textwrap.dedent(s[a:b]),text=True,capture_output=True,check=True).stdout.rstrip();replacement=textwrap.indent(formatted,'    ');s=s[:a]+replacement+s[b:];start=a+len(replacement)
p.write_text(s)
ids=json.loads((r/'baseline-identities.json').read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();shutil.copytree(r/'baseline',r/'candidate',ignore=shutil.ignore_patterns('target','dist'))
for name in ids['baseline_inputs']:
 if sha(repo/name)!=ids['baseline_inputs'][name]:shutil.copy2(repo/name,r/'candidate'/name)
ids['candidate_inputs']={p:sha(r/'candidate'/p) for p in ids['baseline_inputs']};(r/'prebuild-identities.json').write_text(json.dumps(ids,indent=2)+'\n');(r/'implementation.patch').write_bytes(subprocess.check_output(['git','diff','--','crates'],cwd=repo));print('OLS_RESIDUAL_SOURCE_SNAPSHOT_READY')
