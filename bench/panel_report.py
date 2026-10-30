#!/usr/bin/env python3
"""Render validated aggregate Veneto evidence; no private rows or FE vectors."""
from __future__ import annotations

import argparse
from collections import Counter
import copy
from datetime import datetime
import hashlib
import json
from pathlib import Path

import campaign_report as report
from panel_collect import load

ORDER = ['fixest', 'fixest-strict', 'pyfixest', 'pyfixest-within', 'pyfixest-within-strict', 'julia', 'julia-strict', 'reghdfe']
DESCRIPTIONS = {'fixest': 'fixest 0.14.2 default', 'fixest-strict': 'fixest strict',
                'pyfixest': 'PyFixest 0.60.0 (Rust MAP)', 'pyfixest-within': 'PyFixest main (within LSMR)', 'pyfixest-within-strict': 'PyFixest main (within LSMR, strict)',
                'julia': 'FixedEffectModels.jl (tolerance 1e-6)', 'julia-strict': 'FixedEffectModels.jl (1e-10)'}


def reference_arms(manifest):
    return [arm for arm in ORDER if any(arm in c['arms'] for c in manifest['cells'])]


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validated_summary(campaign):
    reporting = copy.deepcopy(campaign)
    reporting['manifest']['policy'].update(campaign['manifest']['policy']['scientific_tolerances'])
    frame = report.summarize(reporting)
    if not frame.loc[frame.arm == 'fereg', 'eligible'].all():
        raise ValueError('native summary eligibility failed; do not resample')
    return frame


def native_plot(frame, panel, path):
    selected = frame[(frame.design == 'veneto-'+panel) & (frame.arm == 'fereg')].sort_values('threads')
    fig, ax = report.plt.subplots(figsize=(10, 3.2), constrained_layout=True)
    ax.errorbar(selected.threads, selected.seconds,
                yerr=[selected.seconds-selected.seconds_min, selected.seconds_max-selected.seconds],
                marker='o', color=report.BLUE, capsize=4, linewidth=2)
    ax.set(xlabel='Physical cores', ylabel='Command seconds', ylim=(0, selected.seconds_max.max()*1.18))
    ax.set_xticks([1, 2, 4, 8, 16]); ax.grid(axis='y', alpha=.6)
    for row in selected.itertuples():
        ax.annotate(f'{row.seconds:.2f}', (row.threads, row.seconds), xytext=(0, 10),
                    textcoords='offset points', ha='center', color=report.INK)
    fig.savefig(path, dpi=180)
    report.plt.close(fig)


def timing(row):
    if row is None:
        return '-'
    if not row.timed_repetitions:
        return 'Unavailable'
    value = f'{row.observed_seconds:.3f}' + ('' if row.eligible else '†' if row.agreement == 'convention' else '*')
    if row.timed_repetitions == row.repetitions:
        return value+f'<br/><font size="6">[{row.seconds_min:.3f}, {row.seconds_max:.3f}]</font>'
    return value+'<br/><font size="6">n=1; later positions skipped</font>'


