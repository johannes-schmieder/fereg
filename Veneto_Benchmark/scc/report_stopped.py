"""Single PDF from the stopped SCC campaign, with failures and partial coverage."""
import argparse
from pathlib import Path

import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.lines import Line2D
from matplotlib.ticker import ScalarFormatter
import numpy as np
from pypdf import PdfReader, PdfWriter
from reportlab.pdfgen import canvas
from reportlab.lib.colors import HexColor
from reportlab.lib.styles import ParagraphStyle
from reportlab.platypus import Paragraph, Table, TableStyle
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont

from common import read_json, write_json, sha256

BASE = Path(__file__).resolve().parents[1]
DATA = BASE / 'results/scc/stopped-analysis'
TEMP = BASE / 'work/stopped-report'
OUT = BASE / 'output/pdf/Veneto_Benchmark.pdf'
W, H = 842, 595
CORES = [1, 2, 4, 8, 14, 28]
ARMS = {'ols': ['fereg', 'reghdfe', 'greg', 'areg', 'fixest', 'pyfixest'],
        'ppml': ['feppml', 'ppmlhdfe', 'gglm', 'fixest', 'pyfixest']}
LABEL = {'fereg': 'fereg', 'reghdfe': 'reghdfe', 'greg': 'greg', 'areg': 'areg (NC)',
         'fixest': 'fixest (R)', 'pyfixest': 'pyfixest', 'feppml': 'feppml', 'ppmlhdfe': 'ppmlhdfe', 'gglm': 'gglm (Poisson)'}
COLOR = dict(zip(['fereg', 'reghdfe', 'greg', 'areg', 'fixest', 'pyfixest'],
                 ['#1768AC', '#D97720', '#16877A', '#8364B0', '#CB4B79', '#71604E']))
COLOR.update(feppml=COLOR['fereg'], ppmlhdfe=COLOR['reghdfe'], gglm=COLOR['greg'])
INK, MUTED = '#15334D', '#526575'
ROWS = read_json(DATA / 'all-finished-calls.json')
MAIN = [r for r in ROWS if r['accepted_complete_task']]
AUDIT = read_json(DATA / 'audit.json')
FAIL = read_json(DATA / 'failure-audit.json')
PAGES = []


def para(c, text, x, y, width, size=10.5, leading=None, color=MUTED):
    p = Paragraph(text, ParagraphStyle('body', fontName='Helvetica', fontSize=size,
                                      leading=leading or size * 1.38, textColor=HexColor(color)))
    _, height = p.wrap(width, H)
    assert y - height > 55, (text[:100], y, height)
    p.drawOn(c, x, y - height)
    return y - height


def new_page(number, title, subtitle):
    path = TEMP / f'page-{number:02d}.pdf'
    PAGES.append(path)
    c = canvas.Canvas(str(path), pagesize=(W, H))
    title_size = min(24, 732 * 24 / pdfmetrics.stringWidth(title, 'Helvetica-Bold', 24))
    c.setFillColor(HexColor(INK)); c.setFont('Helvetica-Bold', title_size)
    c.drawString(55, 548, title)
    para(c, subtitle, 55, 531, 730, 10)
    c.setStrokeColor(HexColor('#DCE5EC')); c.line(55, 48, 787, 48)
    c.setFillColor(HexColor(MUTED)); c.setFont('Helvetica', 8)
    c.drawString(55, 28, 'VENETO  /  SCC BENCHMARK  /  STOPPED 12 SEPTEMBER 2026')
    c.drawRightString(787, 28, str(number))
    return c


