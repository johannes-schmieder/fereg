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
    for variant in ['baseline', 'sf1', 'owf1']:
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
            verified += 1
        scans.append(kind)

source_hashes = json.loads((folder / 'sources-hashes.json').read_text())
known_sources = {record['original_sha256'] for record in source_hashes.values()}
with zipfile.ZipFile(folder / 'checks.zip') as bundle:
    hashes = json.loads((folder / 'checks-hashes.json').read_text())
    joined = json.loads(bundle.read('current-graph-adjudication1.json'))
    assert joined['controller_sha256'] in known_sources
    assert joined['candidate_complete_cells'] == len(joined['records']) == 168
    for name, digest in joined['evidence_sha256'].items():
        assert hashes[name]['original_sha256'] == digest, name
    for name in hashes:
        if name.endswith('inference1.json') and name != 'm5-ring-cycle-inference1.json':
            audit = json.loads(bundle.read(name))
            assert audit['complete'] and audit['controller_sha256'] in known_sources
            assert audit['oracle_source_sha256'] in known_sources
            cache = audit.get('oracle_cache_sha256')
            assert cache is None or cache in {entry['original_sha256'] for entry in hashes.values()}
    for name in ['cycle-effect-adjudication1.json', 'cycle-fw-confirm-decimal1.json']:
        audit = json.loads(bundle.read(name))
        assert audit['source_sha256'] in known_sources
        assert audit['independent_inference_receipt_sha256'] in {entry['original_sha256'] for entry in hashes.values()}
    freeze = json.loads(bundle.read('final-candidate-freeze1.json'))
    assert freeze['plugin'] == next(item['plugin'] for item in sources if item['variant'] == 'owf1')


receipt = dict(schema='current-two-fe-coverage-archive-integrity-v1', status='pass',
    stored_entries_verified=verified, local_output_arrays_omitted=omitted,
    content_addressed_objects_verified=objects, sources=sources,
    supplement_hash_scans=scans, verifier_sha256=sha(Path(__file__).read_bytes()))
output = folder / 'integrity.json'
assert not output.exists()
output.write_text(json.dumps(receipt, indent=2) + '\n')
print('CURRENT_TWO_FE_COVERAGE_ARCHIVE_INTEGRITY_PASS', verified, omitted, len(sources))
