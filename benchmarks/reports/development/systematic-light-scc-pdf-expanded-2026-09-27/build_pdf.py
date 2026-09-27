"""Build the illustrated SCC report from frozen receipts and derived figures."""
import csv
import json
from pathlib import Path
from statistics import median
from xml.sax.saxutils import escape

from reportlab.lib import colors
from reportlab.lib.enums import TA_LEFT
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle
from reportlab.platypus import BaseDocTemplate, Frame, PageTemplate, Paragraph, Spacer, Table, TableStyle, Image, PageBreak

ROOT=Path(__file__).resolve().parent
REPO=ROOT.parents[3]
SOURCE=ROOT.parent/'systematic-light-scc-final-2026-09-27'
OUTPUT=REPO/'output/pdf/fereg-scc-benchmark-report-2026-09-27-expanded.pdf'
OUTPUT.parent.mkdir(parents=True,exist_ok=True)
W,H=A4; WIDTH=W-96
NAVY=colors.HexColor('#183549');TEAL=colors.HexColor('#008579');CORAL=colors.HexColor('#c95b42');MUTED=colors.HexColor('#586d79');PALE=colors.HexColor('#eef4f5')
styles={
 'body':ParagraphStyle('body',fontName='Helvetica',fontSize=10,leading=14,textColor=NAVY,spaceAfter=9),
 'small':ParagraphStyle('small',fontName='Helvetica',fontSize=8.5,leading=11.7,textColor=MUTED,spaceAfter=7),
 'title':ParagraphStyle('title',fontName='Helvetica-Bold',fontSize=30,leading=34,textColor=NAVY,spaceAfter=16),
 'h1':ParagraphStyle('h1',fontName='Helvetica-Bold',fontSize=23,leading=28,textColor=NAVY,spaceAfter=13),
 'h2':ParagraphStyle('h2',fontName='Helvetica-Bold',fontSize=12,leading=16,textColor=TEAL,spaceBefore=9,spaceAfter=7),
 'lead':ParagraphStyle('lead',fontName='Helvetica',fontSize=12,leading=17,textColor=NAVY,spaceAfter=14),
 'cell':ParagraphStyle('cell',fontName='Helvetica',fontSize=8.7,leading=11.4,textColor=NAVY),
 'th':ParagraphStyle('th',fontName='Helvetica-Bold',fontSize=8.5,leading=11,textColor=colors.white),
 'code':ParagraphStyle('code',fontName='Courier',fontSize=7.4,leading=10,textColor=MUTED,spaceAfter=7),
}
story=[]


def p(text,kind='body'):
    story.append(Paragraph(text,styles[kind]))


def heading(kicker,title):
    p(kicker.upper(),'h2');p(title,'h1')


def figure(name,height,caption):
    story.append(Image(str(ROOT/'figures'/f'{name}.png'),width=WIDTH,height=height))
    p(caption,'small')


def table(rows,widths=None):
    data=[[Paragraph(str(x),styles['th' if i==0 else 'cell']) for x in row] for i,row in enumerate(rows)]
    t=Table(data,colWidths=widths or [WIDTH/len(rows[0])]*len(rows[0]),hAlign='LEFT',repeatRows=1)
    t.setStyle(TableStyle([('BACKGROUND',(0,0),(-1,0),NAVY),('ROWBACKGROUNDS',(0,1),(-1,-1),[colors.white,PALE]),
                          ('VALIGN',(0,0),(-1,-1),'TOP'),('LEFTPADDING',(0,0),(-1,-1),8),('RIGHTPADDING',(0,0),(-1,-1),8),
                          ('TOPPADDING',(0,0),(-1,-1),7),('BOTTOMPADDING',(0,0),(-1,-1),7),
                          ('LINEBELOW',(0,-1),(-1,-1),.5,colors.HexColor('#dfe5e9'))]))
    story.extend([t,Spacer(1,10)])


def page():story.append(PageBreak())


