"""Verify portable evidence, supplements and complete captured source trees."""
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tarfile
import zipfile

folder = Path(__file__).resolve().parent
repo = folder.parents[3]
def sha(data):
    return hashlib.sha256(data).hexdigest()

verified, omitted, objects = 0, 0, 0
with zipfile.ZipFile(folder / 'evidence.zip') as bundle:
    hashes = json.loads((folder / 'archive-hashes.json').read_text())
    for name, record in hashes.items():
        if record.get('storage') == 'local_only':
            omitted += 1
            continue
        content = bundle.read(record.get('archive_member', name))
        assert sha(content) == record.get('archive_sha256', record['portable_sha256']), name
        verified += 1
    for name in bundle.namelist():
        if name.startswith('objects/'):
            assert sha(bundle.read(name)) == Path(name).stem
            objects += 1

sources = []
with zipfile.ZipFile(folder / 'sources.zip') as bundle:
    for variant in ['scb1', 'dtk1', 'scb1d', 'dtk1d']:
        identity = json.loads(bundle.read(f'identity-{variant}.json'))
        data = subprocess.check_output(['git', 'archive', identity['base_commit'],
            'Cargo.toml', 'Cargo.lock', 'crates', 'plugin', 'stata', 'tests', 'scripts', 'references.lock.toml'], cwd=repo)
        with tarfile.open(fileobj=io.BytesIO(data)) as archive:
            reconstructed = {entry.name: sha(archive.extractfile(entry).read()) for entry in archive.getmembers() if entry.isfile()}
        for relative in identity['source']:
            name = variant + '/' + relative
            if name in bundle.namelist():
                reconstructed[relative] = sha(bundle.read(name))
        assert reconstructed == identity['source'], variant
        sources.append(dict(variant=variant, base_commit=identity['base_commit'], plugin=identity['plugin'],
                            source_files=len(reconstructed), source_reconstruction='all_captured_hashes_match'))
scans = []
for kind in ['sources', 'checks']:
    table = json.loads((folder / (kind + '-hashes.json')).read_text())
    with zipfile.ZipFile(folder / (kind + '.zip')) as bundle:
        for name, record in table.items():
            content = bundle.read(name)
            assert sha(content) == record['portable_sha256'], (kind, name)
            # macOS sample itself emits this already-redacted binary path.
        scan_content = content.replace(b'/Users/*/fereg_macos.plugin', b'<redacted-plugin-path>')
        assert b'/Users/' not in scan_content and b'/home/' not in scan_content
            verified += 1
        scans.append(kind)

source_hashes = json.loads((folder / 'sources-hashes.json').read_text())
known_sources = {record['original_sha256'] for record in source_hashes.values()}
with zipfile.ZipFile(folder / 'checks.zip') as bundle:
    hashes = json.loads((folder / 'checks-hashes.json').read_text())
    for variant, count in [('dtk1', 6)]:
        tests = json.loads(bundle.read(f'{variant}-tests1.json'))
        assert tests['controller_sha256'] in known_sources
        assert tests['identity_sha256'] == source_hashes[f'identity-{variant}.json']['original_sha256']
        assert tests['status'] == 'pass' and len(tests['tests']) == count
        for test in tests['tests']:
            assert test['passed'] and test['returncode'] == 0
            assert test['log_sha256'] == hashes[test['log']]['original_sha256']
    for label, count in [('dtk1-screen', 16), ('dtk1-confirm1', 7), ('dtk1-confirm2', 7), ('dtk1-controls', 16)]:
        audit = json.loads(bundle.read(f'{label}-audit1.json'))
        assert audit['source_sha256'] in known_sources
        assert len(audit['records']) == count
        assert all(r['isolation'] == 'no_detected_overlap' for r in audit['records'])
        assert audit['progress_sha256'] == hashes[f'{label}-progress1.json']['original_sha256']
        assert all(all(p['exact_exports'].values()) for r in audit['records'] for p in r['pairs'])
        assert all(len(set(p['peaks'].values())) == 1 for r in audit['records'] for p in r['pairs'])
    solver = json.loads(bundle.read('dtk1-solver-check1.json'))
    assert solver['source_sha256'] in known_sources and solver['status'] == 'pass'
    assert len(solver['records']) == 118 and all(r['exact_non_timing_fields'] for r in solver['records'])
    profile = json.loads(bundle.read('dtk1d-profile-check1.json'))
    assert profile['source_sha256'] in known_sources and profile['status'] == 'pass'
    assert len(profile['records']) == 3 and all(all(r['exact'].values()) for r in profile['records'])
    controls = json.loads(bundle.read('dtk1-controls-audit1.json'))
    assert controls['status'] == 'pass'
    assert all(p['exact_non_timing_solver_fields'] for r in controls['records'] for p in r['pairs'])
    sampling = json.loads(bundle.read('rps1-audit2.json'))
    assert sampling['source_sha256'] in known_sources and sampling['status'] == 'pass'
    assert sampling['identity_sha256'] == source_hashes['identity-scb1.json']['original_sha256']
    assert len(sampling['records']) == 2
    assert sum(r['successful_target_fits'] for r in sampling['records']) == 162
    for r in sampling['records']:
        assert all(r['exact_exports'].values()) and r['exact_non_timing_solver_fields']
        assert len(set(r['allocation_peaks'].values())) == 1
        path = r['roots']['sampled'] + '/c000-r0-scb1/native-stacks.txt'
        assert r['stack_sha256'] == hashes[path]['original_sha256']
    assert sampling['disassembly']['sha256'] == hashes['rps1-dense-reduction-disassembly1.txt']['original_sha256']
    measurements = json.loads((folder / 'measurements.json').read_text())
    assert len(measurements) == 390
    assert all(r['status'] == 'pass' and r['sentinel'] and r['process_returncode'] == 0 for r in measurements)




receipt = dict(schema='fixed-triangle-archive-integrity-v1', status='pass',
    stored_entries_verified=verified, local_output_arrays_omitted=omitted,
    content_addressed_objects_verified=objects, sources=sources,
    machine_home_path_scans=scans, verifier_sha256=sha(Path(__file__).read_bytes()))
output = folder / 'integrity.json'
assert not output.exists()
output.write_text(json.dumps(receipt, indent=2) + '\n')
print('FIXED_TRIANGLE_ARCHIVE_INTEGRITY_PASS', verified, omitted, len(sources))
