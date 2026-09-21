#!/bin/bash
set -euo pipefail
grep -qx OLS_PREPARED_PARENT_PASS ../native2/native-tests.log
export CARGO_TARGET_DIR="$PWD/target-source"
export FEREG_GIT_COMMIT=5240e58-ols-prepared-parent2
(cd source && bash scripts/build_linux.sh) > build-source.txt 2>&1
python3 - <<'HASH'
from pathlib import Path
import json,hashlib
paths={'baseline':Path('baseline/dist/fereg_linux.plugin'),'candidate':Path('source/dist/fereg_linux.plugin')}
artifacts={a:hashlib.sha256(p.read_bytes()).hexdigest() for a,p in paths.items()}
assert artifacts['baseline']=='dd3d00eee8e114a0c325ae14b6d44601eb0d1a8394ec267bbba297067b3bfac4'
Path('artifacts.json').write_text(json.dumps(artifacts,indent=2)+'\n')
HASH
printf 'OLS_PARENT_BUILD_PASS\n' > build-success.txt
python3 harness/generate-singletons.py
python3 harness/run-screen.py
