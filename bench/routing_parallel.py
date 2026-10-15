#!/usr/bin/env python3
"""Prospective full-round parallel SCC screen; descriptive discovery only.

Each logical round is one unchanged full 42-cell producer invocation. An attempt
has its own reserved 28-core allocation and four-hour producer budget. Three
rounds provide nine paired slots. At most one externally evidenced infrastructure
replacement per round is permitted; scientific failures and solver timeouts are
never replacement eligible. An optional independently replayed certificate may
retain the first two complete windows of the old collection; its original failed protocol remains failed.
Finalize after each allocation ends. Independent replay has a 300-second limit
per receipt. The aggregate reserves 1,200 seconds plus processing, including
optional window replay.
"""
from __future__ import annotations

import argparse
import copy
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time

import block_host
import routing
import routing_discovery as discovery
import routing_run
import scc_routing_host as isolation_host

KIND = 'routing_scc_parallel'
DESIGN = {
    'rounds': 3, 'maximum_attempts_per_round': 2, 'measured_slots': 9,
    'warmups_per_round': 1, 'repetitions_per_round': 3,
    'per_call_timeout_s': 120.0, 'per_round_budget_s': 14400.0,
    'maximum_producer_budget_s': 86400.0, 'receipt_replay_timeout_s': 300,
    'aggregate_minimum_replay_budget_s': 1200,
    'order': 'one unchanged full 42-cell producer round per allocation; logical round order at aggregation',
    'stopping': 'observer faults invalidate timing; unchanged producer finishes within its fixed budget; first valid attempt in fixed order; one replacement only after complete unchanged science and independently evidenced infrastructure failure',
    'inference': discovery.DESIGN['inference'],
    'old_timings': 'excluded except rounds 1 and 2 under a separately frozen retrospective window certificate; original protocol remains failed',
}
require = discovery.require
write = discovery.write
load = discovery.load
reference = discovery.reference
checked = discovery.checked


def binding_shape(ref):
    require(isinstance(ref, dict) and set(ref) == {'path', 'sha256'}
            and isinstance(ref['path'], str) and Path(ref['path']).is_absolute()
            and routing.sha(ref['sha256']), 'invalid file binding')


def metadata(plan):
    """Validate portable metadata without opening any deployment-host paths."""
    require(plan.get('kind') == KIND + '_plan' and type(plan.get('schema')) is int and plan['schema'] == 1
            and routing.identity(plan.get('design')) == routing.identity(DESIGN), 'wrong parallel plan')
    parent = routing.unseal(plan['parent'])
    require(parent.get('kind') == discovery.KIND + '_plan' and routing.identity(parent.get('design')) == routing.identity(discovery.DESIGN)
            and parent.get('library_limits') == discovery.LIBRARY_LIMITS,
            'wrong embedded original plan')
    require(plan.get('parent_sha256') == routing.identity(parent), 'embedded parent identity changed')
    spec = routing.unseal(parent['tournament'])
    discovery.validate_spec(spec)
    for name in ('parent_plan', 'proof', 'receipt_generator'):
        binding_shape(plan[name])
    require(routing.sha(plan.get('proof_sha256')), 'missing original proof identity')
    require(isinstance(plan.get('frozen_files'), dict) and plan['frozen_files']
            and all(isinstance(p, str) and Path(p).is_absolute() and routing.sha(h)
                    for p, h in plan['frozen_files'].items()), 'invalid frozen file map')
    require(isinstance(plan.get('output_root'), str) and Path(plan['output_root']).is_absolute()
            and plan['output_root'] != parent.get('output_root'), 'fresh output root required')
    require(plan.get('producer_tools') == parent.get('producer_tools')
            and bool(plan['producer_tools']), 'original producer identity changed')
    for ref in (plan['parent_plan'], plan['proof'], plan['receipt_generator']):
        require(plan['frozen_files'].get(ref['path']) == ref['sha256'], 'missing frozen reference')
    for path, sha in parent.get('frozen_files', {}).items():
        require(plan['frozen_files'].get(path) == sha, 'original frozen binding changed')
    if 'legacy_window' in plan:
        legacy = plan['legacy_window']
        require(set(legacy) == {'reference', 'certificate', 'validator'}, 'wrong window binding')
        for name in ('reference', 'validator'):
            binding_shape(legacy[name])
            require(plan['frozen_files'].get(legacy[name]['path']) == legacy[name]['sha256'],
                    'missing frozen window binding')
        certificate = routing.unseal(legacy['certificate'])
        collection = routing.unseal(certificate['collection'])
        require(certificate.get('kind') == 'routing_scc_window_certificate'
                and type(certificate.get('schema')) is int and certificate['schema'] == 1
                and certificate.get('status') == 'verified'
                and certificate.get('parent_sha256') == routing.identity(parent)
                and certificate.get('proof_sha256') == plan['proof_sha256']
                and certificate.get('collection_sha256') == routing.identity(collection)
                and collection.get('plan_sha256') == routing.identity(parent)
                and collection.get('prerequisites_sha256') == plan['proof_sha256']
                and certificate.get('original_protocol_status') == 'failed'
                and certificate.get('promotion') is False, 'wrong retrospective window certificate')
        expected = [str(Path(parent['output_root']) / f'collection/round-{n}/records.json') for n in (1, 2)]
        require(certificate.get('rounds') == collection.get('rounds', [])[:2]
                and len(certificate['rounds']) == 2
                and [ref['path'] for ref in certificate['rounds']] == expected,
                'window certificate must retain original full rounds one and two')
        require(set(certificate.get('raw_evidence', {})) == discovery.EVIDENCE, 'missing window raw evidence')
        for ref in [*collection['rounds'], *certificate['raw_evidence'].values()]:
            binding_shape(ref)
            require(plan['frozen_files'].get(ref['path']) == ref['sha256'], 'unfrozen window evidence')
    return {'parent': parent, 'spec': spec}


