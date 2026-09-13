from pathlib import Path
import preparation_compare as ols
import ppml_preparation_compare as ppml

BASELINE = '7663a87614e37bea73e5dbc037c3ad0c7afccdda'
ols.BASELINE = ppml.BASELINE = BASELINE
original_measure = ols.measure
ols.measure = lambda *a, **kw: original_measure(*a, solver=a[1].get('solver', 'auto'), **kw)
exec((Path(__file__).parent / 'ppml-compare-science.py').read_text(), ppml.__dict__)

PRIMARY = [
    dict(name='weak_wide', design='weak_d3', n=122880, k=65, threads=4, weight='[aw=w]', vce='robust'),
    dict(name='cmg_low_scores', design='cycle', n=30720, k=10, threads=4, weight='[aw=w]', vce='robust', solver='cmg', fit_options='memory(low)'),
    dict(name='cmg_wide', design='cycle', n=61440, k=50, threads=4, solver='cmg'),
]
CONTROLS = [
    dict(name='small_weak', design='weak_d3', n=7680, k=1, threads=1, weight='[aw=w]', vce='robust'),
    dict(name='complete_iid', design='one_fe', n=245760, k=50, threads=16),
    dict(name='saved_one_fe', design='one_fe', n=30720, k=16, threads=4, weight='[aw=w]', absorb='saved_g=g1', fit_options='residuals(saved_e)', missing=True),
]
if __name__ == '__main__':
    for i, seed in enumerate([104729, 8675309, 20260819]):
        for j, spec in enumerate(PRIMARY):
            ols.pair(spec, seed, i+j, trace=False, prefix='reporting-')
        ppml.pair(dict(name='ppml_weak_checkpoint', design='weak_d3', n=30720, k=17,
            threads=4, weight='[pw=w]', vce='cluster c1', fit_options='offset(off)'),
            seed, i, trace=False, prefix='reporting-')
    for i, spec in enumerate(CONTROLS):
        ols.pair(spec, 31415927, i, trace=False, prefix='reporting-control-')
