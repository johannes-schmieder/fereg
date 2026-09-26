from pathlib import Path
import hashlib,json,re,zipfile
out=Path(__file__).resolve().parent;bundle=out/'receipts-content-addressed.zip';bad=[];reviewed=[]
with zipfile.ZipFile(bundle) as z:
 index=json.loads(z.read('receipt-index.json'))['files'];manifest=json.loads(z.read('archive-hashes.json'))
 for name,blob in index.items():
  data=z.read(blob);assert hashlib.sha256(data).hexdigest()==manifest[name]['archived_sha256']
  text=data.decode()
  if str(Path.home()) in text:bad.append((name,'personal home'))
  if re.search(r'^\s*(Serial number:|Licensed to:|\d+-user.*Stata)',text,re.M):bad.append((name,'licensed header'))
  for match in re.finditer(r'/Users/[^\s"\'<>]+',text):
   token=match.group()
   if token.startswith('/Users/Shared') or (token.startswith('/Users/licensed/private') and 'test_ols_hidden_numeric.py' in name) or (token.startswith('/Users/alice/private/reference.log') and 'test_ols_reference_harness.py' in name):
    reviewed.append(dict(file=name,public_or_synthetic_literal=token))
   else:bad.append((name,token))
assert not bad,bad[:20]
receipt=dict(logical_receipts_verified=len(index),reviewed_public_or_synthetic_literals=reviewed,personal_paths_absent=True,license_headers_absent=True)
(out/'privacy-verification.json').write_text(json.dumps(receipt,indent=2)+'\n');print('privacy verified',len(index),'receipts;',len(reviewed),'reviewed public/synthetic literals')
