"""Render a single vector PDF from complete SCC benchmark receipts."""
import csv
import json
from pathlib import Path

import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.backends.backend_pdf import PdfPages
from matplotlib.ticker import ScalarFormatter
import numpy as np
from pypdf import PdfReader, PdfWriter
from reportlab.pdfgen import canvas
from reportlab.lib.colors import HexColor
from reportlab.lib.styles import ParagraphStyle
from reportlab.platypus import Paragraph, Table, TableStyle

from common import read_json, write_json
from run_scc import ARMS
from outcomes import audited_outcome

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT/'output/pdf'
COLORS = ['#1768AC','#D97720','#16877A','#8364B0','#CB4B79','#71604E']
COLOR_INDEX = {'fereg':0,'feppml':0,'reghdfe':1,'ppmlhdfe':1,'greg':2,'gglm':2,'areg':3,'fixest':4,'pyfixest':5}
LABEL = {'fereg':'fereg','feppml':'feppml','reghdfe':'reghdfe','ppmlhdfe':'ppmlhdfe',
         'greg':'greg','gglm':'gglm (Poisson)','areg':'areg','fixest':'fixest (R)','pyfixest':'pyfixest (Python)'}
WIDTH, HEIGHT = 842, 595


def load():
    plan = read_json(ROOT/'results/scc/campaign-manifest.json')
    complete = read_json(ROOT/'results/scc/campaign-complete.json')
    rows = [read_json(p) for p in sorted((ROOT/'results/scc/campaign').glob('*.json'))]
    expected = (plan['warm_repetitions']+1)*len(plan['threads'])*sum(map(len,ARMS.values()))
    assert len(rows)==complete['calls']==expected
    assert len({(r['cell']['model'],r['arm'],r['cell']['threads'],r['repetition']) for r in rows})==expected
    assert all(r['measurement_fingerprint']==complete['runtime_fingerprint'] for r in rows)
    rows = [audited_outcome(r) for r in rows]
    return rows,plan


def science(rows):
    checks=[]
    for r in rows:
        model=r['cell']['model']; ref='reghdfe' if model=='ols' else 'ppmlhdfe'
        target=next(q for q in rows if q['arm']==ref and q['cell']==r['cell'] and q['repetition']==r['repetition'])
        c={'model':model,'arm':r['arm'],'threads':r['cell']['threads'],'repetition':r['repetition'],
           'command_status':r['status'],'reference':ref}
        if r['status']=='pass' and target['status']=='pass' and r.get('coefficients') and target.get('coefficients'):
            aa={v['name']:v for v in r['coefficients']};bb={v['name']:v for v in target['coefficients']}
            assert aa.keys()==bb.keys()
            a=np.array([[aa[k]['value'],aa[k]['se']] for k in sorted(aa)])
            b=np.array([[bb[k]['value'],bb[k]['se']] for k in sorted(bb)])
            c.update(coefficient_pass=bool(np.allclose(a[:,0],b[:,0],rtol=1e-7,atol=1e-10)),
                     se_pass=bool(np.allclose(a[:,1],b[:,1],rtol=1e-6,atol=1e-10)),
                     maximum_absolute_b_difference=float(np.max(abs(a[:,0]-b[:,0]))),
                     maximum_relative_se_difference=float(np.max(abs(a[:,1]-b[:,1])/abs(b[:,1]))),
                     same_N=r['metadata'].get('N')==target['metadata'].get('N'),
                     sample_exact=(r.get('sample_sha256')==target.get('sample_sha256')) if r.get('sample_sha256') else None)
            for key in (['rss'] if model=='ols' else ['ll','deviance']):
                av=r['metadata'].get(key,r['metadata'].get('loglik') if key=='ll' else None)
                bv=target['metadata'].get(key)
                if av is not None and bv is not None:
                    c[key+'_difference']=av-bv
                    c[key+'_relative_difference']=abs(av-bv)/max(1,abs(bv))
        checks.append(c)
    write_json(ROOT/'results/scc/scientific-comparisons.json',checks)
    return checks


