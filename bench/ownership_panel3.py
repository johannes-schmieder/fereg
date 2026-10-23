"""Bind the one changed deferred candidate to preserved Panel 2 evidence."""
import qualify as q

ARMS = {'B': 'B', 'O': 'O', 'O+eager': 'O+S'}


def deferred_commitment(text):
    """Pair construction with a successful commitment inside the same action."""
    pending, installations, attempts = False, 0, 0
    for line in text.splitlines():
        if line.startswith('FEREG_SOLVER_TRACE|'):
            fields = dict(part.split('=', 1) for part in line.split('|')[1:] if '=' in part)
            if fields.get('phase') == 'auto_schur_route' and fields.get('installed') == '1':
                q.require(not pending, 'deferred commitment has no construction')
                pending = True
                installations += 1
            elif fields.get('phase') == 'routing_action':
                q.require(not pending, 'deferred construction crossed an action boundary')
        elif line.startswith('FEREG_CELLS|event=attempt|'):
            q.require(pending, 'deferred construction preceded successful CMG commitment')
            pending = False
            attempts += 1
    q.require(not pending, 'deferred commitment has no construction')
    return dict(installations=installations, attempts=attempts)


def prior_result(plan, manifest):
    binding = plan['panel2_result']
    q.require(isinstance(binding, dict) and set(binding) == {'results', 'assessment'},
              'panel3 needs separate preserved result and assessment bindings')
    paths = {name: q.resolve(manifest, path) for name, path in binding.items()}
    q.require(all(plan['files'].get(binding[name]) == q.digest(path) for name, path in paths.items()),
              'panel2 evidence identity changed')
    prior, assessment = (q.read(paths[name]) for name in ('results', 'assessment'))
    q.require(prior['status'] == 'complete' and prior['panel'] == 2
              and prior['platform'] == plan['platform'] and prior['ownership'] == plan['ownership'] == 'both'
              and prior['inputs_sha256'] == plan['files'][plan['inputs']]
              and prior['candidate_sha256'] != plan['files'][plan['artifacts']['candidate']['path']]
              and not prior['diagnostic'] and not prior['failures'] and not prior['exclusions']
              and prior['retries'] == 0 and prior['native_calls_started'] == len(prior['calls']) == 1248,
              'panel2 completion/input identity or changed candidate missing')
    q.require(assessment['status'] == 'verified' and assessment['configuration_O'] == 'both'
              and assessment['native_calls'] == 0 and assessment['total_retained_native_calls'] == 1344
              and assessment['source_verified_before_after'] is True
              and assessment['bindings']['scored_result_sha256'] == q.digest(paths['results']),
              'panel2 independent assessment does not bind the preserved result')
    rows = assessment['independent_statistics']['cells_vs_ownership']
    q.require(len(rows) == 24 and len({r['cell'] for r in rows}) == 24
              and any(r['adverse_point_estimate'] and r['mean_excess_s'] > 0 for r in rows),
              'panel3 requires a retained complete-command regression')
    expected = {(cell, arm) for cell in prior['cells'] for arm in ('B', 'O', 'S', 'O+S')}
    warmups = {(c['cell'], c['arm']): c for c in prior['calls'] if c['phase'] == 'warmup'}
    q.require(len(warmups) == len(expected) == 96 and set(warmups) == expected
              and all(c['status'] == c['oracle'] == 'passed' for c in prior['calls']),
              'panel2 warmup coverage or successful calls missing')
    return prior, warmups


def same_host(prior, observed):
    # Exact historical scientific comparisons require the same physical host.
    before = prior['host']['host']['hostname'].split('.')[0]
    q.require(observed['host']['hostname'].split('.')[0] == before,
              'panel3 historical controls require the Panel2 physical host')


def compare_prior(warmups, cell, arm, current):
    if arm not in ARMS:
        return
    before = warmups[cell, ARMS[arm]]
    for key in ('fingerprint', 'worker_signature', 'model_signature', 'route_trace', 'route_hierarchy',
                'cell_diagnostic', 'resource_checks'):
        q.require(current[key] == before[key], 'panel3 changed preserved ' + arm + ' ' + key)
    def actions(call):
        # Available host memory can change. Each raw limit is checked separately;
        # actual charged bytes, action count, preset and iteration use stay exact.
        return [{k: v for k, v in row.items() if k != 'memory_bytes_limit'}
                for row in call['action_resources']]
    q.require(actions(current) == actions(before), 'panel3 changed preserved action resources')
    def gates(call):
        return [line for line in call['solver_trace_lines'] if '|phase=two_fe_gates|' in line]
    q.require(gates(current) == gates(before), 'panel3 changed preserved gate decisions')