def _validate_window(plan, proof):
    legacy = plan['legacy_window']
    certificate = routing.unseal(legacy['certificate'])
    require(load(checked(legacy['reference'])) == certificate, 'window certificate reference differs')
    validator = checked(legacy['validator'])
    result = subprocess.run([sys.executable, str(validator), 'validate-stdin'],
        input=json.dumps({'parent_plan': metadata(plan)['parent'], 'proof': proof,
                          'certificate': certificate}, allow_nan=False),
        text=True, capture_output=True, timeout=300, check=False)
    require(result.returncode == 0 and len(result.stdout) <= 4096, 'window replay validator failed')
    require(json.loads(result.stdout) == {'schema': 1, 'status': 'verified',
            'certificate_sha256': routing.identity(certificate)}, 'window replay identity differs')
    return certificate


def current_files():
    result = dict(routing_run.tool_identities())
    for module in (sys.modules[__name__], isolation_host, discovery, routing, block_host):
        path = Path(module.__file__).resolve(strict=True)
        result[path] = discovery.digest(path)
    return {str(path): sha for path, sha in result.items()}


def validate_plan(plan):
    value = metadata(plan)
    for path, sha in plan['frozen_files'].items():
        checked({'path': path, 'sha256': sha})
    require(load(checked(plan['parent_plan'])) == value['parent'], 'original plan reference differs')
    discovery.validate_plan(value['parent'])
    require(plan['producer_tools'] == {p.name: h for p, h in routing_run.tool_identities().items()},
            'producer/checker source changed')
    require(all(plan['frozen_files'].get(p) == h for p, h in current_files().items()),
            'current parallel/host/producer source is not frozen')
    require(plan['receipt_generator'] == reference(isolation_host.__file__), 'wrong current receipt generator')
    proof = load(checked(plan['proof']))
    require(routing.identity(proof) == plan['proof_sha256'], 'original proof identity changed')
    _prerequisite(value['parent'], value['spec'], proof)
    if 'legacy_window' in plan:
        legacy = plan['legacy_window']
        require(load(checked(legacy['reference'])) == routing.unseal(legacy['certificate']),
                'window certificate reference differs')
        require(legacy['validator'] == reference(Path(__file__).parent / 'routing_window.py'),
                'wrong current window validator')
    return value['spec']


def _prerequisite(parent, spec, proof):
    wrappers = discovery.checked_wrappers(parent, spec, proof)
    original = routing.read_json(checked(proof['records']))
    discovery.validate_prerequisite_identity(parent, original)
    outcomes = discovery.prerequisite_status(spec, original)
    return wrappers, outcomes, discovery.expected_fingerprints(proof, original, outcomes)


