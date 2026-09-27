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
OUTPUT=REPO/'output/pdf/fereg-scc-benchmark-report-2026-09-27.pdf'
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
p('The lighter SCC benchmark campaign','lead')
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
figure('heatmap',555,'Figure 3. Main categorical K10 configurations. Values are medians of four valid paired ratios; green is faster and brown slower. NR = registered but not rankable. -- = not registered. Gray cells never enter performance summaries.')
p('OLS sparse-reference disagreements are conspicuous. PPML is often strong on sparse designs against the strict reference, while large strong-graph cases remain expensive. These tables do not establish causes; the phase and thread evidence on the next page is more diagnostic.','small')

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
p('Repository: benchmarks/reports/development/systematic-light-scc-final-2026-09-27/ (README, generated CSVs, audit, accounting, bundles and hashes). This illustrated derivative and captured phase extracts: systematic-light-scc-pdf-2026-09-27/. Later local bias-correction work is not covered by these timings.','small')


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
