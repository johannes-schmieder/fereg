import os
from pathlib import Path
import shutil,json,hashlib,subprocess
r=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'built-identities.json').read_text());records=json.loads((r/'diagnostic-built.json').read_text())
for arm,key in [('candidate','candidate_inputs')]:
 src=r/arm;dest=r/('diagnostic-'+arm);assert all(sha(src/p)==h for p,h in ids[key].items());shutil.copytree(src,dest,ignore=shutil.ignore_patterns("target","dist"));rel='crates/fereg-core/src/ols/linear_session.rs';p=dest/rel;s=p.read_text();start=s.index('    fn rebuild_predictors(\n');end=s.index('\n    #[cfg(test)]\n    fn root_solver_weight',start);part=s[start:end];needle='        let rows = self.response.len();';assert part.count(needle)==1;part=part.replace(needle,'''        let trace = std::env::var_os("FEREG_PHASE_TIMING").is_some();
        let total_started = Instant::now();
        let rows = self.response.len();''',1)
 needle='''            "unable to reserve weighted-linear absorbed predictor",
        )?;''';assert part.count(needle)==1;part=part.replace(needle,needle+'''
        let initialize_seconds = total_started.elapsed().as_secs_f64();
        let accumulate_started = Instant::now();''',1)
 needle='        let mut certificate = LinearReconstructionCertificate::new();';assert part.count(needle)==1;part=part.replace(needle,'''        let accumulate_seconds = accumulate_started.elapsed().as_secs_f64();
        let certificate_started = Instant::now();
'''+needle,1)
 needle='        Ok(certificate)';assert part.count(needle)==1;part=part.replace(needle,'''        if trace {
            eprintln!("FEREG_PHASE|retained_predictor|rows={rows}|generated={}|kept={}|initialize={initialize_seconds:.9}|accumulate={accumulate_seconds:.9}|certificate={:.9}|total={:.9}", self.prepared.fit_model().columns.len(), fit.finite_rank(), certificate_started.elapsed().as_secs_f64(), total_started.elapsed().as_secs_f64());
        }
'''+needle,1)
 s=s[:start]+part+s[end:];p.write_text(s);diff=subprocess.run(['diff','-u','--label','a/'+rel,'--label','b/'+rel,str(src/rel),str(p)],capture_output=True);assert diff.returncode==1;(r/('diagnostic-'+arm+'.patch')).write_bytes(diff.stdout);records[arm]={'inputs':{p:sha(dest/p) for p in ids[key]},'original_plugin':ids[arm+'_plugin'],'source_patch_sha256':sha(r/('diagnostic-'+arm+'.patch'))}
(r/'diagnostic-identities.json').write_text(json.dumps(records,indent=2)+'\n');print('DIAGNOSTIC_SNAPSHOTS_PASS')
