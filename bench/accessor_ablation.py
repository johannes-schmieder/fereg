"""Same-binary serial-reader ablation, dispatched by qualify.py input.

Both readers use identical estimation workers. Verify science and input bits
before timing; retain every fixed observation, including regressions.
"""
import csv
import hashlib
import json
import os
import tempfile
from pathlib import Path
from parallel_input import FIELDS, command_report as paired_report, run_arm, stata_string


def digest(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(8 << 20), b''):
            h.update(chunk)
    return h.hexdigest()


def identities(config):
    root = Path(__file__).resolve().parents[1]
    files = [root / ('bench/' + name) for name in
             ('accessor_ablation.do', 'accessor_ablation.py', 'parallel_input.py', 'qualify.py')]
    files += [Path(config['arms']['candidate']['plugin'])]
    files += sorted(Path(config['arms']['candidate']['ado']).glob('*.ado'))
    return {str(p.relative_to(root)): digest(p) for p in files}


def execute(config, mode, environment=None):
    args = (f'{mode} {stata_string(Path(config["output"]))} {config["rows"]} '
            f'{config["controls"]} {config["threads"][0]} {config["blocks"]} {config["seed"]}')
    tempfile.tempdir = str(Path(config['output']).resolve())
    log = run_arm(config, 'candidate', args, environment, fixture='accessor_ablation')
    (Path(config['output']) / f'{mode}.log').write_text(log)


def command_verify(config):
    if config['arms']['incumbent'] != config['arms']['candidate']:
        raise ValueError('readers must use the same ado tree and binary')
    if len(config['threads']) != 1 or config['threads'][0] < 2:
        raise ValueError('one fixed multiworker count is required')
    if config['blocks'] < 4 or config['blocks'] % 2 or config['reps'] != 1:
        raise ValueError('use even alternating pairs with one call per reader')
    if config['rows'] <= 5003 or config['controls'] < 4:
        raise ValueError('fixture requires >5003 rows and >=4 controls')
    output = Path(config['output'])
    output.mkdir(parents=True, exist_ok=True)
    if (output / 'plan.json').exists():
        raise RuntimeError('verification attempted already; preserve before any bounded repair')
    pins = identities(config)
    plan = {'config': config, 'identities': pins,
            'order': [['incumbent', 'candidate'] if i % 2 == 0 else
                      ['candidate', 'incumbent'] for i in range(config['blocks'])]}
    (output / 'plan.json').write_text(json.dumps(plan, indent=2)+'\n')
    trace = output / 'verify.trace'
    execute(config, 'verify', dict(os.environ, FEREG_SOLVER_TRACE='1', FEREG_SOLVER_TRACE_FILE=str(trace)))
    groups, current = {}, None
    for line in trace.read_text().splitlines():
        fields = dict(p.split('=', 1) for p in line.split('|')[1:] if '=' in p)
        if fields.get('phase') == 'accessor_ablation':
            current = fields['arm']
            if current in groups:
                raise RuntimeError('duplicate science fit')
            groups[current] = {'digests': [], 'windows': []}
        elif current and fields.get('phase') == 'input_digest':
            groups[current]['digests'].append(fields['digest'])
        elif current and fields.get('phase') == 'input_transport':
            groups[current]['windows'].append(fields)
    if set(groups) != {'serial1', 'serial2', 'parallel1', 'parallel2', 'public', 'one'}:
        raise RuntimeError('missing science fit trace')
    digests = []
    for arm, group in groups.items():
        if len(group['digests']) != 1:
            raise RuntimeError(f'{arm}: missing input digest')
        digests += group['digests']
        if bool(group['windows']) != (arm in {'parallel1', 'parallel2', 'public'}):
            raise RuntimeError(f'{arm}: expected reader did not execute')
        for w in group['windows']:
            if int(w['workers']) != config['threads'][0] or int(w['rows']) != config['rows']:
                raise RuntimeError('unexpected transport workers or rows')
    if len(set(digests)) != 1 or pins != identities(config):
        raise RuntimeError('input bits or source/binary changed')
    with (output / 'science.csv').open() as handle:
        science = list(csv.DictReader(handle))
    result = {'status': 'passed', 'fits': groups, 'science': science,
              'input_sha256': digest(output / 'input.dta'), 'trace_sha256': digest(trace),
              'same_worker_b_V_sample_discrete': 'exact',
              'independent_areg_max_abs_tolerance': 1e-8,
              'cross_worker_max_abs_b_tolerance': 1e-9,
              'cross_worker_max_abs_V_tolerance': 1e-8}
    (output / 'verification.json').write_text(json.dumps(result, indent=2)+'\n')
    print('Science, repeatability, worker agreement, public/hidden and transport checks passed.', flush=True)
    return result


def command_run(config):
    output = Path(config['output'])
    plan = json.loads((output / 'plan.json').read_text())
    verification = json.loads((output / 'verification.json').read_text())
    if plan['config'] != config or plan['identities'] != identities(config):
        raise RuntimeError('frozen configuration/source/binary changed')
    if verification['status'] != 'passed' or verification['input_sha256'] != digest(output / 'input.dta'):
        raise RuntimeError('science prerequisite or frozen input invalid')
    if (output / 'results.csv').exists():
        raise RuntimeError('timing attempted already; no resampling')
    (output / 'results.csv').write_text(','.join(FIELDS)+'\n')
    execute(config, 'timing', {k: v for k, v in os.environ.items() if not k.startswith('FEREG_')})
    print('All fixed timing pairs finished.', flush=True)


def command_report(config):
    output = Path(config['output'])
    if 'END_FIXTURE|accessor_ablation|rc=0' not in (output / 'timing.log').read_text():
        raise RuntimeError('missing successful application marker')
    plan = json.loads((output / 'plan.json').read_text())
    if plan['config'] != config or plan['identities'] != identities(config):
        raise RuntimeError('source/binary changed during timing')
    with (output / 'results.csv').open() as handle:
        rows = list(csv.DictReader(handle))
    if any(int(row['rows']) != config['rows'] or int(row['n']) != config['rows'] for row in rows):
        raise RuntimeError('wrong timing sample size')
    result = paired_report(config, cells=('loading',))
    result['verification'] = json.loads((output / 'verification.json').read_text())
    result['plan'] = json.loads((output / 'plan.json').read_text())
    for key in ('stata', 'output'):
        result['plan']['config'].pop(key, None)
    result['plan']['config']['arms'] = {'incumbent': 'forced serial reader',
                                       'candidate': 'qualified parallel reader; same binary'}
    result['raw_sha256'] = digest(output / 'results.csv')
    (output / 'summary.json').write_text(json.dumps(result, indent=2)+'\n')
    return result
