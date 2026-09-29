#!/usr/bin/env python3
"""Validate scalar/array accounting and combine compact SCC summaries."""
import argparse
import csv
import io
import json
from pathlib import Path
import tomllib


def accounting(text, expected_tasks):
    records = []
    current = {}
    for line in text.splitlines():
        if line.startswith('====='):
            if current:
                records.append(current)
            current = {}
        elif line.strip():
            parts=line.split(None,1)
            if len(parts)==2:
                current[parts[0]]=parts[1].strip()
    if current:
        records.append(current)
    if len(records)!=expected_tasks:
        raise ValueError('accounting is missing, duplicate or not yet complete')
    ids=[]
    for record in records:
        if record.get('failed')!='0' or record.get('exit_status')!='0':
            raise ValueError('SGE reports a failed task')
        if record.get('slots')!='28':
            raise ValueError('task did not reserve the declared whole node')
        ids.append(record.get('taskid','undefined'))
    if expected_tasks>1 and set(ids)!={str(i) for i in range(1,expected_tasks+1)}:
        raise ValueError('array accounting does not cover every task exactly once')
    return records


def expected_groups(suite, tier, cell_name="all", thread_name="all"):
    spec=suite['tiers'][tier]
    cells=[{**suite['defaults'],**cell} for cell in suite['cells']]
    cells=[cell for cell in cells if (not spec.get('cells') or cell['id'] in spec['cells'])
           and (not spec.get('non_scc') or not cell['scc'])]
    result=set()
    for cell in cells:
        if cell_name!='all' and cell['id']!=cell_name:
            continue
        threads=set(spec['threads']+(spec.get('ladder_threads',[]) if cell['id'] in spec.get('ladder_cells',[]) else []))
        if thread_name!='all':
            threads &= {int(thread_name)}
        result.update((cell['id'],str(thread),arm) for thread in threads for arm in suite['comparators'][cell['model']])
    if not result:
        raise ValueError('task is outside the declared suite')
    return result


def combine(root):
    jobs=list(csv.reader(io.StringIO((root/'jobs.tsv').read_text()),delimiter='\t'))
    for phase, job, tasks in jobs:
        accounting((root/f'{phase}-qacct.txt').read_text(),int(tasks))
    expected=int(next(row[2] for row in jobs if row[0]=='benchmark'))
    rows=[]
    suite=tomllib.loads((root/'suite.toml').read_text())
    manifest=[line.split() for line in (root/'tasks.tsv').read_text().splitlines()]
    source=json.loads((root/'source.json').read_text())
    if len(manifest)!=expected:
        raise ValueError('task manifest and scheduler task counts differ')
    signals=(root/'completion.txt').read_text().splitlines()
    if sum(line.startswith('FEREG_BENCH_COMPLETE ') for line in signals)!=expected:
        raise ValueError('application completion signals are missing or duplicate')
    for task in range(1,expected+1):
        path=root/'summaries'/f'task-{task}.csv'
        part=list(csv.DictReader(io.StringIO(path.read_text())))
        if not part:
            raise ValueError('empty task summary')
        actual={(r['cell'],r['threads'],r['arm']) for r in part}
        required=expected_groups(suite,part[0]['tier'],*manifest[task-1])
        if actual!=required or len(part)!=len(required):
            raise ValueError('task summary misses or duplicates declared groups')
        rows.extend(part)
    identities={(r['source_sha'],r['dirty'],r['tier'],r['platform'],r['engine'],r['solver']) for r in rows}
    if len(identities)!=1 or any(r['dirty']!='False' for r in rows):
        raise ValueError('task source/platform/policy identities disagree')
    if any(r['source_sha']!=source['commit'] for r in rows):
        raise ValueError('summary source differs from the staged commit')
    keys=[(r['cell'],r['threads'],r['arm']) for r in rows]
    if len(set(keys))!=len(keys):
        raise ValueError('duplicate benchmark group')
    if any(r['tier']=='smoke' and (r['status']!='ok' or r['accuracy']!='pass' or r['repetitions']!='5') for r in rows):
        raise ValueError('SCC smoke did not pass every command and accuracy check')
    if set(keys)==expected_groups(suite,rows[0]['tier']):
        for row in rows:
            row['partial']=False
    buffer=io.StringIO()
    writer=csv.DictWriter(buffer,fieldnames=list(rows[0]),lineterminator='\n')
    writer.writeheader()
    writer.writerows(rows)
    if len(buffer.getvalue().encode())>100000:
        raise ValueError('combined summary exceeds 100 KB')
    return buffer.getvalue()


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('root',type=Path)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    summary=combine(args.root)
    with args.output.open('x') as stream:
        stream.write(summary)
    print('SCC_BENCH_COLLECTED',args.output)
