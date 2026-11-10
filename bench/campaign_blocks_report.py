"""Summaries for frozen comparator/native blocks; never pool denominators."""
from collections import defaultdict,Counter
from statistics import median
import json
import pandas as pd

def paired_summary(campaign):
    from campaign_report import completed_seconds
    cells={c['id']:c for c in campaign['manifest']['cells']}
    records={(r['cell'],r['arm'],r['repetition']):r for r in campaign['records']}
    grouped=defaultdict(list)
    for r in campaign['records']:
        c=cells[r['cell']]
        if r['arm'] not in [c['block_family'],c['block_family']+'-strict']:continue
        native='fereg-strict' if r['arm'].endswith('-strict') else 'fereg'
        anchor=records[r['cell'],native,r['repetition']]
        grouped[c['source_cell'],c['block_family'],r['arm']].append((c,r,anchor))
    out=[]
    for (source,family,arm),group in sorted(grouped.items()):
        times=[];anchors=[];ratios=[];samples=[];statuses=Counter()
        for c,r,a in group:
            statuses[r['status']]+=1
            t,u=completed_seconds(r),completed_seconds(a)
            if t is not None:times.append(t)
            if u is not None:anchors.append(u)
            if t is not None and u is not None:
                if r.get('host')!=a.get('host') or r.get('job')!=a.get('job') or r.get('task')!=a.get('task'):
                    raise ValueError('timing ratio crosses comparison allocations')
                ratios.append(t/u)
            if 'numerical_capture' in r and 'numerical_capture' in a:
                samples.append(r['numerical_capture']['sample_sha256']==a['numerical_capture']['sample_sha256'])
        c=group[0][0]
        out.append(dict(cell=source,family=family,arm=arm,profile='tighter' if arm.endswith('-strict') else 'default',
            **{k:c[k] for k in ['model','design','rows','threads','vce','save_fe','regressors','section']},
            planned_repetitions=c['source_repetitions'],observed_positions=len(group),timed_repetitions=len(times),
            seconds=median(times) if times else None,paired_native_seconds=median(anchors) if anchors else None,
            paired_repetitions=len(ratios),paired_ratio=median(ratios) if ratios else None,
            timeouts=sum(statuses[k] for k in ['timeout','command-timeout']),
            statuses=json.dumps(dict(statuses),sort_keys=True),sample_comparisons=len(samples),different_samples=sum(not s for s in samples),
            hosts=len({r.get('host') for _,r,_ in group})))
    return pd.DataFrame(out)

def render_blocks(campaign,output):
    from campaign_report import CampaignPDF,ARM_NAMES,number
    frame=paired_summary(campaign);frame.to_csv(output/'paired-summary.csv',index=False)
    pdf=CampaignPDF(output/'fereg-scc-benchmark.pdf',campaign['context']['run_id'])
    pdf.timing_footnote='Default and tighter stopping criteria differ by package. Ratios pair each fit with fereg in the same allocation. Numerical deviations are reported separately.'
    pdf.page('Default and tighter settings')
    pdf.text('Every comparator family and predetermined repetition shares an isolated node with default and tighter fereg. The command limit is 3,600 seconds for every implementation. A timeout is a censored result; it is not a completed timing.')
    pdf.text('Tables give medians of completed command times, with completed/planned counts. Native columns use the corresponding profile in each matched block. Ratios are medians of within-block comparator/fereg ratios. Absolute medians across blocks include host variation. No estimate is filtered from timing summaries because its numerical results differ.')
    pdf.text('Package-default inference is retained. Tighter profiles change stopping settings only. Coefficient/SE/covariance, prediction, objective and sample deviations are retained in the numerical JSON; unequal-sample objective differences are unavailable. Native independent smoke and consistency checks remain required.')
    pdf.text('Rows and prediction arrays are temporary node-scratch outputs. Compact captures, output hashes, complete status records and precomputed numerical deviations are retained. BLAS uses one thread; each estimator receives the stated physical-core budget.')
    for profile in ['default','tighter']:
        for model in ['ols','ppml']:
            rows=frame[(frame.profile==profile)&(frame.model==model)]
            for part,start in enumerate(range(0,len(rows),18),1):
                pdf.page(f'{model.upper()}: {profile} settings ({part})')
                table=[['Design / N / cores / VCE / saved','Package','n / plan','Seconds','Native s','Paired ratio']]
                for r in rows.iloc[start:start+18].itertuples():
                    label=f'{r.design} / {r.rows:,} / {r.threads} / {r.vce} / {int(r.save_fe)}'
                    timing=number(r.seconds) if r.timed_repetitions else ('>3,600' if r.timeouts else 'unavailable')
                    table.append([label,ARM_NAMES.get(r.arm,r.arm),f'{r.timed_repetitions}/{r.planned_repetitions}',timing,number(r.paired_native_seconds),number(r.paired_ratio)])
                pdf.table(table,[195,105,50,60,60,70],padding=4)
    pdf.build();return pdf.path
