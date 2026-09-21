"""Collect aggregate SCC receipts and accept completeness before plotting."""
import argparse
import json
import subprocess
from pathlib import Path
from accounting import parse
from common import read_json, write_json
from run_scc import ARMS, THREADS

ROOT=Path(__file__).resolve().parents[1]
REMOTE='/projectnb/welfgr/fereg/runs/20260911-veneto-d44e36b'


def main():
    parser=argparse.ArgumentParser();parser.add_argument('job_id',type=int);args=parser.parse_args()
    output=ROOT/'results/scc';download=ROOT/'work/scc-collected'
    download.mkdir(exist_ok=True)
    query=subprocess.run(['ssh','scc','qacct -j '+str(args.job_id)],capture_output=True,text=True,check=False)
    accounting=parse(query.stdout,args.job_id,range(1,25))
    write_json(output/'campaign-accounting.json',accounting)
    assert accounting['status']=='pass','Complete successful accounting is required before final report'
    subprocess.run(['rsync','-rt','scc:'+REMOTE+'/results/',str(download)+'/'],check=True)
    subprocess.run(['rsync','-rt','scc:'+REMOTE+'/logs/',str(download/'logs')+'/'],check=True)
    accepted=read_json(download/'smoke-accepted.json');assert accepted['status']=='pass'
    identity=accepted['runtime_fingerprint'];count=0
    for task_id in range(1,25):
        task=download/f'campaign/task-{task_id:02d}'
        complete=read_json(task/'COMPLETE.json')
        assert complete['runtime_fingerprint']==identity and complete['calls']==11
        assert complete['job_id']==str(args.job_id) and complete['task_id']==task_id
        logs=list((download/'logs').glob(f'*{args.job_id}.{task_id}'))
        assert len(logs)==1 and 'VENETO_SCC_TASK_COMPLETE' in logs[0].read_text()
        assert 'Traceback (most recent call last)' not in logs[0].read_text()
        rep=(task_id-1)//6;threads=THREADS[(task_id-1)%6]
        rows=[read_json(p) for p in task.glob('*-r*-*.json')]
        assert len(rows)==11
        assert {(r['cell']['model'],r['arm']) for r in rows}=={(m,a) for m,arms in ARMS.items() for a in arms}
        host=next(r['hostname'] for r in accounting['records'] if r['task_id']==task_id)
        for row in rows:
            assert row['repetition']==rep and row['cell']['threads']==threads
            assert row['measurement_fingerprint']==identity and row['job_id']==str(args.job_id)
            assert row['host'].split('.')[0]==host.split('.')[0]
            assert len(row['cpu_affinity'])==threads
            name=f"{row['cell']['model']}-t{threads}-r{rep}-{row['arm']}.json"
            write_json(output/'campaign'/name,row);count+=1
    manifest=read_json(download/'campaign/task-01/manifest.json')
    write_json(output/'campaign-manifest.json',manifest)
    write_json(output/'runtime.json',read_json(download/'campaign/task-01/runtime.json'))
    write_json(output/'smoke-accepted.json',accepted)
    write_json(output/'campaign-complete.json',{'calls':count,'runtime_fingerprint':identity,'job_id':str(args.job_id),'status':'pass'})
    print('Accepted',count,'SCC invocation receipts; ready for report_scc.py')


if __name__=='__main__':main()
