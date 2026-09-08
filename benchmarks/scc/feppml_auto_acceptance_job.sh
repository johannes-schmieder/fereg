#!/usr/bin/env bash
#$ -P welfgr
#$ -N feppml_auto_science
#$ -pe omp28 28
#$ -l h_rt=02:00:00
#$ -l mem_per_core=3G
#$ -l m_core=28
#$ -l num_proc=28
#$ -j y
#$ -m a
source /etc/bashrc
set -euo pipefail
: "${RUN_ROOT:?RUN_ROOT is required}"
[[ "$RUN_ROOT" =~ ^/projectnb/welfgr/fereg/dev/[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$ ]]
[[ "${NSLOTS:-}" == 28 && "${JOB_ID:-}" =~ ^[1-9][0-9]*$ ]]
[[ "${SGE_TASK_ID:-}" =~ ^[1-9][0-9]*$ ]]
module --force purge
module load stata-mp/19 python3/3.12.4
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
export RAYON_NUM_THREADS="$NSLOTS"
task_name=$(printf 'task-%03d' "$SGE_TASK_ID")
qhost -h "$HOSTNAME" -F m_core,cpu_arch,cpu_type > "$RUN_ROOT/logs/qhost-$task_name.txt"
python3 - "$RUN_ROOT" "$SGE_TASK_ID" "$(command -v stata-mp)" <<'PY'
import hashlib, json, os, re, subprocess, sys
from pathlib import Path
root = Path(sys.argv[1])
task = int(sys.argv[2])
config = json.loads((root/'input/acceptance-tasks.json').read_text())
if task not in config['pending_tasks']:
    raise SystemExit('task not in frozen pending inventory')
if hashlib.sha256((root/'input/main-job.sh').read_bytes()).hexdigest() != config['wrapper_sha256']:
    raise SystemExit('wrapper changed')
entry = config['tasks'][str(task)]
host_text = (root/f'logs/qhost-task-{task:03d}.txt').read_text()
cores = re.findall(r'(?:^|[:\s])m_core=([^\s]+)', host_text, re.MULTILINE)
if len(set(cores)) != 1 or float(cores[0]) != 28:
    raise SystemExit('unexpected physical core count')
plugin = root/'source/dist/fereg_linux.plugin'
if hashlib.sha256(plugin.read_bytes()).hexdigest() != config['linux_plugin_sha256']:
    raise SystemExit('smoke-tested plugin changed')
output = root/f'work/acceptance/task-{task:03d}'
command = [sys.executable, str(root/'source/scripts/ppml_auto_acceptance.py'),
    '--task', str(task), '--candidate', str(root/'source'),
    '--source-identity', str(root/'input/source-files.json'), '--plugin', str(plugin),
    '--dataset', entry['dataset'], '--dataset-sha256', entry['sha256'],
    '--stata', sys.argv[3], '--output', str(output)]
subprocess.run(command, check=True)
with (output/'scheduler-binding.json').open('x') as stream:
    json.dump({'job_id': os.environ['JOB_ID'], 'task_id': task, 'slots': 28,
               'host': os.environ['HOSTNAME'],
               'configuration_sha256': hashlib.sha256((root/'input/acceptance-tasks.json').read_bytes()).hexdigest(),
               'wrapper_sha256': config['wrapper_sha256']}, stream, indent=2)
print('ACTUAL_AUTO_SCC_TASK_PASS', task)
PY
