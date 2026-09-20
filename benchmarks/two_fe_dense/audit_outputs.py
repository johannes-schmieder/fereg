"""Independent normal-equation and reconstruction audit of saved native outputs."""
import argparse
import json
from pathlib import Path

import numpy as np
import pandas as pd


def audit(root, label, variant):
    records = []
    for result_path in sorted((root/'runs').glob(f'{label}-*-{variant}/result.json')):
        result = json.loads(result_path.read_text())
        cell = result['cell']
        if cell.get('outputs','none') == 'none':
            continue
        assert result['status']=='pass' and result['arm']=='fereg'
        run = result_path.parent
        request = json.loads((run/'request.json').read_text())
        data = Path(request['data'])
        info = json.loads(data.with_suffix('.json').read_text())
        n,k = cell['n'],cell['k']
        assert result['sample_rows']==n
        matrix = np.memmap(data.with_suffix('.bin'),dtype='<f8',mode='r',shape=(len(info['columns']),n))
        columns = {name:matrix[j] for j,name in enumerate(info['columns'])}
        weight_kind = cell.get('weight_kind','none')
        weight = np.ones(n) if weight_kind=='none' else columns['w']
        total_weight = float(weight.sum())
        b = pd.read_csv(run/'b.csv').sort_values(['row','column']).value.to_numpy()
        assert len(b)==k+1
        xb = np.full(n,b[-1])
        for j in range(k):
            xb += columns[f'x{j+1}']*b[j]
        mode = cell['outputs']
        outputs = np.fromfile(run/'outputs.bin',dtype='<f8').reshape(5 if mode=='all' else 2,n)
        assert np.isfinite(outputs).all()
        prediction = xb + outputs[0] + outputs[1]
        residual = columns['y']-prediction
        weighted_residual = weight*residual
        group_means = []
        for effect,values in zip(('g1','g2'),outputs[:2]):
            _, first, codes = np.unique(columns[effect],return_index=True,return_inverse=True)
            np.testing.assert_array_equal(values,values[first][codes])
            counts = np.bincount(codes,weights=weight)
            means = np.bincount(codes,weights=weighted_residual)/counts
            group_means.append(float(np.max(np.abs(means))))
        finite_score = max(abs(float(columns[f'x{j+1}']@weighted_residual))/total_weight for j in range(k))
        assert max(group_means)<1e-9 and finite_score<1e-9
        assert abs(float(weighted_residual.sum()/total_weight))<1e-9
        assert abs(float(np.average(outputs[0],weights=weight)))<1e-9
        _, components = np.unique(columns['component'],return_inverse=True)
        centers = np.bincount(components,weights=weight*outputs[1])/np.bincount(components,weights=weight)
        assert np.max(np.abs(centers))<1e-9
        rss = float(residual@weighted_residual)
        if weight_kind in ('aw','pw'):
            rss *= n/total_weight
        np.testing.assert_allclose(result['metadata']['rss'],rss,rtol=1e-8)
        errors = None
        if mode=='all':
            expected = np.array([residual,xb,prediction])
            np.testing.assert_allclose(outputs[2:],expected,rtol=1e-7,atol=1e-9)
            errors = np.max(np.abs(outputs[2:]-expected),axis=1).tolist()
        records.append(dict(run=run.name,status='pass',group_residual_mean_max=group_means,
                            finite_score_per_observation=finite_score,
                            output_reconstruction_max_errors=errors))
    assert records
    path = root/f'{label}-{variant}-independent-outputs.json'
    path.write_text(json.dumps(records,indent=2,sort_keys=True)+'\n')
    print('INDEPENDENT_OUTPUT_AUDIT_PASS',len(records))


if __name__=='__main__':
    p=argparse.ArgumentParser()
    p.add_argument('root',type=Path);p.add_argument('label');p.add_argument('variant')
    a=p.parse_args();audit(a.root,a.label,a.variant)
