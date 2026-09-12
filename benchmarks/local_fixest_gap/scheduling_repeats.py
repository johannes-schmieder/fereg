"""Bounded checks of the recurrence crossover and a small one-FE timing difference."""
import argparse
from pathlib import Path
from final_compare import invoke, baseline_compare
from run import compare


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--source', type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument('--part', choices=['one-fe-repeat', 'thread-eight'], required=True)
    args = parser.parse_args()
    if args.part == 'one-fe-repeat':
        for rep in range(3):
            arms = [('baseline', args.baseline), ('candidate', args.source)]
            if rep % 2:
                arms.reverse()
            paths = {name: invoke(f'final-one-fe-repeat{rep}-{name}', 'one_fe',
                     983040, 50, 4, 104729, 'fereg', source, trace=True)
                     for name, source in arms}
            print('BASELINE', baseline_compare(paths['candidate'], paths['baseline']), flush=True)
    else:
        for design in ['strong_d3', 'strong_d6']:
            paths = {name: invoke('final-thread-eight-' + name, design, 983040, 50,
                     8, 104729, arm, source, trace=True) for name, arm, source in
                     [('baseline', 'fereg', args.baseline), ('candidate', 'fereg', args.source),
                      ('fixest', 'fixest', args.baseline)]}
            print('BASELINE', baseline_compare(paths['candidate'], paths['baseline']), flush=True)
            print('SCIENCE', compare(paths['candidate'], paths['fixest']), flush=True)


if __name__ == '__main__':
    main()