def table(c, data, widths, y, size=9.5, padding=7):
    t = Table(data, colWidths=widths)
    t.setStyle(TableStyle([
        ('BACKGROUND', (0, 0), (-1, 0), HexColor('#EAF0F5')),
        ('TEXTCOLOR', (0, 0), (-1, 0), HexColor(INK)),
        ('FONTNAME', (0, 0), (-1, 0), 'Helvetica-Bold'),
        ('FONTNAME', (0, 1), (-1, -1), 'Helvetica'),
        ('FONTSIZE', (0, 0), (-1, -1), size),
        ('LEADING', (0, 0), (-1, -1), size * 1.32),
        ('TOPPADDING', (0, 0), (-1, -1), padding),
        ('BOTTOMPADDING', (0, 0), (-1, -1), padding),
        ('TEXTCOLOR', (0, 1), (-1, -1), HexColor(INK)),
        ('ROWBACKGROUNDS', (0, 1), (-1, -1), [HexColor('#FFFFFF'), HexColor('#F6F8FA')]),
        ('LINEBELOW', (0, 0), (-1, 0), .7, HexColor('#9AACB9')),
        ('VALIGN', (0, 0), (-1, -1), 'MIDDLE')]))
    _, height = t.wrap(W, H)
    assert y - height > 45
    t.drawOn(c, 55, y - height)
    return y - height


def block(c, label, text, x, y, width, size=10.5):
    y = para(c, '<b>' + label + '</b>', x, y, width, size + .5, color=INK)
    return para(c, text, x, y - 7, width, size) - 20


def overview():
    c = new_page(1, 'Veneto: performance and numerical reliability',
                 'A real worker-employer benchmark. Final report from the campaign stopped at the user\'s request.')
    for x, value, label in [(55, '3.16 million', 'observations'), (305, '110 + 14', 'complete-task + supplementary calls'),
                             (555, '18.98', 'production node-hours, including cancellation')]:
        c.setFillColor(HexColor('#EAF0F5')); c.roundRect(x, 421, 232, 75, 7, fill=1, stroke=0)
        c.setFillColor(HexColor(INK)); c.setFont('Helvetica-Bold', 24); c.drawString(x + 15, 464, value)
        para(c, label, x + 15, 450, 205, 9)
    y = 393
    y = block(c, 'OLS: fereg is fast and agrees closely with reghdfe',
              'All ten complete-task fereg fits match the reference coefficients, full slope covariance and observation sample. '
              'At one to four cores, cold command times are 10.6-12.5 times shorter than reghdfe. '
              'The tradeoff is memory: fereg uses 1.66-3.99 GiB across the core ladder, versus about 1.02 GiB for reghdfe.', 55, y, 730)
    y = block(c, 'PPML: the frozen feppml build is unreliable on this panel',
              'Nine of ten complete-task calls return numerical failure r(430); only the cold 14-core call succeeds. '
              'That fit agrees closely with ppmlhdfe, but a single success cannot support a reliable performance claim. '
              'Pyfixest has eight inner demeaning failures and two timeouts; gglm times out in all ten calls.', 55, y, 730)
    y = block(c, 'Comparisons require three qualifications',
              'SCC Stata/MP is licensed for four processors, even when affinity allows more. The full six-core ladder has '
              'one cold observation per cell; complete warm evidence reaches eight cores, with only one repetition. '
              'These results test frozen source d44e36b, preceding the newer local OLS changes.', 55, y, 730)
    para(c, 'Guide: performance figures pp. 2-4; measured values pp. 5-6; numerical audit p. 7; '
         'PPML failure analysis pp. 8-9; design and evidence pp. 10-11.', 55, 85, 730, 9)
    c.save()


