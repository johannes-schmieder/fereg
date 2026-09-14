import os
from pathlib import Path
import shutil,json,hashlib,subprocess
r=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'built-identities.json').read_text());records={}
for arm,key in [('baseline','inputs'),('candidate','candidate_inputs')]:
 src=r/arm;dest=r/('diagnostic-'+arm);assert all(sha(src/p)==h for p,h in ids[key].items());shutil.copytree(src,dest)
 rel='crates/fereg-core/src/ppml/memory.rs';p=dest/rel;s=p.read_text();start=s.index('    pub(crate) fn refresh_working_state(\n');end=s.index('\n    /// Reuse the working-response buffer',start);part=s[start:end]
 needle='        if self.mean_bounds != Some(bounds) {' if arm=='candidate' else '        means_from_eta(&self.eta, bounds, &mut self.mean)?;'
 assert part.count(needle)==1
 prefix='        let mean_reused = self.mean_bounds == Some(bounds);\n' if arm=='candidate' else '        let mean_reused = false;\n'
 part=part.replace(needle,prefix+'        let mean_started = std::time::Instant::now();\n'+needle,1)
 needle='        fill_poisson_working_state('
 assert part.count(needle)==1
 part=part.replace(needle,'''        let mean_seconds = mean_started.elapsed().as_secs_f64();
        if std::env::var_os("FEREG_PHASE_TIMING").is_some() {
            eprintln!("FEREG_PHASE|mean_refresh|rows={}|reused={}|seconds={mean_seconds:.9}|workspace_size={}|bounds_option_size={}", self.eta.len(), u8::from(mean_reused), std::mem::size_of::<Self>(), std::mem::size_of::<Option<PoissonBounds>>());
        }
'''+needle,1)
 p.write_text(s[:start]+part+s[end:]);diff=subprocess.run(['diff','-u','--label','a/'+rel,'--label','b/'+rel,str(src/rel),str(p)],capture_output=True);assert diff.returncode==1;(r/('diagnostic-'+arm+'.patch')).write_bytes(diff.stdout)
 records[arm]={'inputs':{p:sha(dest/p) for p in ids[key]},'original_plugin':ids[arm+'_plugin'],'source_patch_sha256':sha(r/('diagnostic-'+arm+'.patch'))}
(r/'diagnostic-identities.json').write_text(json.dumps(records,indent=2)+'\n');print('MEAN_DIAGNOSTIC_SNAPSHOTS_PASS')
