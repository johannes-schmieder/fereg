"""Exact-input-bound diagnostics for the explicit OLS CMG polishing repair.

These allowances apply only to parent comparisons after validation. Numerical
scope, independent science, fixed-worker repeats and cross-worker decisions
remain the responsibilities of replay_plugins; this module never runs native code.
"""
from __future__ import annotations

import copy
import hashlib
import struct
from pathlib import Path

from replay_bias_reference import read_case

KIND = 'explicit_ols_cmg_polish_v1'
CASES = {
    'ols-08': ('78c09d055b3f8c9d31fc4c7c38070a278021d0ff030227dc6849e8d896cdd6fc', 'fast'),
    'ols-11': ('40e733998d249f047e563d4821226704643963d81ab987a362c612b036eb186c', 'fast'),
    'ols-14': ('0d2c0a27e17151347f92fc1920e94ee005c51af406bf2ed78bef18c70161ed02', 'low'),
    'ols-26': ('6242608b1a38dddc572b2bf2ab74f7d297a17d868e0be1f841b122cb6f02767a', 'low'),
    'ols-29': ('9b57e1d1ad4d4647968cf78a28b3625121c496fd957d9ae9079db1c989d86dcf', 'low'),
}
SOURCES = (
    'crates/fereg-core/src/fe/borrowed_map.rs',
    'crates/fereg-core/src/fe/cmg.rs',
    'crates/fereg-core/src/memory.rs',
    'crates/fereg-core/src/ols/prepared_fit.rs',
    'crates/fereg-plugin/src/result.rs',
    'scripts/replay_plugins.py',
    'scripts/replay_cmg_polish.py',
    'scripts/replay_bias_reference.py',
    'scripts/replay_psd.py',
    'crates/fereg-oracle/python/replay_ols_reference.py',
    'crates/fereg-oracle/python/replay_cmg_reference.py',
)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def validate_bindings(scope, bindings, scope_path, plugin, baseline, harness, cases, root):
    """Reject stale/missing provenance before any native process is started."""
    entries = scope.get('cases', {})
    typed = {name for name, entry in entries.items() if 'diagnostic_contract' in entry}
    if not typed:
        if bindings is not None:
            raise RuntimeError('diagnostic bindings require a typed replay scope')
        return
    if (set(entries) != set(CASES) or typed != set(CASES)
            or any(entries[name]['diagnostic_contract'] != KIND for name in typed)):
        raise RuntimeError('typed CMG scope must name exactly the five frozen OLS CMG cases')
    if not isinstance(bindings, dict) or set(bindings) != {
            'schema', 'kind', 'scope_sha256', 'plugin_sha256', 'baseline_sha256',
            'harness_sha256', 'sources', 'references'}:
        raise RuntimeError('typed CMG scope requires complete source/artifact/reference bindings')
    if (type(bindings['schema']) is not int or bindings['schema'] != 1
            or bindings['kind'] != KIND or baseline is None or scope_path is None):
        raise RuntimeError('invalid typed CMG binding schema or parent')
    for key, path in [('scope_sha256', scope_path), ('plugin_sha256', plugin),
                      ('baseline_sha256', baseline), ('harness_sha256', harness)]:
        if bindings[key] != digest(path):
            raise RuntimeError('typed CMG binding changed: ' + key)
    if (not isinstance(bindings['sources'], dict) or not isinstance(bindings['references'], dict)
            or set(bindings['sources']) != set(SOURCES) or set(bindings['references']) != set(CASES)):
        raise RuntimeError('typed CMG source/reference binding inventory differs')
    for name, expected in bindings['sources'].items():
        if digest(root / name) != expected:
            raise RuntimeError('typed CMG source changed: ' + name)
    for name, expected in bindings['references'].items():
        if digest(cases / (name + '.json')) != expected:
            raise RuntimeError('typed CMG independent reference changed: ' + name)
        case = cases / (name + '.case')
        if entries[name].get('input_sha256') != CASES[name][0]:
            raise RuntimeError('typed CMG scope input identity differs')
        case_contract(case)


