#!/usr/bin/env python3
"""Render a collected, terminal packed SCC campaign into a PDF and compact CSVs."""
from __future__ import annotations
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
from statistics import median

import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd
from reportlab.lib import colors
from reportlab.lib.enums import TA_LEFT
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.units import inch
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import (BaseDocTemplate, Frame, Image,
                               PageBreak, PageTemplate, Paragraph, Spacer, Table, TableStyle)

INK = '#17344B'
BLUE = '#127BA3'
GOLD = '#C88927'
RED = '#A14543'
LIGHT = '#EEF3F6'
ARM_NAMES = {'fereg': 'fereg', 'fixest': 'fixest', 'fixest-strict': 'fixest strict',
             'reghdfe': 'reghdfe', 'ppmlhdfe': 'ppmlhdfe'}
ARM_COLORS = {'fereg': BLUE, 'fixest': GOLD, 'reghdfe': INK, 'ppmlhdfe': INK}
DESIGNS = {'akm-baseline': 'AKM baseline', 'one-fe': 'One FE',
           'two-dense': 'Two FE / dense', 'two-sparse': 'Two FE / sparse',
           'three-dense': 'Three FE / dense', 'three-sparse': 'Three FE / sparse'}
VCE_NAMES = {'iid': 'Regular', 'robust': 'Robust', 'cluster': 'Worker cluster'}


def planned_arms(cell):
    selected = ['fereg', 'fixest']
    if cell['threads'] <= 4:
        selected.append('reghdfe' if cell['model'] == 'ols' else 'ppmlhdfe')
    if (cell['model'] == 'ppml' and cell['design'] in ['akm-baseline', 'two-sparse', 'three-sparse']
            and cell['rows'] == 1000000 and cell['threads'] == 4):
        selected.append('fixest-strict')
    return selected


def load_campaign(root):
    for name, expected in json.loads((root/'collection-sha256.json').read_text()).items():
        path = Path(name)
        if path.is_absolute() or '..' in path.parts or hashlib.sha256((root/path).read_bytes()).hexdigest() != expected:
            raise ValueError('collected evidence changed: '+name)
    manifest = json.loads((root/'manifest.json').read_text())
    context = json.loads((root/'context.json').read_text())
    smoke = json.loads((root/'smoke/results.json').read_text())
    accounting = json.loads((root/'accounting.json').read_text())
    if not smoke.get('native_gate_pass') or not (root/'smoke/COMPLETE').is_file():
        raise ValueError('independent native smoke is not accepted')
    expected_jobs = {(str(context['setup_job']), None), (str(context['smoke_job']), None)}
    expected_jobs.update((str(context['production_job']), str(i))
                         for i in range(1, manifest['policy']['bundles']+1))
    actual_jobs = set()
    for job in accounting['jobs']:
        key = str(job['jobnumber']), None if job.get('taskid') in [None, 'undefined', '0'] else str(job['taskid'])
        if key in actual_jobs:
            raise ValueError('duplicate job accounting')
        actual_jobs.add(key)
        if int(job['failed']) != 0 or int(job['exit_status']) != 0 or int(job['slots']) != 28:
            raise ValueError('job accounting has an unsuccessful or non-full-node task')
        if float(job['ru_wallclock']) >= 12*3600:
            raise ValueError('job exceeded the campaign walltime bound')
    if actual_jobs != expected_jobs or not accounting.get('sentinels_pass'):
        raise ValueError('terminal job accounting or application sentinels are incomplete')
    records, comparisons, datasets, tasks = [], [], {}, []
    for i in range(1, manifest['policy']['bundles']+1):
        taskroot = root/f'task-{i}'
        task = json.loads((taskroot/'results.json').read_text())
        if not (taskroot/'COMPLETE').is_file() or not task.get('coverage_complete'):
            raise ValueError('task coverage is incomplete')
        records.extend(task['records']); comparisons.extend(task['comparisons'])
        datasets.update(task['datasets']); tasks.append(task)
    expected = {(c['id'], a, r) for c in manifest['cells'] for a in planned_arms(c)
                for r in range(1, manifest['policy']['repetitions']+1)}
    actual = [(r['cell'], r['arm'], r['repetition']) for r in records]
    if len(set(actual)) != len(actual) or set(actual) != expected or len(records) != manifest['timed_fits']:
        raise ValueError('manifest and observation coverage disagree')
    if any(str(r.get('job')) != str(context['production_job']) for r in records if 'directory' in r):
        raise ValueError('application observation job identity mismatch')
    return dict(manifest=manifest, context=context, smoke=smoke, accounting=accounting,
                records=records, comparisons=comparisons, datasets=datasets, tasks=tasks)


