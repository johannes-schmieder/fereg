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
    for variant in ['owf1', 'cy1', 'cy2']:
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
            assert b'/Users/' not in content and b'/home/' not in content
            verified += 1
        scans.append(kind)

source_hashes = json.loads((folder / 'sources-hashes.json').read_text())
known_sources = {record['original_sha256'] for record in source_hashes.values()}
with zipfile.ZipFile(folder / 'checks.zip') as bundle:
    for name in bundle.namelist():
        if name in ['cycle-memory-inference1.json']:
            audit = json.loads(bundle.read(name))
            assert audit['complete'] and audit['native_complete'] == audit['planned']
            assert audit['controller_sha256'] in known_sources, name
            assert audit['oracle_source_sha256'] in known_sources, name

with zipfile.ZipFile(folder / 'checks.zip') as bundle:
    audit = json.loads(bundle.read('cycle-memory-routing1.json'))
    assert audit['passing'] == audit['planned'] == 138
    assert audit['controller_sha256'] in known_sources
    assert audit['solver_layout_source_sha256'] in known_sources


with zipfile.ZipFile(folder / 'checks.zip') as bundle:
    hashes=json.loads((folder/'checks-hashes.json').read_text())
    all_checks={r['original_sha256'] for r in hashes.values()}
    for name in ['cycle-memory-inference1.json','cycle-constrained-audit1.json']:
        audit=json.loads(bundle.read(name))
        assert audit['oracle_cache_sha256'] in all_checks
    routes=json.loads(bundle.read('cycle-fastest-audit1.json'))
    assert routes['complete'] and routes['planned']==routes['attempted']==168
    assert all(r['status']=='science_pass' and r['isolation']=='no_detected_overlap' for r in routes['records'])
    assert max(r['median_paired_auto_to_fastest_valid'] for r in routes['cells'])<=1.05
    assert routes['controller_sha256'] in known_sources
    memory=json.loads(bundle.read('cycle-constrained-audit1.json'))
    assert len(memory['records'])==18 and sum(r['status']=='pass' for r in memory['records'])==8
    assert all(r['inference']['status']=='pass' for r in memory['records'] if r['status']=='pass')
    assert all(r['status']=='pass' for r in memory['rejected_route_pairs'])
    assert memory['source_sha256'] in known_sources and memory['output_auditor_sha256'] in known_sources
    tests=json.loads(bundle.read('cycle-memory-tests1.json'))
    assert tests['status']=='pass' and len(tests['tests'])==6
    assert tests['plugin']==next(s['plugin'] for s in sources if s['variant']=='cy2')

receipt = dict(schema='cycle-routing-memory-archive-integrity-v1', status='pass',
    stored_entries_verified=verified, local_output_arrays_omitted=omitted,
    content_addressed_objects_verified=objects, sources=sources,
    machine_home_path_scans=scans, verifier_sha256=sha(Path(__file__).read_bytes()))
output = folder / 'integrity2.json'
assert not output.exists()
output.write_text(json.dumps(receipt, indent=2) + '\n')
print('CYCLE_ROUTING_MEMORY_ARCHIVE_INTEGRITY_PASS', verified, omitted, len(sources))
