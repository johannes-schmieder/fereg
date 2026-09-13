import os
from pathlib import Path
import json,hashlib,shutil,subprocess
root=Path(os.environ['EXPERIMENT_ROOT']);repo=Path(os.environ['REPOSITORY_ROOT']);previous=Path(os.environ['RECOVERY_ADMISSION_ROOT'])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
manifest=json.loads((previous/'v2-source-manifest.json').read_text())['inputs']
candidate=root/'candidate';shutil.copytree(previous/'candidate-v2',candidate)
baseline=Path(os.environ['ACCEPTED_SOURCE'])
inputs={};base={};untracked=[]
for rel in manifest:
 r=subprocess.run(['git','show','HEAD:'+rel],cwd=repo,capture_output=True)
 if r.returncode:untracked.append(rel);continue
 (candidate/rel).write_bytes(r.stdout);base[rel]=hashlib.sha256(r.stdout).hexdigest()
 assert sha(baseline/rel)==base[rel],rel
assert not untracked,untracked
p=candidate/'crates/fereg-core/src/ols/prepared_fit_view.rs';shutil.copyfile(repo/p.relative_to(candidate),p)
p=candidate/'crates/fereg-core/src/ols/prepared_fit.rs';s=p.read_text();test=(root/'cg-regression-source.txt').read_text();marker='    fn prepared_multiway_categorical('
assert s.count(marker)==1;s=s.replace(marker,test+marker);p.write_text(s)
for rel in manifest:inputs[rel]=sha(candidate/rel)
changes=[p for p in inputs if inputs[p]!=base[p]];assert sorted(changes)==['crates/fereg-core/src/ols/prepared_fit.rs','crates/fereg-core/src/ols/prepared_fit_view.rs']
patch=b''
for rel in changes:
 r=subprocess.run(['diff','-u','--label','a/'+rel,'--label','b/'+rel,str(baseline/rel),str(candidate/rel)],capture_output=True);assert r.returncode==1;patch+=r.stdout
(root/'cg-implementation.patch').write_bytes(patch)
(root/'cg-identities.json').write_text(json.dumps({'accepted_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip(),'baseline_inputs':base,'candidate_inputs':inputs,'changed_inputs':changes,'baseline_plugin':sha(baseline/'dist/fereg_macos.plugin'),'kind':'isolated CG-admission repair; excludes saved-Schur/recovery prototype'},indent=2)+'\n')
(root/'cg-cost-protocol.md').write_text('''# Direct prepared CG admission repair

Compare accepted numerical dadd956 at evidence HEAD de6988b, artifact e831a657, against the isolated two-file repair. No saved-Schur or final-recovery planning changes belong to either arm. Direct intercept CG now applies the existing CG-plus-retained-coefficient model before working allocations, using the installed team.

Require the closed-form Rust regression to fail before the repair and pass after it, including exact/one-byte-short admission for direct, explicit-dispatch and preselected-auto CG, unit/supplied weights, simple/categorical topology, retained coefficients and 1/4 installed workers. Verify licensed public exact-cap success and one-byte-short r(909) with full rollback. Run the existing independent 48-fit saved-OLS oracle, plus a direct-CG variant of the small weighted-within dummy fixture, on the isolated candidate. Existing accepted baseline oracle evidence is preserved.

Then compare six fixed accepted CG-repair controls: saved unit, repaired saved PW, weak_d3, low_fw, small_aw and complete_unit. Rotate arm order; no build/test/diagnostic may overlap timings. Preserve exact b/V/sample/status/DoF/solver decisions and saved values at their original tolerances. Record command, native, input, solver setup/solve, allocation/model/scratch and RSS. No speedup is hypothesized. Investigate any slowdown exceeding both 5% and 10 ms with two reversed same-request pairs; no broad new seed campaign. Existing public base-model omissions are not full-route bounds.
''')
print(json.dumps({'changes':changes,'baseline_plugin':sha(baseline/'dist/fereg_macos.plugin'),'count':len(inputs)}))