def render(campaign, frame, output):
    report.configure_plots()
    from matplotlib.font_manager import findfont, FontProperties
    for name, weight in [('CampaignSans', 'normal'), ('CampaignSans-Bold', 'bold')]:
        report.pdfmetrics.registerFont(report.TTFont(name, findfont(FontProperties(family='DejaVu Sans', weight=weight))))
    report.pdfmetrics.registerFontFamily('CampaignSans', normal='CampaignSans', bold='CampaignSans-Bold',
                                       italic='CampaignSans', boldItalic='CampaignSans-Bold')
    references = frame[frame.arm != 'fereg']
    eligible_references = int(references.eligible.sum())
    report.TIMING_FOOTNOTE = ('* Comparator timing failed the frozen scientific/output gate. '
                              '† Agreement up to degrees-of-freedom counting conventions. '
                              'Medians with [min, max]; n=1 is one completed command. No fits were retried.')
    arms = reference_arms(campaign["manifest"])
    pdf = report.CampaignPDF(output/'veneto-akm.pdf', campaign['context']['run_id'])
    saved = campaign['manifest']['policy'].get('save_fe', True)
    pdf.text('Veneto AKM<br/>Core scaling and '+('saved effects' if saved else 'estimation without saved effects'), 'TitleCustom')
    date = datetime.strptime(campaign['context']['run_id'][:8], '%Y%m%d').strftime('%d %B %Y')
    pdf.text(date+'  /  Linux SCC  /  Frozen packaged runtime', 'SmallCustom')
    cells, repetitions = campaign['manifest']['cells'], campaign['manifest']['policy']['repetitions']
    pdf.text(f'All {len(cells)*repetitions} fereg fits pass the production gates. '+
             ('Every comparator configuration is ineligible under the unchanged scientific checks, '
              'so its time is descriptive. This campaign does not establish an eligible cross-package speed ranking.'
              if not eligible_references else
              f'{eligible_references} of {len(references)} comparator configurations pass the frozen scientific gates. '
              'Asterisked times remain descriptive.'))
    panels = campaign['manifest']['panels']
    pdf.table([['Retained fevc panel', 'Rows', 'Workers', 'Firms', 'Controls']]+[
        [p.capitalize(), f"{s['rows']:,}", f"{s['workers']:,}", f"{s['firms']:,}", s['regressors']]
        for p, s in panels.items()], [125, 110, 110, 105, 57])
    for panel in panels:
        native = frame[(frame.design == 'veneto-'+panel) & (frame.arm == 'fereg')].sort_values('threads')
        first, last = native.iloc[0], native.iloc[-1]
        pdf.text(f'<b>{panel.capitalize()} panel:</b> {first.seconds:.3f} seconds at one core to '
                 f'{last.seconds:.3f} at sixteen: {first.seconds/last.seconds:.2f}x within-panel scaling. '
                 f'All three-command native ranges and every reference observation follow.')
    pdf.text('Model and command boundary', 'SectionCustom')
    pdf.text('OLS of unadjusted log daily wages on worker and firm intercept effects, with unit '
             'weights and regular IID covariance. '+
             ('Both FE channels are saved inside command timing. ' if saved else
              'No FE are saved; commands time estimation and inference, and comparisons cover slopes, '
              'covariance, sample, rank and RSS. ')+
             'Import, export and checks are outside timing. '+
             ' '.join({'long': 'Long: 1996-2001; normalized age squared and cubed plus 1996-2000 indicators (2001 base).',
                       'short': 'Short: 1999/2001, with a 2001 indicator.'}[p] for p in panels))
    planned = sum(len(c['arms']) for c in campaign['manifest']['cells'])*campaign['manifest']['policy']['repetitions']
    pdf.text(f'{repetitions} repetitions at 1/2/4/8/16 physical cores for fereg, '+
             ', '.join(DESCRIPTIONS[a] for a in arms if a != 'reghdfe')+
             '; reghdfe 6.14.1 at 1/2/4 because Stata is licensed for four cores. '
             f'{planned} planned positions. One isolated allocation per panel.')
    for panel in panels:
        selected = frame[frame.design == 'veneto-'+panel]
        native = selected[selected.arm == 'fereg'].sort_values('threads')
        pdf.page(panel.capitalize()+' panel: native core scaling')
        pdf.text(f"Host {native.iloc[0].host}; {panels[panel]['rows']:,} rows. "
                 'Line: fereg median; whiskers: minimum and maximum of three commands.', 'SmallCustom')
        chart = output/(panel+'-native.png')
        native_plot(frame, panel, chart)
        pdf.figure(chart, 2.25)
        rows = [['Cores', 'fereg', *[report.ARM_NAMES[a] for a in arms]]]
        for cores in [1, 2, 4, 8, 16]:
            items = selected[selected.threads == cores]
            rows.append([cores]+[timing(next((r for r in items.itertuples() if r.arm == arm), None))
                                 for arm in ['fereg', *arms]])
        pdf.table(rows, [37]+[(540-37)//(1+len(arms))]*(1+len(arms)), padding=6)
        details = []
        for arm in arms:
            arm_cells = selected[selected.arm == arm]
            errors = sorted({r['error'] for r in campaign['records']
                             if r['cell'].startswith('veneto-'+panel+'-') and r['arm'] == arm and r.get('error')})
            reasons = sorted({reason for value in arm_cells.reasons for reason in value.split('; ') if reason
                              and reason != 'application unavailable'})
            if any(r['status'] == 'command-timeout' for r in campaign['records']
                   if r['cell'].startswith('veneto-'+panel+'-') and r['arm'] == arm):
                reasons.append('600-second command limit')
            convention = int((arm_cells.agreement == 'convention').sum())
            details.append(f'{report.ARM_NAMES[arm]}: {int(arm_cells.eligible.sum())}/{len(arm_cells)} eligible cells'+
                           (f', {convention} more up to degrees-of-freedom conventions. ' if convention else '. ')+
                           ('; '.join(errors+reasons)+'.' if errors or reasons else 'All checks pass.'))
        pdf.text(' '.join(details), 'SmallCustom')
        base, last = native.iloc[0], native.iloc[-1]
        pdf.text(f'Native 1-to-16-core scaling: {base.seconds/last.seconds:.2f}x '
                 f'({100*(1-last.seconds/base.seconds):.1f}% less command time). '
                 f'8-to-16 cores saves {100*(1-last.seconds/native.iloc[-2].seconds):.1f}%. '
                 'These are descriptive medians on one host, not confidence-qualified estimates.')
    pdf.page('Validation, discrepancies and limits')
    counts = Counter(r['status'] for r in campaign['records'])
    pdf.text(f"All {len(campaign['records'])} positions are retained: {counts['ok']} successful captures, "
             f"{counts['capture-error']} capture errors and {counts['skipped-after-failure']} skipped positions. "
             'Every failed observation remains visible. No replacement observations or tolerance changes were used.')
    pdf.table([['Gate', 'Outcome'],
               ['Scheduler and application', 'Setup, smoke and '+('both production tasks' if len(panels) == 2 else 'the production task')+
                ': failed=0, exit=0; all sentinels present.'],
               ['Native repeated science', f'{len(cells)*(repetitions-1)} exact repeat checks; scientific CSV'+(' and raw FE' if saved else '')+' hashes, reported RSS and discrete decisions stable.'],
               ['Native core consistency', f'{len(cells)*repetitions} checks against the one-core reference, including '
                f'{sum(c["threads"] == 1 for c in cells)*repetitions} one-core checks and '
                f'{sum(c["threads"] != 1 for c in cells)*repetitions} between core counts; four-core witnesses also pass.'],
               ['Independent witnesses', 'Every native dense-design real-data smoke passes coefficients'+(', covariance and fitted values' if saved else ' and covariance')+
                '. Full-panel graph rank'+(', model scores and objective replay pass.' if saved else ' passes.')],
               ['Host evidence', 'Every complete reservation and all-queue census replays successfully; continuous samples report no known competing compute.'],
               ['Retained input audit', 'Exact fevc identifiers. Control-adjustment max errors: '+
                ', '.join(f"{campaign['input_audit'][p]['adjustment_max_error']:.3g} {p}" for p in panels)+' (gate 1e-12).']],
              [132, 375], padding=5)
    pdf.text('Reference accuracy and interpretation', 'SectionCustom')
    if saved:
        pdf.text('The per-panel tables identify every reference discrepancy. The independent saved-predictor '
                 'model-score bound is 1e-7. FE comparisons use observation-weighted centering to remove '
                 'the arbitrary connected-graph additive normalization. A remaining centered-channel '
                 'discrepancy exceeds the frozen tolerance; its underlying numerical cause is not diagnosed here.', 'SmallCustom')
    else:
        pdf.text('The per-panel tables identify every reference discrepancy. Without saved FE, comparisons '
                 'cover slopes, covariance, sample, rank, degrees of freedom and RSS; saved-predictor and '
                 'centered-FE checks do not apply.', 'SmallCustom')
    smoke = [(s['panel'], s['arm'], s['passed']) for s in campaign['smoke']['science'] if s['kind'] == 'dense-oracle']
    failed_smokes = ', '.join(panel+' '+arm for panel, arm, passed in smoke if not passed) or 'none'
    pdf.text('Dense real-data smoke failures: '+failed_smokes+'. Production failures remain visible '
             'despite smoke passes. No full-panel dense oracle was run. Tolerances: coefficients '
             '1e-7/1e-10, covariance 1e-6/1e-10, fitted '
             'values and centered FEs 1e-7/1e-9, objective 1e-8/1e-10 (relative/absolute).', 'SmallCustom')
    pdf.text('Each panel uses one 28-core Broadwell allocation with common socket-ordered CPU '
             'prefixes and NUMA binding for all arms. Sixteen cores crosses sockets. The panels '
             'have different models, sizes and hosts; do not interpret their timing difference as '
             'a controlled panel-length effect. Three process repetitions do not measure between-host '
             'uncertainty. Passive observation covers known compute; ordinary OS activity is retained.', 'SmallCustom')
    context = campaign['context']
    pdf.text('Identity and retention', 'SectionCustom')
    pdf.text('Harness source '+context['source'][:12]+'; packaged native source '+context['runtime_source'][:12]+
             '. Plugin SHA-256 '+context['linux_plugin_sha256']+'. Compact CSV/JSON evidence and '
             'the maintained renderer are committed in bench/. Full captures, '+('saved effects, ' if saved else '')+'host '
             'observations and this report remain private under the recorded SCC campaign root.', 'SmallCustom')
    pdf.build()


def export(root, output):
    campaign = load(root)
    hosts = json.loads((root/'host-validation.json').read_text())
    tasks = {str(task) for task in range(1, len(campaign['manifest'].get('panels', 'ls'))+1)}
    if set(hosts) != tasks or not all(v['passed'] for v in hosts.values()):
        raise ValueError('retained host replay is missing')
    frame = validated_summary(campaign)
    output.mkdir(parents=True, exist_ok=True)
    prefix = campaign['context'].get('export_prefix', 'scc-'+campaign['context']['run_id'][:8]+'-veneto')
    frame.to_csv(output/(prefix+'-summary.csv'), index=False)
    # Keep every planned position, including failures and skipped repetitions.
    (output/(prefix+'-observations.csv')).write_bytes((root/'observations.csv').read_bytes())
    (output/(prefix+'-science.json')).write_bytes((root/'science.json').read_bytes())
    campaign['input_audit'] = json.loads((root/'environment/input-audit.json').read_text())
    render(campaign, frame, output)
    context = campaign['context']
    evidence = {k: context[k] for k in ['run_id', 'remote_root', 'source', 'runtime_source',
                'linux_plugin_sha256', 'fevc_source', 'resources', 'preparation_sha256', 'preparation_notes']}
    runtime = json.loads((root/'environment/runtime.json').read_text())
    critical = {'source/dist/fereg_linux.plugin', 'environment/command-clock.plugin', 'manifest.json'}
    critical.update('source/bench/'+name for name in ['panel_campaign.py', 'campaign.py', 'campaign_stata.do',
                                                    'campaign_r.R', 'campaign_fixest.R', 'campaign_pyfixest.py',
                                                    'campaign_julia.jl', 'campaign_oracle.py',
                                                    'qualify_scc.py', 'block_host.py'])
    critical &= set(runtime)
    preparation = {}
    for name in ['inputs.json', 'long-sample.json', 'short-sample.json']:
        path = root/'preparation'/name
        if sha256(path) != context['preparation_sha256'][name]:
            raise ValueError('preparation metadata hash differs: '+name)
        preparation[name] = json.loads(path.read_text())
    evidence.update(schema=1, manifest=campaign['manifest'], accounting=campaign['accounting'],
                    input_audit=json.loads((root/'environment/input-audit.json').read_text()),
                    prior_panels=json.loads((root/'input/prior-panels.json').read_text()),
                    references=json.loads((root/'environment/reference-origin.json').read_text()),
                    host_validation=hosts, native_cells=[], smoke_science=campaign['smoke']['science'],
                    science=campaign['science'], status_counts=dict(Counter(r['status'] for r in campaign['records'])),
                    eligible_arm_cells=int(frame.eligible.sum()),
                    preparation=preparation,
                    retained_runtime_inventory=dict(sha256=sha256(root/'environment/runtime.json'),
                                                    files=len(runtime), critical_files={k: runtime[k] for k in sorted(critical)}))
    for cell in campaign['manifest']['cells']:
        runs = [r for r in campaign['records'] if r['cell'] == cell['id'] and r['arm'] == 'fereg']
        c = runs[0]['capture']
        # Runs without saved FE have no saved channels or saved-predictor score.
        evidence['native_cells'].append(dict(cell=cell['id'], **{k: c.get(k) for k in
            ['N', 'sample_sha256', 'finite_slope_rank', 'absorbed_dof', 'omitted', 'publication_omitted',
             'singleton_deletions', 'separation_deletions', 'saved_fe_sha256', 'scientific_sha256']},
            exact_reported_rss=[r['metadata']['rss'].strip() for r in runs],
            maximum_model_score=max((r['capture'].get('maximum_model_score') for r in runs
                                     if 'maximum_model_score' in r['capture']), default=None),
            placement={k: runs[0]['placement'][k] for k in ['cpus', 'sockets', 'memory_nodes']},
            cpu_model=next(line.split(':', 1)[1].strip() for line in runs[0]['placement']['lscpu'].splitlines()
                           if line.startswith('Model name:'))))
    evidence['artifacts_sha256'] = {p.name: sha256(p) for p in sorted(output.iterdir())
                                    if p.suffix in ['.pdf', '.csv', '.json'] and not p.name.endswith('-inputs.json')}
    evidence['collected_aggregate_sha256'] = {str(p.relative_to(root)): sha256(p)
        for p in sorted(root.rglob('*')) if p.is_file()}
    evidence['analysis_tools_sha256'] = {name: sha256(Path(__file__).with_name(name))
                                        for name in ['panel_collect.py', 'panel_report.py']}
    (output/(prefix+'-inputs.json')).write_text(json.dumps(evidence, indent=2)+'\n')
    return frame


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('collection', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    export(args.collection, args.output)
