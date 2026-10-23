"""Input-bound, zero-native preparation for the fixed moment-coverage supplement.

Numeric columns are copied byte-for-byte. Only declared request arguments and,
for one-way clustering, the two existing cluster metadata tables change.
"""
from pathlib import Path
import hashlib
import json
import struct

import routing_cases
import routing_oracle
from native_io import case_header, digest

KIND = 'ownership-moments-coverage-v1-inputs'
SEED = 2026106301
CASES = (
    ('weak-robust', 'weak', 'robust', 'fast', (1, 2, 4)),
    ('wide-robust', 'wide', 'robust', 'fast', (2, 4)),
    ('weak-cluster', 'weak', 'cluster', 'fast', (2, 4)),
    ('wide-cluster', 'wide', 'cluster', 'fast', (2, 4)),
    ('weak-low', 'weak', 'robust', 'low', (4,)),
    ('weak-iid', 'weak', 'iid', 'fast', (4,)),
    ('weighted-robust', 'weight-dynamic', 'robust', 'fast', (4,)),
)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def write_new(path, value):
    with path.open('x') as stream:
        json.dump(value, stream, sort_keys=True, indent=2, allow_nan=False)
        stream.write('\n')


def _frame(path):
    with path.open('rb') as stream:
        shape, arguments = case_header(stream)
        require(shape[4] == 0, 'macros are outside supplement scope')
        start = stream.tell()
        for _ in range(shape[1]):
            require(stream.readline() == b'0\n', 'numeric columns required')
            payload = stream.read(shape[0]*17)
            require(len(payload) == shape[0]*17, 'truncated numeric column')
        end = stream.tell()
        stream.seek(start)
        numeric = stream.read(end-start)
        tables = {}
        for _ in range(shape[3]):
            length = int(stream.readline())
            require(0 < length <= 1000, 'matrix name bound')
            name = stream.read(length).decode()
            require(stream.read(1) == b'\n' and name not in tables, 'matrix framing/duplicate')
            rows, cols = map(int, stream.readline().split())
            require(0 < rows*cols <= 100000, 'matrix size bound')
            data = stream.read(rows*cols*17)
            require(len(data) == rows*cols*17, 'truncated matrix')
            tables[name] = (rows, cols, data)
        require(not stream.read(1), 'unexpected trailing bytes')
    return shape, arguments, numeric, tables


def _values(table):
    rows, cols, payload = table
    values = [struct.unpack('>d', bytes.fromhex(line.decode()))[0] for line in payload.splitlines()]
    return [values[i*cols:(i+1)*cols] for i in range(rows)]


def _table(values):
    return (len(values), len(values[0]),
            ''.join(struct.pack('>d', float(v)).hex()+'\n' for row in values for v in row).encode())