def numbers(table):
    return [struct.unpack('>d', bytes.fromhex(v))[0] for v in table['bits']]


def integer(value, minimum=0):
    return type(value) in (int, float) and minimum <= value < 2**53 and value == int(value)


def case_contract(case):
    if case.stem not in CASES:
        raise ValueError('typed CMG contract is not defined for this replay input')
    expected_hash, mode = CASES[case.stem]
    if digest(case) != expected_hash:
        raise ValueError('typed CMG contract requires the exact frozen input')
    args, _, _ = read_case(case, expected_action='estimate-ols')
    if args['solver'] != 'cmg' or args['memory'] != mode:
        raise ValueError('typed CMG contract requires the declared solver and memory mode')
    pool = float(args['poolsize'])
    if not integer(pool, 1):
        raise ValueError('invalid genuine RHS pool size')
    return mode, int(pool)


def diagnostic_paths(mode):
    # Positions AFTER exact_view removed timing columns8/9. These paths are
    # permitted only after validate_rows; never added to numerical_scope_indices.
    positions = [9, 23] if mode == 'low' else [9, 15, 22, 23]
    return [f'/matrices/RSOLVER/bits/{i}' for i in positions]


def validate_rows(result, mode, pool, *, parent=False):
    if mode not in {'fast', 'low'} or not integer(pool, 1):
        raise ValueError('invalid typed CMG actual memory mode or pool size')
    table = result['matrices']['RSOLVER']
    status = result['matrices']['RSTATUS']
    if (result['return_code'] != 0 or table['rows'] != 2 or table['columns'] != 16
            or len(table['bits']) != 32 or status['columns'] != 4
            or not integer(status['rows'], 1) or len(status['bits']) != status['rows'] * 4):
        raise ValueError('typed CMG diagnostic shape/transaction mismatch')
    runtime = numbers(result['matrices']['RRUNTIME'])
    if runtime[2:6] != [1, 1 if mode == 'fast' else 2, 1, pool]:
        raise ValueError('typed CMG diagnostic actual mode/pool mismatch')
    values = numbers(table)
    bounds = [1 if mode == 'fast' else 1 + (status['rows'] + pool - 1) // pool, 1]
    for row_index in range(2):
        row = values[16 * row_index:16 * (row_index + 1)]
        special = not parent and mode == 'fast' and row_index == 1
        # requested, used, stop, fallback, requested/used preconditioner, pilot
        wanted = [7, 4 if special else 7, 2, int(special), 0, 0, 0, 0]
        if [row[i] for i in [0, 1, 3, 10, 12, 13, 14, 15]] != wanted:
            raise ValueError('typed CMG engine/stop/fallback/preconditioner/pilot mismatch')
        warnings, iterations = row[11], row[2]
        if not integer(iterations) or not integer(warnings):
            raise ValueError('invalid typed CMG work/warning count')
        if (parent and warnings != 0) or (special and warnings != 1):
            raise ValueError('typed CMG exact warning count mismatch')
        if warnings > bounds[row_index] or (not special and warnings > 0 and iterations == 0):
            raise ValueError('typed CMG warning count exceeds source-bound action count')


def validate_pair(case, parent, candidate):
    mode, pool = case_contract(case)
    validate_rows(parent, mode, pool, parent=True)
    validate_rows(candidate, mode, pool)
    return diagnostic_paths(mode)


def parent_discrete_view(view, mode):
    """Apply ONLY to candidate-vs-parent comparison, AFTER pair validation.

    Candidate-vs-repeat and candidate-vs-other-workers use the existing whole
    discrete_view unchanged, so concrete warnings/engine decisions remain exact.
    """
    out = copy.deepcopy(view)
    leaves = [11, 27] if mode == 'low' else [11, 17, 26, 27]
    for i in leaves:
        del out['protected']['RSOLVER'][i]
    return out
