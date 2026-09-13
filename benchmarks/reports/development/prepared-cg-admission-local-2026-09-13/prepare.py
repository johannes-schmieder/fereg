import os
from pathlib import Path
import json,hashlib,shutil,subprocess
repo=Path(os.environ['REPOSITORY_ROOT'])
root=Path(os.environ['EXPERIMENT_ROOT'])
previous=Path(os.environ['RECOVERY_ADMISSION_ROOT'])
source=root/'diagnostic'; assert not source.exists()
manifest=json.loads((previous/'v2-source-manifest.json').read_text())['inputs']
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert all(sha(repo/p)==h for p,h in manifest.items())
shutil.copytree(previous/'diagnostic-v2',source)
p=source/'crates/fereg-core/src/memory.rs';s=p.read_text()
a='''    #[must_use]
    pub fn fits(self, solver_workspace_bytes: u128) -> bool {
        self.execution_bytes
            .checked_add(solver_workspace_bytes)
            .is_some_and(|total| total <= self.usable_bytes)
    }'''
b='''    #[must_use]
    #[track_caller]
    pub fn fits(self, solver_workspace_bytes: u128) -> bool {
        let fits = self.execution_bytes
            .checked_add(solver_workspace_bytes)
            .is_some_and(|total| total <= self.usable_bytes);
        let caller = std::panic::Location::caller();
        eprintln!("ADMISSION_AUDIT|file={}|line={}|execution={}|increment={}|usable={}|fits={fits}",
            caller.file(), caller.line(), self.execution_bytes, solver_workspace_bytes, self.usable_bytes);
        fits
    }'''
assert s.count(a)==1;s=s.replace(a,b);p.write_text(s)
p=source/'crates/fereg-core/src/ols/prepared_categorical_data.rs';s=p.read_text()
a='        let recovery_memory = if memory_budget.is_some()'
assert s.count(a)==1;s=s.replace(a,'        let planning_started = std::time::Instant::now();\n'+a)
a='        let fits = |bytes| memory_budget.is_none_or(|budget| budget.fits(bytes));'
b='''        let planning_nanos = planning_started.elapsed().as_nanos();
        eprintln!("RECOVERY_PLAN_AUDIT|n={}|levels={:?}|requested={}|threads={}|nanos={planning_nanos}|plan={recovery_memory:?}",
            self.sample.n_active(),
            self.fe_topology.effect_to_topology().iter().map(|&source| self.fe_topology.registry().source(source).unwrap().n_levels()).collect::<Vec<_>>(),
            requested_channels.iter().filter(|&&requested| requested).count(), rayon::current_num_threads());
'''+a
assert s.count(a)==1;s=s.replace(a,b);p.write_text(s)
changed={p:sha(source/p) for p,h in manifest.items() if sha(source/p)!=h}
assert len(changed)==3,changed
(root/'identities.json').write_text(json.dumps({'accepted_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip(),'base_plugin':sha(previous/'candidate-v2/dist/fereg_macos.plugin'),'base_inputs':manifest,'diagnostic_changes':changed},indent=2)+'\n')
patch=b''
for p in changed:
 r=subprocess.run(['diff','-u','--label','a/'+p,'--label','b/'+p,str(previous/'candidate-v2'/p),str(source/p)],capture_output=True);assert r.returncode==1;patch+=r.stdout
(root/'diagnostic.patch').write_bytes(patch)
(root/'protocol.md').write_text('''# Saved output admission and direct planner cost diagnosis

This diagnostic preserves every numerical decision in candidate 247f889a. It logs existing joint budget checks with call sites, times the FE-scale recovery planner excluding logging, and reuses the previous recovery allocator window. It adds no production feature and is excluded from uninstrumented performance aggregates.

After both-architecture build and native ABI checks, run the exact original unit saved and PW partial-saved requests, plus the previous low FW control. Require full scientific exports and non-time solver fields to match the existing candidate receipts. Compare the actually checked execution-plus-increment budgets with native peaks; public selected bytes remain a base, not a complete route report. Preserve all diagnostic overhead and flagged earlier timings.

If the trace establishes a specific initial joint boundary, use the same fixed input for an exact/one-byte-short artificial-cap diagnostic to verify admission or explicit fallback before allocation. Do not infer universal bounds or close broader P0 from three requests. No new seeds or ordinary A/B timing repeats are authorized by this diagnostic protocol.
''')
print(json.dumps(changed,indent=2))
