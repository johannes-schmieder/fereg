"""Figures derived solely from the immutable SCC receipts (no estimation)."""
import csv
import json
import math
from pathlib import Path
from statistics import median

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.colors import TwoSlopeNorm
from matplotlib.ticker import ScalarFormatter, NullFormatter, FuncFormatter
import numpy as np

ROOT = Path(__file__).resolve().parent
SOURCE = ROOT.parent / "systematic-light-scc-final-2026-09-27"
OUT = ROOT / "figures"
OUT.mkdir(exist_ok=True)


def read(name):
    with (SOURCE / "generated" / name).open() as f:
        return list(csv.DictReader(f))


summary, pairs, runs = map(read, ("summary.csv", "paired-ratios.csv", "runs.csv"))
panels = [("ols", "fixest", "OLS / default fixest"),
          ("ppml", "fixest", "PPML / default fixest"),
          ("ppml", "fixest_strict", "PPML / strict fixest")]
TEAL, NAVY, CORAL, GRAY = "#008579", "#183549", "#c95b42", "#dfe5e9"
COLORS = [NAVY, CORAL, TEAL]
plt.rcParams.update({"font.size": 9, "font.family": "DejaVu Sans", "axes.spines.top": False,
                     "axes.spines.right": False, "axes.labelcolor": NAVY, "text.color": NAVY,
                     "axes.titleweight": "bold", "savefig.facecolor": "white"})


def save(fig, name):
    fig.savefig(OUT / f"{name}.png", dpi=240)
    fig.savefig(OUT / f"{name}.pdf")
    plt.close(fig)


fig, ax = plt.subplots(figsize=(7, 2.55), layout="constrained")
for i, (model, arm, title) in enumerate(panels):
    rows = [x for x in summary if x["model"] == model and x["comparator"] == arm]
    valid = [float(x["ratio_median"]) for x in rows if x["complete"] == "True"]
    counts = [sum(x <= .9 for x in valid), sum(.9 < x <= 1 for x in valid),
              sum(x > 1 for x in valid), len(rows) - len(valid)]
    left = 0
    for n, color in zip(counts, (TEAL, "#9dc7bd", CORAL, GRAY)):
        ax.barh(i, n, left=left, color=color, height=.56)
        if n >= 4: ax.text(left+n/2, i, str(n), ha="center", va="center", color="white" if color in (TEAL, CORAL) else NAVY)
        left += n
ax.set(yticks=range(3), yticklabels=[x[2] for x in panels], xlabel="Registered configurations (four warm repetitions each)", xlim=(0, 75))
ax.invert_yaxis(); ax.spines[["left", "bottom"]].set_visible(False); ax.tick_params(length=0)
from matplotlib.patches import Patch
ax.legend(handles=[Patch(color=c, label=l) for c,l in zip((TEAL,"#9dc7bd",CORAL,GRAY),
    ("At least 10% faster","0-10% faster / tie","Slower","Unranked"))],
    loc="upper center", bbox_to_anchor=(.48,1.3), ncol=2, frameon=False, fontsize=8)
save(fig,"coverage")

fig, ax = plt.subplots(figsize=(7,3.35), layout="constrained")
for (model, arm, title), color in zip(panels, COLORS):
    v=sorted(float(x["ratio_median"]) for x in summary if x["model"]==model and x["comparator"]==arm and x["complete"]=="True")
    ax.step(v, np.arange(1,len(v)+1)/len(v)*100, where="post", color=color, lw=1.8, label=f"{title} (n={len(v)})")
ax.axvline(1,color=NAVY,lw=.8);ax.axvline(.9,color=TEAL,lw=.8,ls="--")
ax.set(xscale="log",xlim=(.025,7),ylim=(0,102),xlabel="Median paired command-time ratio: native / reference (log scale)",ylabel="Share of comparable configurations (%)")
ax.set_xticks([.03,.1,.3,1,3,6]);ax.set_xticklabels(["0.03","0.1","0.3","1","3","6"])
ax.xaxis.set_minor_formatter(NullFormatter())
ax.grid(axis="y",alpha=.18);ax.legend(loc="upper left",fontsize=8,frameon=False)
save(fig,"distribution")

