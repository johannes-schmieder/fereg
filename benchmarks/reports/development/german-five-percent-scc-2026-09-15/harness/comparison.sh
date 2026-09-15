#!/usr/bin/env bash
#$ -P welfgr
#$ -N fereg_german5_compare
#$ -pe omp 16
#$ -l h_rt=03:00:00
#$ -l mem_per_core=3G
#$ -j y
#$ -m a
set -eo pipefail
source /etc/bashrc
set -u
: "${RUN_ROOT:?}" "${FAMILY:?}"
case "$FAMILY" in ols|ppml) ;; *) exit 2;; esac
module --force purge
module load stata-mp/19
module load python3/3.12.4
export OMP_NUM_THREADS=8 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1 RAYON_NUM_THREADS=8
# Leave 8 GiB of the 48 GiB reservation outside native admission for Stata
# and the driver. Both timing arms otherwise have the same thread policy.
python3 - "$RUN_ROOT" <<'PY'
from pathlib import Path
import hashlib,json,sys
r=Path(sys.argv[1]);assert (r/'launcher-smoke-v2/runs.json').exists()
s=json.loads((r/'launcher-smoke-v2/runs.json').read_text());assert all(x['accepted'] for x in s['runs'])
m=json.loads((r/'input/sample-receipt.json').read_text());assert m['roundtrip_passed']
h=hashlib.sha256()
with (r/'input/sample.dta').open('rb') as f:
 for b in iter(lambda:f.read(8*1024**2),b''):h.update(b)
assert h.hexdigest()==m['sample_sha256']
PY
python3 "$RUN_ROOT/harness/run-comparison.py" --root "$RUN_ROOT" --input "$RUN_ROOT/input/sample.dta" --family "$FAMILY" --native-memory-bytes 42949672960
