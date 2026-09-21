"""Archive portable development receipts; keep licensed logs and large arrays local."""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import re
import statistics
import zipfile


def digest(data):
    return hashlib.sha256(data).hexdigest()


def archive(root, output, selected_campaigns=None):
    root=root.resolve();output.mkdir(parents=True,exist_ok=False)
    repo=Path(__file__).resolve().parents[2]
    roots=sorted(p for p in root.iterdir() if p.is_dir() and (p/'plan.json').exists())
    if selected_campaigns is not None:
        assert set(selected_campaigns) <= {p.name for p in roots}
        roots=[p for p in roots if p.name in selected_campaigns]
    records=[];summary=[];campaigns=[];hashes={};omitted={}
    def portable(data):
        return data.replace(str(repo).encode(),b'<repository>').replace(
            b'/tmp/fereg-fixest-local-20260911/work/R',b'<reference-library>')
    with zipfile.ZipFile(output/'evidence.zip','w',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as bundle:
        def add(path):
            name=str(path.relative_to(root));data=path.read_bytes();clean=portable(data)
            hashes[name]=dict(original_sha256=digest(data),portable_sha256=digest(clean))
            bundle.writestr(name,clean)
        for path in sorted(root.glob('identity-*.json')):add(path)
        for pattern in ('slope-unit*.txt','slope-fit-unit*.txt','slope-loader-*.txt','fixtures-*.json','direct-recovery-*.txt','final-check-*.txt','recovery*-slope-unit*.txt','recovery*-spi.txt'):
            for path in sorted(root.glob(pattern)):add(path)
        for folder in roots:
            controller_log=root/(folder.name+'-controller.txt')
            if controller_log.exists():add(controller_log)
            plan=json.loads((folder/'plan.json').read_text())
            completion=json.loads((folder/'complete.json').read_text()) if (folder/'complete.json').exists() else None
            campaigns.append(dict(name=folder.name,plan_sha256=digest((folder/'plan.json').read_bytes()),
                                  completion=completion,headline=folder.name.endswith('-primary')))
            for path in sorted(folder.glob('*.json')):add(path)
            for path in sorted((folder/'data').glob('*.json')):add(path)
            controller=folder/'controller'
            for path in sorted(controller.glob('*')):
                if path.is_file() and path.suffix in ('.py','.R','.do','.json'):add(path)
            for run in sorted((folder/'runs').glob('*')):
                if not run.is_dir():continue
                for path in sorted(run.iterdir()):
                    if path.suffix in ('.json','.csv'):add(path)
                phases=[]
                for path in run.glob('*.txt'):
                    phases += [line for line in path.read_text(errors='replace').splitlines()
                               if line.startswith(('SLOPE_PROFILE|','FEREG_PHASE|'))]
                if phases:bundle.writestr(str(run.relative_to(root))+'/phases.json',json.dumps(phases,indent=2)+'\n')
                for path in run.glob('outputs.bin'):
                    omitted[str(path.relative_to(root))]=dict(sha256=digest(path.read_bytes()),bytes=path.stat().st_size)
                if not (run/'result.json').exists():continue
                result=json.loads((run/'result.json').read_text())
                native=result['measurement_fingerprint'].get('native')
                variant='fixest' if native is None else run.name.split('-r',1)[1].split('-',1)[1]
                record=dict(campaign=folder.name,run=run.name,variant=variant,cell=result['cell'],
                    status=result['status'],seconds=result['seconds'],sentinel=result['sentinel'],
                    process_returncode=result['process_returncode'],input_sha256=result['input_sha256'],
                    sample_sha256=result.get('sample_sha256'),plugin=None if native is None else native['plugin'])
                if (run/'fereg_runtime.csv').exists():
                    with (run/'fereg_runtime.csv').open() as stream:
                        runtime={int(r['row']):float(r['value']) for r in csv.DictReader(stream) if int(r['column'])==2}
                    record.update(input_seconds=runtime[19],native_seconds=runtime[20],allocation_peak_bytes=runtime[13])
                records.append(record)
        for folder in roots:
            if not folder.name.endswith('-primary'):continue
            current=[r for r in records if r['campaign']==folder.name and r['status']=='pass']
            def cell_key(r):
                c=r['cell']
                return (c['k'],c['threads'],c.get('weight_kind','none'),c.get('case','iid'),c.get('intercept',True),len(c.get('slope_names',['z'])))
            for k,t,weight,case,intercept,slopes in sorted({cell_key(r) for r in current}):
                cell=[r for r in current if cell_key(r)==(k,t,weight,case,intercept,slopes)]
                reference={re.search(r'-r(\d+)-',r['run']).group(1):r['seconds'] for r in cell if r['variant']=='fixest'}
                variants={}
                for variant in sorted({r['variant'] for r in cell}):
                    rows=[r for r in cell if r['variant']==variant]
                    values=[r['seconds'] for r in rows]
                    ratios=[r['seconds']/reference[re.search(r'-r(\d+)-',r['run']).group(1)] for r in rows] if reference else []
                    variants[variant]=dict(seconds=values,median=statistics.median(values),
                        paired_ratios=ratios,median_paired_ratio=statistics.median(ratios) if ratios else None)
                summary.append(dict(campaign=folder.name,k=k,threads=t,weight_kind=weight,case=case,intercept=intercept,slopes=slopes,variants=variants))
    for name,data in [('measurements',records),('summary',summary),('campaigns',campaigns),('archive-hashes',hashes),('local-output-hashes',omitted)]:
        (output/(name+'.json')).write_bytes(portable((json.dumps(data,indent=2,sort_keys=True)+'\n').encode()))
    print('SLOPE_ARCHIVE_COMPLETE',len(records),len(roots),flush=True)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('root',type=Path);p.add_argument('output',type=Path)
    p.add_argument('--campaigns',nargs='+')
    a=p.parse_args();archive(a.root,a.output,a.campaigns)
