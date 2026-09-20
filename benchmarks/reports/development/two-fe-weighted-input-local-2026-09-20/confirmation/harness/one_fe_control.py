"""Check that narrow two-FE admission does not slow the one-FE projection route."""
import argparse
import json
import os
from pathlib import Path
import sys

from run import REPO, breadth, load, profile, write


def main(root, candidate, suffix=''):
    root = root.resolve()
    sys.path.insert(0,str(root/'controller'))
    measure = load('one_fe_control_measure',root/'controller/measure.py').measure
    manifest = json.loads((REPO/'benchmarks/systematic/manifest.json').read_text())
    manifest['measurement'].update(command_timeout_seconds=240,rss_interval_seconds=.05)
    os.environ['ONE_FE_BREADTH_OUTPUTS']='none'
    for label,k,weight in [('onefe-unit',1,'none'),('onefe-aw',6,'aw')]:
        if suffix:
            label += '-' + suffix
        parent = 'small-unit-retry' if weight=='none' else 'weighted-aw'
        previous = root/'runs'/f'{parent}-iid-n983040-k{k}-s8675309-shuffle0-t16-r1-densewin'
        request = json.loads((previous/'request.json').read_text())
        shared = json.loads((previous/'result.json').read_text())['measurement_fingerprint']
        shared.pop('native')
        shared['harness_sha256'] = breadth.sha(Path(__file__))
        cell = dict(request['cell'],design='one_fe_control_on_shared_dense_input',
                    effects=['g1'],absorb='g1',exact_dof=False)
        checks = []
        for rep in range(1,4):
            variants = ['densewin',candidate] if rep%2 else [candidate,'densewin']
            outputs = {}
            for variant in variants:
                identity = json.loads((root/f'identity-{variant}.json').read_text())
                source = root/variant
                assert breadth.sha(source/'dist/fereg_macos.plugin')==identity['plugin']
                output = root/'runs'/f'{label}-iid-n983040-k{k}-s8675309-shuffle0-t16-r{rep}-{variant}'
                result = measure(root,cell,'fereg','warm',Path(request['data']),Path(request['warm']),
                                 output,manifest,identity=dict(shared,native=identity),source=source)
                print(output.name,result['status'],result['seconds'],flush=True)
                assert result['status']=='pass'
                outputs[variant]=output
            checks.append(dict(rep=rep,science=profile.compare(outputs['densewin'],outputs[candidate])))
            write(root/(label+'-progress.json'),checks)
        write(root/(label+'-complete.json'),dict(status='pass',checks=checks))
    print('ONE_FE_CONTROL_COMPLETE',flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('root',type=Path)
    parser.add_argument('candidate')
    parser.add_argument('--suffix',default='')
    args=parser.parse_args()
    main(args.root,args.candidate,args.suffix)
