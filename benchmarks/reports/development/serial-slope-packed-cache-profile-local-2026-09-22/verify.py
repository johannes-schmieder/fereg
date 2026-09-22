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
    for variant in ['rrc1', 'sp2d']:
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
    profile = json.loads(bundle.read('sp2d-profile-check2.json'))
    assert profile['status'] == 'pass' and len(profile['records']) == 3
    assert profile['source_sha256'] in known_sources
    for record in profile['records']:
        assert all(record['exact'].values())
        if 'aw-k50' in record['root']:
            assert not record['measured_cache_traces']['sp2d']
        else:
            assert record['measured_cache_traces']['sp2d']
            assert all(int(trace['n']) == 245760 for trace in record['measured_cache_traces']['sp2d'])


receipt = dict(schema='packed-score-cache-profile-integrity-v1', status='pass',
    stored_entries_verified=verified, local_output_arrays_omitted=omitted,
    content_addressed_objects_verified=objects, sources=sources,
    machine_home_path_scans=scans, verifier_sha256=sha(Path(__file__).read_bytes()))
output = folder / 'integrity.json'
assert not output.exists()
output.write_text(json.dumps(receipt, indent=2) + '\n')
print('PACKED_SCORE_CACHE_PROFILE_INTEGRITY_PASS', verified, omitted, len(sources))