designs=["one_fe","two_fe_dense","two_fe_sparse","three_fe_sparse","strong_d2","strong_d3","strong_d6","weak_d3"]
labels=["One FE","Dense 2 FE","Sparse 2 FE","Sparse 3 FE","Strong d=2","Strong d=3","Strong d=6","Weak d=3"]
settings=[(61440,4),(61440,28),(983040,1),(983040,4),(983040,8),(983040,28),(3932160,4),(3932160,28)]
xlabels=["61k\nT4","61k\nT28","983k\nT1","983k\nT4","983k\nT8","983k\nT28","3.93m\nT4","3.93m\nT28"]
fig, axes=plt.subplots(3,1,figsize=(7,7.75),layout="constrained")
cmap=plt.get_cmap("BrBG_r").copy();cmap.set_bad(GRAY)
for ax,(model,arm,title) in zip(axes,panels):
    data=np.full((8,8),np.nan)
    for i,des in enumerate(designs):
        for j,(n,t) in enumerate(settings):
            row=next((r for r in summary if r["model"]==model and r["comparator"]==arm and r["section"]=="main" and r["design"]==des and int(r["n"])==n and int(r["threads"])==t),None)
            if row and row["complete"]=="True":data[i,j]=float(row["ratio_median"])
            label="--" if row is None else "NR" if row["complete"]!="True" else f"{data[i,j]:.2f}"
            if label == "NR" and model == "ols":
                failed = [p for p in pairs if p['phase']=='warm' and all(p[k]==row[k] for k in ('model','design','n','k','threads','section','comparator'))]
                label = "A" if any(p['status']=='accuracy_failure' for p in failed) else "T"
            ax.text(j,i,label,ha="center",va="center",fontsize=7.3,color="white" if math.isfinite(data[i,j]) and (data[i,j]<.25 or data[i,j]>3) else NAVY)
    ax.imshow(np.log2(data),cmap=cmap,norm=TwoSlopeNorm(0,-4,3),aspect="auto")
    ax.set(xticks=range(8),xticklabels=xlabels,yticks=range(8),yticklabels=labels,title=title)
    ax.tick_params(length=0,labelsize=8);ax.spines[:].set_visible(False)
save(fig,"heatmap")

fig,axes=plt.subplots(2,2,figsize=(7,4.5),layout="constrained")
for ax,des,title in zip(axes.flat,["one_fe","two_fe_dense","three_fe_sparse","strong_d3"],["One FE","Dense two FE","Sparse three FE","Strong d=3"]):
    for arm,color,label in [("feppml",NAVY,"feppml"),("fixest",CORAL,"fixest default"),("fixest_strict",TEAL,"fixest strict")]:
        vals=[]
        for t in (1,4,8,28):
            comparator=arm if arm!="feppml" else "fixest"
            eligibility=next(x for x in summary if x['model']=='ppml' and x['comparator']==comparator and x['design']==des and x['n']=='983040' and x['threads']==str(t) and x['section']=='main')
            rows=[x for x in runs if x["model"]=="ppml" and x["arm"]==arm and x["design"]==des and x["n"]=="983040" and x["section"]=="main" and x["threads"]==str(t) and x["phase"]=="warm" and x["status"]=="pass"]
            vals.append(median(float(x['seconds']) for x in rows) if len(rows)==4 and eligibility['complete']=='True' else np.nan)
        ax.plot((1,4,8,28),vals,marker="o",ms=3,lw=1.5,label=label,color=color)
    ax.set(xscale="log",yscale="log",title=title,xlabel="Requested threads",ylabel="Command seconds")
    ax.set_xticks([1,4,8,28]);ax.xaxis.set_major_formatter(ScalarFormatter());ax.xaxis.set_minor_formatter(NullFormatter())
    ticks={"one_fe":[2,3,4,5],"two_fe_dense":[4,6,10,20,30],"three_fe_sparse":[20,50,100,200,500],"strong_d3":[3,5,10,20]}[des]
    ax.set_yticks(ticks)
    ax.yaxis.set_major_formatter(FuncFormatter(lambda value,pos:f'{value:g}'));ax.yaxis.set_minor_formatter(NullFormatter());ax.grid(alpha=.15)
axes[0,0].legend(fontsize=7,frameon=False)
save(fig,"scaling")

