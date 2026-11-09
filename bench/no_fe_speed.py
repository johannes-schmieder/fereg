"""Bounded no-FE command comparison, dispatched by qualify.py input.

This target panel is descriptive; it does not replace protected routing coverage.
Verify actual plugins against independent QR before timing, freeze all inputs,
retain warmups and every measurement, and never retry a failed/timed run.
"""
from __future__ import annotations

import csv
import hashlib
import json
import math
import os
from pathlib import Path
import random
import statistics
import sys

import numpy as np
import parallel_input as base

FIELDS = 'arm,block,k,threads,rep,processors,workers,wall_s,load_s,native_s,total_s'
DIRECT_SCOPE = ['residual_based_fit', 'iid_covariance', 'derived_residual_solver_row']
CENTERED_SCOPE = ['centers_and_norms', 'finite_crossproducts', 'coefficients_and_bread',
                  'residual_based_fit', 'iid_covariance', 'numerical_certificates',
                  'initial_projection_diagnostics', 'finite_factor_trace',
                  'derived_residual_solver_row']


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def binding(config):
    paths = [Path(config['data_dir']) / f for f in ('data.dta', 'reference.json')]
    paths += [Path(config['clock']), Path(__file__), base.ROOT / 'bench/no_fe_speed.do',
              base.ROOT / 'bench/parallel_input.py', base.ROOT / 'bench/qualify.py',
              base.ROOT / 'bench/campaign.json', base.ROOT / 'bench/qualify_scc.py',
              base.ROOT / 'bench/block_host.py', Path(config['stata'])]
    for arm in config['arms'].values():
        paths += [Path(arm['plugin']), *sorted(Path(arm['ado']).glob('*.ado')),
                  *sorted(Path(arm['ado']).glob('*.mata')),
                  *sorted(Path(arm['ado']).glob('*.mo')), *sorted(Path(arm['ado']).glob('*.mlib'))]
    return {str(p.resolve()): digest(p) for p in sorted(set(paths))}


def validate(config):
    kind = config.get('optimization', 'schedule')
    if kind not in ('schedule', 'direct', 'centered') or config.get('numerical_scope', []) != {'schedule': [], 'direct': DIRECT_SCOPE, 'centered': CENTERED_SCOPE}[kind]:
        raise ValueError('candidate numerical and residual-route scope must be declared before timing')
    if set(config['arms']) != {'incumbent', 'candidate', 'regress'}:
        raise ValueError('require incumbent, candidate and regress arms')
    if config['blocks'] != 6 or config['reps'] != 1:
        raise ValueError('this bounded round requires six balanced blocks and one repetition')
    if config['threads'] != [1, 2, 4, 8, 16] or config['widths'] != [1, 16, 64]:
        raise ValueError('unexpected frozen no-FE panel')


def run(config, arm, block, prefix, tracing=False):
    timings, science = prefix.with_suffix('.csv'), prefix.with_suffix('.txt')
    for path in (timings, science):
        with path.open('x') as stream:
            if path == timings:
                stream.write(FIELDS + '\n')
    env = dict(os.environ)
    for key in ('FEREG_SOLVER_TRACE', 'FEREG_SOLVER_TRACE_FILE', 'FEREG_PHASE_TIMING'):
        env.pop(key, None)
    if tracing and arm != 'regress':
        env.update(FEREG_SOLVER_TRACE='1', FEREG_SOLVER_TRACE_FILE=str(prefix.with_suffix('.trace')),
                   FEREG_PHASE_TIMING='1')
    args = ' '.join([base.stata_string(Path(config['data_dir'])), base.stata_string(timings), arm,
                     str(block), '"' + ' '.join(map(str, config['threads'])) + '"',
                     str(config['reps']), '"' + ' '.join(map(str, config['widths'])) + '"',
                     base.stata_string(Path(config['clock'])), base.stata_string(science)])
    log = base.run_arm(config, arm, args, env, fixture='no_fe_speed')
    prefix.with_suffix('.log').write_text(log)
    return timings, science


