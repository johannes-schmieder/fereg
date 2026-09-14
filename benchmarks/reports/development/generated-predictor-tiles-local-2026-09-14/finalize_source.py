import os
from pathlib import Path
import subprocess,json,hashlib,shutil,textwrap
repo=Path(os.environ['REPOSITORY_ROOT']);r=Path(__file__).resolve().parent
spans=[('crates/fereg-core/src/ols/prepared_fit_view.rs','/// Accumulate generated finite predictors','/// Common scientific input boundary',0),('crates/fereg-core/src/ols/prepared_rhs.rs','    #[test]\n    fn generated_predictor_tiles','    #[test]\n    fn empty_rhs',4),('crates/fereg-core/src/ols/linear_session.rs','    fn rebuild_predictors(\n','    #[cfg(test)]\n    fn root_solver_weight',4)]
for name,start,end,indent in spans:
 p=repo/name;s=p.read_text();a=s.index(start);b=s.index(end,a);part=textwrap.dedent(s[a:b]);formatted=subprocess.run([os.path.join(os.environ['RUST_TOOLCHAIN_BIN'], 'rustfmt'),'--edition','2021','--emit','stdout'],input=part,text=True,capture_output=True,check=True).stdout
 formatted=textwrap.indent(formatted,' '*indent)+'\n';p.write_text(s[:a]+formatted+s[b:]);shutil.copy2(p,r/'candidate'/name)
ids=json.loads((r/'prebuild-identities.json').read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids['candidate_inputs']={p:sha(repo/p) for p in ids['baseline_inputs']};(r/'prebuild-identities.json').write_text(json.dumps(ids,indent=2)+'\n');(r/'implementation.patch').write_bytes(subprocess.check_output(['git','diff','--','crates'],cwd=repo))
