"""Functional comparison: retain science guards; inspect observed model offset separately."""
from pathlib import Path
import numpy as np
import pandas as pd
from common import read_json,write_json
from ppml_one_fe_compare import numeric
from ppml_preparation_compare import named_runtime
def compare(paths, unaffected=False):
    a, b = paths['candidate'], paths['baseline']
    ra, rb = read_json(a / 'result.json'), read_json(b / 'result.json')
    assert ra['status'] == rb['status'] == 'pass'
    assert ra['sample_sha256'] == rb['sample_sha256']
    runtime = {arm: named_runtime(path) for arm, path in paths.items()}
    differences = {}

    def check(name, rows=None, columns=None, exact=False, atol=1e-10, rtol=1e-7):
        u, v = numeric(a, name), numeric(b, name)
        assert u.shape == v.shape, name
        if rows is not None:
            u, v = u[rows], v[rows]
        if columns is not None:
            u, v = u[:, columns], v[:, columns]
        assert np.array_equal(np.isnan(u), np.isnan(v)), name
        finite = np.isfinite(u) & np.isfinite(v)
        assert np.all(finite | (np.isnan(u) & np.isnan(v))), name
        if exact:
            assert np.array_equal(u, v, equal_nan=True), name
        else:
            assert np.allclose(u, v, atol=atol, rtol=rtol, equal_nan=True), (name, np.nanmax(np.abs(u-v)))
        differences[name] = float(np.max(np.abs(u[finite] - v[finite]))) if finite.any() else 0.0

    for name in ('feppml_status', 'feppml_dof', 'feppml_fe', 'feppml_cluster',
                 'feppml_subset', 'feppml_separation', 'feppml_drop_counts',
                 'feppml_separation_sequence', 'feppml_receipt'):
        check(name, exact=True)
    check('b')
    check('V', rtol=1e-6)
    check('feppml_fit', rows=[*range(5), 11, 12, *range(16, 27)], exact=True)
    check('feppml_fit', rows=list(range(5, 11)), rtol=1e-7)
    check('feppml_inference', rows=[*range(8), 10, 14, 15], exact=True)
    check('feppml_inference', rows=[8, 9, 11, 12, 13], rtol=1e-6)
    check('feppml_solver', columns=list(range(12)), exact=True)
    for key in ('outer_iterations', 'step_halvings', 'memory_requested', 'memory_selected',
                'linear_route', 'fallback_mask', 'fallback_count', 'threads_requested',
                'threads_effective'):
        assert runtime['candidate'][key] == runtime['baseline'][key], key
    for path in paths.values():
        assert numeric(path, 'feppml_certificate')[-1, 0] == 1
        assert read_json(path / 'result.json')['metadata']['converged'] == 1
    if (a / 'saved_outputs.csv').exists():
        u, v = [pd.read_csv(p / 'saved_outputs.csv').apply(pd.to_numeric, errors='coerce') for p in (a, b)]
        assert u.columns.equals(v.columns)
        for name in u:
            assert np.allclose(u[name], v[name], atol=0 if name in ('obs_id', 'saved_drop') else 1e-9,
                               rtol=0 if name in ('obs_id', 'saved_drop') else 1e-7, equal_nan=True), name
    if unaffected:
        for name in (differences.keys() | {'feppml_certificate'}) - {'feppml_solver'}:
            assert (a / f'{name}.csv').read_bytes() == (b / f'{name}.csv').read_bytes(), name
        check('feppml_solver', columns=[*range(13), *range(16, 22)], exact=True)
    if runtime['candidate']['allocation_audit_present'] == 1:
        for arm, values in runtime.items():
            for key in ('ppml_capacity_stable', 'linear_capacity_stable', 'audit_scratch_stable',
                        'linear_session_allocation_free', 'fresh_warm_equivalent'):
                assert values[key] == 1, (arm, key)
            assert values['audit_current_growth'] == values['audit_peak_growth'] == 0
            assert values['allocation_peak_delta'] <= values['modeled_selected_bytes']
            for key, value in values.items():
                if key.startswith('observed_peak_'):
                    assert value <= values[key.replace('observed_', 'modeled_')], (arm, key)
        for key in ('audit_events_delta', 'allocation_certificate_passed', 'iteration_allocation_free'):
            assert runtime['candidate'][key] == runtime['baseline'][key], key
    write_json(a / 'science.json', dict(sample_rank_dof_weight_cluster_separation_exact=True,
        certificates_pass=True, solver_decisions_exact=True, saved_outputs_pass=True,
        unaffected_exact=unaffected, max_abs_differences=differences,
        inner_iterations={arm: r['inner_iterations'] for arm, r in runtime.items()}))
    write_json(a / 'paired-runtime.json', runtime)

root=Path(__file__).parent
paths={a:root/'runs'/f'psd-functional-n30720-{a}' for a in ('baseline','candidate')}
metadata={a:read_json(p/'result.json')['metadata'] for a,p in paths.items()}
keys=[k for k in metadata['baseline'] if k.startswith('feppml_modeled_peak_') or k in ('feppml_memory_fast_bytes','feppml_memory_low_bytes','feppml_memory_selected_bytes')]
deltas={k:metadata['candidate'][k]-metadata['baseline'][k] for k in keys}
assert len(keys)==16 and set(deltas.values())=={576.0},deltas
rt={a:named_runtime(p) for a,p in paths.items()}
assert rt['baseline']['allocation_peak_delta']==rt['candidate']['allocation_peak_delta']
for a in rt:assert rt[a]['allocation_peak_delta']<=rt[a]['modeled_selected_bytes']
compare(paths,unaffected=True)
write_json(root/'functional-memory-inspection.json',dict(model_deltas=deltas,native_peaks={a:rt[a]['allocation_peak_delta'] for a in rt},
    science_exact=True,admission_pass=True,explanation='Every modeled phase and both mode totals share a 576-byte offset. The unchanged pre-core allocator embedding snapshot is a dynamic model input and is charged equally to every phase. The changed final result validator runs after this snapshot. The individual allocation responsible was not traced; no allocation or memory-saving claim is inferred.'))
print('Full functional exports exact; constant 576-byte modeled offset recorded; native peaks unchanged and admitted.')