def check_science(config, paths):
    references = json.loads((Path(config['data_dir']) / 'reference.json').read_text())
    tolerances = json.loads((base.ROOT / 'bench/campaign.json').read_text())
    # Full-precision decimal export round-trips doubles for exact comparisons.
    seen, errors = {}, {'b': 0., 'V': 0., 'rss_relative': 0.}
    observations = 0
    for path in paths:
        parts = path.stem.split('-')
        file_arm, file_block = parts[-1], '0' if parts[0] == 'verify' else parts[1]
        expected_ids = {(file_arm,file_block,str(k),str(t),str(r)) for k in config['widths']
                        for t in config['threads'] for r in (0,1)}
        identities = set()
        for line in path.read_text().splitlines():
            arm, block, k, threads, rep, *values = line.split()
            identity = (arm,block,k,threads,rep)
            if identity in identities:
                raise ValueError('duplicate scientific observation')
            identities.add(identity)
            k, threads = int(k), int(threads)
            actual = np.asarray(values, dtype=float)
            p = k + 1
            ref = references[str(k)]
            if ref['N'] != 1_000_000:
                raise ValueError('this panel requires one million observations')
            b, v = actual[:p], actual[p:p+p*p]
            rss, n, rank, df = actual[-4:]
            if len(actual) != p + p*p + 4 or not np.all(np.isfinite(actual)):
                raise ValueError('invalid scientific export')
            want_b, want_v = np.asarray(ref['b']), np.asarray(ref['V']).flatten(order='F')
            if not np.allclose(b, want_b, rtol=tolerances['coefficient_rtol'], atol=tolerances['coefficient_atol']):
                raise ValueError(f'{arm}/{k}/{threads}: independent QR coefficients failed')
            if not np.allclose(v, want_v, rtol=tolerances['vce_rtol'], atol=tolerances['vce_atol']):
                raise ValueError(f'{arm}/{k}/{threads}: independent QR covariance failed')
            if not math.isclose(rss, ref['rss'], rel_tol=tolerances['objective_rtol'], abs_tol=tolerances['objective_atol']):
                raise ValueError(f'{arm}/{k}/{threads}: independent QR objective failed')
            if (n, rank, df) != (ref['N'], ref['rank'], ref['df_r']):
                raise ValueError('sample/rank/degrees of freedom changed')
            errors['b'] = max(errors['b'], float(np.max(np.abs(b-want_b))))
            errors['V'] = max(errors['V'], float(np.max(np.abs(v-want_v))))
            errors['rss_relative'] = max(errors['rss_relative'], abs(rss/ref['rss']-1))
            key = (arm, k, threads)
            bits = actual.tobytes()
            if key in seen and seen[key] != bits:
                raise ValueError(f'{key}: fixed-worker science changed')
            seen[key] = bits
            observations += 1
        if identities != expected_ids:
            raise ValueError(f'{path.name}: incomplete scientific observations')
    expected = {(arm,k,t) for arm in config['arms'] for k in config['widths'] for t in config['threads']}
    if set(seen) != expected:
        raise ValueError('incomplete scientific coverage')
    exact = True
    for k in config['widths']:
        for t in config['threads']:
            candidate, incumbent = seen['candidate',k,t], seen['incumbent',k,t]
            exact &= candidate == incumbent
            if config.get('optimization', 'schedule') == 'direct':
                if config.get('numerical_scope') != DIRECT_SCOPE:
                    raise ValueError('undeclared direct residual numerical scope')
                # Only covariance and RSS may differ. Coefficients, sample,
                # rank and degrees of freedom remain exact, including signs.
                p = k + 1
                if candidate[:8*p] != incumbent[:8*p] or candidate[-24:] != incumbent[-24:]:
                    raise ValueError(f'{k}/{t}: unaffected science changed')
            elif config.get('optimization') == 'centered':
                if config.get('numerical_scope') != CENTERED_SCOPE:
                    raise ValueError('undeclared centered numerical scope')
                if candidate[-24:] != incumbent[-24:]:
                    raise ValueError(f'{k}/{t}: unaffected science changed')
            elif candidate != incumbent:
                raise ValueError(f'{k}/{t}: undeclared numerical change')
    for arm in config['arms']:
        for k in config['widths']:
            reference = np.frombuffer(seen[arm,k,config['threads'][0]], dtype=float)
            p = k + 1
            for t in config['threads'][1:]:
                actual = np.frombuffer(seen[arm,k,t], dtype=float)
                checks = [np.allclose(actual[:p], reference[:p], rtol=tolerances['coefficient_rtol'], atol=tolerances['coefficient_atol']),
                          np.allclose(actual[p:p+p*p], reference[p:p+p*p], rtol=tolerances['vce_rtol'], atol=tolerances['vce_atol']),
                          math.isclose(actual[-4], reference[-4], rel_tol=tolerances['objective_rtol'], abs_tol=tolerances['objective_atol']),
                          np.array_equal(actual[-3:], reference[-3:])]
                if not all(checks):
                    raise ValueError(f'{arm}/{k}/{t}: cross-worker science changed')
    return {'observations': observations, 'cross_worker_scientific_agreement': True, 'exact_candidate_incumbent': exact,
            'exact_unaffected_fields': True, 'numerical_scope': config.get('numerical_scope', []),
            'fixed_worker_repeatability': True, 'maximum_errors': errors}


