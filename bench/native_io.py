"""Bounded replay transport and diagnostic readers for native benchmarks."""
from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import re
import shutil
import struct
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from replay_plugins import exact_view, differences  # noqa: E402

CHUNK = 1024 * 1024
HEX = re.compile(r'[0-9a-f]{16}\Z')
CELLS = re.compile(rb'"[0-9a-f]{16}"(?:,"[0-9a-f]{16}")*\Z')


def digest(path: Path) -> str:
    result = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(CHUNK), b''):
            result.update(block)
    return result.hexdigest()


def bounded_line(stream, limit=1000001):
    line = stream.readline(limit + 1)
    if not line.endswith(b'\n') or len(line) > limit:
        raise ValueError('missing or oversized replay line')
    return line


def case_header(stream):
    if bounded_line(stream) != b'FEREG_REPLAY_V1\n':
        raise ValueError('unsupported case format')
    shape_line = bounded_line(stream)
    shape = [int(value) for value in shape_line.split()]
    if len(shape) != 5 or min(shape) < 0:
        raise ValueError('invalid case shape')
    rows, columns, argc, matrices, macros = shape
    if not (0 < rows <= 2147483647 and 0 < columns <= 10000
            and 0 < argc <= 1000 and 0 < matrices <= 200 and macros <= 100):
        raise ValueError('case shape exceeds harness bounds')
    arguments = []
    for _ in range(argc):
        length = int(bounded_line(stream))
        if not 0 < length <= 1000000:
            raise ValueError('invalid framed argument length')
        value = stream.read(length)
        if len(value) != length or b'\0' in value or stream.read(1) != b'\n':
            raise ValueError('malformed framed argument')
        arguments.append(value)
    if arguments[0] not in (b'estimate-ols', b'estimate-ppml'):
        raise ValueError('case must request OLS or PPML')
    if sum(a.startswith(b'threads=') for a in arguments) != 1:
        raise ValueError('case needs exactly one threads argument')
    return shape, arguments


def prepare_case(source: Path, destination: Path, threads: int):
    """Rewrite just the length-framed argument section, then stream the body."""
    if not 1 <= threads <= 1024:
        raise ValueError('threads must be 1..1024')
    with source.open('rb') as stream, destination.open('xb') as output:
        shape, arguments = case_header(stream)
        output.write(b'FEREG_REPLAY_V1\n' + (' '.join(map(str, shape)) + '\n').encode())
        for value in arguments:
            if value.startswith(b'threads='):
                value = f'threads={threads}'.encode()
            output.write(str(len(value)).encode() + b'\n' + value + b'\n')
        shutil.copyfileobj(stream, output, CHUNK)
    return {'rows': shape[0], 'columns': shape[1], 'poisson': arguments[0] == b'estimate-ppml'}


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError(f'duplicate JSON key {key}')
        value[key] = item
    return value


def load_json(raw):
    return json.loads(raw, object_pairs_hook=unique_object,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError(value)))


def read_result(path: Path, rows: int, columns: int):
    """Read only bounded JSON metadata; validate/hash numeric columns in chunks.

    This deliberately accepts the actual harness's numeric output dialect only.
    Numeric benchmark inputs need no generic JSON parser or per-row Python objects.
    The error suffix is validated separately and retained in the exact view.
    """
    marker = b'},"columns":['
    with path.open('rb') as stream:
        head = bytearray()
        while marker not in head:
            block = stream.read(65536)
            if not block or len(head) + len(block) > 32 * CHUNK:
                raise ValueError('missing columns or oversized result head')
            head.extend(block)
        split = head.index(marker) + 1  # include closing matrices object
        parsed = load_json(bytes(head[:split]) + b'}')
        stream.seek(split + len(b',"columns":['))
        hashed = hashlib.sha256(b'[')
        def take(count):
            block = stream.read(count)
            if len(block) != count:
                raise ValueError('truncated result columns')
            hashed.update(block)
            return block
        for column in range(columns):
            if take(1) != (b'[' if column == 0 else b','):
                raise ValueError('invalid column delimiter')
            if column and take(1) != b'[':
                raise ValueError('missing column array')
            remaining = rows
            first = True
            while remaining:
                count = min(remaining, 4096)
                if not first and take(1) != b',':
                    raise ValueError('missing cell separator')
                block = take(19 * count - 1)
                if CELLS.fullmatch(block) is None:
                    raise ValueError('malformed numeric output cell')
                remaining -= count
                first = False
            if take(1) != b']':
                raise ValueError('wrong column length')
        if take(1) != b']':
            raise ValueError('wrong output column count')
        suffix = stream.read(65537)
        if len(suffix) > 65536 or not suffix.startswith(b',"error":'):
            raise ValueError('missing/oversized result error suffix')
        tail = load_json(b'{' + suffix[1:])
        if set(tail) != {'error'} or not isinstance(tail['error'], str):
            raise ValueError('invalid result suffix')
        parsed.update(tail)
        validate_result(parsed)
        return {'head': parsed, 'columns_sha256': hashed.hexdigest()}


def numbers(table):
    return [struct.unpack('>d', bytes.fromhex(bits))[0] for bits in table['bits']]


