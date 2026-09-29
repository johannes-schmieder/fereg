"""Analyze a user-stopped campaign without changing its frozen measurements."""
import argparse
import csv
import json
import subprocess
from collections import Counter, defaultdict
from pathlib import Path

import numpy as np

from common import HERE, cell_id, cells, read_json, sha256, write_json
from report import collect, quantiles, csv_file, panel, composite, tex_escape, DESIGN_LABELS
from campaign import audit

KEYS = ('model', 'design', 'n', 'k', 'threads', 'section', 'arm')


def table(headers, rows, spec):
    return '\n'.join([r'\small\begin{longtable}{' + spec + '}', r'\toprule',
                      ' & '.join(map(tex_escape, headers)) + r'\\\midrule\endhead',
                      *[' & '.join(map(tex_escape, row)) + r'\\' for row in rows],
                      r'\bottomrule\end{longtable}\normalsize'])


def document(title):
    return [r'\documentclass[11pt]{article}', r'\usepackage[margin=0.78in]{geometry}',
            r'\usepackage{graphicx,subcaption,booktabs,longtable,hyperref,array}',
            r'\hypersetup{colorlinks=true,urlcolor=blue,linkcolor=blue}',
            r'\setlength{\parskip}{5pt}\setlength{\parindent}{0pt}',
            r'\begin{document}', r'\title{' + title + r'}\author{}\date{11 September 2026}\maketitle']


def compile_pdf(output, name, tex):
    (output / f'{name}.tex').write_text('\n\n'.join(tex + [r'\end{document}']) + '\n')
    for _ in range(2):
        p = subprocess.run(['pdflatex', '-interaction=nonstopmode', '-halt-on-error', name + '.tex'],
                           cwd=output, capture_output=True, text=True)
        if p.returncode:
            raise RuntimeError(p.stdout[-5000:])