def check_trace(config, path):
    direct = config.get('optimization') == 'direct' and path.stem.endswith('candidate')
    centered = config.get('optimization') == 'centered' and path.stem.endswith('candidate')
    phase = 'centered_projection' if centered else 'structural_direct' if direct else 'map_wrapper'
    groups, current = {}, None
    for line in path.read_text().splitlines():
        pieces = line.split('|')
        fields = dict(p.split('=',1) for p in pieces[1:] if '=' in p)
        if fields.get('phase') == 'no_fe_benchmark':
            current = (int(fields['k']), int(fields['threads']))
            if current in groups:
                raise ValueError('duplicate traced cell')
            groups[current] = {'projection':[], 'result':[], 'input':[], 'moments':[], 'factor':[], 'compatibility':[], 'raw':[], 'layout':[]}
        elif current:
            if pieces[:2] == ['FEREG_PHASE',phase] and int(fields['columns' if centered else 'rhs']) == current[0]+2:
                groups[current]['projection'].append(fields)
            if pieces[:2] == ['FEREG_PHASE','centered_moments']:
                groups[current]['moments'].append(fields)
            if fields.get('phase') == 'finite_factor':
                groups[current]['factor'].append(fields)
            if fields.get('phase') == 'centered_compatibility':
                groups[current]['compatibility'].append(fields)
            if pieces[:2] == ['FEREG_PHASE','dense_raw']:
                groups[current]['raw'].append(fields)
            if fields.get('phase') == 'input_layout':
                groups[current]['layout'].append(fields)
            if fields.get('phase') == 'no_fe_result':
                groups[current]['result'].append(fields)
            if fields.get('phase') == 'input_digest':
                groups[current]['input'].append(fields['digest'])
    if set(groups) != {(k,t) for k in config['widths'] for t in config['threads']}:
        raise ValueError('incomplete traced path coverage')
    for cell, group in groups.items():
        if (len(group['projection']) != 2 or len(group['result']) != 2 or
            len(group['input']) != 2 or len(set(group['input'])) != 1):
            raise ValueError(f'{cell}: incomplete or unstable traced path')
        if centered and (len(group['moments']) != 2 or len(group['factor']) != 2 or group['compatibility'] or
                         any(v.get('method') != 'cholesky' for v in group['factor']) or
                         any(v.get('rows') != '1000000' or int(v.get('columns',0)) != cell[0] for v in group['moments'])):
            raise ValueError(f'{cell}: centered finite path did not execute')
        if centered and config.get('require_packed_loading', False):
            expected = str(int(cell[1] > 1))
            expected_bool = 'true' if cell[1] > 1 else 'false'
            if (len(group['raw']) != 2 or len(group['layout']) != 2 or
                any(any(v.get(key) != expected for key in ('loaded_rows', 'reused_rows', 'reused_centered_summary')) for v in group['raw']) or
                any(v.get('packed_rhs') != expected_bool for v in group['layout'])):
                raise ValueError(f'{cell}: declared packed loading/summary handoff did not execute')
        for projection, result in zip(group['projection'],group['result']):
            expected = {} if centered else ({'raw_statistics_count':'1', 'center_count':'1',
                         'residual_norms_count':'1', 'certificate_count':'1'} if direct else
                        {'norms_count':'1', 'initial_certificate_count':'1', 'sweep_count':'0'})
            if (projection['rows'] != '1000000' or any(projection.get(k) != v for k,v in expected.items()) or
                result['iterations'] != '1' or result['n_hdfe'] != '0' or result['solver'] != 'direct' or
                (direct or centered) and (result.get('residual_stop') != '5' or result.get('residual_iterations') != '0')):
                raise ValueError(f'{cell}: structural-constant path did not execute')
    return {f'k{k}-t{t}':group for (k,t),group in groups.items()}


