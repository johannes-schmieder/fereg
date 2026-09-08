#!/usr/bin/env python3
"""Bounded exact-source validation of a frozen policy, without auto promotion."""
from __future__ import annotations

import argparse
import importlib.util
import math
import os
from pathlib import Path
import platform
import signal
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / 'benchmarks/manifests/feppml_bounded_auto_validation_v1.json'
spec = importlib.util.spec_from_file_location('cal', ROOT / 'scripts/ppml_auto_calibration.py')
assert spec and spec.loader
cal = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cal)
dev = cal.dev


def manifest() -> dict:
    m = dev.read_json(MANIFEST)
    if (m['schema_version'] != 1 or m['qualification_eligible'] is not False
            or m['activate_auto'] is not False or m['timeout_policy']['policy_refit'] is not False
            or m['timeout_policy']['per_fit_lower_bound'] is not None):
        raise dev.DevelopmentError('invalid bounded diagnostic policy')
    if any(not isinstance(v, int) or v <= 0 for v in m['seconds'].values()):
        raise dev.DevelopmentError('invalid process budget')
    if m['seconds']['local_total'] > 14400 or m['seconds']['scope'] + 60 > 1800:
        raise dev.DevelopmentError('validation exceeds its wallclock envelope')
    return m


def scopes(m: dict) -> list[dict]:
    result = [s for t in m['prior_v3_tasks'] for s in cal.scopes('scc_holdout', t)]
    if len(result) != m['expected_prior_scopes']:
        raise dev.DevelopmentError('prior scope inventory changed')
    g = m['guardrails']
    for index, (topology, k) in enumerate(g['topology_controls']):
        for threads in g['threads']:
            result.append(dict(id=f'bounded-{topology}-n{g["n"]}-k{k}-t{threads}',
                               n=g['n'], controls=k, topology=topology, difficulty='easy',
                               threads=threads, seed=g['seed_base'] + index))
    dev._validate_scopes(result, m['expected_scc_scopes'])
    return result


def requested_solver(scope: dict, policy: dict) -> str:
    # No fitted regions were admitted by the pre-holdout frozen policy.
    if policy != {'one_fe': 'map', 'default_multi_fe': 'lsmr',
                  'unsupported_or_unadmitted': 'map', 'regions': [],
                  'training_ratio_to_default': 1.0}:
        raise dev.DevelopmentError('unexpected frozen routing policy')
    return 'map' if scope['topology'].endswith('_1fe') else 'lsmr'


def process_budget(environment: dict, selected: str, seconds: dict) -> int:
    mode = environment.get('FEREG_PPML_SOLVER_MODE')
    arm = environment.get('FEREG_PPML_SOLVER_ARM', '')
    if mode == 'run':
        if arm in (*dev.COMPARATORS, dev.ADJUDICATOR):
            return seconds['reference']
        if arm == selected:
            return seconds['selected']
        return seconds['nonselected_map'] if arm == 'map' else seconds['other_solver']
    return seconds['setup'] if mode in ('generate', 'prepare-reference') else seconds['public_state']


def terminate_group(process: subprocess.Popen, grace: float) -> None:
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=grace)
    except subprocess.TimeoutExpired:
        pass
    # Reap descendants even if the direct parent handled SIGTERM first.
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait()


class BoundedStata:
    def __init__(self, selected: str, seconds: dict, deadline: float):
        self.selected, self.seconds, self.deadline = selected, seconds, deadline
        self.records: list[dict] = []

    def __call__(self, stata: str, environment: dict, console: Path,
                 driver: Path = dev.DRIVER) -> dict:
        budget = min(process_budget(environment, self.selected, self.seconds),
                     self.deadline - time.monotonic())
        if budget <= 0:
            raise dev.DevelopmentError('scope budget exhausted before next process')
        console.parent.mkdir(parents=True, exist_ok=True)
        clean = {k: v for k, v in os.environ.items() if not k.startswith('FEREG_PPML_SOLVER_')}
        clean.update(environment)
        clean['FEREG_PPML_SOLVER_STATA_LOG'] = str(console.with_suffix('.stata.log'))
        if clean.get('FEREG_SOLVER_TRACE'):
            clean['FEREG_SOLVER_TRACE_FILE'] = str(console.with_suffix('.solver.trace'))
        command = [stata, '-b', 'do', str(driver)]
        start = time.monotonic()
        expired = False
        with console.open('xb') as target:
            process = subprocess.Popen(command, cwd=console.parent, env=clean, stdout=target,
                                       stderr=subprocess.STDOUT, start_new_session=True)
            try:
                rc = process.wait(timeout=budget)
            except subprocess.TimeoutExpired:
                expired = True
                terminate_group(process, self.seconds['terminate_grace'])
                rc = 124
            except BaseException:
                terminate_group(process, self.seconds['terminate_grace'])
                raise
        record = dict(command=command, console=str(console), process_returncode=rc,
                      budget_seconds=budget, elapsed_seconds=time.monotonic() - start,
                      budget_exhausted=expired, per_fit_lower_bound=None)
        self.records.append(record)
        dev.exclusive_json(console.with_suffix('.process.json'), record)
        return record