def chart_page(number, model, failures=False):
    fig, axes = plt.subplots(1, 2, figsize=(W / 72, H / 72))
    fig.subplots_adjust(left=.10, right=.95, bottom=.265, top=.77, wspace=.30)
    title = 'PPML: time and memory before failure' if failures else ('OLS: command time and memory' if model == 'ols' else 'PPML: successfully returned estimates')
    fig.text(.066, .922, title, fontsize=23, weight='bold', color=INK)
    fig.text(.066, .881, 'One cold call at each core setting; hollow diamonds show the single warm call in complete tasks.', fontsize=10, color=MUTED)
    arms = ['feppml', 'pyfixest', 'gglm'] if failures else (ARMS[model] if model == 'ols' else ['feppml', 'ppmlhdfe', 'fixest'])
    for arm in arms:
        rr = [r for r in MAIN if r['cell']['model'] == model and r['arm'] == arm]
        for ax, metric in zip(axes, ['seconds', 'peak_rss_gib']):
            cold = [r for r in rr if r['phase'] == 'cold']
            valid = [r for r in cold if (r['status'] != 'pass' if failures else r['status'] == 'pass' or arm == 'areg')]
            ys = []
            for t in CORES:
                r = next((r for r in valid if r['cell']['threads'] == t), None)
                ys.append(np.nan if r is None else (r.get('seconds') or r['command_time_lower_bound']) if metric == 'seconds' else r['peak_rss_gib'])
            label = LABEL[arm]
            if failures:
                ax.plot(CORES, ys, linestyle=':', linewidth=1.6, color=COLOR[arm], label=label)
                for r in valid:
                    value = (r.get('seconds') or r['command_time_lower_bound']) if metric == 'seconds' else r[metric]
                    timeout = r['status'] == 'command_timeout'
                    marker_style = {'facecolors': 'none', 'edgecolors': COLOR[arm]} if timeout else {'color': COLOR[arm]}
                    ax.scatter([r['cell']['threads']], [value], marker='^' if timeout else 'x',
                               s=60, linewidths=1.7, zorder=4, **marker_style)
            else:
                ax.plot(CORES, ys, color=COLOR[arm], linewidth=2.4 if arm == 'fereg' else 1.8,
                        marker='x' if arm == 'areg' else 'o', linestyle=':' if arm == 'areg' else '-', markersize=5.5, label=label)
            warm = [r for r in rr if r['phase'] == 'warm' and (r['status'] != 'pass' if failures else r['status'] == 'pass' or arm == 'areg')]
            for r in warm:
                value = (r.get('seconds') or r['command_time_lower_bound']) if metric == 'seconds' else r[metric]
                ax.scatter([r['cell']['threads']], [value], marker='D', s=53, facecolors='none', edgecolors=COLOR[arm], linewidths=1.1, zorder=5)
    for ax, metric in zip(axes, ['seconds', 'peak_rss_gib']):
        ax.set_xscale('log', base=2); ax.set_xticks(CORES, [str(t) for t in CORES])
        ax.set_xlim(.88, 32); ax.set_xlabel('CPU affinity cores', labelpad=10)
        ax.grid(axis='y', alpha=.20); ax.set_axisbelow(True)
        ax.set_title('Elapsed time' if metric == 'seconds' else 'Memory', loc='left', weight='bold', fontsize=14, pad=14)
        if metric == 'seconds':
            ax.set_yscale('log'); ax.yaxis.set_major_formatter(ScalarFormatter())
            ax.set_ylabel('Seconds (log scale)')
            if failures:
                ax.set_ylim(6, 3600); ax.set_yticks([10, 30, 100, 300, 1000, 2400])
            elif model == 'ols':
                ax.set_ylim(4.5, 2000); ax.set_yticks([10, 30, 100, 300, 1000])
            else:
                ax.set_ylim(100, 2000); ax.set_yticks([100, 200, 500, 1000, 2000])
        else:
            ax.set_ylim(0, 6.6 if failures else 5.6 if model == 'ppml' else 4.5)
            ax.set_ylabel('Absolute peak process RSS (GiB)')
    handles, labels = axes[0].get_legend_handles_labels()
    fig.legend(handles, labels, loc='lower center', bbox_to_anchor=(.52, .145), ncol=3, frameon=False, columnspacing=2.7, fontsize=10)
    if failures:
        notes = ['Crosses: error; open triangles: 2,400-second timeout. Diamonds: warm outcomes of the same type.',
                 'These are time-to-failure and memory observed before failure, not time or peak memory to finish. No successful gglm/Pyfixest PPML fit.',
                 'Feppml\'s only successful call (14 cores) appears on page 3. Dotted lines only guide the eye between failed calls.']
    elif model == 'ols':
        notes = ['Areg crosses are nonconverged returns (NC); all other markers are native successful returns. Numerical equivalence is audited on page 7.',
                 'No repeated-sample error bars are estimable. Stata/MP is capped at 4 processors; the native Rust and R/Python thread controls are separate.',
                 'Affinities 1/4/14 ran on scc-wo4; 2/8/28 on scc-wj3. The 14-core slowdown is confounded with host.']
    else:
        notes = ['Feppml has one successful cold call at 14 cores (152.0 s, 4.92 GiB); other outcomes appear on page 4. No connecting feppml curve is inferred.',
                 'Fixest is faster than ppmlhdfe throughout the observed ladder, with higher RSS; its small slope differences exceed the strict audit threshold.',
                 'Stata/MP is capped at 4 processors. All plotted points share the same input rows and controls; no complete 3-repetition warm cell exists.']
    for y, text in zip([.116, .091, .066], notes):
        fig.text(.066, y, text, fontsize=8.2, color=MUTED)
    fig.text(.066, .028, 'VENETO  /  SCC BENCHMARK  /  STOPPED 12 SEPTEMBER 2026', fontsize=8, color=MUTED)
    fig.text(.935, .028, str(number), ha='right', fontsize=8, color=MUTED)
    path = TEMP / f'page-{number:02d}.pdf'; PAGES.append(path)
    fig.savefig(path); plt.close(fig)


