"""Trace retained pass optimizations after the untraced primary comparison."""
import argparse
from pathlib import Path
from final_compare import BASE, baseline_compare, invoke


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument('--part', choices=['phase', 'narrow-repeat', 'outlier-repeat'], default='phase')
    parser.add_argument('--baseline', type=Path)
    args = parser.parse_args()
    if args.part != 'phase':
        if args.baseline is None:
            parser.error('--baseline is required for paired repeats')
        if args.part == 'narrow-repeat':
            cases = [('final-narrow-repeat', 'weak_d3', 61440, 10, 4, seed)
                     for seed in [59047, 59059, 59063]]
        else:
            cases = [('final-outlier-' + str(rep), 'one_fe', 983040, 50, 16, 8675309)
                     for rep in range(2)]
        for rep, (label, design, n, k, threads, seed) in enumerate(cases):
            arms = [('baseline', args.baseline), ('candidate', args.source)]
            if rep % 2:
                arms.reverse()
            paths = {name: invoke(label + '-' + name, design, n, k, threads,
                     seed, 'fereg', source, trace=True) for name, source in arms}
            print('BASELINE', baseline_compare(paths['candidate'], paths['baseline']), flush=True)
        return
    for threads in [4, 16]:
        for design in ['one_fe', 'strong_d3', 'strong_d6']:
            path = invoke('final-phase-candidate', design, 983040, 50, threads,
                          104729, 'fereg', args.source, trace=True)
            primary = BASE / 'runs' / f'final-candidate-{design}-n983040-k50-t{threads}-s104729-fereg'
            print('TRACE PARITY', baseline_compare(path, primary), flush=True)


if __name__ == '__main__':
    main()
