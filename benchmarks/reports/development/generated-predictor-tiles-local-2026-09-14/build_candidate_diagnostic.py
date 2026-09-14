import os
from pathlib import Path
import os,subprocess,json,hashlib
r=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();ids=json.loads((r/'diagnostic-identities.json').read_text());primary=json.loads((r/'built-identities.json').read_text());repo=Path(os.environ['REPOSITORY_ROOT']);assert sha(repo/'dist/fereg_macos.plugin')==primary['installed_dist'];records=[]
for arm in ['candidate']:
 source=(r/('diagnostic-'+arm)).resolve();assert source.parent==r.resolve();assert all(sha(source/p)==h for p,h in ids[arm]['inputs'].items());env=os.environ.copy();env.update(RUSTUP_TOOLCHAIN='1.85.1-aarch64-apple-darwin',CARGO_NET_OFFLINE='true',CARGO_TARGET_DIR=os.environ['ISOLATED_BUILD_TARGET'],FEREG_GIT_COMMIT='ef38dd5',FEREG_BUILD_DATE='2026-09-14')
 for file in ['crates/fereg-core/src/lib.rs','crates/fereg-plugin/src/lib.rs']:os.utime(source/file,None)
 with (r/('diagnostic-'+arm+'-build.log')).open('wb') as log:p=subprocess.run(['bash',str(source/'scripts/build_macos.sh')],cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT)
 txt=(r/('diagnostic-'+arm+'-build.log')).read_text();assert p.returncode==0;assert txt.count('Compiling fereg-core')==2 and txt.count('Compiling fereg-plugin')==2;assert 'actual-plugin ABI/self-test harness: PASS' in txt;assert sha(repo/'dist/fereg_macos.plugin')==primary['installed_dist'];ids[arm]['plugin']=sha(source/'dist/fereg_macos.plugin');ids[arm]['build_log_sha256']=sha(r/('diagnostic-'+arm+'-build.log'));(r/'diagnostic-built.json').write_text(json.dumps(ids,indent=2)+'\n');print(arm,ids[arm]['plugin'],flush=True)
print('PREDICTOR_DIAGNOSTIC_BUILDS_TERMINAL_PASS',flush=True)