def group(rows,model,arm,t):
    return [r for r in rows if r['cell']['model']==model and r['arm']==arm and r['cell']['threads']==t and r['phase']=='warm']


def title(fig,heading,subtitle,page):
    fig.text(.075,.925,heading,fontsize=23,fontweight='bold',color='#15334D')
    fig.text(.075,.884,subtitle,fontsize=10,color='#526575')
    fig.text(.075,.035,'VENETO  /  SCC BENCHMARK  /  SEPTEMBER 2026',fontsize=8,color='#647888')
    fig.text(.94,.035,str(page),fontsize=9,color='#647888',ha='right')


def figure_pages(rows,plan):
    sample=read_json(ROOT/'results/sample.json')
    plt.rcParams.update({'font.family':'DejaVu Sans','font.size':10,'axes.spines.top':False,
                         'axes.spines.right':False,'axes.edgecolor':'#9AACB9','axes.labelcolor':'#34495E',
                         'xtick.color':'#526575','ytick.color':'#526575','pdf.fonttype':42})
    with PdfPages(OUT/'figures.pdf') as pdf:
        for page,model in enumerate(ARMS,1):
            fig,axes=plt.subplots(1,2,figsize=(WIDTH/72,HEIGHT/72))
            fig.subplots_adjust(left=.09,right=.96,bottom=.25,top=.76,wspace=.28)
            title(fig,'OLS: log daily pay' if model=='ols' else 'PPML: daily pay in levels',
                  f"{sample['N']:,} observations  |  Worker + employer effects  |  Identical controls and input values",page)
            for metric,ax in zip(['seconds','peak_rss_gib'],axes):
                for ai,arm in enumerate(ARMS[model]):
                    color = COLORS[COLOR_INDEX[arm]]
                    med=[];lo=[];hi=[]
                    for t in plan['threads']:
                        selected=group(rows,model,arm,t)
                        valid=[r[metric] for r in selected if r['status']=='pass']
                        if len(valid)==plan['warm_repetitions']:
                            q=np.quantile(valid,[.25,.5,.75]);lo.append(q[1]-q[0]);med.append(q[1]);hi.append(q[2]-q[1])
                        else: med.append(np.nan);lo.append(0);hi.append(0)
                        timed=[r for r in selected if r['status']=='command_timeout']
                        if timed and metric=='seconds':
                            ax.scatter([t],[plan['measurement']['command_timeout_seconds']],marker='^',facecolors='none',edgecolors=color,s=65)
                        failed=[r[metric] for r in selected if r['status']!='pass' and r['status']!='command_timeout' and r.get(metric) is not None]
                        if len(failed)==plan['warm_repetitions']:
                            q=np.quantile(failed,[.25,.5,.75])
                            ax.errorbar([t],[q[1]],yerr=[[q[1]-q[0]],[q[2]-q[1]]],color=color,marker='x',ms=7,lw=0,elinewidth=1,capsize=3)
                        if metric=='peak_rss_gib' and len(timed)==plan['warm_repetitions']:
                            ax.scatter([t],[np.median([r[metric] for r in timed])],marker='^',facecolors='none',edgecolors=color,s=65)
                    ax.errorbar(plan['threads'],med,yerr=[lo,hi],color=color,marker=['o','s','D','v','P','X'][COLOR_INDEX[arm]],
                                lw=2 if ai else 2.8,ms=5,capsize=3,label=LABEL[arm])
                ax.set_xscale('log',base=2);ax.set_xticks(plan['threads'],[str(t) for t in plan['threads']])
                ax.set_xlabel('CPU affinity cores',labelpad=9)
                ax.grid(axis='y',alpha=.22);ax.set_axisbelow(True)
                if metric=='seconds':
                    ax.set_yscale('log');ax.yaxis.set_major_formatter(ScalarFormatter());ax.set_ylabel('Command wall time (seconds; log scale)')
                    ax.set_title('Runtime',loc='left',fontsize=14,fontweight='bold',pad=15)
                else:
                    ax.set_ylim(bottom=0);ax.set_ylabel('Absolute peak process RSS (GiB)')
                    ax.set_title('Memory',loc='left',fontsize=14,fontweight='bold',pad=15)
            handles,labels=axes[0].get_legend_handles_labels()
            fig.legend(handles,labels,loc='lower center',bbox_to_anchor=(.52,.13),ncol=3,frameon=False,columnspacing=2.5)
            fig.text(.075,.098,'Medians / IQR of 3 warm calls. Solid markers: successful; x: error/nonconvergence; triangles: timeout bounds.',fontsize=8.5,color='#526575')
            fig.text(.075,.077,'Whole 28-core nodes reserved; Stata/MP is license-capped at 4 processors. Numerical comparisons appear on page 5.',fontsize=8.5,color='#526575')
            pdf.savefig(fig);plt.close(fig)