heading('Development evidence | 27 September 2026','How competitive is fereg?')
p('The lighter SCC benchmark campaign | Expanded edition','lead')
p('<b>A strong Stata alternative, and competitive with fixest in important PPML regimes. The broad 10% speed advantage is not yet achieved.</b>')
table([['568 / 568','16h 09m','120 node-hours'],['Groups completed and audited','Production submission to finish','Versus 159 forecast; 24% less']], [WIDTH/3]*3)
figure('coverage',185,'Figure 1. All registered configurations stay in the denominator. Gray means no complete set of four scientifically eligible timing pairs; it is not a loss or a win.')
p('The central result','h2')
p('Across complete configurations, the median native/reference time ratio is <b>1.159 for OLS versus default fixest</b>, <b>1.157 for PPML versus default fixest</b>, and <b>0.806 for PPML versus strict fixest</b>. Ratios below one favor fereg.')
p('The median meets the 10% target in 14/44, 16/51 and 34/55 comparable configurations, respectively. Against reghdfe and ppmlhdfe, every repetition in the complete configurations favors fereg.')
p('Important limits','h2')
p('Native outcomes include 28 timeouts and four application failures. Scientific discrepancies remain unranked. This is a frozen-source development campaign, not release qualification and not a comparison to the later local bias-correction work.','small')
p('Measured source: <b>58284f76</b> | SCC array: <b>7747518</b> | Four warm seeds per configuration | IID OLS / robust PPML | No fitted-mean publication','small')

page();heading('01 | Performance distributions','A mixed picture against fixest')
p('Strong gains and material losses coexist. An aggregate median alone hides both the slow tail and the configurations we cannot rank.')
figure('distribution',245,'Figure 2. Cumulative distribution of configuration medians; each median uses four paired ratios. Vertical lines mark the 0.90 target and parity. Populations differ across references.')
table([['Comparison','Complete / total','Median ratio','10% wins','Losses'],
       ['OLS / default fixest','44 / 68','1.159','14','25'],['PPML / default fixest','51 / 72','1.157','16','30'],['PPML / strict fixest','55 / 72','0.806','34','18'],
       ['OLS / reghdfe','64 / 68','0.088','64','0'],['PPML / ppmlhdfe','58 / 72','0.117','58','0']], [180,90,78,70,WIDTH-418])
p('Accuracy is part of the comparison','h2')
p('Strict stock fixest uses glm.tol=1e-10 and fixef.tol=1e-11. On the <b>46 PPML configurations comparable to both settings</b>, the median ratio is 1.165 against default and 0.819 against strict. The median strict/default reference-time ratio is 1.405. Some of the apparent speed gap therefore depends on the required accuracy, not just implementation.')
p('This does not make default fixest universally invalid: where it passes the scientific checks, its speed advantage is real. The independent smoke and pilot adjudication provide additional scientific evidence; pair agreement alone is not an independent oracle.','small')

page();heading('02 | Where the wins and losses occur','Coverage before averages')
figure('heatmap',555,'Figure 3. Main categorical K10 configurations. Values are medians of four valid paired ratios; green is faster and brown slower. OLS: A = scientific disagreement; T = native timeout. PPML: NR = unranked. -- = not registered. Gray cells never enter performance summaries.')
p('OLS sparse-reference disagreements are conspicuous. PPML is often strong on sparse designs against the strict reference, while large strong-graph cases remain expensive. Pages 9-14 explain graph degree, exact reasons for unranked cells, every comparator, failures, slope evidence and earlier OLS wins.','small')

