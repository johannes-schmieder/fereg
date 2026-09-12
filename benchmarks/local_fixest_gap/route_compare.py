"""Focused local Schur-route comparisons; sequential estimator processes only."""
import argparse
import copy
from pathlib import Path

from common import sha256, write_json
from final_compare import invoke, baseline_compare, certify
from measure import measure
from route_holdouts import custom
from run import BASE, MANIFEST, compare


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--part', required=True, choices=['primary', 'holdout', 'narrow'])
    parser.add_argument('--baseline', required=True, type=Path)
    parser.add_argument('--source', type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    if args.part == 'primary':
        for threads in [4, 16]:
            for design in ['one_fe', 'strong_d3', 'strong_d6']:
                for rep, seed in enumerate([104729, 8675309, 20260819]):
                    arms = [('baseline', 'fereg', args.baseline),
                            ('candidate', 'fereg', args.source),
                            ('fixest', 'fixest', args.baseline)]
                    arms = arms[rep % 3:] + arms[:rep % 3]
                    paths = {name: invoke('final-' + name, design, 983040, 50,
                             threads, seed, arm, source)
                             for name, arm, source in arms}
                    print('SCIENCE', compare(paths['candidate'], paths['fixest']), flush=True)
                    print('BASELINE', baseline_compare(paths['candidate'], paths['baseline']), flush=True)
    elif args.part == 'narrow':
        for design in ['strong_d3', 'weak_d3']:
            for k in [1, 10]:
                paths = {name: invoke('final-narrow-' + name, design, 61440, k,
                         4, 59047, 'fereg', source)
                         for name, source in [('candidate', args.source), ('baseline', args.baseline)]}
                print('BASELINE', baseline_compare(paths['candidate'], paths['baseline']), flush=True)
    else:
        for threads in [4, 16]:
            for design, n in [('weak_d3', 61440), ('two_fe_dense', 245760)]:
                paths = {name: invoke('final-holdout-' + name, design, n, 50,
                         threads, 59047, arm, source, trace=True)
                         for name, arm, source in [('baseline', 'fereg', args.baseline),
                            ('candidate', 'fereg', args.source), ('fixest', 'fixest', args.baseline)]}
                print('BASELINE', baseline_compare(paths['candidate'], paths['baseline']), flush=True)
                print('SCIENCE', compare(paths['candidate'], paths['fixest']), flush=True)
        for design in ['bottleneck_ring', 'disconnected_blocks', 'unbalanced', 'cycle']:
            data = custom(design, 61440, 50, 59047)
            warm = custom(design, 7680, 50, 59048)
            cell = {'model': 'ols', 'design': design, 'n': 61440, 'k': 50,
                    'threads': 4, 'section': 'width', 'effects': ['g1', 'g2']}
            paths = {}
            for label, source in [('candidate', args.source), ('baseline', args.baseline)]:
                path = BASE / 'runs' / f'final-holdout-{label}-{design}-n61440-k50-t4-s59047-fereg'
                paths[label] = path
                spec = copy.deepcopy(MANIFEST)
                spec['measurement']['command_timeout_seconds'] = 45
                identity = {'candidate_base_commit': MANIFEST['candidate_commit'],
                            'variant': source.name,
                            'plugin_sha256': sha256(source / 'dist/fereg_macos.plugin'),
                            'trace': True, 'solver': 'auto', 'seed': 59047}
                result = measure(BASE, cell, 'fereg', 'warm', data, warm, path,
                                 spec, identity=identity, source=source, trace=True)
                print(path.name, result['status'], result['seconds'], flush=True)
                if result['status'] == 'pass':
                    write_json(path / 'native-science.json', certify(path))
            print('BASELINE', baseline_compare(paths['candidate'], paths['baseline']), flush=True)


if __name__ == '__main__':
    main()
