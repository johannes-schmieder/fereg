#!/bin/bash -l
#$ -P welfgr
#$ -N fereg_one_fe_cpu
#$ -pe omp 4
#$ -l cpu_type=E5-2680v4,mem_per_core=3G,h_rt=00:15:00
#$ -j y
set -euo pipefail
: "${RUN_ROOT:?}"
[[ "$RUN_ROOT" == /projectnb/welfgr/fereg/dev/20260928-one-fe-profile-01 && "$NSLOTS" == 4 ]]
module purge
module load python3/3.12.4 stata-mp/19
export PYTHONDONTWRITEBYTECODE=1 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
python3 - "$RUN_ROOT" <<'CHECK'
from pathlib import Path
import hashlib,json,sys
r=Path(sys.argv[1]);s=Path('/projectnb/welfgr/fereg/dev/20260928-publication-target-01/source')
for f,d in json.loads((r/'deployment.json').read_text()).items():assert hashlib.sha256((r/f).read_bytes()).hexdigest()==d,f
for f,d in json.loads((r/'source-manifest.json').read_text())['files'].items():assert hashlib.sha256((s/f).read_bytes()).hexdigest()==d,f
CHECK
reference=/projectnb/welfgr/fereg/runs/20260926-systematic-light-58284f76
"$reference/env/python/bin/python" "$RUN_ROOT/profile.py" --reference "$reference" --sources-root /projectnb/welfgr/fereg/dev
printf 'FEREG_ONE_FE_CPU_SUCCESS job=%s\n' "$JOB_ID"
