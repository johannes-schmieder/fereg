"""Verify restored hashes and distinguish source test literals from private metadata."""
from pathlib import Path
import argparse,hashlib,json,re
p=argparse.ArgumentParser();p.add_argument('restored',type=Path);args=p.parse_args()
manifest=json.loads((args.restored/'archive-hashes.json').read_text());known=[]
for name,item in manifest.items():
 data=(args.restored/name).read_bytes()
 assert hashlib.sha256(data).hexdigest()==item['archived_sha256'],name
 text=data.decode()
 if name.endswith('/scripts/resolve_reference_root.py'):
  text=text.replace('/Users/Shared/','$PUBLIC_SHARED/');known.append(name)
 if name.endswith('/tests/python/test_ols_hidden_numeric.py'):
  text=text.replace('/Users/licensed/private','$SYNTHETIC_REDACTION_INPUT');known.append(name)
 if name.endswith('/tests/python/test_ols_reference_harness.py'):
  text=text.replace('/Users/alice/private/reference.log','$SYNTHETIC_REDACTION_INPUT');known.append(name)
 assert not re.search(r'/Users/|/private/var/folders/|/var/folders/',text),name
 assert not re.search(r'^\s*(Serial number:|Licensed to:|License expires:|\d+-user.*Stata)',text,re.M),name
print(json.dumps(dict(restored_logical_receipts=len(manifest),restored_hashes_verified=True,privacy_scan_passed=True,privacy_known_source_files=known,privacy_scope='Public shared-reference paths and two synthetic redaction-test literals are distinguished from private run metadata.'),indent=2))
