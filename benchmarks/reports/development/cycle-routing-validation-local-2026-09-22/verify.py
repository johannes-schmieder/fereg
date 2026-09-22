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
    for variant in ['owf1', 'cy2']:
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
    hashes = json.loads((folder / 'checks-hashes.json').read_text())
    freeze = json.loads(bundle.read('cycle-candidate-freeze1.json'))
    assert freeze['controller_sha256'] in known_sources
    assert all(digest in known_sources for digest in freeze['runner_sources'].values())
    assert freeze['selection_plan_sha256'] == hashes['training-selection-plan.json']['original_sha256']
    for name, digest in freeze['evidence'].items(): assert digest == hashes[name]['original_sha256']
    for variant, record in freeze['artifacts'].items():
        assert record['identity_sha256'] == source_hashes[f'identity-{variant}.json']['original_sha256']
        assert record['plugin'] == next(s['plugin'] for s in sources if s['variant'] == variant)
    adjudication = json.loads(bundle.read('cycle-validation-adjudication1.json'))
    assert adjudication['source_sha256'] in known_sources
    for name, digest in adjudication['evidence_sha256'].items():
        assert digest == hashes[name]['original_sha256']
    assert len(adjudication['blocks']) == 2
    for block in adjudication['blocks']:
        assert block['seed'] == freeze['seeds'][block['block'] - 1]
        assert len(block['applications']) == 960
        for kind in ['inference', 'routing', 'fastest']:
            audit = json.loads(bundle.read(f"cycle-validation-block{block['block']}-{kind}1.json"))
            assert audit['controller_sha256'] in known_sources
        inference = json.loads(bundle.read(f"cycle-validation-block{block['block']}-inference1.json"))
        assert inference['oracle_source_sha256'] in known_sources
        routing = json.loads(bundle.read(f"cycle-validation-block{block['block']}-routing1.json"))
        assert routing['solver_layout_source_sha256'] in known_sources
        outputs = json.loads(bundle.read(f"cycle-validation-block{block['block']}-outputs1.json"))
        assert outputs['source_sha256'] in known_sources and outputs['auditor_sha256'] in known_sources

receipt = dict(schema='cycle-routing-validation-archive-integrity-v1', status='pass',
    stored_entries_verified=verified, local_output_arrays_omitted=omitted,
    content_addressed_objects_verified=objects, sources=sources,
    machine_home_path_scans=scans, verifier_sha256=sha(Path(__file__).read_bytes()))
output = folder / 'integrity2.json'
assert not output.exists()
output.write_text(json.dumps(receipt, indent=2) + '\n')
print('CYCLE_ROUTING_VALIDATION_ARCHIVE_INTEGRITY_PASS', verified, omitted, len(sources))
