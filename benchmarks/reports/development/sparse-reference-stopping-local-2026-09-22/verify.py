"""Verify this bounded diagnostic archive without reclassifying reference failures."""
import hashlib,io,json,subprocess,tarfile,zipfile
from pathlib import Path
folder=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
counts={}
for bundle_name,table_name in [('evidence.zip','archive-hashes.json'),('diagnostics.zip','diagnostics-hashes.json'),('sources.zip','sources-hashes.json')]:
 table=json.loads((folder/table_name).read_text())
 with zipfile.ZipFile(folder/bundle_name) as bundle:
  for name,entry in table.items():
   raw=bundle.read(entry.get('archive_member',name))
   assert sha(raw)==entry.get('archive_sha256',entry['portable_sha256']),(bundle_name,name)
  counts[bundle_name]=len(table)
with zipfile.ZipFile(folder/'sources.zip') as sources:
 identity=json.loads(sources.read('identity-dtk1.json'))
 raw=subprocess.check_output(['git','archive',identity['base_commit'],'Cargo.toml','Cargo.lock','crates','plugin','stata','tests','scripts','references.lock.toml'],cwd=folder.parents[3])
 with tarfile.open(fileobj=io.BytesIO(raw)) as tree:base={e.name:sha(tree.extractfile(e).read()) for e in tree.getmembers() if e.isfile()}
 for relative,digest in identity['source'].items():
  if 'dtk1/'+relative in sources.namelist():assert sha(sources.read('dtk1/'+relative))==digest
  else:assert base[relative]==digest
 native_sources=len(identity['source'])
with zipfile.ZipFile(folder/'diagnostics.zip') as diagnostics:
 hashes=json.loads((folder/'diagnostics-hashes.json').read_text())
 audit=json.loads(diagnostics.read('work/spr1-adjudication.json'))
 assert sha(diagnostics.read('work/spr1-adjudication.json'))==sha((folder/'adjudication.json').read_bytes())
 assert audit['source_sha256']==hashes['work/spr1-audit2.py']['original_sha256']
 assert len(audit['records'])==6 and not audit['headline_timing_eligible']
 reference=json.loads(diagnostics.read('fixest/source-identity.json'))
 assert reference['archive_sha256']=='17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9'
 for kind,key in [('original','original_files'),('diagnostic','diagnostic_files')]:
  for path in ['src/demeaning.cpp','src/fixest_main.h','R/estimation.R','R/methods.R']:
   assert sha(diagnostics.read('fixest/'+kind+'/'+path))==reference[key][path]
 for r in audit['records']:
  assert r['status']=='reference_stopping_error_localized' and r['exact_instrumentation'] and r['diagnostic_tightened_reference_pass']
  for mode,record in r['modes'].items():
   for name,digest in record['source_hashes'].items():
    assert digest==hashes[f"spr1-{mode}-diagnostic/{r['index']}/{name}"]['original_sha256']
   assert record['diagnostic']['reference_binary_sha256']==('bece258f6c6fc3fc47accea45c7b004b0c495d46170d78fa2d6302e66de5fbf9' if mode=='stock' else '840ef97e8b39f814e72840b51a7d6225589a9e6ff38ab6689856e9063b13d0be')
  assert r['modes']['stock']['stock_original_exact']
  assert sum(r['modes']['stock']['output_mismatches'])>0
  assert all(e==1 for e in r['modes']['trace']['diagnostic']['diagnostic_exit'])
  assert all(e==6 for e in r['modes']['tightnossr']['diagnostic']['diagnostic_exit'])
  assert sum(r['modes']['tightnossr']['output_mismatches'])==0
  assert r['modes']['tightnossr']['covariance_mismatches']==0
  assert (r['modes']['stock']['covariance_mismatches']>0)==(r['index']==773)
 for name,count in [('spr1-experiment-complete.json',12),('spr1-tight-experiment-complete.json',12)]:
  complete=json.loads(diagnostics.read('work/'+name))
  assert complete['status']=='completed' and len(complete['records'])==count
  assert all(r['returncode']==0 for r in complete['records'])
with zipfile.ZipFile(folder/'evidence.zip') as evidence:
 hashes=json.loads((folder/'archive-hashes.json').read_text())
 for r in audit['records']:
  index=r['index']
  complete=json.loads(evidence.read(f'spr1-current/current-{index}-complete.json'))
  assert complete['status']=='reference_science_mismatch'
  checks=[c for c in complete['checks'] if 'oracle' in c and c['run'].endswith('-dtk1')]
  assert len(checks)==1 and checks[0]['oracle']['status']=='pass'
  for arm,key in [('dtk1','native_result_sha256'),('fixest','stock_result_sha256')]:
   matches=[(n,v) for n,v in hashes.items() if f'/current-{index}-' in n and n.endswith(f'-{arm}/result.json')]
   assert len(matches)==1 and matches[0][1]['original_sha256']==r['bindings'][key]
 records=json.loads((folder/'measurements.json').read_text())
 assert len(records)==12
 assert all(r['status']=='pass' and r['sentinel'] and r['process_returncode']==0 for r in records)
 assert sum(r['arm']=='fereg' for r in records)==6
 assert all(r['plugin']=='77b2b35764ae216267503b34a9965834bd3a6c8ef60e0f307bdf62c379f5b151' for r in records if r['arm']=='fereg')
receipt=dict(schema='sparse-reference-stopping-integrity-v1',status='pass',archive_entries=counts,native_source_files=native_sources,ordinary_worker_calls=12,reference_diagnostic_fits=30,stock_reference_mismatches_preserved=6,source_sha256=sha(Path(__file__).read_bytes()))
(folder/'integrity.json').write_text(json.dumps(receipt,indent=2)+'\n')
print('SPARSE_REFERENCE_STOPPING_ARCHIVE_INTEGRITY_PASS',sum(counts.values()))
