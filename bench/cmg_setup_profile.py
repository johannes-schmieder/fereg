#!/usr/bin/env python3
"""Run the fixed 18-call CMG setup attribution panel after cmg_setup.py.

Usage: cmg_setup_profile.py REPOSITORY TASK_ROOT OUTPUT
The task root holds plan-PLATFORM.json, setup-profile.patch, the independently
built artifacts/PLATFORM/setup-profile.plugin, and the completed diagnostic.
This profile never supplies qualification timing. The caller owns host isolation.
"""
from pathlib import Path
import os
import platform
import subprocess
import sys


class Bindings:
    """One initial hash per file, then qualify's cheap stat/alias checks."""
    def __init__(self, q):
        self.q = q
        self.aliases = {}
        self.stamps = {}
        self.hashes = {}
        self.sources = None

    def bind(self, path, expected=None):
        self.check()
        path = Path(path).absolute()
        resolved = path.resolve(strict=True)
        if resolved not in self.hashes:
            before = self.q.stamp(resolved)
            self.hashes[resolved] = self.q.digest(resolved)
            self.q.require(self.q.stamp(resolved) == before, 'profile input changed while hashing')
            self.stamps[resolved] = before
        self.q.require(expected is None or self.hashes[resolved] == expected,
                       'profile input identity mismatch: ' + str(path))
        self.aliases[path] = resolved
        self.check()
        return self.hashes[resolved]

    def verify_sources(self, expected):
        self.sources = self.q.execution_sources()
        self.stamps.update(self.q.verify_sources(expected))
        self.hashes.update({self.sources[name]: digest for name, digest in expected.items()})
        self.check()

    def watch_request(self, path):
        path = Path(path).absolute()
        resolved = path.resolve(strict=True)
        self.aliases[path] = resolved
        self.stamps[resolved] = self.q.stamp(resolved)

    def forget_request(self, path):
        resolved = self.aliases.pop(Path(path).absolute())
        self.stamps.pop(resolved)

    def check(self):
        self.q.require(all(path.resolve(strict=True) == resolved for path, resolved in self.aliases.items()),
                       'profile input alias changed')
        self.q.require(all(self.q.stamp(path) == before for path, before in self.stamps.items()),
                       'profile input changed during execution')
        self.q.require(self.sources is None or self.q.execution_sources() == self.sources,
                       'profile execution source aliases changed')


def load_context(repo, root, system):
    """Read and bind the existing frozen plan and completed reference; no calls."""
    import qualify as q
    import cmg_setup
    q.require(Path(q.__file__).resolve() == (repo / 'bench/qualify.py').resolve(strict=True),
              'profile imported qualify from another repository')
    guard = Bindings(q)
    plan = root / f'plan-{system}.json'
    plan_hash = guard.bind(plan)
    manifest = q.read(plan)
    q.require(manifest['seed'] == 2026102103 and manifest['platform'] == system, 'profile plan seed/platform changed')
    cmg_setup.validate(manifest)
    q.validate_cell_policy(manifest)
    q.require(manifest['artifacts'][cmg_setup.POST]['revision'] == cmg_setup.REVISIONS[cmg_setup.POST],
              'profile hidden baseline revision changed')
    guard.verify_sources(manifest['execution_sources'])
    required_files = {manifest['harness'], *[a['path'] for a in manifest['artifacts'].values()]}
    for item in manifest['cases']:
        required_files.add(item['input'])
        required_files.update(item[key] for key in ('metadata',) if item.get(key))
        required_files.update(item['oracle'][key] for key in ('path', 'arrays') if key in item['oracle'])
    q.require(required_files <= set(manifest['files']), 'profile plan omits a required file binding')
    for name, expected in manifest['files'].items():
        guard.bind(plan.parent / name, expected)
    base_path = root / ('diagnostic-Darwin/run-0-0/results.json' if system == 'Darwin'
                        else 'runner/target/tmp/cmg-upgrade/diagnostic-Linux/run-0-0/results.json')
    base_hash = guard.bind(base_path)
    base = q.read(base_path)
    q.require(base['status'] == 'complete' and not base['failures'] and base['budget_compliant'] is True,
              'profile requires a completed baseline without failures')
    q.require(base['manifest_sha256'] == q.routing.identity(manifest)
              and base['execution_sources'] == manifest['execution_sources']
              and base['platform'] == system and base['replica'] == base['shard'] == 0,
              'profile baseline is not bound to this frozen manifest')
    expected_cells = {f"{item['id']}-t{workers}" for item in manifest['cases'] for workers in manifest['threads']}
    q.require(len(base['cases']) == len(expected_cells) == 12
              and {c['id'] for c in base['cases']} == expected_cells,
              'profile baseline case coverage changed')
    for name in ('high-degree-weak', 'weak-low'):
        for workers in (1, 4, 8):
            cells = [c for c in base['cases'] if c['id'] == f'{name}-t{workers}']
            q.require(len(cells) == 1 and cells[0]['status'] == 'complete'
                      and cells[0]['aliases'] == q.artifact_aliases(manifest), 'profile baseline cell is incomplete')
            warmups = [c for c in cells[0]['warmups'] if c['execution'] == cmg_setup.POST]
            q.require(len(warmups) == 1 and q.routing.sha(warmups[0]['fingerprint']),
                      'profile baseline lacks its hidden scientific fingerprint')
    plugin = root / f'artifacts/{system}/setup-profile.plugin'
    patch_hash = guard.bind(repo / 'bench/cmg_setup_profile.patch')
    guard.bind(root / 'setup-profile.patch', patch_hash)
    identity = {
        'base_revision': cmg_setup.REVISIONS[cmg_setup.POST],
        'manifest_sha256': q.routing.identity(manifest), 'plan_file_sha256': plan_hash,
        'patch_sha256': patch_hash, 'plugin_sha256': guard.bind(plugin),
        'baseline_sha256': manifest['files'][manifest['artifacts'][cmg_setup.POST]['path']],
        'baseline_result_sha256': base_hash, 'helper_sha256': guard.bind(Path(__file__)),
        'calls': 18, 'warmups_per_cell': 1, 'recorded_calls_per_cell': 2, 'timeout_s': 120,
        'purpose': 'phase attribution only; not a speed or activation claim',
    }
    guard.check()
    return plan, manifest, base, plugin, identity, guard


