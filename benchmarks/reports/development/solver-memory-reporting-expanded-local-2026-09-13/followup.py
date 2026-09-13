import compare
from common import sha256, write_json
from collect_preparation import runtime
ols = compare.ols
ordinary_measure = ols.measure

def measure(*args, **kwargs):
    if args[1].get('same_artifact'):
        kwargs['source'] = ols.BASE / 'baseline'
        kwargs['identity'] = dict(kwargs['identity'], same_artifact=True,
            plugin_sha256=sha256(kwargs['source'] / 'dist/fereg_macos.plugin'))
    return ordinary_measure(*args, **kwargs)

ols.measure = measure
for i, seed in enumerate([15485863, 67867967, 49979687]):
    for same in ([True, False] if i % 2 == 0 else [False, True]):
        spec = dict(compare.PRIMARY[0], name='weak_aa' if same else 'weak_ab', same_artifact=same)
        paths = ols.pair(spec, seed, i+int(same), trace=False, prefix='reporting-followup-')
        if same:
            a, b = (runtime(paths[arm] / 'fereg_runtime.csv') for arm in ['baseline', 'candidate'])
            assert a[25] == b[25]
            artifacts = [ols.read_json(paths[arm] / 'result.json')['measurement_fingerprint']['plugin_sha256'] for arm in ['baseline', 'candidate']]
            assert artifacts[0] == artifacts[1]
            write_json(paths['candidate'] / 'same-artifact.json', dict(plugin_sha256=artifacts[0], model_exact=True))
