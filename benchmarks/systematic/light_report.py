"""Pair-specific development reporting, with explicit exclusions and failures."""
from collections import Counter, defaultdict
import csv
from pathlib import Path
import statistics

from common import cell_id, manifest_for, read_json, write_json
from campaign import audit
from light import groups, skips


def pair_eligible(group, comparison, integrity_ok, smoke_ok):
    oracle = group.get('oracle_checks')
    return (integrity_ok and smoke_ok and (oracle is None or (bool(oracle) and all(oracle.values())))
            and comparison['status']=='pass' and comparison['checks'].get('sample_exact') is True)


def collect(root, manifest, completeness, smoke_ok):
    errors={(e['cell'],e['repetition']) for e in completeness['integrity_errors']}
    rows, pairs = [], []
    for group in groups(manifest):
        c,r=group['cell'],group['repetition']
        directory=root/'results'/cell_id(c)/f'{"cold" if r==0 else "warm"}-{r}'
        marker=directory/'GROUP.COMPLETE.json'
        g=read_json(marker) if marker.exists() else None
        omitted=skips(c,r,manifest)
        base={k:c[k] for k in ('model','design','n','k','threads','section')}
        base.update(repetition=r,phase='cold' if r==0 else 'warm')
        records={}
        for arm in manifest['arms'][c['model']]:
            path=directory/arm/'result.json'
            d=read_json(path) if path.exists() else {}
            records[arm]=d
            rows.append(dict(base,arm=arm,status=omitted.get(arm,{}).get('status',d.get('status','missing')),
                             reason=omitted.get(arm,{}).get('reason',''),seconds=d.get('seconds'),
                             elapsed_seconds=d.get('elapsed_seconds'),peak_rss_gib=d.get('peak_rss_gib'),
                             timeout_bound=d.get('command_time_lower_bound'),job_id=d.get('job_id'),host=d.get('host')))
        candidate=manifest['arms'][c['model']][0]
        comparisons={x['right']:x for x in g['comparisons']} if g else {}
        for arm in manifest['arms'][c['model']][1:]:
            comparison=comparisons.get(arm,dict(status=omitted.get(arm,{}).get('status','missing'),checks={}))
            eligible=bool(g and pair_eligible(g,comparison,(cell_id(c),r) not in errors,smoke_ok))
            a,b=records[candidate],records[arm]
            eligible=eligible and a.get('status')=='pass' and b.get('status')=='pass'
            pairs.append(dict(base,comparator=arm,status=comparison['status'],eligible=eligible,
                              failed_checks=';'.join(k for k,v in comparison['checks'].items() if not v),
                              ratio=a['seconds']/b['seconds'] if eligible else None,
                              candidate_seconds=a.get('seconds'),comparator_seconds=b.get('seconds'),
                              rss_ratio=a['peak_rss_gib']/b['peak_rss_gib'] if eligible and b.get('peak_rss_gib') else None))
    return rows,pairs


def csv_file(path, rows):
    with path.open('w',newline='') as f:
        if rows:
            writer=csv.DictWriter(f,fieldnames=list(rows[0]));writer.writeheader();writer.writerows(rows)


def render(root,output,allow_incomplete=False):
    from light_control import verify_acceptance
    manifest=manifest_for(root)
    completeness=audit(root,manifest)
    if completeness['status']!='complete' and not allow_incomplete:
        raise ValueError('Registered groups or integrity evidence are missing')
    try:
        verify_acceptance(root,'smoke');smoke_ok=True
    except (ValueError,FileNotFoundError):
        smoke_ok=False
    rows,pairs=collect(root,manifest,completeness,smoke_ok)
    output.mkdir(parents=True,exist_ok=True)
    csv_file(output/'runs.csv',rows);csv_file(output/'paired-ratios.csv',pairs)
    write_json(output/'completeness.json',completeness)
    write_json(output/'manifest.json',manifest)
    write_json(output/'provenance.json',read_json(root/'env/frozen.json'))
    grouped=defaultdict(list)
    for row in pairs:
        if row['phase']=='warm':grouped[tuple(row[k] for k in ('model','design','n','k','threads','section','comparator'))].append(row)
    summary=[]
    for key,records in grouped.items():
        valid=[r for r in records if r['eligible']]
        full=len(valid)==manifest['warm_repetitions'] and {r['repetition'] for r in valid}==set(range(1,manifest['warm_repetitions']+1))
        values=[r['ratio'] for r in valid]
        q=statistics.quantiles(values,n=4,method='inclusive') if full else [None]*3
        summary.append(dict(zip(('model','design','n','k','threads','section','comparator'),key),
                            validated_repetitions=len(valid),complete=full,ratio_q1=q[0],ratio_median=q[1],ratio_q3=q[2],
                            ratio_min=min(values) if full else None,ratio_max=max(values) if full else None,
                            candidate_median=statistics.median(r['candidate_seconds'] for r in valid) if full else None,
                            comparator_median=statistics.median(r['comparator_seconds'] for r in valid) if full else None,
                            losses=sum(r['ratio']>1 for r in valid),ten_percent_wins=sum(r['ratio']<=.9 for r in valid)))
    csv_file(output/'summary.csv',summary)
    counts=dict(Counter(r['status'] for r in rows))
    lines=['# Lighter systematic development benchmarks','',
           f"Candidate `{manifest['candidate_commit']}`; {completeness['complete_groups']}/{completeness['expected_groups']} complete groups. Status: {completeness['status']}. Independent smoke accepted: {smoke_ok}.",'',
           'Four warm repetitions per configuration; eight cold anchor groups. Command timers exclude generation, import, initialization and extraction; whole-arm elapsed time is separate. OLS requests IID inference; PPML robust inference. This campaign does not time fitted-mean publication. Each pair runs sequentially on one reserved 28-core E5-2680v4 node; Stata control work remains license-capped. RSS is absolute process-tree peak, sampled every 10 ms. All generated inputs are synthetic.', '',
           'Comparisons require successful commands, identical input and exact retained sample, coefficient/full covariance/objective checks, native convergence and accepted independent smoke. Eligibility is pair-specific development evidence: a failed or excluded ppmlhdfe arm does not disqualify a passing fixest pair. Default fixest and fixest_strict (glm.tol=1e-10, fixef.tol=1e-11) are separate. No tolerances were changed. gtools arms without exact samples remain diagnostic. This is not qualification.', '',
           'Summary rows require all four valid warm repetitions. Ratios below one favor fereg; min/max and quartiles describe repetitions, not confidence intervals. All application failures, exclusions, scientific discrepancies and missing groups remain in the CSV files. Historical exclusions are budget decisions, not new observed failures.', '',
           f'Arm status counts: {counts}', '',
           '| Model | Comparator | Complete configurations | Median ratio across configurations | Range of configuration medians | Repetition losses |',
           '|---|---|---:|---:|---:|---:|']
    for model,arms in manifest['arms'].items():
        for arm in arms[1:]:
            selected=[r for r in summary if r['model']==model and r['comparator']==arm and r['complete']]
            v=[r['ratio_median'] for r in selected]
            lines.append(f"| {model} | {arm} | {len(v)} | {statistics.median(v):.3f} | {min(v):.3f}–{max(v):.3f} | {sum(r['losses'] for r in selected)} |" if v else f'| {model} | {arm} | 0 | — | — | — |')
    (output/'README.md').write_text('\n'.join(lines)+'\n')
    return completeness