page();heading('03 | Threading and solver behavior','The clearest actionable anomaly')
figure('scaling',310,'Figure 4. PPML at N983,040, K10. Each point is the median command time across four seeds with passing reference comparisons. Missing strict points remain omitted. Both axes are logarithmic; lower is faster.')
p('<b>Sparse three-FE PPML gets much worse at 28 threads:</b> 24.33 seconds at T8 versus 194.64 at T28. The same seeded designs underlie both settings, although fits ran in separate reserved tasks.')
table([['Captured native telemetry','T8','T28'],['Median FE-projection seconds','16.35','152.68'],['Median strict-final-solve seconds','5.46','27.36'],['Reported inner iterations (range)','293-313','1,195-1,559'],['Reported final linear routes','MAP or LSMR','MAP']], [WIDTH-180,90,90])
p('The projection increase is measured. Routing, convergence behavior and scheduling are hypotheses to test, not yet a proven causal chain. The route field describes the reported final route, not a full per-iteration trace; iteration counts across solver families are not common operation counts. A controlled T8/T28 replay with explicit solver choices is the next discriminating experiment.','small')

page();heading('04 | Slopes and the remaining slow tail','PPML slope work paid off; OLS still lags')
figure('slopes',180,'Figure 5. Dense two-partition slopes at N61,440. OLS uses default fixest; PPML uses strict fixest. The line shows all four ratio extrema, not a confidence interval.')
p('Native PPML slope medians are 0.555s fast and 0.587s low, versus strict fixest at 1.245s and 1.809s. Native OLS takes 0.486s and 0.868s, versus 0.151s and 0.200s. The PPML coefficient-space/joint solver improvements do not automatically transfer to public OLS; their documented routing excludes it (ADRs 0123 and 0134).')
p('Three-FE slope comparisons remain incomplete: default fixest disagrees scientifically, and retained warm-1 strict-reference timeouts prevent four-repeat PPML headlines. Cross-job diagnostic timings are not substituted.','small')
table([['Largest strict-PPML losses','Native seconds','Strict seconds','Paired ratio'],
 ['strong_d6, N3.93m, T28','32.633','8.888','3.712'],['strong_d3, N3.93m, T28','36.326','13.185','2.755'],
 ['strong_d6, N3.93m, T4','35.153','15.137','2.325'],['weak_d3, N3.93m, T28','93.803','47.006','2.034']], [WIDTH-210,75,75,60])
p('What consumes the time?','h2')
p('For large strong_d6 at T28, captured median phase times are 15.55s projection, 10.26s finite solve, 1.88s strict final solve, and only 0.072s inference. Strong_d3 shows the same broad pattern. These timer boundaries include their associated checks; they do not isolate every certificate or kernel. But covariance formation is plainly not the principal target in these cases.')

page();heading('05 | Reliability and memory','Failures remain part of the result')
figure('reliability_memory',200,'Figure 6. Native outcomes include cold anchors. Memory ratios use the same complete four-repeat configurations as the timing comparisons. RSS is sampled process-tree peak, including the language runtime.')
table([['Native outcome','OLS','PPML'],['Passed command','264','272'],['Command timeout','12','16'],['Application failure','0','4 (rc459)']], [WIDTH-180,90,90])
p('Timeouts cluster in large strong_d2, large sparse three-FE designs, and the sparse three-FE zero-outcome appendix. All four sparse two-FE zero-outcome calls fail with rc459. The frozen caps vary from 120 to 1,481 seconds for native failures; a timeout is a lower bound at its registered cap, not a known completion time.')
p('All-command evidence contains 2,936 passes, 227 worker failures, 151 command timeouts and two startup timeouts. Twelve historical budget exclusions and 80 unsupported-surface exclusions remain separate. Of 2,748 attempted pair comparisons, 1,930 pass, 393 fail scientific checks and 425 are unavailable. A discrepancy does not identify which estimator is wrong.')
p('Memory is generally a competitive strength','h2')
p('Across complete configurations, median native/reference RSS ratios are 0.679 for OLS/default fixest, 0.781 for PPML/default and 0.738 for PPML/strict. There are exceptions: PPML reaches 1.97x the strict reference in the upper tail. Successful native peaks are 2.99 GiB OLS and 6.49 GiB PPML. Scheduler maxvmem reaches 7.413G across the whole jobs.','small')

