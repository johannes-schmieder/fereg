"""P1 bounded candidate-run and mixed-source SPI loading comparisons."""
import argparse

import preparation_compare as ols
import ppml_preparation_compare as ppml
from common import sha256, write_json

BASELINE = '5411b9b4d37aab1a856b12431db84ecfce1c6f22'
PRIMARY = [
    ('ols', dict(name='candidate_aw', design='one_fe', n=983040, k=50, threads=16,
                 weight='[aw=w]', restriction='if obs_id <= _N/2', missing=True)),
    ('ols', dict(name='mixed_string_cluster', design='one_fe', n=245760, k=50, threads=16,
                 weight='[fw=w]', vce='cluster s_c1', string_columns='c1', missing=True)),
    ('ppml', dict(name='candidate_ppml_offset', design='one_fe', n=245760, k=50, threads=16,
                  weight='[pw=w]', restriction='if obs_id <= _N/2', fit_options='offset(off)', missing=True)),
]


def pair(model, spec, seed, rotation=0, prefix=''):
    comparator = ols if model == 'ols' else ppml
    paths = comparator.pair(spec, seed, rotation=rotation, prefix=prefix)
    for arm, path in paths.items():
        write_json(path / 'input-experiment-identity.json', dict(
            baseline_commit=BASELINE, variant=arm,
            plugin_sha256=sha256(ols.BASE / arm / 'dist/fereg_macos.plugin'),
            source_delta='candidate-input implementation.patch',
            string_recipe='s_<column> = missing ? empty : trimmed %21.0g; outside timing',
        ))
    return paths


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--part', choices=['pilot', 'primary', 'controls', 'ppml-controls', 'adjustment'], required=True)
    args = parser.parse_args()
    ols.BASELINE = ppml.BASELINE = BASELINE
    if args.part == 'pilot':
        pair(*PRIMARY[0], 104729, prefix='pilot-')
    elif args.part == 'primary':
        for j, (model, spec) in enumerate(PRIMARY):
            for r, seed in enumerate([104729, 8675309, 20260819]):
                pair(model, spec, seed, rotation=j+r)
    elif args.part == 'adjustment':
        spec = dict(name='complete_adjustment', design='one_fe', n=245760, k=50,
                    threads=16, weight='[pw=w]', fit_options='offset(off)')
        for r, seed in enumerate([104729, 8675309, 20260819]):
            pair('ppml', spec, seed, rotation=r)
    else:
        if args.part == 'controls':
            for j, spec in enumerate(ols.CONTROLS):
                pair('ols', spec, 31415927, rotation=j)
        # Shared OLS/PPML recipes can have the same display name but different
        # widths. Keep their derived-input cache identities distinct.
        for j, spec in enumerate(ppml.CONTROLS[6:]):
            pair('ppml', dict(spec, name='ppml_' + spec['name']), 31415927, rotation=j)
