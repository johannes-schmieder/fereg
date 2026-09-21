#!/bin/bash
set -euo pipefail
grep -qx OLS_PUBLICATION_ADMISSION_PASS ../native/native-tests.log
for arm in baseline source; do
 export CARGO_TARGET_DIR="$PWD/target-$arm"
 if [ "$arm" = baseline ]; then export FEREG_GIT_COMMIT=5240e589400c; else export FEREG_GIT_COMMIT=5240e58-ols-publication-admission1; fi
 bash "$arm/scripts/build_linux.sh" > "build-$arm.txt" 2>&1
done
python3 - <<'HASH'
from pathlib import Path
import json,hashlib
paths={'baseline':Path('baseline/dist/fereg_linux.plugin'),'candidate':Path('source/dist/fereg_linux.plugin')}
Path('artifacts.json').write_text(json.dumps({a:hashlib.sha256(p.read_bytes()).hexdigest() for a,p in paths.items()},indent=2)+'\n')
HASH
printf 'OLS_PUBLICATION_BUILDS_PASS\n' > build-success.txt
python3 harness/run-screen.py