def paragraph(c,text,x,y,width,size=10,color='#34495E',leading=None):
    style=ParagraphStyle('body',fontName='Helvetica',fontSize=size,leading=leading or size*1.4,textColor=HexColor(color))
    p=Paragraph(text,style);_,h=p.wrap(width,HEIGHT);p.drawOn(c,x,y-h);return y-h


def heading(c,text,sub,page):
    c.setFillColor(HexColor('#15334D'));c.setFont('Helvetica-Bold',23);c.drawString(58,548,text)
    paragraph(c,sub,58,531,725,10)
    c.setFont('Helvetica',8);c.setFillColor(HexColor('#647888'));c.drawString(58,23,'VENETO  /  SCC BENCHMARK  /  SEPTEMBER 2026');c.drawRightString(790,23,str(page))


def table(c,data,widths,x,y,size=10):
    t=Table(data,colWidths=widths)
    t.setStyle(TableStyle([('BACKGROUND',(0,0),(-1,0),HexColor('#EAF0F5')),('TEXTCOLOR',(0,0),(-1,0),HexColor('#15334D')),
         ('FONTNAME',(0,0),(-1,0),'Helvetica-Bold'),('FONTNAME',(0,1),(-1,-1),'Helvetica'),('FONTSIZE',(0,0),(-1,-1),size),
         ('TOPPADDING',(0,0),(-1,-1),9),('BOTTOMPADDING',(0,0),(-1,-1),9),('LINEBELOW',(0,0),(-1,0),.7,HexColor('#9AACB9')),
         ('ROWBACKGROUNDS',(0,1),(-1,-1),[HexColor('#FFFFFF'),HexColor('#F7F9FB')]),('VALIGN',(0,0),(-1,-1),'MIDDLE')]))
    _,h=t.wrap(WIDTH,HEIGHT);t.drawOn(c,x,y-h);return y-h


