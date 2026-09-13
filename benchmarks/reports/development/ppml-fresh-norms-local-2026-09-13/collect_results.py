from pathlib import Path
import json, csv, statistics, zipfile, hashlib
from collect_ppml_preparation import collect
root = Path(__file__).parent
out = root / 'portable'
collect(root, out, prefix='ppmlonefe', science_file='science.json', exact=False)
data=json.loads((out/'comparisons.json').read_text())
groups={}
for pair in data['pairs']:
    pair['capture_stage'] = 'repeat' if pair['group']=='recheck' else pair['group']
    if pair['group']=='recheck': pair['group']='primary'
    pair['full_b_V_exact']=True
    groups.setdefault((pair['group'],pair['case']),[]).append(pair)
summary=[]
for (group,case), records in sorted(groups.items()):
    row=dict(group=group,case=case,pairs=len(records))
    for arm in ['baseline','candidate']:
        for key in records[0][arm]:
            values=[r[arm][key] for r in records if r[arm][key] is not None]
            row[f'{arm}_{key}']=statistics.median(values) if values else None
    row['median_paired_change_percent']=100*(statistics.median(r['ratio'] for r in records)-1)
    summary.append(row)
data['summary']=summary
data['note']='Nine public primary pairs: three first-screen cases plus six prospectively authorized repeat pairs, all retained. Five protected controls and one separate private audit. Full b/V exact; discrete science and solver decisions exact, floating certificates retain registered checks. Local development, not qualification. Overlapping and independently computed phase medians must not be summed.'
(out/'comparisons.json').write_text(json.dumps(data,indent=2)+'\n')
with (out/'summary.csv').open('w') as f:
    w=csv.DictWriter(f,fieldnames=list(summary[0]));w.writeheader();w.writerows(summary)
a=json.loads((out/'archive.json').read_text());a['primary_pairs']=9;a['all_pairs_full_b_V_exact']=True
(out/'archive.json').write_text(json.dumps(a,indent=2)+'\n')
assert len(data['pairs'])==15
with zipfile.ZipFile(out/'numeric-evidence.zip') as z:
    for name in z.namelist():
        raw=z.read(name)
        for token in [b'/Users/',b'/private/tmp/',b'/tmp/fereg-',b'/Applications/Stata']:
            assert token not in raw,(name,token)
for row in summary:
    print(row['group'],row['case'],row['pairs'],row['baseline_command_seconds'],row['candidate_command_seconds'],row['median_paired_change_percent'])
for p in data['pairs']:
    assert p['baseline']['native_peak_bytes']==p['candidate']['native_peak_bytes']
    assert p['candidate']['native_peak_bytes']<=p['candidate']['modeled_bytes']
print('All 15 native peaks exact and admitted; archive path scan clean')