def cell(r):
    if r is None:
        return '--'
    status = r['status']
    suffix = {'pass': '', 'nonconvergence': ' NC', 'worker_failure': ' E', 'command_timeout': ' T'}[status]
    sec = f"{r['seconds']:.2f}" if r['seconds'] is not None else '>=2400'
    return f"{sec}{suffix}\n{r['peak_rss_gib']:.2f} GiB"


def grid(number, phase):
    warm = phase == 'warm'
    c = new_page(number, 'Measured values: ' + ('warm calls' if warm else 'cold calls'),
                 'Each cell: command seconds above, peak process RSS below. Each displayed measurement is one call.')
    data = [['Model / command'] + [f'{t} cores' + (' *' if warm and t >= 14 else '') for t in CORES]]
    for model, arms in ARMS.items():
        for arm in arms:
            line = [('OLS / ' if model == 'ols' else 'PPML / ') + LABEL[arm].replace(' (NC)', '')]
            for t in CORES:
                rr = [r for r in ROWS if r['cell']['model'] == model and r['arm'] == arm and r['cell']['threads'] == t and r['phase'] == phase]
                assert len(rr) <= 1
                line.append(cell(rr[0]) if rr else '--')
            data.append(line)
    y = table(c, data, [154] + [96] * 6, 494, 8.8, padding=3.5)
    note = 'E: native error. NC: explicitly nonconverged. T: timeout; its time is a lower bound. Failed/unfinished-fit RSS is memory observed before termination.'
    y = para(c, note, 55, y - 16, 732, 9)
    if warm:
        y = para(c, '* The 14/28-core columns are supplementary calls from interrupted tasks 11/12. They are excluded from the main figures and complete-task counts. '
                 'OLS calls finished and were audited individually. PPML at 14 cores: gglm timed out; ppmlhdfe was cancelled. At 28 cores: feppml failed; gglm was cancelled. '
                 'Other blank cells were not reached. Cancellation is not estimator nonconvergence.', 55, y - 12, 732, 9)
    else:
        para(c, 'All six cold tasks completed. Cold means no synthetic initialization call in the fresh estimator process; it does not mean a cold operating-system file cache. '
             'The benchmark kept each command\'s native default solver and stopping policy, so differences in reported convergence and accuracy matter.', 55, y - 12, 732, 9)
    c.save()