def summarize(campaign):
    records = {(r['cell'], r['arm'], r['repetition']): r for r in campaign['records']}
    comparisons = {(r['cell'], r['arm'], r['repetition']): r for r in campaign['comparisons']}
    count = campaign['manifest']['policy']['repetitions']
    worker_checks = check_workers(campaign)
    repeat_checks = check_native_repetitions(campaign)
    rows = []
    for cell in campaign['manifest']['cells']:
        for arm in planned_arms(cell):
            runs = [records[(cell['id'], arm, rep)] for rep in range(1, count+1)]
            valid = all(r['status'] == 'ok' for r in runs)
            reasons = set()
            paired = []
            if arm != 'fereg':
                for rep, run in enumerate(runs, 1):
                    native = records[(cell['id'], 'fereg', rep)]
                    comparison = comparisons.get((cell['id'], arm, rep), {})
                    if not comparison.get('eligible') or native['status'] != 'ok' or run['status'] != 'ok':
                        reasons.update(comparison.get('reasons', ['application unavailable']))
                    else:
                        paired.append(float(run['metadata']['seconds'])/float(native['metadata']['seconds']))
            if cell['id'] in worker_checks:
                reasons.update(worker_checks[cell['id']])
            reasons.update(repeat_checks.get(cell['id'], []))
            eligible = valid and not reasons if arm == 'fereg' else valid and not reasons and len(paired) == count
            times = [float(r['metadata']['seconds']) for r in runs if r['status'] == 'ok']
            resolved_memory = valid and all(r.get('memory_sweeps', 0) >= 10 for r in runs)
            memory = median(r['estimator_tree_rss_bytes'] for r in runs)/2**30 if resolved_memory else None
            rows.append(dict(cell=cell['id'], model=cell['model'], design=cell['design'], rows=cell['rows'],
                             threads=cell['threads'], vce=cell['vce'], save_fe=cell['save_fe'],
                             section=cell['section'], arm=arm, successful_repetitions=sum(r['status'] == 'ok' for r in runs),
                             eligible=eligible, reasons='; '.join(sorted(reasons)),
                             seconds=median(times) if valid else None,
                             seconds_min=min(times) if valid else None, seconds_max=max(times) if valid else None,
                             speedup=median(paired) if eligible and paired else None,
                             rss_gib=memory, memory_resolved=resolved_memory,
                             host=runs[0].get('host'), statuses='; '.join(r['status'] for r in runs)))
    return pd.DataFrame(rows)


def check_native_repetitions(campaign):
    """Require exact repeated native coordinates and stable discrete decisions."""
    failures = {}
    for cell in campaign['manifest']['cells']:
        records = sorted((r for r in campaign['records'] if r['cell'] == cell['id'] and r['arm'] == 'fereg'),
                         key=lambda r: r['repetition'])
        reasons = set()
        if any(r['status'] != 'ok' for r in records):
            reasons.add('native application unavailable')
        else:
            keys = ['N', 'sample_sha256', 'finite_slope_rank', 'omitted', 'singleton_deletions',
                    'separation_deletions', 'absorbed_dof', 'clusters', 'covariance_policy', 'beta', 'covariance']
            metric = 'rss' if cell['model'] == 'ols' else 'll'
            budget_key = 'fereg_threads' if cell['model'] == 'ols' else 'feppml_threads'
            for record in records:
                if float(record['metadata'].get(budget_key, 'nan')) != cell['threads']:
                    reasons.add('unverified native worker budget')
                if any(record['capture'].get(k) != records[0]['capture'].get(k) for k in keys):
                    reasons.add('native repeatability')
                if float(record['metadata'][metric]) != float(records[0]['metadata'][metric]):
                    reasons.add('native objective repeatability')
        failures[cell['id']] = sorted(reasons)
    return failures