page();heading('06 | Interpretation and priorities','Where further optimization is justified')
p('We are convincingly competitive in Stata and selectively competitive with fixest. The remaining losses are too structured and too large to dismiss as the unavoidable cost of stronger checking. They also do not support a universal claim that one package is faster.','lead')
p('1. Fix the 28-thread sparse three-FE behavior','h2')
p('<b>Evidence:</b> nearly eightfold worse native time at T28 than T8, with a large projection-time increase and changed reported route/iteration behavior. <b>Next:</b> identical-input, explicit-route comparisons; count projections, weight preparations, fallbacks and acceptance checks. Test workload-aware concurrency and solver selection. Do not add a benchmark-name or favorable-seed guard.')
p('2. Redesign the large categorical weighted-linear work','h2')
p('<b>Evidence:</b> strong_d3/d6 retain 2.8-3.7x strict-reference losses at T28. Projection and finite-solve phases dominate; increasing threads offers little native improvement. <b>Next:</b> attribute retained-control construction, row passes, RHS batching, reductions and memory traffic under a fixed weight frame. Preserve independent acceptance, but remove duplicated work where equivalence can be proved.')
p('3. Transfer the successful slope organization to OLS','h2')
p('<b>Evidence:</b> PPML dense slopes win by large margins while OLS loses 3.2-4.4x. The admitted PPML coefficient/joint workspace has explicit OLS exclusions. <b>Next:</b> establish a one-shot OLS owner for reusable geometry and joint response/control work, then validate with independent weighted explicit-design oracles. Sharing a kernel is a coherent candidate; removing guards blindly is not.')
p('4. Close reliability and comparability gaps first','h2')
p('<b>Evidence:</b> sparse-zero native failures, repeated timeouts and sparse OLS reference disagreements. <b>Next:</b> localize failed certificates and sample/separation handling on captured seeds; use fresh independent oracles for disagreements. Keep failed attempts in the denominator. This work expands the population we can responsibly rank.')
p('Lower priority','h2')
p('Small-fit overhead may merit a bounded cleanup after those larger gaps. Inference-only tuning, blanket certificate reductions, further micro-tuning of already-winning dense PPML slopes, and another full campaign before focused experiments have weak justification here. No new optimization experiment was run to prepare this PDF.','small')

page();heading('07 | Method and provenance','Scope and scientific acceptance')
table([['Family','Native','Pinned comparators'],['OLS, IID','fereg','reghdfe, areg, greg, fixest::feols, pyfixest'],['PPML, robust','feppml','ppmlhdfe, gglm, fixest::fepois (default and strict), pyfixest']], [90,70,WIDTH-160])
p('The stock fixest library is unmodified version 0.14.2. Strict PPML changes only glm.tol=1e-10 and fixef.tol=1e-11. R/Python PPML covariance is normalized from HC0 by N/(N-1) under the frozen comparison contract. Ordinary fixest weights are not a Stata frequency-weight inference oracle; this campaign is unweighted.')
p('Four predeclared warm seeds per configuration; eight separate cold anchors. Each comparison group runs sequentially within one reserved 28-core E5-2680v4 task, with requested solver threads 1/4/8/28. Stata control-plane execution remains license-capped at four processors. N ranges from 61,440 to 3,932,160; main K10, selected K50 and slope K6. Inputs are synthetic.')
p('Command timers exclude generation, import, warm-up and extraction, but include the public estimator preparation, solve and requested inference. Fitted-mean publication is not timed. Whole-arm elapsed is separately retained; its difference from command time is not a clean setup/output decomposition. No compilation, profiling or competing fits were scheduled alongside headline fits on their reserved nodes.')
p('Scientific eligibility','h2')
p('Successful commands, identical input, exact retained sample, coefficients, full covariance, objective and applicable convergence/rank/DoF checks precede timing rank. Every headline configuration requires four passing paired repetitions. Default failures stay separate from strict results. Accepted independent smoke and exact-input QR/SVD/Poisson/Hessian pilot adjudication complement reference comparisons; no new independent oracle was run on every large cell.')
p('Exact identities','h2')
p('Estimator/controller: 58284f76a65b226e6df09a48cafa5caa68461af2','code')
p('Runtime fingerprint:<br/>041bf47963205620c13f2137c17a25a47afd70d73c5d03c42a936bbb39ce29e6','code')
p('Linux plugin SHA-256:<br/>1f59671842c4ff436d6975057eb1fcf2d32cec302bc6a64b8adc73d91588c023','code')
p('Production array 7747518: all 16 scheduler and application receipts pass. The audit finds 568/568 groups, no pending groups and no integrity errors. All 2,748 captured comparisons replay exactly. Original pilot rejection and warm-1 timeouts remain intact under ADR0226.','small')
p('Evidence map','h2')
p('Repository: benchmarks/reports/development/systematic-light-scc-final-2026-09-27/ (README, generated CSVs, audit, accounting, bundles and hashes). Original phase extracts: systematic-light-scc-pdf-2026-09-27/. Expanded diagnostic evidence: systematic-light-scc-pdf-expanded-2026-09-27/. Later local bias-correction work is not covered by these timings.','small')


