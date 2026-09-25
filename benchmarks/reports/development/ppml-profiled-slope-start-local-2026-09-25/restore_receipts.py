#!/usr/bin/env python3
"""Verify and restore the exact logical files from deduplicated development receipts."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import zipfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('destination', type=Path, help='New directory; must not already exist')
args = parser.parse_args()
root = Path(__file__).resolve().parent
bundle = root / 'receipts-content-addressed.zip'
expected = bundle.with_suffix('.sha256').read_text().split()[0]
if hashlib.sha256(bundle.read_bytes()).hexdigest() != expected:
    raise SystemExit('Receipt container hash mismatch')
args.destination.mkdir(parents=True, exist_ok=False)
with zipfile.ZipFile(bundle) as archive:
    index = json.loads(archive.read('receipt-index.json'))
    manifest = json.loads(archive.read('archive-hashes.json'))
    if index['schema'] != 'fereg-deduplicated-receipts-v1' or index['files'].keys() != manifest.keys():
        raise SystemExit('Unsupported or inconsistent receipt index')
    for name, blob in index['files'].items():
        path = PurePosixPath(name)
        if path.is_absolute() or '..' in path.parts:
            raise SystemExit('Invalid logical receipt path')
        data = archive.read(blob)
        if hashlib.sha256(data).hexdigest() != manifest[name]['archived_sha256']:
            raise SystemExit(f'Logical receipt hash mismatch: {name}')
        destination = args.destination.joinpath(*path.parts)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
    (args.destination / 'archive-hashes.json').write_bytes(archive.read('archive-hashes.json'))
print(f'Verified and restored {len(manifest)} logical receipts')