def analyze(root, output, pilot_root):
    output.mkdir(parents=True, exist_ok=True)
    (output / 'panels').mkdir(exist_ok=True)
    manifest = read_json(HERE / 'manifest.json')
    rows, ratios = collect(root, manifest)
    completeness = audit(root, manifest)
    csv_file(output / 'runs.csv', rows)
    csv_file(output / 'paired-ratios.csv', ratios)
    write_json(output / 'coverage-audit.json', completeness)
    grouped = defaultdict(list)
    for r in rows:
        if r['phase'] == 'warm':
            grouped[tuple(r[k] for k in KEYS)].append(r)
    summary = []
    for key, records in sorted(grouped.items()):
        row = dict(zip(KEYS, key))
        row['available_repetitions'] = len(records)
        row['valid_repetitions'] = sum(r['eligible'] for r in records)
        for metric in ('seconds', 'peak_rss_gib'):
            q = quantiles(records, metric)
            for i, label in enumerate(('q1', 'median', 'q3')):
                row[metric + '_' + label] = None if q is None else float(q[i])
        summary.append(row)
    csv_file(output / 'six-repetition-summary.csv', summary)
    lookup = {tuple(r[k] for k in KEYS): r for r in summary}

    def val(design, n, t, arm, metric='seconds', k=10, section='main'):
        row = lookup.get(('ols', design, n, k, t, section, arm), {})
        return row.get(metric + '_median')

    def fmt(x, digits=2):
        return '--' if x is None else f'{x:.{digits}f}'

    paired = defaultdict(list)
    for r in ratios:
        if r['phase'] == 'warm':
            paired[tuple(r[k] for k in ('model', 'design', 'n', 'k', 'threads', 'section', 'comparator'))].append(r)
    pair_summary = []
    for key, records in sorted(paired.items()):
        row = dict(zip(('model', 'design', 'n', 'k', 'threads', 'section', 'comparator'), key))
        for metric in ('seconds', 'peak_rss_gib'):
            q = quantiles(records, metric)
            row[metric + '_candidate_ratio'] = None if q is None else float(q[1])
        q = quantiles(records, 'seconds')
        row['speedup_median'] = None if q is None else float(np.median([1/r['seconds'] for r in records]))
        pair_summary.append(row)
    csv_file(output / 'paired-summary.csv', pair_summary)

    # Read the versioned public solver matrix using the tested d44e36b schema.
    diagnostics = []
    for row in rows:
        if row['arm'] != 'fereg' or row['phase'] != 'warm':
            continue
        p = root / 'results' / cell_id(row) / f"warm-{row['repetition']}" / 'fereg'
        matrix = {(int(r['row']), int(r['column'])): float(r['value'])
                  for r in csv.DictReader((p / 'fereg_solver.csv').open())}
        result = read_json(p / 'result.json')
        diagnostics.append({k: row[k] for k in ('design', 'n', 'k', 'threads', 'section', 'repetition', 'eligible')} |
                           {'seconds': row['seconds'], 'peak_rss_gib': row['peak_rss_gib'],
                            'solver_code': int(matrix[1, 2]), 'preconditioner_code': int(matrix[1, 14]),
                            'setup_seconds': matrix[1, 9], 'solve_seconds': matrix[1, 10],
                            'native_peak_allocation_gib': result['metadata']['fereg_native_allocation_bytes'] / 2**30})
    csv_file(output / 'fereg-solver-diagnostics.csv', diagnostics)

    pilot_rows = []
    for p in sorted((pilot_root / 'results').glob('**/GROUP.COMPLETE.json')):
        g = read_json(p)
        statuses = {c['right']: c['status'] for c in g['comparisons']}
        for arm, status in g['arm_status'].items():
            r = read_json(p.parent / arm / 'result.json')
            pilot_rows.append({k: g['cell'][k] for k in ('model', 'design', 'n', 'k', 'threads')} |
                              {'arm': arm, 'status': status, 'comparison_status': statuses.get(arm, 'candidate'),
                               'seconds': r['seconds'], 'peak_rss_gib': r['peak_rss_gib'],
                               'error': r['metadata'].get('error', '')})
    csv_file(output / 'pilot-all-results.csv', pilot_rows)

    # Capture even interrupted arms and off-grid development evidence separately.
    inventory = []
    for p in sorted((root / 'results').glob('**/result.json')):
        r = read_json(p)
        inventory.append({'path': str(p.relative_to(root)), 'job_id': r.get('job_id'), 'task_id': r.get('task_id'),
                          'model': r['cell']['model'], 'design': r['cell']['design'], 'arm': r['arm'],
                          'status': r['status'], 'seconds': r.get('seconds'), 'peak_rss_gib': r.get('peak_rss_gib'),
                          'group_complete': (p.parent.parent / 'GROUP.COMPLETE.json').exists(),
                          'measurement_fingerprint': r.get('measurement_fingerprint'),
                          'error': r.get('metadata', {}).get('error', '')})
    csv_file(output / 'all-arm-inventory.csv', inventory)
    counts = Counter((r['model'], r['design']) for r in rows if r['arm'] in ('fereg', 'feppml'))
    accepted_job_ids = {('7522274', i) for i in range(1, 6)}
    accounting = read_json(root / 'accounting/7522398.json')
    accepted_job_ids |= {(str(accounting['job_id']), r['task_id']) for r in accounting['records'] if not r['failed'] and not r['exit_status']}
    accepted_markers = []
    for p in (root / 'results').glob('**/GROUP.COMPLETE.json'):
        g = read_json(p)
        if g.get('measurement_fingerprint') != read_json(root / 'env/frozen.json')['fingerprint']:
            continue
        candidate = 'fereg' if g['cell']['model'] == 'ols' else 'feppml'
        r = read_json(p.parent / candidate / 'result.json')
        if (r['job_id'], int(r['task_id'])) in accepted_job_ids:
            accepted_markers.append(g)
    coverage = {'status': 'stopped by user; partial evidence', 'registered_complete_markers': completeness['complete_groups'],
                'planned_registered_groups': completeness['expected_groups'], 'accepted_production_tasks': 78,
                'production_node_hours': sum(r['wall_seconds'] for r in accounting['records']) / 3600,
                'production_reserved_core_hours': sum(r['wall_seconds'] * r['slots'] for r in accounting['records']) / 3600,
                'cancelled_started_task': 79, 'cancelled_never_started_tasks': [80, 237],
                'accepted_frozen_groups_including_offgrid_pilot': len(accepted_markers),
                'accepted_registered_groups': sum(g['cell'] in cells(manifest) for g in accepted_markers),
                'accepted_ols_groups': sum(g['cell']['model'] == 'ols' for g in accepted_markers),
                'accepted_ppml_groups': sum(g['cell']['model'] == 'ppml' for g in accepted_markers),
                'six_valid_ols_candidate_cells': sum(s['arm'] == 'fereg' and s['seconds_median'] is not None for s in summary),
                'six_valid_ppml_candidate_cells': sum(s['arm'] == 'feppml' and s['seconds_median'] is not None for s in summary),
                'registered_group_counts': [{'model': k[0], 'design': k[1], 'count': v} for k, v in sorted(counts.items())]}
    write_json(output / 'stopped-coverage.json', coverage)

    # The figure atlas keeps the paper's individual panels and subcaption assembly.
    note = ('Partial campaign. Points require all six valid warm repetitions; bars show IQR, not confidence intervals. '
            'Crosses mark missing or unvalidated points. Both axes are logarithmic. Every task reserves 28 cores; '
            'Stata-controlled computation is capped at four. OLS IID inference, ten controls. See runs.csv for exclusions.')
    atlas = document('Stopped systematic campaign: figure atlas')
    atlas += [r'\textbf{Partial results; campaign cancelled at the requested stopping point.}',
              'No unmeasured results are imputed. PPML has pilot evidence only and is tabulated in the discussion report. '
              'Each panel is a separate vector PDF combined through LaTeX subcaption. Observation panels use either four cores or maximum program capacity. Core panels hold N at 983,040.', r'\clearpage']
    selected_composites = {}
    available_designs = [d for d in manifest['designs'] if any(s['model'] == 'ols' and s['design'] == d and s['seconds_median'] is not None for s in summary)]
    for version in ('stata', 'expanded'):
        arms = manifest['arms']['ols'] if version == 'expanded' else manifest['arms']['ols'][:4]
        for d in available_designs:
            selected = [r for r in rows if r['model'] == 'ols' and r['design'] == d and r['section'] == 'main' and r['arm'] in arms]
            names = {}
            for axis in ('maximum', 'fixed', 'cores'):
                subset = [r for r in selected if (r['n'] == manifest['scaling_n'] if axis == 'cores' else r['threads'] == (28 if axis == 'maximum' else 4))]
                for metric in ('seconds', 'peak_rss_gib'):
                    name = f'absolute-ols-{version}-{d}-{axis}-{metric}.pdf'
                    panel(output/'panels'/name, subset, arms, 'arm', axis, metric, d, manifest)
                    names[axis, metric] = name
            for axis in ('maximum', 'fixed'):
                text = composite(f'OLS {DESIGN_LABELS[d]}: {version}, {axis} capacity',
                                 [names[axis, 'seconds'], names['cores', 'seconds'], names[axis, 'peak_rss_gib'], names['cores', 'peak_rss_gib']],
                                 ['Time by observations', 'Time by requested cores', 'RSS by observations', 'RSS by requested cores'], note)
                atlas.append(text)
                selected_composites[d, version, axis] = text
    for suite in ('panels', 'paper'):
        designs = [d for d, s in manifest['designs'].items() if s['suite'] == suite]
        for arm in manifest['arms']['ols'][1:]:
            selected = [r for r in ratios if r['model'] == 'ols' and r['suite'] == suite and r['section'] == 'main' and r['comparator'] == arm]
            if not any(r['eligible'] for r in selected):
                continue
            names = {}
            for axis in ('maximum', 'fixed', 'cores'):
                subset = [r for r in selected if (r['n'] == manifest['scaling_n'] if axis == 'cores' else r['threads'] == (28 if axis == 'maximum' else 4))]
                for metric in ('seconds', 'peak_rss_gib'):
                    name = f'ratio-ols-{suite}-{arm}-{axis}-{metric}.pdf'
                    panel(output/'panels'/name, subset, designs, 'design', axis, metric, suite, manifest, ratio=True)
                    names[axis, metric] = name
            for axis in ('maximum', 'fixed'):
                atlas.append(composite(f'fereg/{arm}: {suite}, {axis} capacity',
                                       [names[axis,'seconds'], names['cores','seconds'], names[axis,'peak_rss_gib'], names['cores','peak_rss_gib']],
                                       ['Time ratios by observations', 'Time ratios by requested cores', 'RSS ratios by observations', 'RSS ratios by requested cores'],
                                       note + ' Ratios are computed within seed/node pairs. Values below one favor fereg.'))
    compile_pdf(output, 'figure-atlas', atlas)

    tex = document('Fereg benchmark findings after stopping the campaign')
    tex += [r'\textbf{Decision: stop the campaign, preserve the evidence, and use the completed comparisons to prioritize improvements.}',
            r'\section{What the evidence says}',
            'Fereg is particularly strong against reghdfe on difficult sparse fixed effects. It is not generally the fastest command: fixest leads on several well-connected designs, and greg is competitive on simple models. The most actionable weakness is a large setup-cost jump associated with an automatic solver change on the largest degree-three graph. Parallel scaling and peak memory are additional opportunities.',
            tex_escape(f"The stopped run contains {coverage['registered_complete_markers']} completed markers out of 2,128 planned registered groups. Strict scheduler acceptance retains {coverage['accepted_registered_groups']} registered groups. Including the two off-grid pilot groups, {coverage['accepted_frozen_groups_including_offgrid_pilot']} frozen comparison groups have accepted accounting: {coverage['accepted_ols_groups']} OLS and {coverage['accepted_ppml_groups']} PPML. There are {coverage['six_valid_ols_candidate_cells']} OLS configurations with all six valid warm repetitions, and no such PPML configuration."),
            'Array 7522398 is cancelled and its monitor deleted. Tasks 1--78 finished successfully; task 79 was interrupted by cancellation. Tasks 80--237 never started. Six groups checkpointed by task 51 will not be rerun. Finished calls from interrupted task 79 are retained as observations but excluded from the quantitative comparisons below because its task accounting failed.',
            r'\textbf{Scope matters.} The array ran OLS first. Most evidence comes from six OLS designs: one FE, dense two FE, sparse three FE, and the paper degree-two/three/six graphs. Sparse two-FE production is incomplete; the bottleneck design and PPML have only pilot evidence. This is a selected, unfinished grid, not a balanced overall ranking.',
            r'\textbf{How to read the numbers.} Unless labelled pilot, a table entry is a median across six successful warm fits passing the frozen scientific checks, on identical seeded data. A dash means the six-valid-run requirement was not met. Four-core tables give a common CPU budget. Maximum-capacity tables permit fereg, fixest and pyfixest 28 threads; Stata-controlled work remains license-capped at four. Every task reserved an isolated 28-core node.',
            r'\clearpage\section{Strengths and weaknesses at 3.93 million observations}',
            'The following are command times in seconds, with ten controls. All finite entries satisfy the six-repetition scientific rule. They are medians of times, so ratios of the displayed rounded entries need not exactly equal the median paired speedups in paired-summary.csv.']
    designs = ['one_fe','two_fe_dense','three_fe_sparse','strong_d2','strong_d3','strong_d6']
    for t in (4,28):
        tex += [r'\subsection*{' + ('Common four-core budget' if t==4 else 'Maximum available capacity') + '}',
                table(['Design','fereg','reghdfe','areg','greg','fixest','pyfixest'],
                      [[DESIGN_LABELS[d]] + [fmt(val(d,3932160,t,a)) for a in manifest['arms']['ols']] for d in designs], 'lrrrrrr')]
    tex += ['The sparse three-FE result is the clearest strength: at the common four-core budget fereg takes 27.54 seconds against 1,343.15 for reghdfe, roughly 49 times faster. At maximum capacity the corresponding times are 19.54 and 1,373.89 seconds, roughly 70 times. Other commands do not provide six validated repetitions there, so their raw execution times cannot support equivalent speed claims.',
            'Dense two FE is a broader success: fereg is roughly eight times faster than reghdfe at four cores, and also beats areg, greg and pyfixest. However, fixest is faster: 8.25 versus 11.15 seconds at four cores, and 3.99 versus 7.65 at maximum capacity.',
            'On the degree-three graph fereg is only comparable to areg at maximum capacity, loses to greg, and takes about 8.4 times as long as fixest. On the degree-six graph it beats the Stata alternatives but takes about 3.8 times as long as fixest. Thus beating reghdfe does not establish an efficient frontier.',
            r'\clearpage\section{A concrete improvement target: solver setup}',
            'On the degree-three graph, all six warm maximum-capacity runs report CMG through N=1,966,080, then LSMR with Schwarz preconditioning at N=3,932,160. The recorded public solver matrix identifies both the route and its setup/solve times. The schema was checked against candidate source d44e36b.']
    diag_table=[]
    for n in manifest['sizes']:
        ds=[d for d in diagnostics if d['design']=='strong_d3' and d['n']==n and d['k']==10 and d['threads']==28 and d['section']=='main']
        assert len(ds)==6 and all(d['eligible'] for d in ds)
        routes=set(d['solver_code'] for d in ds); assert len(routes)==1
        diag_table.append([f'{n:,}', 'CMG' if routes=={7} else 'LSMR/Schwarz'] + [fmt(float(np.median([d[m] for d in ds]))) for m in ('seconds','setup_seconds','solve_seconds','peak_rss_gib')])
    tex += [table(['N','Route','Call (s)','Setup (s)','Solve (s)','RSS (GiB)'],diag_table,'rlrrrr'),
            'Doubling N at the last step multiplies time by 4.35 and RSS by 4.22. Main-absorption setup rises from 0.21 to 12.01 seconds and is about 63% of the larger call. This localizes a substantial cost to native solver setup; it is not explained by Stata launch or data import, which are excluded from the timer.',
            r'\textbf{Interpretation, not a causal solver experiment.} The route switch and setup jump are observed together. The campaign did not compare alternative forced solvers on that cell. The priority is to inspect why auto changes route, profile Schwarz construction and allocations, and test whether a cheaper certified setup or a different admissible route improves this case. No estimator settings were changed for this report.',
            r'\clearpage']
    tex.append(selected_composites['strong_d3','expanded','maximum'])
    tex += [r'\section{Parallel scaling and the memory tradeoff}',
            'At N=983,040 and ten controls, increasing fereg from one to 28 threads yields only about two- to three-and-a-half-fold speedups across these designs. The tables report time across thread counts, followed by largest-sample RSS.']
    tex += [table(['Design','1 core (s)','4 cores (s)','8 cores (s)','14 cores (s)','28 cores (s)'],
                  [[DESIGN_LABELS[d]]+[fmt(val(d,983040,t,'fereg')) for t in (1,4,8,14,28)] for d in designs], 'lrrrrr')]
    tex += ['Sparse three FE is fastest at eight threads in these measurements: 4.13 seconds, versus 4.25 at 28. Dense two FE and the degree-two/three/six graphs show little improvement past 14. These data support investigating thread selection by workload size and graph structure; they do not establish a universal optimal thread count.',
            table(['Largest-N design','fereg 4 cores','fereg 28 cores','reghdfe','fixest (28)'],
                  [[DESIGN_LABELS[d]]+[fmt(v) for v in [val(d,3932160,4,'fereg','peak_rss_gib'),val(d,3932160,28,'fereg','peak_rss_gib'),val(d,3932160,28,'reghdfe','peak_rss_gib'),val(d,3932160,28,'fixest','peak_rss_gib')]] for d in designs], 'lrrrr'),
            'The RSS table is in GiB. Memory is a strength on one FE and broadly competitive on dense two FE and degree six. It is a weakness on sparse three FE and the largest degree-three graph: at 28 threads fereg uses about three times reghdfe RSS in each case. For sparse three FE, moving from four to 28 threads increases RSS from 4.03 to 7.16 GiB for a time reduction from 27.54 to 19.54 seconds.',
            r'\clearpage\section{PPML: useful pilot evidence, no completed repetition grid}',
            'Only the five accepted pilot groups support substantive PPML timing discussion. Each is one warm invocation, not a six-run result. Raw times below are seconds. D denotes a scientific discrepancy, F a native command failure, I an initialization failure/timeout, and T the 2,400-second command cap. D entries are not validated speed comparisons.']
    prows=[]
    for d,n,k,t in [('one_fe',61440,10,4),('two_fe_sparse',983040,50,28),('three_fe_sparse',983040,10,4),('strong_d6',3932160,10,28),('weak_d3',61440,10,1)]:
        line=[DESIGN_LABELS[d],f'{n/1e6:.3f}',str(k),str(t)]
        for a in manifest['arms']['ppml']:
            x=next(r for r in pilot_rows if r['model']=='ppml' and r['design']==d and r['arm']==a)
            if x['status']=='command_timeout': value='T'
            elif x['status']!='pass':value='I' if a=='gglm' else 'F'
            else:value=fmt(x['seconds'])+(' D' if x['comparison_status']=='accuracy_failure' else '')
            line.append(value)
        prows.append(line)
    tex += [table(['Design','N (M)','K','Cores','feppml','ppmlhdfe','gglm','fixest','pyfixest'],prows,'lrrrrrrrr'),
            'Feppml is promising on sparse three FE: 40.71 seconds against 544.61 for ppmlhdfe and 81.56 for fixest, with both comparisons passing. On the large degree-six graph the direction reverses against the other languages: feppml takes 221.54 seconds, fixest 15.94 and pyfixest 23.51. It is roughly tied with gglm there. This makes well-connected PPML a high-priority follow-up, but the single pilot run cannot establish its dispersion or generality.',
            'On one FE, feppml takes 0.79 seconds versus 0.08 for gglm and 0.27 for fixest. On the largest PPML pilot, feppml RSS is 6.87 GiB versus 3.04 for ppmlhdfe and 3.67 for fixest. The same themes recur: simple-case cost and native memory deserve attention. A timeout of the wide ppmlhdfe fit does not supply scientific parity for that cell, so it is not presented as a validated lower-bound speedup.',
            r'\clearpage\section{Accuracy, failures and limits to interpretation}',
            'All candidate invocations in accepted completed pilot/production groups returned successfully. That is an application result, not a blanket claim of exact agreement. Four warm comparisons at the largest degree-two graph fail the coefficient tolerance against reghdfe: seeds in repetitions 3 and 6, at both four and 28 threads. Maximum absolute differences are about 3.4--4.2e-9. Sample identity, degrees of freedom, RSS, covariance and standard errors pass. Their largest-N figure points remain excluded. The evidence does not identify which command is closer to the exact solution.',
            'Several other commands fail the frozen coefficient, covariance or objective checks on difficult designs. These labels mean failure of this declared comparison rule, not proof of a broadly incorrect estimator. Differences in native stopping rules and degrees-of-freedom conventions may matter. In particular, a fast return that misses the comparison criteria is not treated as a performance win.',
            'Pyfixest has native demeaning iteration-limit failures in sparse production designs and four pilot fits. Gglm has pilot initialization failures and a startup timeout. An initialization error establishes no target-fit runtime or failure; the cold invocation is preserved independently. Failures, tiny disagreements and unavailable points remain in the CSV data.']
    status_counts=Counter((r['arm'], r['status'] if r['status']!='pass' else (r['science'] or 'unavailable')) for r in rows)
    tex += [table(['Arm','Recorded outcome','Registered invocations'],[[a,s,n] for (a,s),n in sorted(status_counts.items()) if s!='pass'],'llr'),
            r'\clearpage\section{Priorities suggested by the completed evidence}',
            r'\begin{enumerate}\item \textbf{Audit the large degree-three auto-route transition.} Explain the switch to LSMR/Schwarz, then profile its 12-second setup and allocation peak. Any optimization must retain the same data, inference, tolerances and certificates.',
            r'\item \textbf{Make thread use more selective.} Compare eight/14-thread execution with 28-thread execution on representative sparse and well-connected graphs. Investigate extra allocations and coordination costs; the current data do not show that all 28 cores earn their memory cost.',
            r'\item \textbf{Improve well-connected and simple-model paths.} Fixest is faster even under a common four-core budget. One-FE and degree-six cases can reveal avoidable native setup, data passes or inference overhead. Treat these as profiling hypotheses, not established explanations.',
            r'\item \textbf{Separate PPML workload classes.} Preserve the sparse-design advantages while investigating the large degree-six gap and memory use. A small repeated follow-up would be more informative than restarting the entire grid.',
            r'\item \textbf{Adjudicate the tiny degree-two discrepancies independently.} Use a targeted high-accuracy oracle or certified residual/objective check before deciding whether candidate or comparator stopping needs attention. Do not relax the frozen acceptance threshold.\end{enumerate}',
            r'\section{What made this campaign costly}',
            f"Production consumed {coverage['production_node_hours']:.1f} node-hours, or {coverage['production_reserved_core_hours']:,.0f} reserved core-hours, excluding setup, smoke and pilot work. These are allocations, not measured useful CPU time. The design had 2,128 groups with five or six estimators each. Fresh-process initialization, slow comparators, 40-minute command caps and queue delays increased elapsed time. Ordering by model/design left production PPML unstarted. A future bounded campaign should interleave blocks for broader early coverage. No further jobs are scheduled.",
            r'\section{Measurement and reproducibility}',
            'Candidate source is d44e36bfa5ad7e41a06752d1008f0b9683f8c8dd. The frozen runtime inventory, manifest, dependency pins and accepted scheduler records are included in evidence.zip. The pinned software is Stata/MP 19, R 4.5.2, Python 3.12.4, fixest 0.14.2 and pyfixest 0.60.0. Both models use public commands and native default algorithms; OLS uses IID inference, PPML robust covariance with the documented N/(N-1) normalization for R/Python output.',
            'Time excludes process startup, import, validation and output serialization. RSS is an absolute process-tree maximum sampled every 10 ms during invocation; sub-interval peaks may be missed. Six valid warm repetitions are required for finite aggregate points. Pairwise ratios are formed on the same input, seed, node and task before summarization. The independent small-sample oracle smoke passed for both models and all eight designs, but does not substitute for the missing large-sample grid.',
            'Atlas panels are separate vector PDFs assembled with subcaption. The CSVs retain incomplete and unsuccessful observations; strict summaries exclude the cancelled task. Raw logs are collected separately; evidence.zip contains numerical receipts and log hashes. This is not release qualification.']
    compile_pdf(output,'discussion',tex)
    (output/'README.md').write_text('# Stopped systematic benchmark\n\nCampaign cancelled at the user\'s request. No queued or checkpointed comparisons will resume.\n\n- discussion.pdf: analysis, tables, solver diagnostics and priorities.\n- figure-atlas.pdf: individual vector panels combined through LaTeX subcaption.\n- six-repetition-summary.csv and paired-summary.csv: strict valid aggregates only.\n- runs.csv: registered completed-group observations, including excluded records.\n- pilot-all-results.csv: all ten pilot groups, including off-grid observations.\n- all-arm-inventory.csv: every collected arm receipt, including incomplete groups.\n- evidence.zip: compact numerical evidence and frozen provenance.\n\nA dash or missing figure point is not an estimator speed or zero. PPML is pilot-only. The cancelled task is excluded from strict summaries.\n')
    return coverage


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('root',type=Path);p.add_argument('output',type=Path);p.add_argument('pilot_root',type=Path)
    a=p.parse_args();print(json.dumps(analyze(a.root,a.output,a.pilot_root),indent=2))