page();heading('08 | Reading the designs','Degree is not the number of fixed effects')
p('<b>strong_d2, strong_d3 and strong_d6 all have two categorical FE partitions.</b> The suffix is the number of observations per worker and its worker-firm graph degree. The earlier chat description of d6 as six-dimensional was incorrect.')
table([['Design family','Actual construction','What it stresses'],
 ['strong_d2 / d3 / d6','Worker and firm FE; d=2/3/6 rows per worker. Long-range firm memberships; worker-count / firm-count ratio about 40.','Graph connectivity and the number of FE coefficients relative to N. At fixed N, d=2 has more workers than d=6.'],
 ['weak_d3','Two FE; three rows per worker; shallow hub-tree and leaf construction.','Bottlenecks between graph regions despite the same worker degree as strong_d3.'],
 ['one_fe / two_fe_dense','Five rows per worker; worker FE alone, or worker plus firm. Dense panel has 20% move probability at each transition.','Routine one-/two-partition absorption.'],
 ['two_fe_sparse / three_fe_sparse','2% move probability; three-FE adds a crossed county-like categorical effect.','Weak mobility, disconnected components and additional projection interactions.']], [110,215,WIDTH-325])
p('The strong label names a construction; it does not guarantee easy conditioning at every size. Group counts grow with N. The dense and sparse panel labels concern mobility, not a dense dummy-matrix implementation. K is the number of ordinary controls; T is requested solver threads.','small')
p('What this lighter campaign actually covers','h2')
table([['Included','Not included in these timing claims'],
 ['OLS IID; PPML robust (heteroskedasticity-consistent)','Clustered OLS or PPML inference'],
 ['Unweighted fits, K10 main and selected K50','Analytic, probability or frequency weights; generated/factor-term screens'],
 ['Categorical FE and eight slope configurations','Separate saved-FE publication or prediction/fitted-mean publication'],
 ['Two native slope memory modes','A controlled memory-mode-only comparison or constrained-RAM qualification']], [WIDTH/2]*2)
p('Worker code requests absorb(), native threads(), native slope memory(), and robust PPML. It does not request saving FE or publishing fitted values. Coefficients, covariance and sample IDs are extracted after the command timer for the scientific comparison. This is a deliberate coverage limit of the lighter campaign.','small')

page();heading('09 | Every comparator','Broader than fixest')
figure('all_comparators',215,'Figure 7. Distributions across complete configurations only. Populations differ by comparator. greg and gglm have no rankable series because their workers expose only retained counts, not exact sample IDs.')
expanded=json.loads((ROOT/'analysis.json').read_text())
rows=[['Model / reference','Complete','Median time','10% wins','Losses','Median RSS']]
for r in expanded['comparators']:
    fmt=lambda x: '--' if x is None else f'{x:.3f}'
    rows.append([r['model'].upper()+' / '+r['comparator'].replace('_strict',' strict'),f"{r['complete_configurations']}/{r['registered_configurations']}",fmt(r['median_ratio']),r['median_ten_percent_wins'] if r['complete_configurations'] else '--',r['median_losses'] if r['complete_configurations'] else '--',fmt(r['median_rss_ratio'])])