def numerical():
    c = new_page(7, 'Numerical agreement: exact samples, different accuracy',
                 'Complete tasks only. Comparisons use the matched reghdfe (OLS) or ppmlhdfe (PPML) call on the same node.')
    data = [['Model / command', 'Native success', 'Slope pass', 'Full V pass', 'Max |slope gap|']]
    for s in AUDIT['summary']:
        arm, model = s['arm'], s['model']
        if arm in ['reghdfe', 'ppmlhdfe']:
            continue
        cc = [q for q in AUDIT['comparisons'] if q['arm'] == arm and q['model'] == model and q['status'] == 'pass']
        n = len(cc); valid = [q for q in cc if 'coefficient_pass' in q]
        bpass = sum(q['coefficient_pass'] for q in valid); vpass = sum(q.get('covariance_pass', False) for q in valid)
        gap = max((q['max_absolute_b_difference'] for q in valid), default=None)
        data.append([model.upper() + ' / ' + LABEL[arm], f'{n}/10', f'{bpass}/{len(valid)}' if valid else '--',
                     f'{vpass}/{len(valid)}' if valid else '--', f'{gap:.2e}' if gap is not None else '--'])
    y = table(c, data, [240, 120, 105, 115, 150], 490, 9, 5)
    y = para(c, 'Acceptance envelopes: slopes rtol 1e-7 / atol 1e-10; SE and full 4-by-4 slope covariance rtol 1e-6 / atol 1e-10. '
             'The two reference commands each return successfully in all ten tasks. Reference self-comparisons are omitted.', 55, y - 13, 732, 9)
    y = block(c, 'A graph rank difference explains most OLS SE gaps',
              'An independent graph calculation finds 41,901 connected components. FE rank is 1,133,933 + 125,707 - 41,901 = 1,217,739; '
              'with four controls the residual DoF is 1,941,977. Treating the graph as connected instead gives 1,900,077. '
              'The implied SE ratio is sqrt(1,941,977 / 1,900,077) = 1.010966. Rescaling DoF explains Pyfixest\'s SE gap to about 6e-11; '
              'fixest retains about 2e-6 relative residual error. Greg\'s nearly uniform gap is consistent with this mechanism, but its DoF is not exported.',
              55, y - 14, 732, 9.5)
    para(c, 'Fixest\'s largest OLS slope gap is 1.87e-6, only 0.000326 reference SE; its PPML gap is 6.76e-8, or 0.000000731 reference SE. '
         'These are small in substantive terms but fail the prespecified strict slope check. Native successful returns do not establish equal numerical accuracy. '
         'Greg only provides a sample count; returned observation hashes match for the other successful comparisons.', 55, y, 732, 9)
    c.save()


def feppml_failure():
    c = new_page(8, 'Feppml: repeated numerical failure, exact cause unresolved',
                 'Nine r(430) returns in ten complete tasks; a supplementary 28-core warm call returns the same error.')
    y = table(c, [['Evidence', 'What the saved results establish'],
                  ['Failed cells', 'Cold 1/2/4/8/28 cores; warm 1/2/4/8 cores. Supplement: warm 28.'],
                  ['Time before failure', '9.74-20.80 s in complete tasks; 9.57 s in the supplementary call.'],
                  ['Returned result', 'Native rc=430; no slope/covariance/sample result. Worker rc=0 is bookkeeping.'],
                  ['Successful cell', 'Cold 14 cores: 152.01 s; 4.92 GiB; full slope b/V/sample agreement.']],
              [145, 585], 492, 9.5, 7)
    y -= 19
    left = block(c, 'Confirmed: numerical rejection, not a resource kill',
                 'The frozen error map assigns r(430) to numerical nonconvergence, r(909) to allocation failure and r(1) to interruption. '
                 'The recorded command returns after its DONE handshake, well before the 2,400-second cap. '
                 'The later scheduler exit 137 belongs to the user-cancelled tasks, not these completed r(430) calls.', 55, y, 349, 10)
    left = block(c, 'The passing run is informative but not a diagnosis',
                 'It uses the LSMR solver family, 15 outer iterations, 594 inner iterations, 10 step halvings and one strict-profile rejection. '
                 'Its recorded FE projection costs 82.65 s and the strict final solve 33.17 s. '
                 'The matching b/V/sample shows that this source can solve this input; it does not validate the failed calls.', 55, left, 349, 10)
    right = block(c, 'Why r(430) does not identify one failing check',
                  'The frozen source can return it from the initial weighted solve, an ordinary LSMR recurrence or normal-equation certificate, '
                  'line search, final rank/Hessian checks, or exhausted outer iterations. The LSMR path already performs a tighter continuation '
                  'when its recurrence or fresh certificate fails. The receipt alone cannot locate which path ran.', 438, y, 349, 10)
    right = block(c, 'Limits of the thread pattern',
                  'Cold and warm failures recur at similar times on the same input. The 14-core success and 1/4-core failures share scc-wo4, '
                  'so node identity alone does not explain the pattern. Changed reduction order or solver decisions are plausible. '
                  'There is no repeated 14-core fit, deterministic-mode comparison or failing-iteration trace to prove that mechanism.', 438, right, 349, 10)
    para(c, '<b>Critical evidence gap:</b> the frozen worker uses capture quietly, suppressing the native error text. '
         'Its generic failure_stage="result_extraction_or_status" is a wrapper label, not proof of an extraction defect. '
         'A logged exact-input reproduction is required to assign a specific root cause; no estimator was rerun for this report.', 55, min(left, right) - 1, 732, 9.5)
    c.save()