def command_verify(config):
    validate(config)
    output = Path(config['output'])
    output.mkdir(parents=True, exist_ok=True)
    files = binding(config)
    paths, traces = [], {}
    for arm in config['arms']:
        _, path = run(config, arm, 0, output / f'verify-{arm}', tracing=True)
        paths.append(path)
        if arm != 'regress':
            traces[arm] = check_trace(config, output / f'verify-{arm}.trace')
    summary = check_science(config, paths)
    if binding(config) != files:
        raise ValueError('preflight inputs changed')
    summary.update(status='passed', binding=files, paths=traces)
    (output / 'science-summary.json').write_text(json.dumps(summary, indent=2)+'\n')
    print(json.dumps({k:v for k,v in summary.items() if k != 'binding'}), flush=True)
    return summary


def command_run(config):
    validate(config)
    output = Path(config['output'])
    science = json.loads((output / 'science-summary.json').read_text())
    if science['status'] != 'passed' or science['binding'] != binding(config):
        raise ValueError('scientific preflight is stale')
    arms = sorted(config['arms'])
    random.Random(config['seed']).shuffle(arms)
    order = []
    for block in range(config['blocks']):
        offset = block // 2
        pair = arms[offset:] + arms[:offset]
        order.append(pair if block % 2 == 0 else pair[::-1])
    with (output / 'plan.json').open('x') as stream:
        json.dump({'config':config, 'binding':science['binding'], 'order':order}, stream, indent=2)
    import qualify_scc
    monitor = qualify_scc.MacMonitor if sys.platform == 'darwin' else qualify_scc.HostMonitor
    with monitor(output / 'host', max(config['threads'])) as host:
        for block, arms in enumerate(order, 1):
            for arm in arms:
                host.check()
                run(config, arm, block, output / f'block-{block}-{arm}')
                host.check()
                print(f'block {block}/{arm} complete', flush=True)


def command_report(config):
    output = Path(config['output'])
    plan = json.loads((output / 'plan.json').read_text())
    if plan['config'] != config or plan['binding'] != binding(config):
        raise ValueError('frozen configuration/artifacts changed')
    host = json.loads((output / 'host/host.json').read_text())
    if not host['isolation_ok']:
        raise ValueError('host isolation was not verified')
    paths = [output / f'block-{b}-{arm}' for b, arms in enumerate(plan['order'],1) for arm in arms]
    science = check_science(config, [*(output.glob('verify-*.txt')), *(p.with_suffix('.txt') for p in paths)])
    rows = []
    for path in paths:
        with path.with_suffix('.csv').open() as stream:
            rows += list(csv.DictReader(stream))
    expected = {(arm,str(b),str(k),str(t),str(r)) for arm in config['arms']
                for b in range(1,7) for k in config['widths'] for t in config['threads'] for r in (0,1)}
    ids = [tuple(row[k] for k in ('arm','block','k','threads','rep')) for row in rows]
    if set(ids) != expected or len(ids) != len(expected):
        raise ValueError('missing/duplicate timing calls')
    for row in rows:
        for metric in ('wall_s','load_s','native_s','total_s'):
            if row['arm'] == 'regress' and metric != 'wall_s':
                if row[metric] != '.':
                    raise ValueError('regress has no isolated native phase')
                continue
            value = float(row[metric])
            if not math.isfinite(value) or value < 0 or (metric == 'wall_s' and not 0 < value < 120):
                raise ValueError('invalid timing observation')
    cells, regressions = [], []
    for k in config['widths']:
        for t in config['threads']:
            medians = {}
            for arm in config['arms']:
                subset = [r for r in rows if (r['arm'],int(r['k']),int(r['threads']),r['rep']) == (arm,k,t,'1')]
                medians[arm] = {}
                for metric in ('wall_s','load_s','native_s','total_s'):
                    values = [float(r[metric]) for r in subset if r[metric] != '.']
                    if any(not math.isfinite(v) or v < 0 for v in values):
                        raise ValueError('invalid timing')
                    medians[arm][metric] = statistics.median(values) if values else None
                medians[arm]['processors'] = sorted({int(r['processors']) for r in subset})
            inc, cand = (medians[a]['wall_s'] for a in ('incumbent','candidate'))
            regression = cand-inc > max(.05*inc,.001)
            if regression:
                regressions.append([k,t])
            cells.append({'k':k, 'threads':t, 'medians':medians, 'regression':regression})
    summary = {'status':'regression' if regressions else 'passed', 'scope':'descriptive target panel; protected qualification separate',
               'science':science, 'cells':cells, 'regressions':regressions, 'observations':rows,
               'resampling':False, 'binding':plan['binding'], 'host':host}
    (output / 'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps({k:v for k,v in summary.items() if k not in ('binding','observations')},indent=2))
    return summary