def check_workers(campaign):
    """Check native science against the four-core cell on the same input."""
    cells = campaign['manifest']['cells']
    records = {(r['cell'], r['repetition']): r for r in campaign['records'] if r['arm'] == 'fereg'}
    policy = campaign['manifest']['policy']
    failures = {}
    def same_values(a, b, rtol, atol):
        a, b = np.asarray(a), np.asarray(b)
        return a.shape == b.shape and np.isfinite(a).all() and np.isfinite(b).all() and np.all(
            abs(a-b) <= atol+rtol*np.maximum(abs(a), abs(b)))
    for cell in cells:
        if cell['threads'] == 4:
            continue
        baseline = [c for c in cells if c['threads'] == 4 and all(c[k] == cell[k] for k in
                    ['model', 'design', 'rows', 'vce', 'save_fe'])]
        if len(baseline) != 1:
            failures[cell['id']] = ['missing four-core scientific witness']
            continue
        reasons = set()
        for rep in range(1, policy['repetitions']+1):
            a, b = records[(cell['id'], rep)], records[(baseline[0]['id'], rep)]
            if a['status'] != 'ok' or b['status'] != 'ok':
                reasons.add('worker science unavailable'); continue
            x, y = a['capture'], b['capture']
            budget_key = 'fereg_threads' if cell['model'] == 'ols' else 'feppml_threads'
            if float(a['metadata'].get(budget_key, 'nan')) != cell['threads']:
                reasons.add('unverified native worker budget')
            for key in ['N', 'sample_sha256', 'finite_slope_rank', 'omitted', 'singleton_deletions',
                        'separation_deletions', 'absorbed_dof', 'clusters', 'covariance_policy']:
                if x.get(key) != y.get(key):
                    reasons.add('worker '+key)
            for name, tolerance in [('beta', 'coefficient'), ('covariance', 'vce')]:
                if not same_values(x[name], y[name], policy[tolerance+'_rtol'], policy[tolerance+'_atol']):
                    reasons.add('worker '+name)
            metric = 'rss' if cell['model'] == 'ols' else 'll'
            if not same_values([float(a['metadata'][metric])], [float(b['metadata'][metric])],
                               policy['objective_rtol'], policy['objective_atol']):
                reasons.add('worker objective')
        failures[cell['id']] = sorted(reasons)
    return failures


def number(value, digits=2):
    if value is None or not np.isfinite(value):
        return 'NR'
    if value >= 100:
        return f'{value:,.1f}'
    return f'{value:.{digits}f}'


def size_label(n):
    return f'{n/1e6:g}m' if n >= 1e6 else f'{n/1e3:g}k'


def configure_plots():
    plt.rcParams.update({'font.family': 'DejaVu Sans', 'font.size': 9,
                         'axes.spines.top': False, 'axes.spines.right': False,
                         'axes.edgecolor': '#CBD5DB', 'axes.labelcolor': INK,
                         'xtick.color': INK, 'ytick.color': INK, 'text.color': INK,
                         'axes.titleweight': 'bold', 'grid.color': '#DDE4E9',
                         'figure.facecolor': 'white', 'savefig.facecolor': 'white'})


def scaling_plot(frame, output):
    fig, axes = plt.subplots(1, 2, figsize=(10, 3.4), constrained_layout=True)
    for ax, model in zip(axes, ['ols', 'ppml']):
        subset = frame[(frame.model == model) & (frame.design == 'akm-baseline') &
                       (frame.rows == 1000000) & (~frame.save_fe) &
                       (frame.vce == ('iid' if model == 'ols' else 'robust'))]
        for arm in ['fereg', 'fixest', 'reghdfe' if model == 'ols' else 'ppmlhdfe']:
            selected = subset[(subset.arm == arm) & subset.eligible].sort_values('threads')
            ax.plot(selected.threads, selected.seconds, marker='o', color=ARM_COLORS[arm], label=ARM_NAMES[arm])
        ax.set(title=model.upper(), xlabel='Physical cores', ylabel='Median estimation seconds')
        ax.set_xticks([1, 4, 8, 16]); ax.set_yscale('log'); ax.grid(axis='y', alpha=.6)
    axes[1].legend(frameon=False, fontsize=8)
    fig.savefig(output, dpi=180); plt.close(fig)


def breadth_plot(frame, model, output):
    fig, axes = plt.subplots(3, 2, figsize=(10, 7.5), constrained_layout=True)
    for ax, design in zip(axes.flat, DESIGNS):
        subset = frame[(frame.model == model) & (frame.design == design) & (frame.threads == 4) &
                       (~frame.save_fe) & (frame.vce == ('iid' if model == 'ols' else 'robust'))]
        for arm in ['fereg', 'fixest', 'reghdfe' if model == 'ols' else 'ppmlhdfe']:
            selected = subset[(subset.arm == arm) & subset.eligible].sort_values('rows')
            ax.plot(selected.rows, selected.seconds, marker='o', color=ARM_COLORS[arm], label=ARM_NAMES[arm])
        ax.set_title(DESIGNS[design], loc='left', fontsize=10)
        ax.set_xscale('log'); ax.set_yscale('log')
        ax.set_xticks([100000, 1000000, 4000000], labels=['100k', '1m', '4m'])
        ax.grid(axis='y', alpha=.6); ax.set_ylabel('Seconds')
    axes[0, 0].legend(frameon=False, fontsize=8)
    fig.savefig(output, dpi=180); plt.close(fig)