def other_ppml():
    c = new_page(9, 'PPML: projection limits and a difficult pay distribution',
                 'Distinct outcomes require distinct explanations. A timeout is censored evidence, not proof of nonconvergence.')
    y = block(c, 'Pyfixest: the failure is inside weighted fixed-effect projection',
              'Eight complete-task calls raise "ValueError: Demeaning failed after 10000 iterations." '
              'The exact pinned source raises this when its demeaner returns success=False. Its default is Rust MAP, '
              'with an initial 1e-6 FE tolerance and a 10,000-iteration inner cap; IRLS may tighten the tolerance. '
              'The separate outer Poisson cap is 25. Raising the outer cap alone would not resolve this recorded inner failure. '
              'The two one-core calls instead hit 2,400 seconds; their eventual outcomes are unknown. No failed outer-iteration number or residual trajectory was saved.',
              55, 489, 732, 10.5)
    y = block(c, 'Gglm: completion takes longer than the observation window',
              'All ten complete-task calls reach the 2,400-second command cap; the supplementary 14-core call does too. '
              'The 28-core warm call is interrupted by cancellation and is not counted as a timeout. '
              'The logs contain no usable iteration history or terminal convergence result. They cannot establish a stall, a particular failing solve, '
              'or that the estimator would never converge. Gglm and Pyfixest PPML together consume 65.0% of measured command time/bounds in complete tasks.',
              55, y, 732, 10.5)
    y = block(c, 'The input rules out zero-outcome separation',
              'All 3,159,720 outcomes are strictly positive and finite. For each observation, y*eta - exp(eta) tends to minus infinity '
              'as eta tends to either infinity when y is positive. Over the closed linear predictor space this yields a finite optimum; '
              'redundant FE normalizations affect parameter identification, not existence of the fitted predictor. '
              'Thus zero-outcome separation is not a valid explanation here. The successful feppml fit drops no observations.',
              55, y, 732, 10.5)
    geometry = FAIL['data_geometry']
    para(c, '<b>Numerical difficulty remains plausible:</b> daily pay ranges from 0.004525 to 255,393 (56.4 million to one); '
         'the median is 112.26 and the 99th percentile 440.03. Only 24.0% of workers are observed at multiple firms. '
         'The largest graph component contains 91.95% of observations. These are plausible sources of difficult weighted projections; '
         'disconnection alone and raw-pay range do not prove the failed solver\'s conditioning. No outliers, sample rules or tolerances were changed.', 55, y, 732, 10.5)
    c.save()


