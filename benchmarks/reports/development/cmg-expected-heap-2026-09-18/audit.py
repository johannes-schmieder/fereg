#!/usr/bin/env python3
"""Replay native memory decisions from small preserved diagnostics; no fitting."""
from pathlib import Path
import importlib.util,json,tempfile,zipfile
root=Path(__file__).resolve().parent
repo=root.parents[3]
spec=importlib.util.spec_from_file_location('memory_audit',repo/'scripts/audit_pragmatic_memory.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
accepted=0
with zipfile.ZipFile(root/'diagnostics.zip') as archive,tempfile.TemporaryDirectory() as temp:
 for group in ['calibration','holdouts','holdouts-r2']:
  for row in json.loads((root/group/'calibration.json').read_text()):
   if group=='holdouts' and row['case']=='wider_k':continue
   case=row['case'];receipt=row['receipt'];arm=receipt['arm'];work=Path(temp)/group/case;work.mkdir(parents=True)
   prefix=f'{group}/{case}/calibration/'
   for name in [arm+'_memory.csv','stderr.txt']:(work/name).write_bytes(archive.read(prefix+name))
   (work/'result.json').write_text(json.dumps(receipt))
   result=module.audit_case(work)
   assert result==row['memory_audit'],(group,case)
   assert result['native_accuracy_pass'] and not result['whole_action_accepted']
   accepted+=1
pairs=json.loads((root/'calibration/pairs.json').read_text())
assert len(pairs)==27 and all(p['science']=='pass' for p in pairs)
assert all(a['status']=='pass' and a['sentinel'] for p in pairs for a in p['arms'].values())
assert not any(p['flag'] for p in json.loads((root/'calibration/summary.json').read_text())['cases'])
assert not any(p['timing_flags'] for p in json.loads((root/'calibration/phase-summary.json').read_text()))
assert accepted==16
german=json.loads((root/'german/validation.json').read_text())
assert german['science_pass'] and german['retained_rows']==4361733
assert all(german['exact_scientific_tables'].values()) and all(german['saved_columns_exact'].values())
assert module.audit_case(root/'german')==german['memory_audit']
assert german['memory_audit']['native_accuracy_pass'] and not german['whole_action_accepted']
print('CMG_EXPECTED_HEAP_EVIDENCE_PASS: 16 synthetic cases, German 5%, 27 scientific pairs; whole-action remains partial')