def freeze(config):
    parent_ref, proof_ref = reference(config['parent_plan']), reference(config['proof'])
    parent, proof = load(parent_ref['path']), load(proof_ref['path'])
    spec = discovery.validate_plan(parent)
    _prerequisite(parent, spec, proof)
    require(reference(config['receipt_generator']) == reference(isolation_host.__file__),
            'freeze the current reviewed host collector')
    root = routing.scratch(Path(config['output_root']))
    require(not root.exists() and root != Path(parent['output_root']), 'freeze a fresh parallel output root')
    frozen = dict(parent['frozen_files'])
    for path, sha in current_files().items():
        require(path not in frozen or frozen[path] == sha,
                'current sources must be separate from unchanged original frozen paths')
        frozen[path] = sha
    for ref in [parent_ref, proof_ref, proof['records'], *proof['wrappers'].values(),
                *[reference(p) for p in config.get('source_checks', [])]]:
        path = checked(ref)
        require(str(path) not in frozen or frozen[str(path)] == ref['sha256'], 'original frozen source changed')
        frozen[str(path)] = ref['sha256']
    result = {'kind': KIND + '_plan', 'schema': 1, 'design': copy.deepcopy(DESIGN),
              'parent': routing.seal(parent), 'parent_sha256': routing.identity(parent),
              'parent_plan': parent_ref, 'proof': proof_ref, 'proof_sha256': routing.identity(proof),
              'output_root': str(root), 'producer_tools': parent['producer_tools'],
              'receipt_generator': reference(isolation_host.__file__), 'frozen_files': frozen}
    if config.get('window_certificate'):
        cert_ref = reference(config['window_certificate'])
        certificate = load(cert_ref['path'])
        validator_ref = reference(Path(__file__).parent / 'routing_window.py')
        result['legacy_window'] = {'reference': cert_ref, 'certificate': routing.seal(certificate),
                                   'validator': validator_ref}
        collection = routing.unseal(certificate['collection'])
        for ref in [cert_ref, validator_ref, *collection['rounds'], *certificate['raw_evidence'].values()]:
            checked(ref)
            require(ref['path'] not in frozen or frozen[ref['path']] == ref['sha256'],
                    'window evidence conflicts with original binding')
            frozen[ref['path']] = ref['sha256']
    validate_plan(result)
    if 'legacy_window' in result:
        _validate_window(result, proof)
    return result


def round_view(plan, logical_round, attempt):
    metadata(plan)
    require(type(logical_round) is int and logical_round in (1, 2, 3)
            and type(attempt) is int and attempt in (1, 2), 'invalid logical round/attempt')
    require('legacy_window' not in plan or logical_round == 3, 'certified rounds one and two cannot be rerun')
    view = copy.deepcopy(routing.unseal(plan['parent']))
    view.update(output_root=str(Path(plan['output_root']) / f'round-{logical_round}' / f'attempt-{attempt}'),
                frozen_files=copy.deepcopy(plan['frozen_files']), receipt_generator=plan['receipt_generator'],
                round_contract={'parallel_plan_sha256': routing.identity(plan),
                                'logical_round': logical_round, 'attempt': attempt})
    return view


def producer(plan, proof, logical_round, attempt):
    spec = validate_plan(plan)
    require(routing.identity(proof) == plan['proof_sha256'], 'wrong original proof')
    parent = metadata(plan)['parent']
    if 'legacy_window' in plan:
        _validate_window(plan, proof)
    wrappers, outcomes, exact = _prerequisite(parent, spec, proof)
    view = round_view(plan, logical_round, attempt)
    output = Path(view['output_root']) / 'collection/round-1'
    result = discovery.invoke(parent, spec, wrappers, output)
    discovery.round_data(parent, spec, result, outcomes, exact)
    validate_plan(plan)
    write(output.parent / 'producer-complete.json', {
        'kind': KIND + '_producer_complete', 'round_contract': view['round_contract'],
        'records': reference(output / 'records.json')})
    return result


def _snapshot_partial(directory):
    """Save an interruption boundary before unchanged producer cleanup runs."""
    saved = directory.parent / 'interruption'
    saved.mkdir(exist_ok=False)
    refs = {}
    for name in ('records.json', 'current.json', 'current.json.columns', 'current.log'):
        source = directory / name
        if source.exists():
            destination = saved / name
            shutil.copyfile(source, destination)
            refs[name] = reference(destination)
    return refs