def selected_diagnostics(run: dict, selected: str) -> dict:
    scope = run['scopes'][0]
    rows = [r for r in run['records'] if r['implementation'] == 'candidate' and r['arm'] == selected]
    engine = 1 if selected == 'map' else 2 if scope['controls'] == 0 else 3
    processes = [p for p in run['processes'] if p['stage'] == 'run'
                 and p['implementation'] == 'candidate' and p['arm'] == selected]
    complete = (len(processes) == 1 and processes[0]['accepted_rc'] == 0
                and not processes[0].get('budget_exhausted', False)
                and len(rows) == run['repetitions']
                and {r['repetition'] for r in rows} == set(range(1, run['repetitions'] + 1)))
    certified = complete and all(
        r['rc'] == 0 and r['converged'] == 1 and r['selected_engine_code'] == engine
        and r['effective_engine_code'] == engine and r['route_code'] == dev.ROUTE_CODES[selected]
        and r['effective_threads'] == scope['threads'] and r['fallback_count'] == 0
        and len(r['certificate_values']) == 16 and r['certificate_values'][-1] == 1
        and len(r['runtime_values']) == 108 and math.isfinite(r['runtime_values'][22])
        and 0 <= r['runtime_values'][22] <= 1.25 for r in rows)
    return {'solver': selected, 'execution_complete': complete,
            'recorded_runtime_certification_pass': bool(certified),
            'median_seconds': statistics.median(r['seconds'] for r in rows) if certified else None,
            'median_inner_iterations': statistics.median(r['inner_iterations'] for r in rows) if certified else None,
            'engines': dev.solver_engine_summary(rows) if certified else {}}


def controller_identity() -> dict:
    paths = ['scripts/ppml_bounded_validation.py', 'scripts/ppml_auto_calibration.py',
             'benchmarks/manifests/feppml_bounded_auto_validation_v1.json',
             'benchmarks/scc/feppml_bounded_validation_job.sh',
             'tests/python/test_ppml_bounded_validation.py']
    files = {p: dev.sha256_file(ROOT / p) for p in paths}
    files.update(dev.implementation_identity()['files'])
    return {'files': files, 'sha256': dev.canonical_sha256(files)}


def prepare(args: argparse.Namespace) -> None:
    m = manifest()
    policy = dev.read_json(args.policy)
    if dev.sha256_file(args.policy) != m['frozen_policy_sha256']:
        raise dev.DevelopmentError('frozen policy changed')
    planned = scopes(m)
    for scope in planned:
        requested_solver(scope, policy['policy'])
    args.output.mkdir(parents=True, exist_ok=False)
    dev.exclusive_json(args.output / 'config.json', {
        'manifest_sha256': dev.sha256_file(MANIFEST), 'controller': controller_identity(),
        'policy': policy, 'policy_sha256': dev.sha256_file(args.policy), 'scopes': planned,
        'qualification_eligible': False, 'activate_auto': False})


def validate_config(path: Path) -> tuple[dict, dict]:
    m, config = manifest(), dev.read_json(path)
    if (config['manifest_sha256'] != dev.sha256_file(MANIFEST)
            or config['controller'] != controller_identity() or config['scopes'] != scopes(m)
            or dev.canonical_sha256(config['policy']) != m['frozen_policy_sha256']):
        raise dev.DevelopmentError('prospective controller, policy or scopes changed')
    return m, config