table(rows,[145,64,74,65,60,WIDTH-408])
p('Native is consistently faster than reghdfe and ppmlhdfe in complete comparisons. OLS also usually beats areg and pyfixest. PPML/pyfixest has only 21 complete configurations: native loses 13 and its median time ratio is 1.423. That selected subset is not representative of all 72 configurations.','small')
p('Stata reference execution is capped at four licensed processors; native/R requests extend to 28. All areg calls use Stata 19 absorb() with the registered FE dimensions. Unsupported slope cells remain exclusions in the pinned workers for areg, greg, gglm and pyfixest; no expanded-dummy replacement is timed. A missing validated worker API is not a universal statement about a package\'s capabilities.','small')

page();heading('10 | Why OLS cells are unranked','Most NR cells contain completed fits')
p('Of 68 OLS/fixest configurations, 44 have four valid comparisons. The other <b>24 comprise 21 scientific disagreements and three native-timeout configurations</b>. In the main categorical grid alone, there are 15 disagreement cells and three timeout cells. The remaining six disagreements are four K50 and two slope cells.')
table([['Unranked group','Configurations','Recorded issue'],
 ['Sparse two FE','8','Coefficients and residual sum of squares (RSS); standard errors, often covariance too.'],
 ['Sparse three FE, categorical','9','Standard errors in every case; often covariance; some coefficients or RSS.'],
 ['strong_d2, N983,040','2','Coefficients and RSS; usually standard errors.'],
 ['Sparse three-FE slopes','2','Default fixest reports nonconvergence; coefficients, covariance, RSS and standard errors disagree.'],
 ['Large strong_d2 (T4/T28), large sparse three FE (T4)','3','Native command timed out; no valid timing pair.']], [160,80,WIDTH-240])
p('A concrete inference mismatch','h2')
p('For sparse two-FE OLS at N61,440, seed 104729, an independent graph calculation reproduces both captured group-column hashes and finds <b>38 connected components</b>. With 12,288 workers and 615 firms, FE rank is 12,288 + 615 - 38 = <b>12,865</b>. With K10, residual DoF is <b>48,565</b>, exactly as fereg and reghdfe report. Default fixest counts 37 more parameters and uses residual DoF 48,528.')
p('For the corresponding three-FE example, fereg/reghdfe report residual DoF 48,535 versus fixest 48,498. Rescaling only the captured fixest covariance by the DoF ratio reduces the maximum relative diagonal difference to <b>2.35e-9</b>. This diagnostic strongly identifies the inference scaling mismatch in that example; it does not change a frozen result or adjudicate every three-way rank.')
p('Across all 84 warm OLS/fixest discrepancies, 83 also pass the reghdfe comparison; the remaining one is the independently adjudicated slope pilot whose original reghdfe attempt timed out. Agreement is corroboration, not an independent oracle. Reference stopping is another plausible contributor: on strong_d2, one default fixest RSS is 119,761.9136 versus native 119,761.0431. No replacement timing ranks are assigned.','small')

page();heading('11 | Failure attribution','Which problems belong to which command?')
p('A command failure, a timeout, and a scientific disagreement are different outcomes. Scheduler success means the comparison task recorded its evidence; it does not mean every estimator succeeded.')
rows=[['Model / command','Pass','Timeout','Failure','Excluded']]
for model in ('ols','ppml'):
    for arm,v in expanded['arm_status'][model].items():
        rows.append([model.upper()+' / '+arm.replace('_strict',' strict'),v.get('pass',0),v.get('command_timeout',0)+v.get('startup_timeout',0),v.get('worker_failure',0),v.get('budget_exclusion',0)+v.get('unsupported_surface',0)])