def methods():
    c = new_page(10, 'Design and interpretation', 'A development benchmark on a real panel, not an exact KSS replication or a release qualification.')
    blocks = [
        ('Panel construction', 'The Veneto Workers Histories delivery has 45,985,568 job-spell rows. Keep the dominant-pay job for each worker-year '
         'in 1999-2001, break ties deterministically, select Veneto employers and ages 19-63, and require positive pay and paid days. '
         'Seven rounds of common worker/firm singleton pruning leave 3,159,720 observations, 1,133,933 workers and 125,707 firms.'),
        ('Model and inference', 'Worker and firm effects plus age squared /100, age cubed /1000, and year-2000/year-2001 indicators. '
         'Linear age is omitted because it is collinear with worker and year effects. OLS models log daily pay with IID covariance; '
         'PPML models continuous daily pay in source units with robust covariance. R/Python PPML HC0 is multiplied by N/(N-1) after timing.'),
        ('CPU and timing', 'Whole 28-core Intel E5-2680v4 nodes, 84 GiB reservation, nested affinity subsets 1/2/4/8/14/28. '
         'Stata uses min(requested cores,4); native fereg/feppml and R/Python thread settings are separate. Reghdfe parallel() is not used. '
         'Timers cover the public command including inference; loading, import audits, synthetic warm-up and exports are excluded.'),
        ('Memory and repetition', 'RSS is the absolute process-tree peak during the command, including retained data/runtime memory. '
         'The requested polling sleep is 10 ms; loop overhead makes observed average intervals roughly 15-25 ms, so short peaks may be missed. '
         'Cold and warm calls run in fresh processes; warm means one independent synthetic initialization. No process-baseline subtraction or cold/warm pooling.'),
        ('Accuracy and scope', 'Original column values are checked by IEEE-754 hashes before timing. Small independent explicit-dummy checks pass '
         'for both candidates on 3,732 real observations and on the synthetic SCC smoke. They do not certify the full panel. '
         'The benchmark uses native default solvers/stopping rules and preserves disagreement; no tuning was performed after observing failures.'),
        ('Data acknowledgement', 'VWH was developed at Universita Ca\' Foscari Venezia under Giuseppe Tattara and supplied by Fondazione Rodolfo Debenedetti. '
         'It is the same source family used by Kline, Saggio and Solvsten (2020), <i>Econometrica</i> 88(5), 1859-1898 '
         '(<link href="https://doi.org/10.3982/ECTA16410" color="#1768AC">DOI 10.3982/ECTA16410</link>). '
         'Byte identity with their delivery is unverified. This sample omits their additional wage-change and leave-out connectivity restrictions.')]
    y = 489
    for label, body in blocks:
        para(c, '<b>' + label + '</b>', 55, y, 147, 10, color=INK)
        bottom = para(c, body, 218, y, 568, 9.7)
        y = bottom - 20
    c.save()


