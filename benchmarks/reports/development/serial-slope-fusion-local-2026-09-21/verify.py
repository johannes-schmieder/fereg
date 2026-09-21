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
    for variant in ['wr2', 'sf1', 'wr2-profile']:
        identity = json.loads(bundle.read(f'identity-{variant}.json'))
        data = subprocess.check_output(['/opt/homebrew/bin/git', 'archive', identity['base_commit'],
            'Cargo.toml', 'Cargo.lock', 'crates', 'plugin', 'stata', 'tests', 'scripts', 'references.lock.toml'], cwd=repo)
        with tarfile.open(fileobj=io.BytesIO(data)) as archive:
            reconstructed = {entry.name: sha(archive.extractfile(entry).read()) for entry in archive.getmembers() if entry.isfile()}
        for relative in json.loads(bundle.read(variant + '/captured-differences.json')):
            reconstructed[relative] = sha(bundle.read(variant + '/' + relative))
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
            assert b'/Users/johannes' not in content and b'/home/johannes' not in content
            verified += 1
        scans.append(kind)

source_hashes = json.loads((folder / 'sources-hashes.json').read_text())
known_sources = {record['original_sha256'] for record in source_hashes.values()}
with zipfile.ZipFile(folder / 'checks.zip') as bundle:
    for name in bundle.namelist():
        if name.endswith('-inference1.json') and name != 'sw1-memory-audit1.json':
            audit = json.loads(bundle.read(name))
            assert audit['complete'] and audit['native_complete'] == audit['planned']
            assert audit['controller_sha256'] in known_sources, name

with zipfile.ZipFile(folder / 'checks.zip') as bundle:
    audit = json.loads(bundle.read('sf1-combined-audit1.json'))
    assert audit['controller_sha256'] in known_sources
    assert len(audit['records']) == 10
    assert all(r['timing_isolation'] == 'no_detected_overlap' for r in audit['records'])
    assert all(all(pair['exact_exports'].values()) for r in audit['records'] for pair in r['pairs'])

receipt = dict(schema='serial-slope-fusion-archive-integrity-v1', status='pass',
    stored_entries_verified=verified, local_output_arrays_omitted=omitted,
    content_addressed_objects_verified=objects, sources=sources,
    machine_home_path_scans=scans, verifier_sha256=sha(Path(__file__).read_bytes()))
output = folder / 'integrity.json'
assert not output.exists()
output.write_text(json.dumps(receipt, indent=2) + '\n')
print('SERIAL_SLOPE_FUSION_ARCHIVE_INTEGRITY_PASS', verified, omitted, len(sources))