table(rows,[WIDTH-248,62,62,62,62])
p('Native: genuine failure cases, unresolved root causes','h2')
p('All four feppml rc459 failures are sparse two-FE zero-outcome fits, after about 109-111 seconds. They are real native robustness failures to investigate. The timed Stata worker uses capture quietly, and the retained output does not identify the failed internal condition. We cannot yet distinguish a convergence/certificate limitation, separation difficulty or implementation bug. These are not four demonstrated cases of silently wrong estimates.')
p('The 28 native timeouts mean the fit exceeded its predeclared cap (120-1,481 seconds). They show an operational performance problem, not by themselves a wrong-answer bug. Larger budgets would not constitute a repair, and no failed fit was rerun here.','small')
p('Comparator failures and harness limitations','h2')
p('All <b>150 pyfixest worker failures</b> are explained by the captured 10,000-iteration demeaning limit: 141 report it in result metadata and nine fail during warm-up with the same exception. gglm has 65 worker failures and ppmlhdfe eight, all before the target READY marker; their captured metadata does not localize the underlying condition. gglm also has two startup timeouts. The excluded column combines explicit budget skips and unsupported worker surfaces; it never counts them as observed failures.','small')

page();heading('12 | Complete slope evidence','Two- and three-partition slopes')
p('Eight configurations, four seeds each: <b>all 32 native slope calls passed</b>. Every fit includes intercepts. Two-FE cases add one slope to each partition; three-FE cases add two slopes to each of g1 and g2, plus a categorical g3. All have N61,440.')
rows=[['Model / FE','Mode; K/T','Native seconds: median [min-max]','fixest ratio or coverage','Stata HDFE ratio or coverage']]
for s in expanded['slope_configurations']:
    model=s['model']; fast=s['section']=='slopes_fast'; d='2' if s['design']=='two_fe_dense' else '3'
    refarm='fixest' if model=='ols' else 'fixest_strict'; stata='reghdfe' if model=='ols' else 'ppmlhdfe'
    def reference(arm):
        r=next(x for x in s['reference_results'] if x['comparator']==arm)
        return f"{float(r['ratio_median']):.3f} (4/4)" if r['complete']=='True' else f"{r['validated_repetitions']}/4 eligible"
    lo,hi=s['native_range_seconds']
    rows.append([model.upper()+f' / {d} FE',('fast; 10/28' if fast else 'low; 6/4'),f"{s['native_median_seconds']:.3f}<br/>[{lo:.3f}-{hi:.3f}]",reference(refarm),reference(stata)])
table(rows,[78,85,135,100,WIDTH-398])
p('Fixest means default for OLS and strict for PPML; Stata HDFE means reghdfe / ppmlhdfe. A ratio requires all four valid pairs. Default fixest PPML fails all slope comparisons. Low versus fast also changes K and threads, so these rows cannot isolate the effect of memory mode. The reference is not forced into a matching native memory implementation.','small')
p('Independent scientific evidence for the difficult slopes','h2')
p('The original fast three-FE OLS warm-1 reghdfe attempt and both three-FE PPML warm-1 strict-fixest attempts timed out at 120s. Separate exact-input adjudication validated the original native outputs using independent QR/SVD and Poisson/final-Hessian oracles. Maximum native coefficient errors were <b>8.89e-16 OLS</b> and <b>5.20e-15 / 2.81e-14 PPML</b>; full covariance, objective, sample and rank checks passed.')
p('The three-FE slope model ranks were 38,711 (K10) and 38,707 (K6). Subsequent diagnostic reference runtimes of 734.7s, 322.3s and 517.1s are not paired headline timings and do not fill the missing original repetition. Three-FE PPML has three valid strict-reference pairs per mode, but no four-repeat median claim.','small')
p('What this supports','h2')
p('Dense PPML slopes beat strict fixest substantially; dense OLS slopes still lose substantially. Three-FE native slopes have strong independent accuracy evidence but incomplete fixest timing coverage. Saved-effect recovery, clustered slope inference, weights and fresh rank-deficient slope designs require separate performance evidence.')