def _proc(pid):
    try:
        fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
        return {'pid': pid, 'ppid': int(fields[1]), 'start': int(fields[19])}
    except FileNotFoundError:
        return None


def _owned_descendants(process):
    """Capture Linux child identities before cooperative interrupt/escalation."""
    root = _proc(process.pid)
    if root is None:
        return {}
    found, pending = {process.pid: root}, [root]
    while pending:
        parent = pending.pop()
        try:
            children = Path(f"/proc/{parent['pid']}/task/{parent['pid']}/children").read_text().split()
        except FileNotFoundError:
            continue
        for text in children:
            child = _proc(int(text))
            if child and child['ppid'] == parent['pid'] and _proc(parent['pid']) == parent:
                found[child['pid']] = child
                pending.append(child)
    return found


def _stop_owned(process):
    """Always reap our producer; only captured matching child PIDs may escalate."""
    if process.poll() is not None:
        return {'returncode': process.wait(), 'escalated': False}
    owned, capture_error = {}, None
    try:
        owned = _owned_descendants(process)
    except (OSError, ValueError, IndexError) as error:
        capture_error = str(error)
    try:
        os.killpg(process.pid, signal.SIGINT)
    except ProcessLookupError:
        pass  # The unreaped child exited between poll and signal.
    try:
        return {'returncode': process.wait(timeout=30), 'escalated': False}
    except (subprocess.TimeoutExpired, KeyboardInterrupt, SystemExit, OSError):
        try:
            owned.update(_owned_descendants(process))
        except (OSError, ValueError, IndexError) as error:
            capture_error = str(error)
        try:
            for pid, identity in reversed(list(owned.items())):
                if pid != process.pid and _proc(pid) == identity:
                    try:
                        os.kill(pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
        finally:
            # Descendant inspection/signal failures cannot skip reaping our child.
            try:
                process.kill()  # Direct owned Popen child; its PID cannot be reused unreaped.
            except ProcessLookupError:
                pass
            code = process.wait(timeout=10)
        require(capture_error is None and process.pid in owned,
                'producer cleanup could not establish every child identity; fail this scheduler job')
        return {'returncode': code, 'escalated': True}


def _operation(plan, plan_path, logical_round, attempt, view, proof, output, failure_event):
    require(output == Path(view['output_root']) / 'collection', 'wrong observed output')
    output.mkdir(parents=True, exist_ok=False)
    machine = discovery.host()
    require(machine['library_limits'] == discovery.LIBRARY_LIMITS, 'set frozen library limits before launch')
    require(machine['slots'] == 28 and machine['machine'] == proof['host']['machine'] == 'x86_64',
            'run on the frozen complete 28-core x86_64 allocation')
    result = {'kind': discovery.KIND + '_collection', 'plan_sha256': routing.identity(view),
              'prerequisites_sha256': routing.identity(proof), 'round_contract': view['round_contract'],
              'host': machine, 'rounds': [], 'status': 'incomplete', 'collection_pid': os.getpid(),
              'started_wall_time_ns': time.time_ns(), 'started_monotonic_ns': time.monotonic_ns()}
    command = [sys.executable, str(Path(__file__).resolve()), 'producer', '--plan', str(plan_path),
               '--round', str(logical_round), '--attempt', str(attempt)]
    process, interrupted, partial, cleanup, observer_fault = None, False, {}, None, False
    prior_term, prior_int = signal.getsignal(signal.SIGTERM), signal.getsignal(signal.SIGINT)
    def terminate(_signum, _frame):
        raise KeyboardInterrupt('supervisor received SIGTERM')
    signal.signal(signal.SIGTERM, terminate)
    try:
        with (output / 'producer.log').open('x') as log:
            process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            while process.poll() is None:
                if failure_event.wait(.1):
                    observer_fault = True
                    # Preserve the unchanged full scientific round. A required
                    # observer fault invalidates timing and never resets its budget.
                    time.sleep(.1)
    except BaseException as error:
        interrupted = True
        result['supervisor_error'] = f'{type(error).__name__}: {error}'
        try:
            partial = _snapshot_partial(output / 'round-1')
        except (OSError, ValueError) as snapshot_error:
            result['snapshot_error'] = str(snapshot_error)
    finally:
        cleanup_signals = []
        signal.signal(signal.SIGINT, lambda signum, frame: cleanup_signals.append(signum))
        signal.signal(signal.SIGTERM, lambda signum, frame: cleanup_signals.append(signum))
        try:
            if process is not None:
                cleanup = _stop_owned(process)
        except BaseException as error:
            result['supervisor_error'] = 'producer cleanup failed: ' + str(error)
        finally:
            signal.signal(signal.SIGTERM, prior_term)
            signal.signal(signal.SIGINT, prior_int)
            if cleanup_signals:
                interrupted = True
                result['supervisor_error'] = 'supervisor interrupted during owned producer cleanup'
            record = output / 'round-1/records.json'
            if record.exists():
                result['rounds'] = [reference(record)]
            result.update(completed_wall_time_ns=time.time_ns(), completed_monotonic_ns=time.monotonic_ns(),
                          producer_returncode=cleanup['returncode'] if cleanup else None,
                          producer_cleanup=cleanup, supervisor_interrupted=interrupted, observer_failure_seen=observer_fault,
                          interruption_artifacts=partial)
            complete = output / 'producer-complete.json'
            if cleanup and cleanup['returncode'] == 0 and complete.exists() and not interrupted:
                report = load(complete)
                require(report['round_contract'] == view['round_contract']
                        and report['records'] == reference(record), 'producer completion identity differs')
                require(discovery.host() == machine, 'allocation changed')
                result['status'] = 'complete'
            write(output / 'collection.json', result)
    return result


def collect(plan, proof, logical_round, attempt, plan_path):
    validate_plan(plan)
    require(load(plan_path) == plan and routing.identity(proof) == plan['proof_sha256'], 'wrong frozen plan/proof')
    view = round_view(plan, logical_round, attempt)
    if attempt == 2:
        prior = inspect_attempt(plan, proof, logical_round, 1)
        require(prior['status'] == 'infra_failure' and 'accounting' in prior,
                'replacement requires terminal first-attempt infrastructure proof')
    def operation(p, science, output, event):
        return _operation(plan, plan_path, logical_round, attempt, p, science, output, event)
    return isolation_host.observe(view, proof, operation=operation)


def _infra_errors(view, collection):
    errors = []
    process_path = Path(view['output_root']) / 'isolation/process_observations.jsonl'
    end_ns = collection['completed_monotonic_ns']
    if process_path.exists():
        endings = [r for r in isolation_host.rows(process_path) if r.get('kind') == 'end']
        require(len(endings) <= 1, 'ambiguous observer end')
        if endings:
            require(endings[0].get('collection_sha256') == routing.identity(collection)
                    and type(endings[0].get('monotonic_ns')) is int, 'observer end identity differs')
            end_ns = endings[0]['monotonic_ns']
    for name in ('scheduler.jsonl', 'process_observations.jsonl'):
        path = Path(view['output_root']) / 'isolation' / name
        if not path.exists():
            continue
        for row in isolation_host.rows(path):
            if row.get('kind') == 'error':
                require(type(row.get('monotonic_ns')) is int and type(row.get('wall_time_ns')) is int
                        and isinstance(row.get('error'), str) and row['error']
                        and collection['started_monotonic_ns'] <= row['monotonic_ns']
                        <= end_ns, 'ambiguous external observer error')
                errors.append({'file': reference(path), 'observation': row})
    require(errors, 'missing raw external infrastructure error proof')
    return errors


def inspect_attempt(plan, proof, logical_round, attempt):
    """Fail closed: classify from retained bytes, never from retry assertions."""
    spec = validate_plan(plan)
    require(routing.identity(proof) == plan['proof_sha256'], 'wrong original proof')
    view = round_view(plan, logical_round, attempt)
    root = Path(view['output_root'])
    collection_path = root / 'collection/collection.json'
    collection = load(collection_path)
    require(collection.get('kind') == discovery.KIND + '_collection'
            and collection.get('plan_sha256') == routing.identity(view)
            and collection.get('prerequisites_sha256') == routing.identity(proof)
            and collection.get('round_contract') == view['round_contract'], 'attempt binding differs')
    result = {'kind': KIND + '_attempt', **view['round_contract'],
              'collection': reference(collection_path), 'status': 'failed'}
    try:
        require(len(collection.get('rounds', [])) == 1, 'missing or duplicate producer records')
        records_path = checked(collection['rounds'][0])
        require(records_path == (root / 'collection/round-1/records.json').resolve(), 'wrong producer destination')
        records = routing.read_json(records_path)
        parent = metadata(plan)['parent']
        _, outcomes, exact = _prerequisite(parent, spec, proof)
        if collection['status'] == 'complete':
            discovery.round_data(parent, spec, records, outcomes, exact)
            receipt_path = root / 'isolation/receipt.json'
            if receipt_path.exists():
                receipt = load(receipt_path)
                # Independent frozen replay: a separate 300-second budget per receipt.
                discovery.isolation(view, collection, receipt)
                result.update(status='complete', isolation=reference(receipt_path))
            else:
                # A cleanup observer fault can occur after producer success.
                # Complete science remains checked before any infra classification.
                result.update(status='infra_failure', infrastructure_errors=_infra_errors(view, collection))
        else:
            raise ValueError('incomplete producer science cannot be replaced automatically')
    except (ValueError, OSError, KeyError, TypeError, AttributeError, IndexError, subprocess.SubprocessError) as error:
        result['error'] = str(error)
    if result['status'] == 'infra_failure':
        account = root / 'isolation/accounting.json'
        if account.exists():
            try:
                status = isolation_host.accounting(routing.read_json(account), collection)
                require(status['failed'] == 0 and status['exit_status'] in (0, 1),
                        'infrastructure attempt has unexplained terminal scheduler failure')
                result['accounting'] = reference(account)
            except (ValueError, OSError, KeyError, TypeError) as error:
                result.update(status='failed', error='invalid terminal accounting: ' + str(error))
    return result


def finalize(plan, proof, logical_round, attempt):
    validate_plan(plan)
    require(routing.identity(proof) == plan['proof_sha256'], 'wrong original proof')
    view = round_view(plan, logical_round, attempt)
    collection = load(Path(view['output_root']) / 'collection/collection.json')
    try:
        directory = Path(view['output_root']) / 'isolation'
        receipt_path, account = directory / 'receipt.json', directory / 'accounting.json'
        if receipt_path.exists():
            receipt = load(receipt_path)
        elif account.exists():
            # Offline recovery reuses captured accounting; it never replaces it.
            isolation_host.accounting(routing.read_json(account), collection)
            raw = {key: reference(directory / (key + ('.json' if key == 'accounting' else '.jsonl')))
                   for key in discovery.EVIDENCE}
            receipt = {'kind': discovery.KIND + '_isolation', 'schema': 1,
                'plan_sha256': routing.identity(view), 'collection_sha256': routing.identity(collection),
                'generator': view['receipt_generator'], 'host': collection['host'], 'rounds': collection['rounds'],
                'raw_evidence': raw, **isolation_host.derived(view, collection, raw)}
            isolation_host.validate(view, collection, receipt)
            write(receipt_path, receipt)
        else:
            # This captures terminal accounting even when isolation cannot qualify.
            receipt = isolation_host.finalize(view, collection)
        isolation_host.validate(view, collection, receipt)
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError):
        result = inspect_attempt(plan, proof, logical_round, attempt)
        if result['status'] != 'infra_failure':
            raise
        require('accounting' in result, 'terminal infrastructure accounting required')
        return result
    return inspect_attempt(plan, proof, logical_round, attempt)


