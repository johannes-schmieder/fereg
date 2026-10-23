"""Bind the one changed deferred candidate to preserved Panel 2 evidence."""
import qualify as q

ARMS = {'B': 'B', 'O': 'O', 'O+eager': 'O+S'}


def deferred_commitment(text):
    """Pair construction with a successful commitment inside the same action."""
    pending, unclosed, installations, attempts = False, False, 0, 0
    actions = []
    for line in text.splitlines():
        if line.startswith('FEREG_SOLVER_TRACE|'):
            fields = dict(part.split('=', 1) for part in line.split('|')[1:] if '=' in part)
            if fields.get('phase') == 'auto_schur_route' and fields.get('installed') == '1':
                q.require(not pending and not unclosed, 'deferred commitment has no closed action')
                pending = True
                unclosed = True
                installations += 1
            elif fields.get('phase') == 'routing_action':
                q.require(not pending, 'deferred construction crossed an action boundary')
                unclosed = False
                actions.append(line)
        elif line.startswith('FEREG_CELLS|event=attempt|'):
            q.require(pending, 'deferred construction preceded successful CMG commitment')
            pending = False
            attempts += 1
    q.require(not pending and not unclosed, 'deferred commitment has no closed action')
    # Validate every raw cap, including the zero-installation path. The generic
    # legacy parser deliberately permits missing actions; this mechanism cannot.
    q.require(actions, 'deferred action resource evidence missing')
    q.science.resource_diagnostics('\n'.join(actions), None, False, {})
    return dict(installations=installations, attempts=attempts)


def compare_actions(current, control):
    """Protect action coverage/classification while allowing scoped cell work."""
    def shape(detail):
        actions = detail['action_resources']
        q.require(actions, 'deferred action resource evidence missing')
        for row in actions:
            q.require(row['final'] in ('0', '1') and row['preset'] == 'routing_two_gates_v1',
                      'deferred action classification/preset changed')
            for key in ('iterations', 'memory_bytes'):
                q.require(0 <= row[key + '_used'] <= row[key + '_limit'], 'deferred action raw cap exceeded')
        return [(row['final'], row['preset'], row['iterations_limit']) for row in actions]
    q.require(shape(current) == shape(control), 'deferred action sequence differs from control')


def control_sources(proof, current, archived=None):
    """Reuse controls only across verified changes to non-control entrypoints."""
    expected = proof['plan']['execution_sources']
    if expected == current:
        return
    q.require(archived is not None and set(expected) == set(current)
              and {name for name in current if current[name] != expected[name]}
              <= {'ownership_cells', 'ownership_panel3'}, 'control execution source drift')
    q.require(q.digest(archived) == expected['ownership_cells'], 'archived control source differs')
    import ast
    import ownership_cells as oc
    def control_ast(path):
        module = ast.parse(path.read_text())
        module.body = [node for node in module.body if not (isinstance(node, ast.FunctionDef)
            and node.name in {'freeze', 'run', 'report', 'compare_warmups'})]
        return ast.dump(module, include_attributes=False)
    q.require(control_ast(archived) == control_ast(oc.Path(oc.__file__)),
              'control-reachable source changed')


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