class CampaignPDF:
    def __init__(self, path, identity):
        self.path, self.identity, self.story = path, identity, []
        self.styles = getSampleStyleSheet()
        for name in ['Normal', 'BodyText']:
            self.styles[name].fontName = 'CampaignSans'
            self.styles[name].fontSize = 10
            self.styles[name].leading = 14
            self.styles[name].textColor = colors.HexColor(INK)
        self.styles.add(ParagraphStyle('TitleCustom', fontName='CampaignSans-Bold', fontSize=29,
                                       leading=34, textColor=colors.HexColor(INK), spaceAfter=18))
        self.styles.add(ParagraphStyle('SectionCustom', fontName='CampaignSans-Bold', fontSize=18,
                                       leading=23, textColor=colors.HexColor(INK), spaceAfter=12))
        self.styles.add(ParagraphStyle('SmallCustom', fontName='CampaignSans', fontSize=8,
                                       leading=11, textColor=colors.HexColor(INK)))
        self.styles.add(ParagraphStyle('CellCustom', fontName='CampaignSans', fontSize=8,
                                       leading=10, textColor=colors.HexColor(INK), alignment=TA_LEFT))

    def text(self, value, style='BodyText'):
        self.story.extend([Paragraph(value, self.styles[style]), Spacer(1, 8)])

    def page(self, title):
        if self.story:
            self.story.append(PageBreak())
        self.text(title, 'SectionCustom')

    def table(self, rows, widths=None):
        data = [[Paragraph(str(value), self.styles['CellCustom']) for value in row] for row in rows]
        table = Table(data, colWidths=widths, repeatRows=1, hAlign='LEFT')
        table.setStyle(TableStyle([
            ('BACKGROUND', (0, 0), (-1, 0), colors.HexColor(LIGHT)),
            ('VALIGN', (0, 0), (-1, -1), 'TOP'), ('LEFTPADDING', (0, 0), (-1, -1), 7),
            ('RIGHTPADDING', (0, 0), (-1, -1), 7), ('TOPPADDING', (0, 0), (-1, -1), 6),
            ('BOTTOMPADDING', (0, 0), (-1, -1), 6),
            ('LINEBELOW', (0, 0), (-1, 0), .7, colors.HexColor('#B7C9D4')),
            ('ROWBACKGROUNDS', (0, 1), (-1, -1), [colors.white, colors.HexColor('#F7F9FA')]),
            ('LINEBELOW', (0, -1), (-1, -1), .5, colors.HexColor('#D9E2E7'))]))
        self.story.extend([table, Spacer(1, 12)])

    def figure(self, path, height):
        self.story.extend([Image(str(path), width=7.05*inch, height=height*inch), Spacer(1, 8)])

    def build(self):
        def decoration(canvas, doc):
            canvas.saveState()
            canvas.setStrokeColor(colors.HexColor(BLUE)); canvas.setLineWidth(2)
            canvas.line(36, 760, 576, 760)
            canvas.setFont('Helvetica', 8); canvas.setFillColor(colors.HexColor(INK))
            canvas.drawString(36, 775, 'FEREG  /  SCC BENCHMARK CAMPAIGN')
            canvas.drawString(36, 23, self.identity)
            canvas.drawRightString(576, 23, str(doc.page))
            canvas.restoreState()
        doc = BaseDocTemplate(str(self.path), pagesize=(612, 792),
                              title='fereg SCC benchmark campaign', author='fereg project',
                              leftMargin=36, rightMargin=36, topMargin=50, bottomMargin=40)
        doc.addPageTemplates(PageTemplate(id='campaign', frames=[Frame(36, 40, 540, 700,
                                                 leftPadding=0, rightPadding=0,
                                                 topPadding=0, bottomPadding=0)], onPage=decoration))
        doc.build(self.story)


def comparison_cell(frame, cell, arm, metric='seconds'):
    selected = frame[(frame.cell == cell) & (frame.arm == arm)]
    if selected.empty:
        return '-'
    row = selected.iloc[0]
    if not row.eligible:
        return 'NR'
    return number(row[metric])