fig,ax=plt.subplots(figsize=(7,2.5),layout="constrained")
specs=[('ols','slopes_fast','fixest','OLS fast / K10 / T28'),('ols','slopes_low','fixest','OLS low / K6 / T4'),('ppml','slopes_fast','fixest_strict','PPML fast / K10 / T28'),('ppml','slopes_low','fixest_strict','PPML low / K6 / T4')]
for i,(model,section,arm,label) in enumerate(specs):
    r=next(x for x in summary if x['model']==model and x['design']=='two_fe_dense' and x['section']==section and x['comparator']==arm)
    a,b,c=map(float,[r['ratio_min'],r['ratio_median'],r['ratio_max']])
    ax.plot([a,c],[i,i],color=CORAL if model=='ols' else TEAL,lw=2);ax.plot(b,i,'o',color=CORAL if model=='ols' else TEAL)
    ax.text(c*1.10,i,f'{b:.2f}x',va='center',fontsize=9)
ax.axvline(1,color=NAVY,lw=.8);ax.axvline(.9,color=TEAL,ls='--',lw=.8)
ax.set(xscale='log',xlim=(.2,7),yticks=range(4),yticklabels=[x[3] for x in specs],xlabel='Native / reference command time; point = median, line = min-max')
ax.set_xticks([.25,.5,1,2,4]);ax.set_xticklabels(['0.25','0.5','1','2','4']);ax.xaxis.set_minor_formatter(NullFormatter());ax.invert_yaxis();ax.spines['left'].set_visible(False);ax.tick_params(axis='y',length=0)
save(fig,'slopes')

fig,axes=plt.subplots(1,2,figsize=(7,2.7),layout='constrained')
for i,(arm,n) in enumerate([('fereg',276),('feppml',292)]):
    rows=[r for r in runs if r['arm']==arm];start=0
    for status,col in [('pass',TEAL),('command_timeout','#d89945'),('worker_failure',CORAL)]:
        count=sum(r['status']==status for r in rows);axes[0].barh(i,count,left=start,color=col,height=.5)
        if count>30:axes[0].text(start+count/2,i,str(count),ha='center',va='center',fontsize=8,color='white')
        start+=count
axes[0].set(yticks=[0,1],yticklabels=['OLS','PPML'],xlabel='Native attempts (cold + warm)',title='Passes and failures');axes[0].spines[['left','bottom']].set_visible(False)
axes[0].legend(handles=[Patch(color=c,label=l) for c,l in [(TEAL,'Pass'),('#d89945','Timeout'),(CORAL,'Failure')]],loc='upper center',bbox_to_anchor=(.5,-.22),ncol=3,fontsize=7,frameon=False)
for (model,arm,title),color in zip(panels,COLORS):
    values=[]
    for row in summary:
        if row['model']!=model or row['comparator']!=arm or row['complete']!='True':continue
        rr=[x for x in pairs if x['phase']=='warm' and all(x[k]==row[k] for k in ['model','design','n','k','threads','section','comparator'])]
        values.append(median(float(x['rss_ratio']) for x in rr))
    values.sort();axes[1].step(values,np.arange(1,len(values)+1)/len(values)*100,where='post',color=color,lw=1.4,label=title.replace('fixest','').replace(' / ','/'))
axes[1].axvline(1,color=NAVY,lw=.8);axes[1].set(xlabel='Native / reference peak RSS',ylabel='Comparable configurations (%)',title='Memory generally favors native',xlim=(.25,2.05));axes[1].legend(fontsize=6.7,frameon=False,loc='lower right')
save(fig,'reliability_memory')

fig,axes=plt.subplots(1,2,figsize=(7,3.1),layout='constrained')
for ax,model,arms in zip(axes,['ols','ppml'],[['reghdfe','areg','pyfixest','fixest'],['ppmlhdfe','pyfixest','fixest','fixest_strict']]):
    for arm,color in zip(arms,[TEAL,'#8064a2',CORAL,NAVY]):
        v=sorted(float(r['ratio_median']) for r in summary if r['model']==model and r['comparator']==arm and r['complete']=='True')
        ax.step(v,np.arange(1,len(v)+1)/len(v)*100,where='post',color=color,lw=1.4,label=f'{arm.replace("_strict"," strict")} (n={len(v)})')
    ax.axvline(1,color=GRAY,lw=1.2)
    ax.set(xscale='log',xlim=(.01,7),ylim=(0,102),title=model.upper(),xlabel='Native / reference time',ylabel='Comparable configurations (%)')
    ax.set_xticks([.01,.1,1,5]);ax.set_xticklabels(['0.01','0.1','1','5']);ax.xaxis.set_minor_formatter(NullFormatter())
    ax.legend(fontsize=7,frameon=False,loc='upper left');ax.grid(axis='y',alpha=.15)
save(fig,'all_comparators')