page();heading('13 | Earlier OLS wins','Your recollection is correct')
p('The September 21 Mac report recorded <b>93/93 timing wins in each of two five-pair blocks</b>: 77 primary and 16 holdout cells, 1,860 calls. That evidence remains valid for its captured owf1 artifact. The current SCC campaign is a different experiment, not a before/after regression test.')
table([['Dimension','Earlier Mac OLS blocks','This SCC campaign'],
 ['Artifact / platform','Captured owf1 Mac plugin','58284f76 Linux plugin; E5-2680v4'],
 ['Threads / sizes','T16 throughout; N245,760 and N983,040','T1/4/8/28; N61,440 to N3,932,160'],
 ['Controls / designs','K1/3/5/6/13/50; two-way categorical designs, including disconnected, cycle and bottleneck cases','Main K10, selected K50; one/two/three FE, degree designs and a slope appendix'],
 ['Weights / inference','Unit, analytic and probability weights; IID, robust, one-/two-way clustering','Unweighted; IID OLS and robust PPML'],
 ['Requested outputs','Selected saved FE, residuals and fitted outputs','No saved-FE or fitted-output publication'],
 ['Treatment of discrepancies','Timing wins recorded separately from default-reference inference: 75 pass, 18 discrepancies per block','Any failed scientific comparison removes that pair from timing rank; four required per configuration']], [105,197,WIDTH-302])
p('The old report also records 186 independent native inference checks and 100 independent saved-output audits passing. It does not claim every default-reference result agrees, and it does not establish performance for later source or Linux. The old 93/93 statement and the new complete-comparison win rate therefore have different denominators.','small')
p('The new results still contain substantial OLS wins','h2')
p('At N983,040/K10, dense two-FE OLS ratios are <b>0.514 at T8 and 0.499 at T28</b>; strong_d3 ratios are about <b>0.60 and 0.53</b>. Across eligible OLS configurations, T4 wins 4/19 while T28 wins 10/19. T8 wins 3/3, but samples only three comparable configurations. Coverage differs, so these aggregate thread counts alone do not identify a regression.')
p('The next useful bridge is a small matched replay of earlier and current artifacts on identical designs, hardware, threads, inference and outputs. That can separate source regressions from platform, scale and case-mix effects. No such bridge or new estimator campaign was run for this report.','small')
p('Sources for the additions','h2')
p('Frozen generated CSVs and captured result hashes; systematic generate.py, light.py and worker sources; exact-input slope adjudication result; current-ols-final-blocks-local-2026-09-21; ols-round-closeout-local-2026-09-22. Expanded diagnostic extracts, independent topology calculation and hashes are preserved alongside this PDF builder.','small')

def decorate(canvas,doc):
    canvas.saveState();canvas.setStrokeColor(TEAL);canvas.setLineWidth(2);canvas.line(48,H-32,W-48,H-32)
    canvas.setFont('Helvetica-Bold',8);canvas.setFillColor(NAVY);canvas.drawString(48,H-23,'FEREG / SCC BENCHMARK REVIEW')
    canvas.setFont('Helvetica',8);canvas.setFillColor(MUTED);canvas.drawRightString(W-48,H-23,'27 SEPTEMBER 2026')
    canvas.setStrokeColor(colors.HexColor('#dfe5e9'));canvas.setLineWidth(.5);canvas.line(48,35,W-48,35)
    canvas.setFont('Helvetica',7.5);canvas.drawString(48,23,'Development evidence | Source 58284f76 | Ratios below 1 favor fereg')
    canvas.drawRightString(W-48,23,str(doc.page));canvas.restoreState()


doc=BaseDocTemplate(str(OUTPUT),pagesize=A4,title='fereg: SCC benchmark campaign and interpretation',
                    author='fereg development',subject='OLS and PPML performance, accuracy, coverage and priorities')
doc.addPageTemplates(PageTemplate(id='report',frames=[Frame(48,45,WIDTH,H-99,leftPadding=0,rightPadding=0,topPadding=0,bottomPadding=0)],onPage=decorate))
doc.build(story)
print(OUTPUT)