def amend_case(source, target, vce='robust', memory='fast'):
    """Return exact edit/provenance metadata; never execute a native estimator."""
    source, target = Path(source), Path(target)
    require(vce in {'robust', 'iid', 'cluster'} and memory in {'fast', 'low'}, 'unsupported amendment')
    before_hash = digest(source)
    original = routing_oracle.read_case(source)
    require(original['vce'] == 'robust', 'supplement requires original HC1 inputs')
    shape, arguments, numeric, tables = _frame(source)
    fields = dict(arg.decode().split('=', 1) for arg in arguments[1:])
    require(len(fields) == len(arguments)-1 and not {'inject', 'break_marker_token'} & fields.keys(),
            'duplicate or injected original request')
    require(vce == 'robust' or int(fields['output_sample'])-1 == 2+int(fields['term_count'])+int(fields['absorb_term_count']),
            'IID/cluster supplement requires unit-weight source')
    # read_case checked every candidate bit equals one and all input fields are
    # finite. input_complete remains false: ordinary validation is preserved.
    edits = dict(candidate_all='1', input_complete='0', vce=vce, memory=memory)
    matrix_edits = {}
    if vce == 'cluster':
        k = int(fields['term_count'])
        require(int(fields['cluster_term_count']) == int(fields['cluster_component_count']) == 0,
                'existing clusters cannot be replaced')
        edits.update(cluster_term_count='1', cluster_component_count='1',
                     cluster_labels=fields['absorb_labels'].split('|')[0], account_cluster_nesting='1')
        for field, values in [('cluster_term_matrix', [[1, 1]]),
                              ('cluster_component_matrix', [[1, 1, k+1]])]:
            name = fields[field]
            require(name in tables, 'missing cluster metadata table')
            matrix_edits[name] = dict(before=_values(tables[name]), after=values)
            tables[name] = _table(values)
    require(set(edits) <= fields.keys(), 'missing canonical request field')
    amended = [arguments[0], *[(key+'='+edits.get(key, value)).encode() for key, value in fields.items()]]
    with target.open('xb') as stream:
        stream.write(('FEREG_REPLAY_V1\n'+' '.join(map(str, shape))+'\n').encode())
        for arg in amended:
            stream.write(str(len(arg)).encode()+b'\n'+arg+b'\n')
        stream.write(numeric)
        for name, (rows, cols, payload) in tables.items():
            stream.write(f'{len(name.encode())}\n{name}\n{rows} {cols}\n'.encode()+payload)
    parsed = routing_oracle.read_case(target)
    require(digest(source) == before_hash and _frame(target)[2] == numeric, 'source/numeric columns changed')
    return dict(original_file=source.name, original_sha256=before_hash,
                original_body_sha256=original['body_sha256'], case_sha256=digest(target),
                body_sha256=parsed['body_sha256'], numeric_columns_sha256=hashlib.sha256(numeric).hexdigest(),
                amended_request_fields={key: dict(before=fields[key], after=value) for key, value in edits.items()},
                amended_matrices=matrix_edits, candidate_all_verified=True, input_complete=False)