def main():
    repo, root, out = (Path(arg).resolve() for arg in sys.argv[1:])
    sys.path.insert(0, str(repo / 'bench'))
    import qualify as q
    import cmg_setup
    import routing

    system = platform.system()
    plan, manifest, base, plugin, identity, guard = load_context(repo, root, system)
    out.mkdir(parents=True, exist_ok=False)
    q.write(out / 'protocol.json', identity)
    records = []
    for name in ('high-degree-weak', 'weak-low'):
        item = next(c for c in manifest['cases'] if c['id'] == name)
        guard.check()
        meta = q.metadata(manifest, item, plan)
        for workers in (1, 4, 8):
            cell = next(c for c in base['cases'] if c['id'] == f'{name}-t{workers}')
            expected = next(c for c in cell['warmups'] if c['execution'] == cmg_setup.POST)['fingerprint']
            request = out / f'{name}-t{workers}.case'
            routing.prepare(q.resolve(plan, item['input']), request, 'incremental-two', workers)
            guard.watch_request(request)
            for repeat in range(3):
                label = f'{name}-t{workers}-{repeat}'
                result, log = out / (label + '.json'), out / (label + '.log')
                env = {k: v for k, v in os.environ.items() if k not in q.ENVIRONMENT}
                env.update({k: '1' for k in q.THREAD_ENV})
                env.update(FEREG_SOLVER_TRACE='1', FEREG_PHASE_TIMING='1')
                guard.check()
                with log.open('x') as stream:
                    subprocess.run([str(q.resolve(plan, manifest['harness'])), str(plugin),
                                    '--replay', str(request), str(result)],
                                   stdout=stream, stderr=subprocess.STDOUT, env=env,
                                   check=True, timeout=120)
                guard.check()
                _, columns, detail = q.read_native(result, log, meta)
                q.require(detail['fingerprint'] == expected,
                          'instrumentation changed science/signature: ' + label)
                trace = log.read_text()
                cmg_setup.resource_diagnostics(trace, cmg_setup.POST, cmg_setup.REVISIONS[cmg_setup.POST])
                phases = [dict(p.split('=', 1) for p in line.split('|')[1:] if '=' in p)
                          for line in trace.splitlines()
                          if line.startswith('FEREG_SOLVER_TRACE|phase=auto_schur_setup_profile|')]
                q.require(phases and all(p['outcome'] == 'built' and p['completed_phases'] == '6'
                                         for p in phases), 'profile did not finish all six phases')
                guard.check()
                records.append({'case': name, 'workers': workers, 'repeat': repeat,
                                'warmup': repeat == 0, 'fingerprint': detail['fingerprint'],
                                'phases': phases, 'log_sha256': q.digest(log)})
                q.write(out / 'results.json', {'status': 'running', 'identity': identity, 'calls': records})
                columns.unlink()
                result.unlink()
            guard.check()
            guard.forget_request(request)
            request.unlink()
            print(name, workers, 'PROFILE_SCIENCE_PASS', flush=True)
    guard.check()
    q.require(len(records) == 18, 'profile call count changed')
    q.write(out / 'results.json', {'status': 'complete', 'identity': identity, 'calls': records})
    print('SETUP_PROFILE_COMPLETE')


if __name__ == '__main__':
    main()