def run(args: argparse.Namespace) -> None:
    m, config = validate_config(args.config)
    source, artifact = dev.source_identity(args.candidate), dev.artifact_identity(args.candidate)
    if (source['commit'] != m['source_sha'] or source['dirty']
            or artifact['sha256'] != m['artifact_sha256'][platform.system()]):
        raise dev.DevelopmentError('candidate is not the exact repaired source and artifact')
    if args.task is not None:
        if not 1 <= args.task <= len(config['scopes']):
            raise dev.DevelopmentError('invalid task number')
        planned = [config['scopes'][args.task - 1]]
        if (os.environ.get('NSLOTS') != '28' or not os.environ.get('JOB_ID', '').isdigit()
                or int(os.environ.get('SGE_TASK_ID', str(args.task))) != args.task):
            raise dev.DevelopmentError('bounded task requires its 28-slot scheduler identity')
        if args.qhost is None:
            raise dev.DevelopmentError('missing exact host receipt')
        host = cal.host_receipt(args.qhost)
        include_oracle, repetitions = False, m['repetitions']
    else:
        planned = dev.profile_scopes(dev.load_manifest(), args.profile)
        include_oracle, repetitions, host = args.profile == 'local_science', 1, None
    args.output.mkdir(parents=True, exist_ok=False)
    start = time.monotonic()
    total_deadline = start + (m['seconds']['scope'] if args.task else m['seconds']['local_total'])
    receipts = []
    for index, scope in enumerate(planned, 1):
        selected = requested_solver(scope, config['policy']['policy'])
        runner = BoundedStata(selected, m['seconds'], min(total_deadline, time.monotonic() + m['seconds']['scope']))
        output = args.output / f'scope-{index:03d}'
        document = dev.execute_scopes(
            dev.load_manifest(), [scope], stata=args.stata, candidate_root=args.candidate,
            baseline_root=None, reference_root=args.reference, output_dir=output,
            repetitions=repetitions, include_oracle=include_oracle, prepare_comparators=True,
            task_identity={'job_id': os.environ.get('JOB_ID'), 'task_id': args.task,
                           'slots': os.environ.get('NSLOTS'), 'scheduler_host': host},
            process_runner=runner)
        dev.exclusive_json(output / 'run.json', document)
        analysis = dev.analyze_document(document)
        dev.exclusive_json(output / 'analysis.json', analysis)
        selected_result = selected_diagnostics(document, selected)
        receipt = {'scope': scope, 'source': source, 'artifact': artifact,
                   'run_sha256': dev.sha256_file(output / 'run.json'),
                   'analysis_sha256': dev.sha256_file(output / 'analysis.json'),
                   'selected': selected_result, 'science_pass': analysis['science_pass'],
                   'route_pass': analysis['route_pass'], 'memory': cal.memory_check(document),
                   'process_timeouts': [r for r in runner.records if r['budget_exhausted']]}
        dev.exclusive_json(output / 'receipt.json', receipt)
        receipts.append(receipt)
        print('BOUNDED_SCOPE', scope['id'], selected_result['recorded_runtime_certification_pass'],
              analysis['science_pass'], analysis['route_pass'], flush=True)
        if not selected_result['recorded_runtime_certification_pass']:
            raise dev.DevelopmentError('selected repaired route failed; evidence preserved')
        if args.task is None and (not analysis['science_pass'] or not analysis['route_pass']):
            raise dev.DevelopmentError('local science or smoke gate failed')
    if dev.source_identity(args.candidate) != source or dev.artifact_identity(args.candidate) != artifact:
        raise dev.DevelopmentError('source or plugin changed during execution')
    validate_config(args.config)
    dev.exclusive_json(args.output / 'receipt.json', {
        'kind': 'bounded_validation_capture', 'qualification_eligible': False,
        'activate_auto': False, 'config_sha256': dev.sha256_file(args.config),
        'source': source, 'artifact': artifact, 'task': args.task,
        'elapsed_seconds': time.monotonic() - start, 'scopes': receipts,
        'all_full_comparisons_pass': all(r['science_pass'] and r['route_pass'] and r['memory']['pass'] for r in receipts)})
    dev.exclusive_text(args.output / 'CAPTURE.COMPLETE', 'BOUNDED_CAPTURE_COMPLETE_NOT_SCIENTIFIC_ACCEPTANCE\n')
    print('BOUNDED_CAPTURE_COMPLETE', flush=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    p = commands.add_parser('prepare')
    p.add_argument('--policy', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p = commands.add_parser('run')
    p.add_argument('--config', type=Path, required=True)
    p.add_argument('--candidate', type=Path, required=True)
    p.add_argument('--reference', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--stata', required=True)
    p.add_argument('--qhost', type=Path)
    modes = p.add_mutually_exclusive_group(required=True)
    modes.add_argument('--task', type=int)
    modes.add_argument('--profile', choices=['local_science', 'local_smoke'])
    args = parser.parse_args()
    try:
        (prepare if args.command == 'prepare' else run)(args)
    except dev.DevelopmentError as error:
        parser.exit(1, f'Bounded validation failed: {error}\n')


if __name__ == '__main__':
    main()
