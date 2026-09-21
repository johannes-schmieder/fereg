"""Independent exact construction for the protected one-FE output auditor."""
import json
from pathlib import Path
import tempfile

import numpy as np

from audit_outputs import audit


def check():
    n = 16
    group = np.repeat(np.arange(4), 4)
    x = np.tile([-1., -1., 1., 1.], 4)
    effect = np.repeat([-3., -1., 1., 3.], 4)
    xb = 2. + .4 * x
    for kind in ['none', 'aw', 'fw']:
        weight = np.ones(n) if kind == 'none' else np.tile([1., 2., 1., 2.], 4)
        residual = np.tile([1., -1., -1., 1.] if kind == 'none' else [2., -1., -2., 1.], 4)
        y = xb + effect + residual
        rss = float(residual @ (weight * residual))
        if kind == 'aw':
            rss *= n / weight.sum()
        for mode in ['fe', 'all']:
            with tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                data = root / 'input.dta'
                columns = ['y', 'x1', 'g1', 'w']
                np.array([y, x, group, weight], dtype='<f8').tofile(data.with_suffix('.bin'))
                data.with_suffix('.json').write_text(json.dumps(dict(columns=columns)))
                run = root / 'runs' / 'check-case-native'
                run.mkdir(parents=True)
                result = dict(status='pass', arm='fereg', sample_rows=n,
                              metadata=dict(rss=rss), cell=dict(effects=['g1'], n=n, k=1,
                              outputs=mode, weight_kind=kind))
                (run / 'result.json').write_text(json.dumps(result))
                (run / 'request.json').write_text(json.dumps(dict(data=str(data))))
                (run / 'b.csv').write_text('row,column,value\n1,1,0.4\n1,2,2.0\n')
                output = np.array([effect, residual, xb, xb + effect] if mode == 'all' else [effect], dtype='<f8')
                output.tofile(run / 'outputs.bin')
                audit(root, 'check', 'native')
                records = json.loads((root / 'check-native-independent-outputs.json').read_text())
                assert len(records) == 1 and records[0]['status'] == 'pass'
                output[0, 1] += 1.0
                output.tofile(run / 'outputs.bin')
                try:
                    audit(root, 'check', 'native')
                except AssertionError:
                    pass
                else:
                    raise AssertionError('nonconstant saved FE escaped the independent audit')
    print('ONE_FE_OUTPUT_AUDITOR_CHECK_PASS 6 valid cases and 6 corrupted outputs')


if __name__ == '__main__':
    check()
