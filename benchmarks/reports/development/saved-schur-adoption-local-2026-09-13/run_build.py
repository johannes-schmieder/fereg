import os
from pathlib import Path
import os,subprocess,hashlib,json
root=Path(__file__).resolve().parent;source=(root/'candidate').resolve();repo=Path(os.environ['REPOSITORY_ROOT']).resolve()
assert source!=repo and source.parent==root and source.name=='candidate'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
expected='4187efdfbd6235e63c369d9b42dede6381b8a9e50b5360fd63ee237bd6c11b08';assert sha(repo/'dist/fereg_macos.plugin')==expected
ids=json.loads((root/'prebuild-identities.json').read_text());assert all(sha(source/p)==h for p,h in ids['candidate_inputs'].items())
env=os.environ.copy();env.update(RUSTUP_TOOLCHAIN='1.85.1-aarch64-apple-darwin',CARGO_NET_OFFLINE='true',CARGO_TARGET_DIR=os.path.join(os.environ['NATIVE_BUILD_ROOT'], 'build-target'),FEREG_GIT_COMMIT='bc5a6ae88f9d',FEREG_BUILD_DATE='2026-09-13')
for file in ['crates/fereg-core/src/lib.rs','crates/fereg-plugin/src/lib.rs']:os.utime(source/file,None)
with (root/'build-isolated.log').open('wb') as log:
 p=subprocess.run(['bash',str(source/'scripts/build_macos.sh')],cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT)
assert sha(repo/'dist/fereg_macos.plugin')==expected
assert p.returncode==0,p.returncode
log=(root/'build-isolated.log').read_text();assert log.count('Compiling fereg-core')==2 and log.count('Compiling fereg-plugin')==2
assert os.path.join(os.environ['REPOSITORY_ROOT'], 'crates') not in log
assert 'actual-plugin ABI/self-test harness: PASS' in log
ids['candidate_plugin']=sha(source/'dist/fereg_macos.plugin');ids['protocol']=sha(root/'protocol.md');(root/'built-identities.json').write_text(json.dumps(ids,indent=2)+'\n')
print('Isolated build PASS',ids['candidate_plugin'],flush=True)