def validate_result(result):
    if set(result) != {'schema', 'return_code', 'matrices', 'error'} or type(result['schema']) is not int or result['schema'] != 1:
        raise ValueError('invalid result schema')
    if type(result['return_code']) is not int or result['return_code'] != 0:
        raise ValueError(f"native return code {result['return_code']}: {result['error']}")
    if result['error']:
        raise ValueError('native success carried an error message')
    tables = result['matrices']
    required = {'RB', 'RV', 'RSTATUS', 'RFIT', 'RDOF', 'RSOLVER', 'RFE',
                'RCLUSTER', 'RSUBSET', 'RCERT', 'RRUNTIME', 'RMEMORY', 'RREC'}
    if not isinstance(tables, dict) or not required.issubset(tables):
        raise ValueError('missing result matrices')
    for name, table in tables.items():
        if not isinstance(table, dict) or set(table) != {'rows', 'columns', 'bits'}:
            raise ValueError(f'invalid matrix {name}')
        if any(type(table[key]) is not int or table[key] < 1 for key in ('rows', 'columns')):
            raise ValueError(f'invalid dimensions for {name}')
        bits = table['bits']
        if not isinstance(bits, list) or len(bits) != table['rows'] * table['columns']:
            raise ValueError(f'invalid cell count for {name}')
        if any(not isinstance(b, str) or HEX.fullmatch(b) is None for b in bits):
            raise ValueError(f'invalid floating point bits in {name}')
    for name in ('RREC', 'RCERT'):
        if numbers(tables[name])[-1] != 1.0:
            raise ValueError(f'{name} did not certify completion')


def diagnostics(result, poisson):
    tables = result['matrices']
    required = {'RINF', 'RDROP', 'RSEP', 'RSEQ'} if poisson else {'RMARKB', 'RMARKV'}
    if not required.issubset(tables):
        raise ValueError('missing estimator-specific result matrices')
    runtime, solver = tables['RRUNTIME'], tables['RSOLVER']
    shape = lambda table: (table['rows'], table['columns'])
    if shape(runtime) != ((108, 1) if poisson else (30, 2)):
        raise ValueError('unsupported RRUNTIME layout')
    if shape(solver) != ((4, 22) if poisson else (2, 16)):
        raise ValueError('unsupported RSOLVER layout')
    values = numbers(runtime)
    times = ({'total_s': values[0]} if poisson else
             dict(zip(('load_s', 'native_s', 'total_s'), (values[2*r + 1] for r in (18, 19, 20)))))
    if not poisson and any(values[2*r] != 1.0 for r in (18, 19, 20)):
        raise ValueError('runtime timing is absent')
    if any(not math.isfinite(v) or not 0 <= v < 8e307 for v in times.values()):
        raise ValueError('invalid runtime timing')
    width = solver['columns']
    decoded = numbers(solver)
    # Preserve every solver row, including counters and timings, without rounding.
    return {**times, 'timing_field': 'total_s' if poisson else 'native_s',
            'solver_rows': [decoded[i:i+width] for i in range(0, len(decoded), width)]}


def exact_differences(reference, candidate, poisson):
    changed = differences(exact_view(reference['head'], poisson), exact_view(candidate['head'], poisson))
    if reference['columns_sha256'] != candidate['columns_sha256']:
        changed.append('/columns')
    return changed


def cachegrind_counts(path: Path):
    """Read self Ir costs by function from cachegrind's compressed text format."""
    names, costs = {}, {}
    function = None
    positions = 1  # Cachegrind defaults to line positions and may omit this header.
    events = None
    summary = None
    with path.open(encoding='utf-8') as stream:
        for line in stream:
            line = line.strip()
            if line.startswith('positions:'):
                positions = len(line.split()) - 1
            elif line.startswith('events:'):
                events = line.split()[1:]
                if 'Ir' not in events:
                    raise ValueError('cachegrind omitted Ir')
            elif line.startswith('fn='):
                value = line[3:]
                match = re.fullmatch(r'\((\d+)\)(?: (.*))?', value)
                if match:
                    key, name = match.groups()
                    if name is not None:
                        names[key] = name
                    if key not in names:
                        raise ValueError('unknown compressed cachegrind function')
                    function = names[key]
                else:
                    function = value
            elif line.startswith('summary:'):
                if events is None or summary is not None:
                    raise ValueError('invalid cachegrind summary')
                values = line.split()[1:]
                if len(values) != len(events):
                    raise ValueError('invalid cachegrind summary width')
                summary = int(values[events.index('Ir')])
            elif line and line[0] in '0123456789+-*':
                if events is None or positions is None or function is None:
                    raise ValueError('invalid cachegrind cost record')
                values = line.split()
                if len(values) != positions + len(events):
                    raise ValueError('invalid cachegrind cost width')
                cost = int(values[positions + events.index('Ir')])
                if cost < 0:
                    raise ValueError('negative cachegrind cost')
                costs[function] = costs.get(function, 0) + cost
    if summary is None or summary <= 0 or not costs or sum(costs.values()) != summary:
        raise ValueError('missing/inconsistent cachegrind instruction totals')
    return {'total_Ir': summary, 'functions_Ir': dict(sorted(costs.items(), key=lambda p: (-p[1], p[0])))}