def aggregate(plan, proof, attempts):
    spec = validate_plan(plan)
    require(routing.identity(proof) == plan['proof_sha256'], 'wrong original proof')
    require(isinstance(attempts, list), 'attempt list required')
    labels = [(a['logical_round'], a['attempt']) for a in attempts]
    require(len(labels) == len(set(labels)) and all(n in (1, 2, 3) and m in (1, 2) for n, m in labels),
            'duplicate or invalid attempt')
    fresh_rounds = (3,) if 'legacy_window' in plan else (1, 2, 3)
    require({n for n, _ in labels} == set(fresh_rounds), 'required logical rounds missing or duplicated')
    reports, selected, allocations = [], [], set()
    certificate = _validate_window(plan, proof) if 'legacy_window' in plan else None
    if certificate:
        allocations.add(routing.unseal(certificate['collection'])['host']['job_id'])
    for n in fresh_rounds:
        round_attempts = sorted([m for i, m in labels if i == n])
        require(round_attempts in ([1], [1, 2]), 'attempt one is required')
        present = sorted(p.name for p in (Path(plan['output_root']) / f'round-{n}').glob('attempt-*'))
        require(present == [f'attempt-{m}' for m in round_attempts], 'all attempted outputs must be retained')
        current = [inspect_attempt(plan, proof, n, m) for m in round_attempts]
        # Caller supplies bound on-disk reports, not self-asserted statuses.
        for m, report in zip(round_attempts, current):
            supplied = attempts[labels.index((n, m))]
            require(supplied == report, 'attempt report differs from independently derived classification')
        require(current[0]['status'] == 'complete' or (len(current) == 2
                and current[0]['status'] == 'infra_failure' and 'accounting' in current[0]
                and current[1]['status'] == 'complete'),
                'failed or incomplete round cannot select')
        require(not (len(current) == 2 and current[0]['status'] == 'complete'),
                'a complete first attempt cannot be replaced')
        chosen = current[-1]
        collection = load(checked(chosen['collection']))
        for report in current:
            observed = load(checked(report['collection']))
            job = observed['host']['job_id']
            require(isinstance(job, str) and job and job not in allocations, 'distinct allocation IDs required')
            allocations.add(job)
        reports.extend(current)
        selected.append(chosen)
    parent = metadata(plan)['parent']
    _, outcomes, exact = _prerequisite(parent, spec, proof)
    combined = {}
    refs = list(certificate['rounds']) if certificate else []
    for report in selected:
        collection = load(checked(report['collection']))
        refs.extend(collection['rounds'])
    for ref in refs:
        data = discovery.round_data(parent, spec, routing.read_json(checked(ref)), outcomes, exact)
        for key, values in data.items():
            if values is None:
                require(key not in combined or combined[key] is None, 'availability changed')
                combined[key] = None
            else:
                combined.setdefault(key, []).extend(values)
    require(all(v is None or len(v) == 9 for v in combined.values()), 'nine paired slots required')
    ranking = discovery.score(spec, combined, outcomes)
    envelope = {'kind': KIND + '_collection', 'plan_sha256': routing.identity(plan),
                'proof_sha256': routing.identity(proof), 'attempts': reports, 'selected': selected}
    receipts = [report['isolation'] for report in selected]
    if certificate:
        envelope['legacy_window'] = plan['legacy_window']
        receipts.insert(0, plan['legacy_window']['reference'])
    return {'kind': discovery.KIND + '_shortlist', 'schema': 1,
            'plan_sha256': routing.identity(plan), 'collection_sha256': routing.identity(envelope),
            'isolation_sha256': routing.identity(receipts), 'collection': envelope, 'isolation': receipts,
            'status': 'discovery_only', 'claim': DESIGN['inference'], 'promotion': False,
            'protocol_source': KIND, 'ranking': ranking, 'shortlist': [r['arm'] for r in ranking[:4]]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['freeze', 'collect', 'producer', 'finalize', 'inspect-attempt', 'aggregate'])
    parser.add_argument('--plan', type=Path, required=True)
    parser.add_argument('--round', dest='logical_round', type=int)
    parser.add_argument('--attempt', type=int)
    parser.add_argument('--attempts', type=Path, help='JSON list of derived attempt reports')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if args.command == 'freeze':
        write(routing.scratch(args.output), freeze(routing.read_json(args.plan)))
        return 0
    plan = load(args.plan)
    proof = load(checked(plan['proof']))
    if args.command == 'aggregate':
        result = aggregate(plan, proof, routing.read_json(args.attempts))
    elif args.command == 'producer':
        producer(plan, proof, args.logical_round, args.attempt)
        return 0
    elif args.command == 'collect':
        result = collect(plan, proof, args.logical_round, args.attempt, args.plan)
    elif args.command == 'finalize':
        result = finalize(plan, proof, args.logical_round, args.attempt)
    else:
        result = inspect_attempt(plan, proof, args.logical_round, args.attempt)
    if args.output:
        write(routing.scratch(args.output), result)
    else:
        print(json.dumps(routing.seal(result), allow_nan=False))
    return int(result.get('status') not in ('complete', 'infra_failure', 'discovery_only'))


if __name__ == '__main__':
    raise SystemExit(main())
