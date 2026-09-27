#!/usr/bin/env python3
"""Run one immutable MC range, retaining failed attempts as observations."""
import argparse
import csv
import json
from pathlib import Path
from run_stata_tests import run_fixture, sha256_file


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--work',type=Path,required=True)
    p.add_argument('--out',type=Path,required=True)
    p.add_argument('--engine',choices=['native','legacy'],required=True)
    p.add_argument('--scenario',required=True)
    p.add_argument('--first',type=int,required=True)
    p.add_argument('--last',type=int,required=True)
    p.add_argument('--method',choices=['probe','exact'],default='probe')
    p.add_argument('--probes',type=int,default=256)
    p.add_argument('--stata',required=True)
    p.add_argument('--source-manifest',type=Path,required=True)
    p.add_argument('--plugin',type=Path,required=True)
    args=p.parse_args()
    root=Path(__file__).resolve().parents[1];work=args.work.resolve();out=args.out.resolve()
    (out/'empty').mkdir(parents=True,exist_ok=False)
    fixture=out/'bias_mc.do'
    fixture.write_text(f'''version 18.0
do "{work}/mc_{args.engine}.do" "{work}/runtime" {args.scenario} {args.first} {args.last} "{out}" {args.method} rust {args.probes} jla 512
''')
    result=run_fixture(args.stata,fixture,root,plugin=args.plugin,
                       plugin_sha256=sha256_file(args.plugin),log_dir=out/'logs',require_status=True)
    receipt=dict(schema=1,source=json.loads(args.source_manifest.read_text()),engine=args.engine,scenario=args.scenario,first=args.first,last=args.last,
                 method=args.method,probes=args.probes,plugin_sha256=sha256_file(args.plugin),
                 adapter_manifest_sha256=sha256_file(work/'manifest.json'),stata=result.metadata,
                 stata_rc=result.stata_rc,error=result.error)
    if result.error or result.stata_rc:
        receipt['completed']=False
        (out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
        print(result.log_text[-12000:]);raise SystemExit(1)
    rows=list(csv.DictReader((out/'attempts.csv').open()))
    assert [int(row['rep']) for row in rows]==list(range(args.first,args.last+1))
    receipt.update(completed=True,attempts=len(rows),failures=sum(int(r['rc'])!=0 for r in rows),
                   attempts_sha256=sha256_file(out/'attempts.csv'),moments_sha256=sha256_file(out/'moments.csv'))
    (out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps(receipt,indent=2))

if __name__=='__main__':
    main()
