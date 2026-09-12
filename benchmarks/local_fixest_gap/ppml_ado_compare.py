"""P1 PPML wrapper-pass experiment using the existing paired numeric harness."""
import argparse

import ppml_preparation_compare as comparison

BASELINE = '8be388e280eb64af5de9830204a63bee8de3b33d'


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--part', choices=['primary', 'controls'], required=True)
    args = parser.parse_args()
    comparison.BASELINE = BASELINE
    if args.part == 'primary':
        for j, spec in enumerate(comparison.PRIMARY):
            for r, seed in enumerate([104729, 8675309, 20260819]):
                comparison.pair(spec, seed, rotation=j + r)
    else:
        for r, spec in enumerate(comparison.CONTROLS):
            comparison.pair(spec, 31415927, rotation=r)