def narrative(rows,plan,checks):
    c=canvas.Canvas(str(OUT/'details.pdf'),pagesize=(WIDTH,HEIGHT))
    for page,model in enumerate(ARMS,3):
        heading(c,('OLS' if model=='ols' else 'PPML')+': complete measurement grid',
                'Each cell gives median command seconds / peak RSS GiB, followed by successful warm fits out of 3.',page)
        data=[['Command']+[str(t)+' threads' for t in plan['threads']]]
        for arm in ARMS[model]:
            line=[LABEL[arm]]
            for t in plan['threads']:
                subset=group(rows,model,arm,t);ok=[r for r in subset if r['status']=='pass']
                if len(ok)==plan['warm_repetitions']:
                    line.append(f"{np.median([r['seconds'] for r in ok]):.2f} / {np.median([r['peak_rss_gib'] for r in ok]):.2f}\n3/3")
                else:
                    failures={r['status'] for r in subset if r['status']!='pass'}
                    label=f">={plan['measurement']['command_timeout_seconds']} s" if 'command_timeout' in failures else 'NC' if 'nonconvergence' in failures else 'Error'
                    if len(subset)==3 and all(r.get('seconds') is not None and r.get('peak_rss_gib') is not None for r in subset):
                        label=(f">={plan['measurement']['command_timeout_seconds']}" if 'command_timeout' in failures else f"{'NC' if 'nonconvergence' in failures else 'E'} {np.median([r['seconds'] for r in subset]):.1f}")
                        label+=f" / {np.median([r['peak_rss_gib'] for r in subset]):.2f}"
                    line.append(label+f'\n{len(ok)}/3')
            data.append(line)
        bottom=table(c,data,[140]+[98]*6,58,485,9)
        counts={}
        for r in rows:
            if r['cell']['model']==model: counts[r['status']]=counts.get(r['status'],0)+1
        text='Outcomes including cold diagnostics: '+', '.join(f'{n} {k.replace("_"," ")}' for k,n in counts.items())+'.'
        y=paragraph(c,text,58,bottom-24,720,10)
        y=paragraph(c,'E denotes a command error; NC denotes a reported nonconverged fit. These measurements are not successful estimation times. '
                    f"A timeout is an unfinished fit after {plan['measurement']['command_timeout_seconds']} seconds; its observed peak memory is a lower bound for completion. "
                    'The cold diagnostics and every failed invocation remain in the machine-readable receipts.',58,y-13,720,10)
        paragraph(c,'Memory includes each process\'s data and runtime. Loading and warm-up happen before sampling; '
                    'retained runtime memory still contributes to RSS. No process-baseline subtraction is applied.',58,y-14,720,10)
        c.showPage()
    heading(c,'Numerical agreement and limitations','Timing is a computational outcome; it does not by itself establish equivalent estimates.',5)
    data=[['Model / command','Successful','Slope checks','SE checks','Max |slope difference|']]
    for model in ARMS:
        for arm in ARMS[model]:
            rr=[q for q in checks if q['model']==model and q['arm']==arm and q['repetition']>0]
            cc=[q for q in rr if 'coefficient_pass' in q]
            data.append([('OLS' if model=='ols' else 'PPML')+' / '+LABEL[arm],str(sum(q['command_status']=='pass' for q in rr))+'/'+str(len(rr)),
                         str(sum(q['coefficient_pass'] for q in cc))+'/'+str(len(cc)),str(sum(q['se_pass'] for q in cc))+'/'+str(len(cc)),
                         f"{max(q['maximum_absolute_b_difference'] for q in cc):.2g}" if cc else 'unavailable'])
    y=table(c,data,[220,90,110,100,208],58,485,9)
    y=paragraph(c,'Checks compare each fit with the same-thread/repetition pinned reghdfe (OLS) or ppmlhdfe (PPML). '
                'Slope tolerance: rtol 1e-7, atol 1e-10; SE tolerance: rtol 1e-6, atol 1e-10. '
                'Reference self-comparisons are included. 0/0 means no completed reference comparison.',58,y-14,725,8.5)
    paragraph(c,'Independent checks pass fereg/feppml slopes, SEs and N on 3,732 real observations locally, plus 7,680 synthetic observations on SCC at 1/28 cores. '
                'It does not certify the full panel. Native disconnected-graph DoF rules and stopping criteria can differ. '
                'Successful timings with failed checks remain computational diagnostics.',58,y-10,725,8.5)
    c.showPage()
    heading(c,'Design, provenance and reading the figures','SCC development benchmark; no release qualification or KSS variance-decomposition claim.',6)
    sample=read_json(ROOT/'results/sample.json');runtime=read_json(ROOT/'results/scc/runtime.json')
    blocks=[('Real VWH panel',f"FRDB's Veneto Workers Histories delivery: {sample['counts']['raw_spells']:,} raw job-spell records. "
             f"The benchmark keeps the 1999-2001 dominant-job panel: {sample['N']:,} observations, {sample['workers']:,} workers, "
             f"{sample['firms']:,} firms. Employers are in Veneto; ages are 19-63; pay and paid days are positive. "
             'Worker and firm singletons are removed to a fixed point. The sample differs from KSS selection.'),
            ('Identical model inputs','Worker and firm fixed effects; age squared /100, age cubed /1000, year-2000 and year-2001 indicators. '
             'OLS outcome is log daily pay; PPML outcome is daily pay in source units. OLS uses IID covariance; PPML uses robust covariance. '
             'R/Python PPML HC0 covariance is multiplied by N/(N-1) after timing. No solver tuning follows observed outcomes.'),
            ('Timing and CPU requests','BU SCC Linux: Intel Xeon E5-2680v4 nodes, 28 physical cores. Each task reserves the whole node. '
             'The target process and its children are pinned to nested subsets of 1, 2, 4, 8, 14 or 28 cores. '
             'Stata/MP has a 4-processor license and uses min(request,4); fereg/feppml native thread counts are separate. '
             'One setting/repetition shares a node across commands. Three warm fresh-process fits and one cold diagnostic; '
             'initialization uses independent synthetic data. Reghdfe parallel() is not invoked.'),
            ('Memory and reproducibility','Wall-time timers surround the public estimator including covariance; loading, import audits, initialization and export are excluded. '
             'Process-tree RSS uses a requested 10 ms polling sleep and a start handshake; polling overhead lengthens actual intervals. Brief peaks can be missed. '
             'Every imported column is checked as an IEEE-double SHA-256 sequence. Source, runtime hashes, dependency inventories, '
             'per-call commands, return codes, samples and numerical results are retained under results/ and ignored work/.'),
            ('Versions and source',f"Candidate source {runtime['runtime']['candidate']['commit'][:12]}; pinned reghdfe 6.14.1; gtools 1.11.8; fixest 0.14.2; "
             'PyFixest 0.60.0 at b6f2d284adf0; ppmlhdfe b85665d8f674. Exact archives and plugin identities are in the companion receipts.'),
            ('Data acknowledgement','VWH was developed at Universita Ca\' Foscari Venezia under Giuseppe Tattara and supplied by Fondazione Rodolfo Debenedetti. '
             'Kline, Saggio and Solvsten (2020), Leave-Out Estimation of Variance Components, Econometrica 88(5), 1859-1898. '
             '<link href="https://doi.org/10.3982/ECTA16410" color="#1768AC">doi:10.3982/ECTA16410</link>. '
             'The current delivery is not established as byte-identical to their historical private data.')]
    y=480
    for label,body in blocks:
        y=paragraph(c,'<b>'+label+'</b>',58,y,170,10)
        body_y=y+14
        h=paragraph(c,body,225,body_y,555,9.1)
        y=min(y,h)-18
    assert y>40, y
    c.showPage();c.save()


def main():
    OUT.mkdir(parents=True,exist_ok=True)
    rows,plan=load();checks=science(rows)
    figure_pages(rows,plan);narrative(rows,plan,checks)
    writer=PdfWriter()
    for name in ['figures.pdf','details.pdf']:
        for page in PdfReader(OUT/name).pages: writer.add_page(page)
    writer.add_metadata({'/Title':'Veneto: SCC fixed-effects benchmark','/Author':'Johannes Schmieder',
                         '/Subject':'Runtime, process RSS and numerical comparison across requested CPU threads'})
    target=OUT/'Veneto_Benchmark.pdf'
    with target.open('wb') as f: writer.write(f)
    with (ROOT/'results/scc/runs.csv').open('w') as f:
        w=csv.writer(f);w.writerow(['model','command','requested_threads','repetition','phase','status','seconds','peak_rss_gib'])
        for r in rows:w.writerow([r['cell']['model'],r['arm'],r['cell']['threads'],r['repetition'],r['phase'],r['status'],r['seconds'],r['peak_rss_gib']])
    print(target)


if __name__=='__main__':main()
