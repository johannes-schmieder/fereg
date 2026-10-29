#!/usr/bin/env python3
"""Fixed within entrypoint science; this runner never requests performance timings.

Prepare with exact, clean upstream checkouts and an empty explicit work directory:
  python3 bench/check_within_entrypoints.py prepare --work-dir <work> \
      --base <base-checkout> --api <api-checkout> --cancel <cancel-checkout>
  python3 bench/check_within_entrypoints.py build --work-dir <work> --cargo <cargo-1.85.1>
  python3 bench/check_within_entrypoints.py run --work-dir <work>

The fixed schedule has 72 native calls, no retries, a 120-second call limit and a
2700-second aggregate science budget. A failed or incomplete run needs review;
this script never replaces it. All generated files remain in the work directory.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import struct
import subprocess
import sys
import threading
import time

ROOT = Path(__file__).resolve().parent
SOURCE = ROOT / 'within_entrypoints.rs'
LOCK = ROOT / 'within_entrypoints.lock'
SOURCE_SHA256 = '94f212f00806a2f01d117429edd305b1e56cfd7cdc91a698eee11ed69959d612'
ARMS = {
    'base': ('upstream', '32dde1bdd604f33012e592270781129e09e82a92', 'legacy'),
    'api': ('within-api', 'cf228c412fdadac89a3ff28b12f53b7e1580f3de', 'controlled_default'),
    'cancel': ('within-cancel', 'b596c9854ea078dc40e903300d66a54d6062fcf0', 'cancellable_false_atomic'),
}
SCIENCE = ('input_fnv64', 'science_fnv64', 'maximum_error', 'iterations', 'recurrence_residuals')


def require(ok, message):
    if not ok:
        raise RuntimeError(message)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def read(path):
    return json.loads(Path(path).read_text())


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2, allow_nan=False) + '\n')


def exact_bytes(stream, count):
    pieces = []
    while count:
        piece = stream.read(count)
        require(bool(piece), 'truncated scientific stream')
        pieces.append(piece)
        count -= len(piece)
    return b''.join(pieces)


def read_fields(stream):
    """Hash bounded length-prefixed fields without keeping large arrays in memory."""
    fields = {}
    while True:
        head = stream.read(8)
        if not head:
            break
        require(len(head) == 8, 'truncated field-name length')
        length = struct.unpack('<Q', head)[0]
        require(length <= 4096, 'invalid scientific field name')
        name = exact_bytes(stream, length).decode()
        size = struct.unpack('<Q', exact_bytes(stream, 8))[0]
        require(name not in fields and size <= (1 << 31), 'duplicate or oversized scientific field')
        hashed, remaining, small = hashlib.sha256(), size, bytearray()
        while remaining:
            chunk = exact_bytes(stream, min(remaining, 1 << 20))
            hashed.update(chunk)
            if size <= 512:
                small.extend(chunk)
            remaining -= len(chunk)
        fields[name] = {'bytes': size, 'sha256': hashed.hexdigest()}
        if size <= 512:
            fields[name]['hex'] = small.hex()
    return fields


def capture(binary, cell, stderr_path, timeout=120):
    """Stream field SHA256s with bounded memory; the child has no external timers."""
    argv = [str(binary), str(cell['rows']), str(cell['slopes']), cell['preconditioner'], '--science-stream']
    with Path(stderr_path).open('wb') as stderr:
        process = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=stderr)
        expired = threading.Event()
        def stop():
            expired.set()
            try:
                process.kill()
            except ProcessLookupError:
                pass
        timer = threading.Timer(timeout, stop)
        timer.daemon = True
        timer.start()
        try:
            fields = read_fields(process.stdout)
            code = process.wait()
        except BaseException:
            process.kill()
            process.wait()
            raise
        finally:
            timer.cancel()
    require(not expired.is_set(), f'native science exceeded the {timeout}s call budget')
    require(code == 0, f'native science failed (exit {code}); stderr retained at {stderr_path}')
    def raw(name):
        return bytes.fromhex(fields[name]['hex'])
    def scalar(name, fmt='<Q'):
        return struct.unpack(fmt, raw(name))[0]
    require(scalar('schema_version') == 1, 'wrong scientific stream schema')
    config = struct.unpack('<10Q', raw('configuration'))
    require(config == (cell['rows'], 1, cell['slopes'], 1, 7,
                       struct.unpack('<Q', struct.pack('<d', 1e-12))[0], 1000, 8, 4, 1), 'science configuration differs')
    require(raw('preconditioner').decode() == cell['preconditioner'], 'science preconditioner differs')
    b = 16 if cell['rows'] == 8192 else 64
    a = cell['rows'] // (8 * b)
    dofs = a * max(cell['slopes'], 1) + 2 * b
    for index in range(4):
        prefix = f'rhs{index}'
        require(scalar(prefix + '/layout/n_dofs') == dofs and scalar(prefix + '/layout/n_terms') == 3,
                'coefficient shape differs')
        require(struct.unpack('<6Q', raw(prefix + '/layout/terms')) ==
                (a, max(cell['slopes'], 1), b, 1, b, 1), 'term layout differs')
        for field, size in [('coefficients', dofs * 8), ('demeaned', cell['rows'] * 8),
                            ('layout/addresses', dofs * 24)]:
            require(fields[prefix + '/' + field]['bytes'] == size, f'{prefix}/{field} length differs')
        require(scalar(prefix + '/converged') == 1, 'nonconverged science output')
    reason_text = raw('aux/stop_reasons').decode()
    return {'entrypoint': raw('aux/entrypoint').decode(), 'rows': cell['rows'], 'slopes': cell['slopes'],
            'mode': cell['preconditioner'], 'workers': 1, 'weighted': True, 'tolerance': 1e-12,
            'witness': 1e-8, 'rhs': 4, 'warmups': 1,
            'input_fnv64': f'{scalar("input_fnv64"):016x}', 'science_fnv64': f'{scalar("science_fnv64"):016x}',
            'maximum_error': max(scalar(f'rhs{i}/planted_residual_error', '<d') for i in range(4)),
            'iterations': [scalar(f'rhs{i}/iterations') for i in range(4)],
            'recurrence_residuals': [scalar(f'rhs{i}/residual', '<d') for i in range(4)],
            'controlled_stop_reasons': reason_text.split(',') if reason_text else [],
            'build_ns': None, 'elapsed_ns': None, 'total_ns': None,
            'exact_fields': {k: v for k, v in fields.items() if not k.startswith('aux/')}}


def validate(payload, cell, arm):
    expected = {'entrypoint': ARMS[arm][2], 'rows': cell['rows'], 'slopes': cell['slopes'],
                'mode': cell['preconditioner'], 'workers': 1, 'weighted': True, 'tolerance': 1e-12,
                'witness': 1e-8, 'rhs': 4, 'warmups': 1}
    require(all(payload.get(k) == v for k, v in expected.items()), 'native configuration/entrypoint mismatch')
    require(math.isfinite(payload['maximum_error']) and 0 <= payload['maximum_error'] <= 1e-8,
            'independent forward witness failed')
    require(len(payload['iterations']) == len(payload['recurrence_residuals']) == 4, 'wrong RHS count')
    require(all(type(x) is int and 0 <= x <= 1000 for x in payload['iterations']), 'invalid iterations')
    require(all(math.isfinite(x) and x >= 0 for x in payload['recurrence_residuals']), 'invalid recurrence residual')
    for key in ['input_fnv64', 'science_fnv64']:
        require(isinstance(payload[key], str) and len(payload[key]) == 16
                and all(c in '0123456789abcdef' for c in payload[key]), 'invalid scientific signature')
    reasons = payload['controlled_stop_reasons']
    require((len(reasons) == 4 and all(x in ('ResidualTolerance', 'NormalEquationTolerance') for x in reasons))
            if arm == 'api' else reasons == [], 'invalid controlled stop metadata')
    for key in ['build_ns', 'elapsed_ns', 'total_ns']:
        require(payload[key] is None, 'wrong external timer mode')
    require(isinstance(payload.get('exact_fields'), dict) and bool(payload['exact_fields']),
            'missing exact scientific fields')
    science = {key: payload[key] for key in SCIENCE}
    science['exact_fields'] = payload['exact_fields']
    return science


def same(left, right):
    return all(left[key] == right[key] for key in SCIENCE) and left['exact_fields'] == right['exact_fields']


def cells():
    return [{'id': f'{size}-weighted-slopes{slopes}-{mode}-w1',
             'rows': rows, 'slopes': slopes, 'preconditioner': mode, 'workers': 1, 'weighted': True}
            for size, rows in [('small', 8192), ('memory', 4194304)]
            for slopes in [0, 3] for mode in ['off', 'diagonal', 'additive']]


def configuration():
    return {'workers': 1, 'weighted': True, 'tolerance': 1e-12, 'witness_max_abs': 1e-8,
            'maxiter': 1000, 'local_size': 8, 'additive_seed': 7,
            'additive_reduction': 'ParallelReduction', 'rhs_sequence': [0, 1, 0, 1],
            'warmup_groups': 1, 'retained_groups': 1, 'repetitions': 2,
            'native_processes': 72, 'solves_including_warmups': 576,
            'per_call_timeout_seconds': 120, 'science_budget_seconds': 2700, 'retries': 0,
            'timing': False}


def check_checkout(arm, path):
    require(subprocess.check_output(['git', '-C', str(path), 'rev-parse', 'HEAD'], text=True).strip()
            == ARMS[arm][1], f'{arm}: wrong source revision')
    require(not subprocess.check_output(
        ['git', '-C', str(path), 'status', '--porcelain', '--untracked-files=all'], text=True),
        f'{arm}: implementation checkout is not clean')


def manifest(arm, checkout):
    return f'''[package]
name = "within-entrypoint-{arm}"
version = "0.0.0"
edition = "2021"

[workspace]

[dependencies]
within = {{ path = {json.dumps(str(checkout / 'crates/within'))} }}
rayon = "=1.12.0"

[[bin]]
name = "within-entrypoint-{arm}"
path = "../src/main.rs"

[profile.dev]
opt-level = 2
debug = 0
incremental = false

[features]
default = ["{arm}"]
base = []
api = []
cancel = []

[profile.release]
opt-level = 3
debug = 0
incremental = false
codegen-units = 1
lto = "fat"
'''


def lockfile(arm):
    content = LOCK.read_bytes()
    name = b'name = "within-entrypoint-base"'
    require(content.count(name) == 1, 'lock template has no unique root package')
    return content.replace(name, f'name = "within-entrypoint-{arm}"'.encode(), 1)


def prepare(args):
    work = args.work_dir.resolve()
    require(not work.exists() or (work.is_dir() and not any(work.iterdir())),
            'work directory must be absent or empty; no overwrite')
    require(digest(SOURCE) == SOURCE_SHA256, 'frozen native source changed')
    checkouts = {arm: getattr(args, arm).resolve() for arm in ARMS}
    for arm, checkout in checkouts.items():
        check_checkout(arm, checkout)
    work.mkdir(parents=True, exist_ok=True)
    (work / 'src').mkdir()
    (work / 'src/main.rs').write_bytes(SOURCE.read_bytes())
    declaration = {'schema_version': 1, 'purpose': 'science-only', 'configuration': configuration(),
                   'cells': cells(), 'source_sha256': digest(SOURCE), 'runner_sha256': digest(__file__),
                   'lock_template_sha256': digest(LOCK), 'arms': {}}
    for arm, checkout in checkouts.items():
        directory = work / arm
        directory.mkdir()
        (directory / 'Cargo.toml').write_text(manifest(arm, checkout))
        (directory / 'Cargo.lock').write_bytes(lockfile(arm))
        declaration['arms'][arm] = {'checkout': str(checkout), 'revision': ARMS[arm][1],
                                    'entrypoint': ARMS[arm][2],
                                    'cargo_toml_sha256': digest(directory / 'Cargo.toml'),
                                    'cargo_lock_sha256': digest(directory / 'Cargo.lock')}
    write(work / 'declaration.json', declaration)
    print('Prepared fixed 72-call science declaration; no build or solve executed.', flush=True)
    return 0


def checked_declaration(work):
    declaration = read(work / 'declaration.json')
    require(declaration['schema_version'] == 1 and declaration['purpose'] == 'science-only',
            'wrong declaration schema or mode')
    require(declaration['configuration'] == configuration() and declaration['cells'] == cells(),
            'fixed science schedule changed')
    require(digest(SOURCE) == digest(work / 'src/main.rs') == declaration['source_sha256'] == SOURCE_SHA256,
            'native source identity mismatch')
    require(digest(__file__) == declaration['runner_sha256'], 'runner changed after preparation')
    require(digest(LOCK) == declaration['lock_template_sha256'], 'lock template changed after preparation')
    require(set(declaration['arms']) == set(ARMS), 'wrong arm set')
    for arm in ARMS:
        item = declaration['arms'][arm]
        checkout = Path(item['checkout'])
        require(item['revision'] == ARMS[arm][1] and item['entrypoint'] == ARMS[arm][2], 'wrong arm identity')
        check_checkout(arm, checkout)
        directory = work / arm
        require((directory / 'Cargo.toml').read_text() == manifest(arm, checkout), f'{arm}: manifest differs')
        require((directory / 'Cargo.lock').read_bytes() == lockfile(arm), f'{arm}: dependency lock differs')
        for filename, key in [('Cargo.toml', 'cargo_toml_sha256'), ('Cargo.lock', 'cargo_lock_sha256')]:
            require(digest(directory / filename) == item[key], f'{arm}: {filename} identity differs')
    return declaration


def build(args):
    work = args.work_dir.resolve()
    metadata_path = work / 'build-metadata.json'
    require(not metadata_path.exists() and not (work / 'build-started.json').exists(),
            'build already attempted; no repeat or overwrite')
    declaration = checked_declaration(work)
    cargo_path = args.cargo.resolve()
    toolchain = cargo_path.parent
    env = dict(os.environ, PATH=str(toolchain) + os.pathsep + os.environ.get('PATH', ''),
               CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='4', CARGO_PROFILE_RELEASE_DEBUG='0',
               RUSTFLAGS='', CARGO_ENCODED_RUSTFLAGS='')
    rustc = subprocess.check_output([str(toolchain / 'rustc'), '-Vv'], text=True)
    cargo = subprocess.check_output([str(cargo_path), '--version'], text=True).strip()
    require(rustc.startswith('rustc 1.85.1 ') and cargo.startswith('cargo 1.85.1 '), 'wrong toolchain')
    metadata = {'platform': os.uname().sysname, 'rustc_vv': rustc, 'cargo_version': cargo,
                'environment': {k: env[k] for k in ('CARGO_INCREMENTAL', 'CARGO_BUILD_JOBS',
                                                  'CARGO_PROFILE_RELEASE_DEBUG', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS')},
                'source_sha256': declaration['source_sha256'], 'science_driver_sha256': digest(__file__),
                'declaration_sha256': digest(work / 'declaration.json'),
                'profile': {'name': 'release', 'opt_level': 3, 'debug': 0, 'codegen_units': 1, 'lto': 'fat'},
                'artifacts': {}}
    write(work / 'build-started.json', {'declaration_sha256': metadata['declaration_sha256']})
    for arm in ARMS:
        target = work / 'cargo' / arm
        env['CARGO_TARGET_DIR'] = str(target)
        manifest_path = work / arm / 'Cargo.toml'
        command = [str(cargo_path), 'build', '--release', '--locked', '--offline', '--manifest-path', str(manifest_path)]
        print(f'Building {arm}', flush=True)
        with (work / f'{arm}-build.log').open('wb') as log:
            subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        binary = target / 'release' / f'within-entrypoint-{arm}'
        metadata['artifacts'][arm] = {'source_revision': ARMS[arm][1], 'binary_sha256': digest(binary),
             'binary': str(binary), 'cargo_toml_sha256': digest(manifest_path),
             'cargo_lock_sha256': digest(manifest_path.with_name('Cargo.lock')), 'command': command, 'features': [arm]}
    checked_declaration(work)
    require(metadata['declaration_sha256'] == digest(work / 'declaration.json'), 'declaration changed during build')
    write(metadata_path, metadata)
    print('Three build identities frozen; no solve executed.', flush=True)
    return 0


def run(args):
    work = args.work_dir.resolve()
    output = work / 'preflight.json'
    require(not output.exists(), 'preflight output exists; no repeat or overwrite')
    declaration = checked_declaration(work)
    metadata = read(work / 'build-metadata.json')
    require(metadata['source_sha256'] == declaration['source_sha256']
            and metadata['science_driver_sha256'] == declaration['runner_sha256']
            and metadata['declaration_sha256'] == digest(work / 'declaration.json'), 'build identity mismatch')
    require(set(metadata['artifacts']) == set(ARMS), 'wrong built arm set')
    for arm in ARMS:
        item = metadata['artifacts'][arm]
        binary = work / 'cargo' / arm / 'release' / f'within-entrypoint-{arm}'
        require(Path(item['binary']) == binary and digest(binary) == item['binary_sha256'], 'binary changed')
        require(item['source_revision'] == ARMS[arm][1] and item['features'] == [arm], 'built arm identity changed')
        for key in ['cargo_toml_sha256', 'cargo_lock_sha256']:
            require(item[key] == declaration['arms'][arm][key], 'built manifest/lock identity changed')
    result = {'source_sha256': metadata['source_sha256'], 'runner_sha256': digest(__file__),
              'declaration_sha256': digest(work / 'declaration.json'),
              'build_metadata_sha256': digest(work / 'build-metadata.json'),
              'status': 'running', 'timing': False, 'expected_processes': 72, 'calls': [], 'failure': None}
    write(output, result)
    references, api_stops = {}, {}
    deadline = time.monotonic() + 2700
    try:
        for cell in declaration['cells']:
            for repeat in [1, 2]:
                for arm in ARMS:
                    key = f'{cell["id"]}-{arm}-repeat{repeat}'
                    print(key, flush=True)
                    row = {'cell': cell['id'], 'arm': arm, 'repeat': repeat}
                    result['calls'].append(row)
                    try:
                        remaining = deadline - time.monotonic()
                        require(remaining > 0, 'science preflight exceeded its 2700s budget')
                        row['payload'] = capture(metadata['artifacts'][arm]['binary'], cell,
                                                 work / f'{key}.stderr', min(120, remaining))
                        science = validate(row['payload'], cell, arm)
                        if cell['id'] in references:
                            require(same(references[cell['id']], science), 'common scientific fields changed')
                        else:
                            references[cell['id']] = science
                        if arm == 'api':
                            reasons = row['payload']['controlled_stop_reasons']
                            if cell['id'] in api_stops:
                                require(api_stops[cell['id']] == reasons, 'controlled stop reasons changed across repetitions')
                            else:
                                api_stops[cell['id']] = reasons
                    except BaseException as error:
                        row['error'] = f'{type(error).__name__}: {error}'
                        raise
                    finally:
                        write(output, result)
        require(len(result['calls']) == 72, 'incomplete fixed science schedule')
        result['status'] = 'passed'
    except BaseException as error:
        result['status'] = 'failed'
        result['failure'] = f'{type(error).__name__}: {error}'
    write(output, result)
    print(json.dumps({'status': result['status'], 'completed_processes': len(result['calls']), 'failure': result['failure']}), flush=True)
    return 0 if result['status'] == 'passed' else 2


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest='command', required=True)
    command = commands.add_parser('prepare')
    command.add_argument('--work-dir', type=Path, required=True)
    for arm in ARMS:
        command.add_argument('--' + arm, type=Path, required=True)
    command = commands.add_parser('build')
    command.add_argument('--work-dir', type=Path, required=True)
    command.add_argument('--cargo', type=Path, required=True)
    command = commands.add_parser('run')
    command.add_argument('--work-dir', type=Path, required=True)
    args = parser.parse_args()
    return {'prepare': prepare, 'build': build, 'run': run}[args.command](args)


if __name__ == '__main__':
    sys.exit(main())
