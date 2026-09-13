import os
from pathlib import Path
import os,subprocess,json
root=Path(__file__).parent
cases=[('fereg-core','recovery_'),('fereg-core','wide_ols_schur_guard_preserves_ineligible_routes_and_cycle_reports'),('fereg-core','direct_prepared_cg_checks_joint_budget_with_retained_coefficients'),('fereg-plugin','saved_')]
env=os.environ.copy();env.update(RUSTUP_TOOLCHAIN='1.85.1-aarch64-apple-darwin',CARGO_NET_OFFLINE='true',CARGO_TARGET_DIR=str(root/'test-target'))
results=[]
with (root/'guard-plugin-tests.log').open('wb') as log:
 for package,name in cases:
  command=['cargo','test','--locked','-p',package,'--lib',name]
  p=subprocess.run(command,cwd=root/'candidate',env=env,stdout=log,stderr=subprocess.STDOUT)
  results.append(dict(command=command,rc=p.returncode));(root/'test-results.json').write_text(json.dumps(results,indent=2)+'\n');print(results[-1],flush=True)
  assert p.returncode==0
