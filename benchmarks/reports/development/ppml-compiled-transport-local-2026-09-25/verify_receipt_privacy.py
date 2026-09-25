from pathlib import Path
import hashlib,json,re,zipfile
out=Path('benchmarks/reports/development/ppml-compiled-transport-local-2026-09-25');bundle=out/'receipts-content-addressed.zip';bad=[];seen=set()
with zipfile.ZipFile(bundle) as z:
 idx=json.loads(z.read('receipt-index.json'))['files'];manifest=json.loads(z.read('archive-hashes.json'))
 for name,blob in idx.items():
  data=z.read(blob)
  assert hashlib.sha256(data).hexdigest()==manifest[name]['archived_sha256']
  if str(Path.home()).encode() in data:bad.append((name,'unsanitized home'))
  text=data.decode()
  if re.search(r'^\s*(Serial number:|Licensed to:)',text,re.M):bad.append((name,'licensed header'))
  for m in re.finditer(r'/Users/[^\s"\'<>]+',text):
   token=m.group()
   if name == 'ppml-control-plane-20260925/verify-privacy.py' and token in {r'/Users/[^\s', '/Users/licensed/private', '/Users/alice/private/reference.log'}:continue
   if token.startswith('/Users/Shared'):continue
   if token.startswith('/Users/licensed/private') and 'test_ols_hidden_numeric.py' in name:continue
   if token.startswith('/Users/alice/private/reference.log') and 'test_ols_reference_harness.py' in name:continue
   bad.append((name,'unapproved user path '+token[:80]))
  seen.add(blob)
assert not bad,bad[:15]
r=dict(logical_receipts_verified=len(idx),unique_payloads_verified=len(seen),actual_home_absent=True,licensed_headers_absent=True,user_path_audit_passed=True,restored=True,adjudication='Initial scanner matched its own regex and two explicit synthetic fixture-path allowlists; exact verifier-source literals exempted, archive unchanged.')
(out/'privacy-verification.json').write_text(json.dumps(r,indent=2)+'\n');print(r)