def render(campaign, frame, output):
    output.mkdir(parents=True, exist_ok=True)
    frame.to_csv(output/'configuration-summary.csv', index=False)
    flat_records = []
    for record in campaign['records']:
        flat_records.append({key: record.get(key) for key in
                             ['cell', 'arm', 'repetition', 'status', 'host', 'job', 'task',
                              'estimator_tree_rss_bytes', 'memory_sweeps', 'wall_seconds']} |
                            {'seconds': record.get('metadata', {}).get('seconds'),
                             'error': record.get('error', record.get('metadata', {}).get('error'))})
    pd.DataFrame(flat_records).to_csv(output/'observations.csv', index=False)
    configure_plots()
    from matplotlib.font_manager import findfont, FontProperties
    pdfmetrics.registerFont(TTFont('CampaignSans', findfont(FontProperties(family='DejaVu Sans'))))
    pdfmetrics.registerFont(TTFont('CampaignSans-Bold', findfont(FontProperties(family='DejaVu Sans', weight='bold'))))
    pdfmetrics.registerFontFamily('CampaignSans', normal='CampaignSans', bold='CampaignSans-Bold',
                                italic='CampaignSans', boldItalic='CampaignSans-Bold')
    scaling_plot(frame, output/'scaling.png')
    for model in ['ols', 'ppml']:
        breadth_plot(frame, model, output/f'{model}-breadth.png')
    context, manifest = campaign['context'], campaign['manifest']
    source = context['source'][:12]
    pdf = CampaignPDF(output/'fereg-scc-benchmark.pdf', f"{context['run_id']}  |  source {source}")
    pairs = frame[(frame.arm != 'fereg') & (frame.arm != 'fixest-strict')]
    eligible = pairs[pairs.eligible]
    statuses = Counter(r['status'] for r in campaign['records'])
    jobs = campaign['accounting']['jobs']
    production = [j for j in jobs if str(j['jobnumber']) == str(context['production_job'])]
    pdf.text('A focused comparison<br/>of high-dimensional<br/>fixed-effect estimators', 'TitleCustom')
    pdf.text('OLS &amp; PPML · AKM baseline · 16 controls', 'SectionCustom')
    pdf.text(f"October 2026 · Frozen source <b>{source}</b> · Boston University SCC")
    pdf.table([['Configurations', 'Timed fits', 'Primary cores', 'Whole-node reservations'],
               [len(manifest['cells']), len(campaign['records']), '4; selected 1 / 8 / 16', '4 production nodes × 28 cores']],
              [115, 105, 165, 155])
    pdf.text(f"<b>{len(eligible)} of {len(pairs)} primary package comparisons</b> meet the scientific gates in all three paired repetitions. "
             f"{statuses.get('ok', 0)} of {len(campaign['records'])} application captures succeeded. "
             "Unresolved results are marked NR and do not enter speed comparisons.")
    if not eligible.empty:
        byarm = []
        for arm, group in eligible.groupby('arm'):
            byarm.append(f"{ARM_NAMES[arm]}: {number(group.speedup.min())}–{number(group.speedup.max())}× across {len(group)} eligible configurations")
        pdf.text('<b>Observed comparator / fereg time:</b><br/>'+'<br/>'.join(byarm))
    pdf.text(f"Production occupied {sum(float(j['ru_wallclock']) for j in production)/3600:.2f} node-hours. "
             f"The longest production reservation lasted {max(float(j['ru_wallclock']) for j in production)/3600:.2f} hours. "
             "Every task used 28 reserved slots and finished within the requested walltime.")
    pdf.text('This is a descriptive, fixed-seed campaign. The selected workloads cover distinct dimensions; '
             'the results do not establish a universal package ranking. Comparisons use the same data and physical-core budget within each cell.',
             'SmallCustom')

    pdf.page('The comparison protocol')
    pdf.text('The main panel uses a two-way worker–firm AKM design with one million observations and 16 continuous controls. '
             'Each worker appears in five periods; baseline mobility is 20% per transition. Clusters are workers. '
             'The OLS panel crosses regular, robust and clustered covariance with saved FE on/off. '
             'The PPML panel crosses robust and clustered covariance with saved FE on/off.')
    pdf.table([['Dimension', 'Selected coverage'],
               ['Model', 'OLS and PPML; PPML uses positive, fractional outcomes'],
               ['Physical cores', '1 / 4 / 8 / 16 for fereg and fixest; 1 / 4 for Stata comparators'],
               ['Size', '100,000 / 1,000,000 / 4,000,000 observations'],
               ['Fixed effects', 'One way; two and three way with dense and sparse designs'],
               ['Repetition', 'Three fresh processes per package/configuration; rotating package order'],
               ['Reservation', 'Exactly 28-core nodes, exclusive allocation, sequential fits, four-hour production jobs']],
              [120, 420])
    pdf.text('<b>What is timed.</b> Estimation, covariance and requested FE recovery. A separate small initialization fit precedes each timed command. '
             'For fixest, lazy FE extraction and observation-level materialization are included. Data generation/import, hash checks and disk export are excluded. '
             'BLAS libraries use one thread; each process is bound to the requested number of physical cores.')
    pdf.text('<b>Scientific eligibility.</b> All three repetitions must agree on original sample rows, rank/omission/deletion decisions, cluster count and declared DoF/covariance policy. '
             'All 16 slopes, all 256 slope covariance entries and the objective must agree at SPEC tolerances. Saved-output cells also require prediction agreement and model score checks. '
             'Native coordinates and objectives repeat exactly at fixed worker count; higher-core cells require agreement with their four-core scientific witness. '
             'A bounded independent dummy-design oracle checks the small smoke cases before production.')
    pdf.text('<b>Conventions.</b> PPML fixest disables its K correction and applies N/(N−1) to robust covariance; stock covariance is retained. '
             'Its default and stricter convergence profiles are separate. Stock Stata differences remain visible. '
             'NR means unavailable or unresolved; a displayed ratio requires a successful scientific comparison. '
             'A ratio above 1 means the comparator took longer than fereg.', 'SmallCustom')

    for model in ['ols', 'ppml']:
        pdf.page(f'{model.upper()}: the four-core AKM baseline')
        pdf.text('1,000,000 observations · worker and firm FE · 16 controls · three paired repetitions')
        cells = [c for c in manifest['cells'] if c['model'] == model and c['section'] == 'baseline']
        comparator = 'reghdfe' if model == 'ols' else 'ppmlhdfe'
        rows = [['Covariance / output', 'fereg<br/>seconds', 'fixest<br/>seconds', f'{comparator}<br/>seconds',
                 'fixest /<br/>fereg', f'{comparator} /<br/>fereg']]
        for c in cells:
            rows.append([VCE_NAMES[c['vce']] + (' + saved FE' if c['save_fe'] else ''),
                         comparison_cell(frame, c['id'], 'fereg'), comparison_cell(frame, c['id'], 'fixest'),
                         comparison_cell(frame, c['id'], comparator), comparison_cell(frame, c['id'], 'fixest', 'speedup'),
                         comparison_cell(frame, c['id'], comparator, 'speedup')])
        pdf.table(rows, [170, 70, 70, 80, 70, 80])
        native = frame[(frame.model == model) & (frame.section == 'baseline') & (frame.arm == 'fereg') & frame.eligible]
        if not native.empty:
            pdf.text(f"The fereg medians span {number(native.seconds.min())}–{number(native.seconds.max())} seconds in this panel. "
                     'Package ratios are medians of paired comparator/fereg times. The raw observation CSV preserves each repetition and the full within-cell range.')
        rows = [['Covariance', 'fereg saved / unsaved', 'fixest saved / unsaved', f'{comparator} saved / unsaved']]
        for vce in (['iid', 'robust', 'cluster'] if model == 'ols' else ['robust', 'cluster']):
            row = [VCE_NAMES[vce]]
            for arm in ['fereg', 'fixest', comparator]:
                group = frame[(frame.model == model) & (frame.section == 'baseline') & (frame.vce == vce) & (frame.arm == arm)]
                ratio = None
                if len(group) == 2 and group.eligible.all():
                    ratio = group[group.save_fe].seconds.iloc[0]/group[~group.save_fe].seconds.iloc[0]
                row.append(number(ratio)+'×' if ratio is not None else 'NR')
            rows.append(row)
        pdf.table(rows, [120, 140, 140, 140])
        pdf.text('Output overhead compares the same package and covariance with and without individual saved FE. '
                 'These ratios describe this host and this data; they are not uncertainty intervals. '
                 'The positive PPML inputs deliberately avoid separation, so this campaign does not measure separation handling.', 'SmallCustom')

    pdf.page('Core scaling within a reserved node')
    pdf.text('The full AKM thread panel for each model remains on the same node. The Stata comparators stop at four processors because of the license. '
             'The higher-core comparisons include only fereg and fixest.')
    pdf.figure(output/'scaling.png', 2.4)
    rows = [['Model / cores', 'fereg seconds', 'fixest seconds', 'fixest / fereg']]
    for model in ['ols', 'ppml']:
        subset = frame[(frame.model == model) & (frame.design == 'akm-baseline') & (frame.rows == 1000000) &
                       (~frame.save_fe) & (frame.vce == ('iid' if model == 'ols' else 'robust')) & (frame.arm == 'fereg')]
        for row in subset.sort_values('threads').itertuples():
            rows.append([f'{model.upper()} / {row.threads}', comparison_cell(frame, row.cell, 'fereg'),
                         comparison_cell(frame, row.cell, 'fixest'), comparison_cell(frame, row.cell, 'fixest', 'speedup')])
    pdf.table(rows, [150, 130, 130, 130])
    pdf.text('Logarithmic seconds axis. A missing comparator point is an unresolved scientific comparison. '
             'Selected 16-core clustered/saved and dense/sparse cases appear in the configuration appendix.', 'SmallCustom')

    for model in ['ols', 'ppml']:
        pdf.page(f'{model.upper()}: size and graph structure')
        pdf.text('Four-core fits · no saved FE · '+('regular covariance' if model == 'ols' else 'robust covariance')+
                 ' · logarithmic axes. Comparator points require three eligible repetitions; missing points are NR.')
        pdf.figure(output/f'{model}-breadth.png', 5.65)
        pdf.text('Dense designs repeat Cartesian combinations. Sparse designs use 2% worker mobility; the three-way design adds a 32-level categorical FE. '
                 'Observation counts change the number of workers and firms as well as data volume. Actual graph counts are recorded below.', 'SmallCustom')

    pdf.page('Memory and reservation use')
    pdf.text('Memory is the median observed peak RSS of the estimator process tree during the timed command. '
             'The Python observer and its input/checking arrays are excluded. Sampling occurs every 20 ms; cells with fewer than ten observations in any repetition are NR. '
             'RSS includes imported data and runtime state and is not an incremental allocation measure.')
    rows = [['Baseline / output', 'fereg GiB', 'fixest GiB', 'Stata comparator GiB']]
    for c in manifest['cells']:
        if c['section'] == 'baseline':
            comparator = 'reghdfe' if c['model'] == 'ols' else 'ppmlhdfe'
            rows.append([f"{c['model'].upper()} / {VCE_NAMES[c['vce']]}"+(' / saved' if c['save_fe'] else ''),
                         comparison_cell(frame, c['id'], 'fereg', 'rss_gib'),
                         comparison_cell(frame, c['id'], 'fixest', 'rss_gib'),
                         comparison_cell(frame, c['id'], comparator, 'rss_gib')])
    pdf.table(rows, [240, 90, 90, 120])
    rows = [['Production task', 'Host', 'Wall hours', 'Slots', 'Exit / failed']]
    for job in production:
        rows.append([job['taskid'], job['hostname'], number(float(job['ru_wallclock'])/3600), job['slots'],
                     f"{job['exit_status']} / {job['failed']}"])
    pdf.table(rows, [90, 190, 100, 60, 100])
    pdf.text('The accounting file preserves scheduler memory and CPU metrics separately. Reserved node-hours include preparation, export and checks; '
             'they differ from the sum of estimation stopwatches. Full-node reservation removes competing scheduled jobs, '
             'but this campaign does not claim that operating-system activity was zero.', 'SmallCustom')

    pdf.page('Scientific coverage and unresolved results')
    oracle_results = campaign['smoke']['independent_science']
    native_oracle = [x for x in oracle_results if x['arm'] == 'fereg']
    pdf.text(f"The independent small-data oracle accepted {sum(x['pass'] for x in native_oracle)} of {len(native_oracle)} fereg smoke cases. "
             f"Across all smoke arms it accepted {sum(x['pass'] for x in oracle_results)} of {len(oracle_results)} captures. "
             'The production gate depends on the independent native results. Large-input discrepancies remain unresolved unless independently adjudicated.')
    pdf.table([['Application status', 'Timed fits']]+[[key, value] for key, value in sorted(statuses.items())], [380, 160])
    failures = pairs[~pairs.eligible]
    reason_counts = Counter(reason for value in failures.reasons for reason in value.split('; ') if reason)
    pdf.table([['Primary comparison gate', 'Configurations']]+[[key, value] for key, value in reason_counts.most_common()], [380, 160])
    strict = frame[frame.arm == 'fixest-strict']
    if not strict.empty:
        pdf.text(f"The supplementary stricter fixest profile has {int(strict.eligible.sum())} eligible cells out of {len(strict)}. "
                 'Its timing is not substituted into the primary default-fixest panel.')
        rows = [['PPML stricter profile', 'Seconds', 'strict / fereg', 'Result']]
        for row in strict.itertuples():
            rows.append([DESIGNS[row.design]+' / '+VCE_NAMES[row.vce]+(' / saved' if row.save_fe else ''),
                         number(row.seconds) if row.eligible else 'NR', number(row.speedup) if row.eligible else 'NR',
                         'eligible' if row.eligible else row.reasons or row.statuses])
        pdf.table(rows, [225, 75, 90, 150])
    pdf.text('NR cells remain in the appendix and CSVs. Successful raw runtimes in an unresolved cell are observations, '
             'not validated package comparisons. No tolerance was loosened to make a timing eligible.', 'SmallCustom')

    pdf.page('Artifacts, tolerances and provenance')
    pdf.table([['Artifact', 'Identity / policy'],
               ['fereg source', context['source']],
               ['Run', context['run_id']],
               ['Comparators', 'Pinned reghdfe / ppmlhdfe / ftools / require; fixest 0.14.2'],
               ['Software stack', 'Stata/MP 19 (four licensed processors); R 4.5.2; Python 3.12.4'],
               ['Seed / controls', f"{manifest['policy']['seed']} / 16"],
               ['Coefficient tolerance', '1e−10 absolute + 1e−7 relative'],
               ['Covariance tolerance', '1e−10 absolute + 1e−6 relative'],
               ['Prediction tolerance', '1e−9 absolute + 1e−7 relative'],
               ['Objective tolerance', '1e−10 absolute + 1e−8 relative'],
               ['Bounded stopping', '600 s command; 900 s process; 3 h 50 min cleanup threshold; 4 h production job'],
               ['Delivery', 'PDF, configuration-summary.csv, observations.csv, compact frozen run evidence']],
              [150, 390])
    pdf.text('Executable, frontend, transitive benchmark harness and comparator files are hash-frozen and checked before and after each task. '
             'DTA files are round-trip verified; R verifies every imported numeric column against IEEE-double hashes. '
             'Datasets and full row captures remain in the owner-only SCC run directory. '
             'Job accounting, application sentinels, exact manifest coverage and the independent native smoke all precede this report.')
    if context.get('failed_jobs'):
        pdf.text('Before measurement, an offline build attempt lacked a locked dependency. Its logs were preserved, '
                 'and the exact locked revision was fetched. A later qualification attempt exposed a reference-cache '
                 'integrity defect on rapid file rewrites; content hashes and independent mutation tests repaired the guard. '
                 'Failed attempts were retained before fresh qualification. '
                 'The scheduler request also removed an unconfigured exclusive consumable; reserving all 28 slots on an exactly 28-core host '
                 'provides isolation under SCC’s per-host slot limit. These corrections did not change the estimator source or scientific tolerances.',
                 'SmallCustom')
    pdf.text('The previous comparable September 2026 campaigns used the former runtime and different coverage. '
             'They supplied the scheduling pattern and pinned comparator closure, but their timings are not a before/after baseline for this implementation. '
             'Three repetitions at one seed do not establish confidence intervals or cross-machine reproducibility. '
             'The wider three-way panel measures selected categorical structures, not arbitrary FE hypergraphs.', 'SmallCustom')

    for model in ['ols', 'ppml']:
        comparator = 'reghdfe' if model == 'ols' else 'ppmlhdfe'
        cells = sorted((c for c in manifest['cells'] if c['model'] == model),
                       key=lambda c: (list(DESIGNS).index(c['design']), c['rows'], c['threads'], c['vce'], c['save_fe']))
        midpoint = (len(cells)+1)//2
        for part, chunk in enumerate([cells[:midpoint], cells[midpoint:]], 1):
            pdf.page(f'Configuration appendix: {model.upper()} ({part}/2)')
            pdf.text('Median seconds and comparator/fereg ratios. NR means the full scientific/timing gate is not met; “–” means the comparator was not scheduled. '
                     'K=16 in every cell.', 'SmallCustom')
            rows = [['Design / N', 'Cores / VCE / saved', 'fereg s', 'fixest s', f'{comparator} s', 'fixest ×', 'Stata ×']]
            for c in chunk:
                rows.append([DESIGNS[c['design']]+' / '+size_label(c['rows']),
                             f"{c['threads']} / {c['vce']} / {'yes' if c['save_fe'] else 'no'}",
                             comparison_cell(frame, c['id'], 'fereg'), comparison_cell(frame, c['id'], 'fixest'),
                             comparison_cell(frame, c['id'], comparator), comparison_cell(frame, c['id'], 'fixest', 'speedup'),
                             comparison_cell(frame, c['id'], comparator, 'speedup')])
            pdf.table(rows, [155, 115, 50, 55, 60, 50, 55])

    pdf.page('Generated graph characteristics')
    pdf.text('One row per design and observation count; OLS and PPML use the same graph-generation seed. '
             'Components refer to the graph connecting categorical levels. For three FE, first-pair rank is a two-way statistic, not a full three-way rank claim.',
             'SmallCustom')
    rows = [['Design / N', 'FE levels', 'Components', 'First-pair links', 'Worker mean degree']]
    seen = set()
    for key, data in sorted(campaign['datasets'].items()):
        design = next(c['design'] for c in manifest['cells'] if f"{c['model']}-{c['design']}-n{c['rows']}" == key)
        n = int(key.rsplit('n', 1)[1])
        if (design, n) in seen:
            continue
        seen.add((design, n)); graph = data['topology']
        rows.append([DESIGNS[design]+' / '+size_label(n), ' / '.join(f'{x:,}' for x in graph['levels']),
                     f"{graph['components']:,}", f"{graph['distinct_first_pair_links']:,}" if 'distinct_first_pair_links' in graph else '-',
                     number(graph.get('first_pair_mean_worker_degree'))])
    pdf.table(rows, [165, 135, 70, 95, 75])
    pdf.build()
    return pdf.path


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    campaign = load_campaign(args.root)
    print(render(campaign, summarize(campaign), args.output))