def prepare_inputs(original_inputs, output, prior_inputs=None):
    """Write seven independent references and the fixed twelve-setting panel."""
    original_inputs = Path(original_inputs).resolve()
    output = routing_cases.scratch(Path(output))
    original_panel = original_inputs/'panel.json'
    original_panel_hash = digest(original_panel)
    panel = json.loads(original_panel.read_text())
    require(panel['kind'] == 'ownership-cells-development-v1-inputs' and panel['seed'] == SEED,
            'wrong original frozen panel')
    originals = {case['id']: case for case in panel['cases']}
    prior = None
    prior_hash = None
    unchanged = {}
    if prior_inputs is not None:
        prior_inputs = Path(prior_inputs).resolve()
        prior_path = prior_inputs/'panel.json'
        prior_hash = digest(prior_path)
        prior = json.loads(prior_path.read_text())
        expected_settings = [dict(cell=f'{name}-t{w}', family=name, workers=w,
                                  eligible=vce in {'robust','cluster'} and memory == 'fast'
                                  and family != 'weight-dynamic' and w > 1)
                             for name,family,vce,memory,workers in CASES for w in workers]
        require(prior['kind'] == KIND and prior['seed'] == SEED and prior['native_calls'] == 0
                and prior['settings'] == expected_settings
                and prior['cases'] == [dict(id=name, family=name, input=name+'.case', metadata=name+'.json',
                    oracle=dict(kind='routing',path=name+'.oracle.json'),memory_limit_bytes=None) for name,*_ in CASES]
                and prior['original_panel_sha256'] == original_panel_hash,
                'prior supplementary panel contract changed')
        for name, expected in prior['files'].items():
            require(Path(name).name == name and digest(prior_inputs/name) == expected,
                    'prior supplementary file binding changed')
    source_hashes = {str(Path(__file__).name): digest(Path(__file__)),
                     'routing_oracle.py': digest(Path(routing_oracle.__file__)),
                     'native_io.py': digest(routing_oracle.PARSER_SOURCE),
                     'routing_cases.py': digest(Path(routing_cases.__file__))}
    output.mkdir(parents=True, exist_ok=False)
    cases, settings, files, origins = [], [], {}, {}
    for name, family, vce, memory, workers in CASES:
        origin = originals[family]
        source = (original_inputs/origin['input']).resolve()
        require(source.parent == original_inputs and digest(source) == panel['files'][origin['input']],
                'original input binding mismatch')
        target = output/(name+'.case')
        change = amend_case(source, target, vce, memory)
        if prior is not None and vce == 'cluster':
            old = _frame(prior_inputs/target.name)
            new = _frame(target)
            require(old[1].count(b'account_cluster_nesting=0') == 1
                    and [b'account_cluster_nesting=1' if a == b'account_cluster_nesting=0' else a
                         for a in old[1]] == new[1]
                    and old[0] == new[0] and old[2:] == new[2:]
                    and prior['original_inputs'][source.name] == change['original_sha256'],
                    'prior clustered request differs beyond the nesting flag')
        meta = routing_cases.inspect_case(target, name)
        meta.update(change, family=name, seed=SEED, original_family=family,
                    evidence='Input preparation and independent mathematical reference; no native validation.')
        metadata = target.with_suffix('.json')
        reference = output/(name+'.oracle.json')
        if prior is not None and vce != 'cluster':
            old_case = next(item for item in prior['cases'] if item['id'] == name)
            require(old_case == dict(id=name, family=name, input=target.name, metadata=metadata.name,
                        oracle=dict(kind='routing',path=reference.name),memory_limit_bytes=None),
                    'prior unchanged case identity changed')
            require(target.read_bytes() == (prior_inputs/target.name).read_bytes()
                    and prior['original_inputs'][source.name] == change['original_sha256'],
                    'prior unchanged request differs from original data/declared amendment')
            old_meta = json.loads((prior_inputs/metadata.name).read_text())
            require(all(old_meta[key] == meta[key] for key in meta), 'prior unchanged metadata differs')
            old_reference, old_witnesses = routing_oracle.load_reference(
                prior_inputs/reference.name, meta['case_sha256'], meta['body_sha256'])
            require(old_reference['arrays']['file'] == reference.with_suffix('.npz').name
                    and {p.name for p in old_witnesses} == {reference.name, reference.with_suffix('.npz').name},
                    'prior reference array binding is noncanonical')
            for path in [metadata, reference, reference.with_suffix('.npz')]:
                require(path.name in prior['files'], 'prior unchanged reference file is unbound')
                with path.open('xb') as stream:
                    stream.write((prior_inputs/path.name).read_bytes())
            for path in [target, metadata, reference, reference.with_suffix('.npz')]:
                require(digest(path) == prior['files'][path.name], 'unchanged file copy differs')
                unchanged[path.name] = digest(path)
        else:
            write_new(metadata, meta)
            routing_oracle.write_reference(target, reference)
        _, witnesses = routing_oracle.load_reference(reference, meta['case_sha256'], meta['body_sha256'])
        for path in [target, metadata, *witnesses]:
            files[path.name] = digest(path)
        cases.append(dict(id=name, family=name, input=target.name, metadata=metadata.name,
                          oracle=dict(kind='routing', path=reference.name), memory_limit_bytes=None))
        settings.extend(dict(cell=f'{name}-t{w}', family=name, workers=w,
                             eligible=vce in {'robust', 'cluster'} and memory == 'fast'
                             and family != 'weight-dynamic' and w > 1) for w in workers)
        origins[source.name] = change['original_sha256']
    require(len(cases) == 7 and len(settings) == 12 and sum(s['eligible'] for s in settings) == 8,
            'fixed coverage changed')
    require(digest(original_panel) == original_panel_hash, 'original panel drift')
    require(all(digest(original_inputs/name) == value for name, value in origins.items()), 'original input drift')
    require(source_hashes == {name: digest(Path(__file__).parent/name) for name in source_hashes}, 'preparation source drift')
    if prior is not None:
        require(digest(prior_inputs/'panel.json') == prior_hash
                and all(digest(prior_inputs/name) == value for name,value in prior['files'].items()),
                'prior supplementary input drift')
        require(len(unchanged) == 20, 'unchanged five-case coverage differs')
    result = dict(schema=1, kind=KIND, seed=SEED, cases=cases, settings=settings, files=files,
                  original_panel_sha256=original_panel_hash, original_inputs=origins,
                  preparation_sources=source_hashes, native_calls=0)
    if prior is not None:
        result.update(prior_panel_sha256=prior_hash, unchanged_prior_files=unchanged)
    write_new(output/'panel.json', result)
    return result