def evidence():
    c = new_page(11, 'Coverage, provenance and reproducibility', 'Campaign 7529406 is cancelled; the monitor was deleted. There is no automatic continuation.')
    y = table(c, [['Evidence class', 'Tasks / calls', 'Use in this report'],
                  ['Complete comparison tasks', '1-10 / 110', 'Main figures, numerical comparisons and outcome counts'],
                  ['Finished calls in cancelled tasks', '11-12 / 14', 'Warm table and supplementary failure evidence only'],
                  ['Interrupted commands', '11-12 / 2', 'Cancellation, excluded from estimator failure counts'],
                  ['Not started', 'Tasks 13-24', 'No observations; no imputed timings']], [205, 125, 400], 490, 9.2, 7)
    y = para(c, 'Tasks 1-10 have failed=0, exit_status=0, 28 slots, complete receipts and completion sentinels. '
             'Tasks 11/12 have cancellation accounting failed=100, exit_status=137. Completed tasks use 17.57 node-hours; '
             'all started production tasks use 18.98 node-hours (531.51 reserved core-hours). Smoke/setup and queue waiting are additional.', 55, y - 15, 732, 9.5)
    y = block(c, 'Frozen runtime',
              'Candidate source: d44e36bfa5ad7e41a06752d1008f0b9683f8c8dd. Linux plugin SHA-256 starts 74d3ce3bd0c01332; '
              'campaign runtime fingerprint starts bedd0cbb29c066ea. Versions: Stata/MP 19; reghdfe 6.14.1; gtools 1.11.8; '
              'fixest 0.14.2; Pyfixest 0.60.0 at b6f2d284adf0; ppmlhdfe at b85665d8f674. '
              'Exact hashes and imported column identities are retained with the aggregate receipts.', 55, y - 17, 732, 9.5)
    y = block(c, 'Analysis corrections and reproducibility',
              'Areg returns e(converged)=0 at 50 APM iterations in every observed call. The original collector tested Boolean identity '
              'and missed numeric 0.0. Postprocessing now labels these as nonconvergence; original receipts are preserved. '
              'The <link href="https://www.stata.com/manuals/rareg.pdf" color="#1768AC">official areg manual</link> documents the flag and default iteration cap. '
              'Stopped-analysis receipts include the complete-task audit, full covariance/sample comparisons, graph calculation, '
              'partial-call checks, exact-source failure references and CSVs. Run scc/report_stopped.py to rebuild this PDF from those saved aggregates.',
              55, y, 732, 9.5)
    y = block(c, 'What would settle the remaining PPML questions',
              'A separate diagnostic would need the same frozen binary, input, solver and tolerances, with native errors visible and a saved solver trace. '
              'For feppml, locate the first failing recurrence/certificate and compare a failing core setting with 14 cores; '
              'do not relax its publication checks. For Pyfixest, record the outer iteration and inner weighted residual. '
              'For gglm, record iteration progress before considering a longer cap. These are proposed experiments, not runs performed for this report.',
              55, y, 732, 9.5)
    c.save()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--font-dir', type=Path, required=True, help='Directory containing LiberationSans TTF fonts')
    args = parser.parse_args()
    for font, suffix in [('Helvetica', 'Regular'), ('Helvetica-Bold', 'Bold'),
                         ('Helvetica-Oblique', 'Italic'), ('Helvetica-BoldOblique', 'BoldItalic')]:
        pdfmetrics.registerFont(TTFont(font, str(args.font_dir / f'LiberationSans-{suffix}.ttf')))
    pdfmetrics.registerFontFamily('Helvetica', normal='Helvetica', bold='Helvetica-Bold',
                                  italic='Helvetica-Oblique', boldItalic='Helvetica-BoldOblique')
    assert len(MAIN) == 110 and len(ROWS) == 124
    assert len([r for r in MAIN if r['phase'] == 'cold']) == 66
    assert len([r for r in MAIN if r['phase'] == 'warm']) == 44
    TEMP.mkdir(parents=True, exist_ok=True); OUT.parent.mkdir(parents=True, exist_ok=True)
    plt.rcParams.update({'font.family': 'DejaVu Sans', 'font.size': 10, 'pdf.fonttype': 42,
                         'axes.spines.top': False, 'axes.spines.right': False,
                         'axes.edgecolor': '#9AACB9', 'axes.labelcolor': MUTED,
                         'xtick.color': MUTED, 'ytick.color': MUTED})
    overview(); chart_page(2, 'ols'); chart_page(3, 'ppml'); chart_page(4, 'ppml', failures=True)
    grid(5, 'cold'); grid(6, 'warm'); numerical(); feppml_failure(); other_ppml(); methods(); evidence()
    writer = PdfWriter()
    readers = [PdfReader(path) for path in PAGES]
    for page in readers:
        assert len(page.pages) == 1
        writer.add_page(page.pages[0])
    writer.add_metadata({'/Title': 'Veneto benchmark: SCC performance and PPML failure analysis',
                         '/Subject': 'Stopped campaign; observed runtime, memory, accuracy and numerical failures'})
    with OUT.open('wb') as f:
        writer.write(f)
    write_json(DATA / 'report-build.json', {'pdf_sha256': sha256(OUT), 'pages': len(PAGES),
                                          'builder_sha256': sha256(Path(__file__)),
                                          'calls_sha256': sha256(DATA / 'all-finished-calls.json'),
                                          'main_calls': len(MAIN), 'supplemental_calls': len(ROWS) - len(MAIN),
                                          'visual_QA': 'pending'})
    print(OUT)


if __name__ == '__main__':
    main()
