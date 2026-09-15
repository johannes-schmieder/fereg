from pathlib import Path
import csv,json,hashlib,re
import pandas as pd
import numpy as np
root=Path(__file__).resolve().parent
p=root/'full-fast-attempt1'
comp=json.loads((p/'completion.json').read_text());assert comp['accepted'] and comp['process_rc']==0
assert (p/'result.txt').read_text().strip()=='STATA_RC=0'
fit=dict(line.split('=',1)for line in(p/'fit-result.txt').read_text().splitlines());assert fit['FIT_RC']=='0'
assert re.search(r'^FULL_GERMAN_PPML_COMPLETE$',(p/'run.log').read_text(),re.M)
s={k:float(v)if v.strip()!='.'else None for k,v in csv.reader((p/'scalars.tsv').open(),delimiter='\t')}
for key,value in {'N':88876264,'N_full':91045290,'num_singletons':2169026,'converged':1,'feppml_psd_passed':1,'feppml_two_consecutive':1,'feppml_rank_frozen':1,'feppml_strict_profile_rejections':0,'feppml_fallback_count':0,'feppml_solver_fallback_count':0}.items():assert s[key]==value,(key,s[key])
cert=pd.read_csv(p/'feppml_certificate.tsv',sep='\t',index_col=0);assert cert.loc['composite_passed','value']==1
b=pd.read_csv(p/'b.tsv',sep='\t',index_col=0);v=pd.read_csv(p/'V.tsv',sep='\t',index_col=0);assert np.isfinite(b.to_numpy()).all()and np.isfinite(v.to_numpy()).all();assert list(b.columns)==list(v.columns)==list(v.index)
with pd.read_stata(p/'outputs.dta',iterator=True)as reader:
 first=reader.read(1)
 assert reader._nobs==91045290
 assert first.obs_id.iloc[0]==1
 assert list(first.columns)==['persid','betnr','year','logrwage2','age','edyrs','obs_id','absorbed','sample','fitted_mu']
h=hashlib.sha256()
with(p/'outputs.dta').open('rb')as f:
 for block in iter(lambda:f.read(8*1024**2),b''):h.update(block)
plugin=root/'combined-candidate/dist/fereg_macos.plugin';digest=hashlib.sha256(plugin.read_bytes()).hexdigest();assert digest=='156f3835f6c8349a336a48fb732ed6e4068e501710c4daa3c1e74012d424d31f'
receipt=dict(phase='terminal application/publication validation, before independent full-data audit',accepted_application_and_public_certificates=True,independent_full_data_audit_passed=False,source_commit='ece0db4e3167041ade3c817f7b48b118946643ae',plugin_sha256=digest,completion=comp,command_seconds=float(fit['COMMAND_SECONDS']),scalars=s,certificate=cert['value'].to_dict(),output_bytes=(p/'outputs.dta').stat().st_size,output_sha256=h.hexdigest(),header_input_rows=91045290,output_row_sample_mu_assertions='all-row Stata assertions before terminal sentinel; independent streamed replay pending',captured_outputs={x.name:hashlib.sha256(x.read_bytes()).hexdigest()for x in p.iterdir()if x.is_file()and x.suffix in['.tsv','.json','.ster']})
(root/'full-fast-terminal-check.json').write_text(json.dumps(receipt,indent=2,allow_nan=False)+'\n')
print('FULL_FAST_TERMINAL_PUBLICATION_CHECK_PASS',s['N'],s['ic'],digest,flush=True)
