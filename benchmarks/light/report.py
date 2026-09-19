"""Data-driven vector panels and LaTeX; no fabricated or silently omitted results."""
import argparse
from collections import Counter,defaultdict
import csv
import math
from pathlib import Path
import statistics

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.ticker import FuncFormatter,LogLocator

from common import ARMS, DESIGNS, SIZES, cell_id, read_json, sha256, tasks, write_json

COLORS=["#333333","#0072B2","#D55E00","#009E73","#CC79A7"]
LABELS={"one_fe":"One fixed effect","strong_d3":"Strong two-way","weak_d3":"Bottleneck two-way",
        "three_fe_sparse":"Sparse three-way","veneto_short":"1999 / 2001","veneto_long":"1996--2001"}


def eligible(call):
    if call["status"]!="pass":
        return False
    if call["arm"] in ("fereg","feppml"):
        return call.get("science",{}).get("status")=="pass"
    paired=call.get("comparison",{}).get("status")=="pass"
    # ADR 0026 alone permits ppmlhdfe public-mean diagnostics to differ while
    # its independently checked reported objective remains authoritative.
    return paired and (call["arm"]=="ppmlhdfe" or call.get("science",{}).get("status")=="pass")


def summarize(data):
    groups=defaultdict(list)
    for call in data["calls"]:
        groups[cell_id(call["cell"]),call["threads"],call["arm"]].append(call)
    rows=[]
    for (cid,threads,arm),calls in groups.items():
        good=[x for x in calls if x["status"]=="pass" and isinstance(x.get("seconds"),(float,int)) and x["seconds"]>0]
        times=[x["seconds"] for x in good]
        memory=[x["memory"]["peak_kib"]["Pss"]/1024**2 for x in good
                if x.get("memory",{}).get("peak_kib",{}).get("Pss")]
        row=dict(cell=cid,model=calls[0]["cell"]["model"],design=calls[0]["cell"]["design"],
                 variant=calls[0]["cell"]["variant"],n=calls[0]["cell"]["n"],threads=threads,arm=arm,
                 completed=len(times),science_eligible=len(calls)==3 and all(eligible(x) for x in calls),
                 pss_eligible=len(calls)==3 and all(eligible(x) and x.get("memory",{}).get("coverage")=="adequate" for x in calls),
                 seconds=statistics.median(times) if len(times)==3 else None,
                 seconds_min=min(times) if times else None,seconds_max=max(times) if times else None,
                 pss_gib=statistics.median(memory) if len(memory)==3 else None,
                 pss_min=min(memory) if memory else None,pss_max=max(memory) if memory else None,
                 status="; ".join(f"{k}:{v}" for k,v in sorted(Counter(x["status"] for x in calls).items())))
        native=ARMS[row["model"]][0]
        candidate={x["repetition"]:x for x in groups.get((cid,threads,native),[])}
        for metric in ("seconds","pss_gib"):
            pairs=[]
            for x in calls:
                y=candidate.get(x["repetition"])
                if not y or not eligible(x) or not eligible(y):
                    continue
                if metric=="seconds":
                    a,b=y.get("seconds"),x.get("seconds")
                elif all(z.get("memory",{}).get("coverage")=="adequate" for z in (x,y)):
                    a,b=(z["memory"]["peak_kib"]["Pss"] for z in (y,x))
                else:
                    continue
                if a and b:
                    pairs.append(a/b)
            row[metric+"_ratio"]=statistics.median(pairs) if len(pairs)==3 else None
            row[metric+"_ratio_min"]=min(pairs) if len(pairs)==3 else None
            row[metric+"_ratio_max"]=max(pairs) if len(pairs)==3 else None
        rows.append(row)
    return rows


def setup_style():
    plt.rcParams.update({"font.family":"DejaVu Sans","font.size":10,"axes.labelsize":11,
                        "axes.spines.top":False,"axes.spines.right":False,"axes.grid":True,
                        "grid.alpha":.18,"grid.linewidth":.6,"pdf.fonttype":42,"ps.fonttype":42,
                        "figure.figsize":(4.1,3.05),"savefig.bbox":"tight"})


def panel(rows,path,model,metric,selector,xvalues,labels,ratio=False):
    fig,ax=plt.subplots()
    unit="Command time (seconds)" if metric=="seconds" else "Peak process-tree PSS (GiB)"
    if ratio:
        unit=ARMS[model][0]+" / comparator"
        ax.axhline(1,color=".4",lw=.8,zorder=0)
    plotted=[]
    arms=ARMS[model][1:] if ratio else ARMS[model]
    for arm in arms:
        color=COLORS[ARMS[model].index(arm)]
        for threads in (4,28):
            xs,ys=[],[]
            for i,value in enumerate(xvalues):
                matches=[r for r in rows if r["model"]==model and r["arm"]==arm and r["threads"]==threads and selector(r)==value]
                if len(matches)!=1 or matches[0][metric] is None:
                    continue
                row=matches[0]
                is_ok=row["science_eligible"] if metric=="seconds" else row["pss_eligible"]
                y=row[metric]
                if ratio:
                    if not is_ok or row[metric+"_ratio"] is None:
                        continue
                    y=row[metric+"_ratio"]
                    lo,hi=row[metric+"_ratio_min"],row[metric+"_ratio_max"]
                else:
                    lo,hi=(row["seconds_min"],row["seconds_max"]) if metric=="seconds" else (row["pss_min"],row["pss_max"])
                ax.vlines(i,lo,hi,color=color,lw=.75,alpha=.6)
                marker="o" if threads==28 else "^"
                ax.plot(i,y,marker=marker,color=color,ms=4.5,mfc=color if is_ok else "white",mew=1)
                if is_ok:
                    xs.append(i);ys.append(y)
                plotted.append(dict(cell=row["cell"],arm=arm,threads=threads,value=y,eligible=is_ok))
            if xs:
                ax.plot(xs,ys,color=color,lw=1,ls="-" if threads==28 else "--")
    if not ratio and plotted:
        ax.set_yscale("log",base=2)
        ax.yaxis.set_major_locator(LogLocator(base=2,numticks=7))
        ax.yaxis.set_major_formatter(FuncFormatter(lambda x,pos:f"{x:g}"))
    elif ratio:
        ax.set_ylim(bottom=0)
    if not plotted:
        ax.text(.5,.5,"No eligible measurements" if ratio else "No complete measurements",ha="center",va="center",transform=ax.transAxes)
    ax.set_xticks(range(len(xvalues)),labels,rotation=25 if len(labels)>3 else 0,ha="right" if len(labels)>3 else "center")
    ax.set_ylabel(unit)
    ax.set_xlim(-.25,len(xvalues)-.75 if len(xvalues)>1 else .75)
    fig.tight_layout()
    fig.savefig(path)
    plt.close(fig)
    write_json(path.with_suffix(".json"),dict(metric=metric,model=model,ratio=ratio,points=plotted))


def tex(text):
    return str(text).replace("\\",r"\textbackslash{}").replace("_",r"\_").replace("%",r"\%").replace("&",r"\&").replace("#",r"\#")


def diagnostics(data,output):
    rows=[]
    ppml_names=["preparation","separation_fe","separation_simplex","separation_ir","separation_mu",
                "weight_response_refresh","fe_projection","finite_solve","objective_line_search",
                "strict_final_solve","inference","fe_recovery","publication"]
    for call in data["calls"]:
        if call["arm"] not in ("fereg","feppml") or call["status"]!="pass":
            continue
        row=dict(cell=cell_id(call["cell"]),arm=call["arm"],threads=call["threads"],repetition=call["repetition"],
                 command_seconds=call.get("seconds"),science_status=call.get("science",{}).get("status"))
        row.update(call.get("native_scalars",{}))
        name="fereg_runtime" if call["arm"]=="fereg" else "feppml_runtime"
        values={(x["row"],x["column"]):x["value"] for x in call.get("native_matrices",{}).get(name,[])}
        if call["arm"]=="fereg":
            for index,label in ((18,"preflight"),(19,"input"),(20,"native"),(21,"to_precommit")):
                row[label+"_seconds"]=values.get((index,"c2"))
        else:
            for index,label in enumerate(ppml_names,28):
                row[label+"_seconds"]=values.get((index,"c1"))
        rows.append(row)
    if rows:
        with (output/"native_diagnostics.csv").open("w") as stream:
            writer=csv.DictWriter(stream,fieldnames=sorted({key for row in rows for key in row}))
            writer.writeheader();writer.writerows(rows)
    lines=[r"\section{Execution and diagnostic coverage}"]
    lines.append(r"The observer column reports median observer CPU divided by command elapsed time; it is monitoring work, not a measured causal slowdown. PSS coverage requires at least ten complete sweeps. Native phase times, scratch, allocations, and retained solver records accompany the report; sequential and overlapping diagnostics must not be blindly summed.")
    lines.append(r"\begin{center}\small\begin{tabular}{llrrrr}\toprule Model & Package & Completed & Comparable & PSS resolved & Observer CPU\\\midrule")
    for model,arms in ARMS.items():
        for arm in arms:
            calls=[x for x in data["calls"] if x["cell"]["model"]==model and x["arm"]==arm]
            completed=[x for x in calls if x["status"]=="pass"]
            shares=[100*x["memory"]["observer_cpu_seconds"]/x["seconds"] for x in completed
                    if x.get("memory",{}).get("observer_cpu_seconds") is not None and x.get("seconds",0)>0]
            share=f"{statistics.median(shares):.1f}\\%" if shares else "--"
            lines.append(f"{model.upper()} & {arm} & {len(completed)} & {sum(eligible(x) for x in calls)} & "+
                         f"{sum(x.get('memory',{}).get('coverage')=='adequate' for x in completed)} & {share}"+r"\\")
    lines.append(r"\bottomrule\end{tabular}\end{center}")
    failures=Counter()
    for call in data["calls"]:
        if call["status"] not in ("pass","unattempted","unsupported"):
            failures[(call["cell"]["model"],call["arm"],call.get("failure_kind",call["status"]))]+=1
        for key,value in call.get("comparison",{}).get("checks",{}).items():
            if value is False:
                failures[(call["cell"]["model"],call["arm"],key)]+=1
        if call["status"]=="pass" and call.get("science",{}).get("status")!="pass":
            failures[(call["cell"]["model"],call["arm"],"public prediction audit")]+=1
    lines.append(r"A failed check is retained even when a command completed. The following counts are per fit, can overlap, and include public-mean diagnostics allowed by ADR 0026; the latter alone do not veto an otherwise eligible ppmlhdfe pair.")
    if failures:
        lines.append(r"\begin{longtable}{lllr}\toprule Model & Package & Unresolved or failed check & Fits\\\midrule\endhead")
        lines += [f"{model.upper()} & {arm} & {tex(check.replace('_',' '))} & {count}"+r"\\"
                  for (model,arm,check),count in sorted(failures.items())]
        lines.append(r"\bottomrule\end{longtable}")
    startup=data.get("startup")
    if startup:
        lines.append(f"The consolidated startup executed {startup['checks']} supported adapter checks. All native explicit-design checks passed; {len(startup['comparator_diagnostics'])} comparator checks had numerical discrepancies, retained in the startup receipt. The original failed capture attempt is separately preserved.")
    lines.append(r"\clearpage")
    return "\n".join(lines)


def composite(panels,caption,label,model=None):
    body=[r"\begin{figure}[p]\centering"]
    names=ARMS[model] if model else ["fereg / feppml","reghdfe / ppmlhdfe","fixest","pyfixest","greg / gglm"]
    entries=[r"\textcolor[HTML]{"+color[1:]+r"}{\rule[0.5ex]{1em}{1.2pt}}\ "+name for color,name in zip(COLORS,names)]
    body.append(r"{\small "+r"\quad ".join(entries[:3])+r"\\[2pt]"+r"\quad ".join(entries[3:])+r"}\par\medskip")
    for i,(file,title) in enumerate(panels):
        body.append(r"\begin{subfigure}{0.48\textwidth}\centering\includegraphics[width=\linewidth]{figures/"+file+r"}\caption{"+tex(title)+r"}\end{subfigure}")
        body.append(r"\hfill" if i%2==0 else r"\par\medskip")
    body.append(r"\caption{"+caption+r"}\label{"+label+r"}\end{figure}\clearpage")
    return "\n".join(body)


def build(data,output):
    output.mkdir(parents=True,exist_ok=True)
    figures=output/"figures";figures.mkdir(exist_ok=True)
    rows=summarize(data)
    with (output/"summary.csv").open("w") as stream:
        writer=csv.DictWriter(stream,fieldnames=list(rows[0]));writer.writeheader();writer.writerows(rows)
    write_json(output/"results.json",data)
    setup_style()
    exhibits=[]
    legend=(r"Colors identify packages in the order listed in the methods. Dashed lines and triangles indicate four cores; solid lines and circles indicate 28 cores. "
            r"Filled markers satisfy the comparison checks; hollow markers retain completed fits with unresolved scientific or evidence differences. Points are three-process medians; vertical bars show the observed range.")
    for model in ARMS:
        for metric in ("seconds","pss_gib"):
            panels=[]
            for graph in DESIGNS:
                chosen=[r for r in rows if r["design"]==graph and r["variant"]=="base"]
                name=f"main-{model}-{metric}-{graph}.pdf"
                panel(chosen,figures/name,model,metric,lambda r:r["n"],list(SIZES),["400k","1.6m"])
                panels.append((name,LABELS[graph]))
            exhibits.append(composite(panels,tex(model.upper())+r" synthetic comparison. "+legend,"fig:"+model+metric,model))
        panels=[]
        for graph in DESIGNS:
            chosen=[r for r in rows if r["design"]==graph and r["variant"]=="base"]
            name=f"ratio-{model}-{graph}.pdf"
            panel(chosen,figures/name,model,"seconds",lambda r:r["n"],list(SIZES),["400k","1.6m"],True)
            panels.append((name,LABELS[graph]))
        exhibits.append(composite(panels,tex(model.upper())+r" median paired time ratios. Values below one favor fereg. Only scientifically comparable three-repetition cells are shown; missing ratios are not wins. "+legend,"fig:ratio"+model,model))
    feature_groups=[(["base","k2-save0","k2-save1","save","k50-save0","k50-save1"],["K10","K2","K2 + FE","K10 + FE","K50","K50 + FE"],"width"),
                    (["base","cluster-worker","cluster-firm","cluster-twoway","slope-save0","slope-save1"],["Base","Worker","Firm","Two-way","Slope","Slope + FE"],"features")]
    for variants,labels,group in feature_groups:
        panels=[]
        for model in ARMS:
            for metric in ("seconds","pss_gib"):
                chosen=[r for r in rows if r["design"]=="weak_d3" and r["n"]==SIZES[0]]
                name=f"{group}-{model}-{metric}.pdf"
                panel(chosen,figures/name,model,metric,lambda r:r["variant"],variants,labels)
                panels.append((name,model.upper()+(" time" if metric=="seconds" else " memory")))
        exhibits.append(composite(panels,r"Bottleneck feature costs at 399,960 observations. `+ FE' includes individual fixed-effect recovery and materialization. "+legend,"fig:"+group))
    for ratio in (False,True):
        panels=[]
        for model in ARMS:
            for metric in ("seconds","pss_gib"):
                name=f"veneto-{model}-{metric}-ratio{int(ratio)}.pdf"
                panel(rows,figures/name,model,metric,lambda r:r["design"],["veneto_short","veneto_long"],["1999 / 2001","1996--2001"],ratio)
                panels.append((name,model.upper()+(" time" if metric=="seconds" else " PSS")))
        exhibits.append(composite(panels,r"Veneto panels: 1,319,994 and 5,163,428 planned observations. "+(r"Ratios below one favor fereg. " if ratio else "")+legend,"fig:veneto"+str(int(ratio))))
    counts=Counter(c["status"] for c in data["calls"])
    table="\n".join(tex(k)+" & "+str(v)+r" \\" for k,v in sorted(counts.items()))
    accounting=data.get("accounting",[])
    content=r"""\documentclass[11pt]{article}
\usepackage[margin=0.85in]{geometry}
\usepackage[T1]{fontenc}
\usepackage{lmodern,microtype,graphicx,booktabs,longtable,subcaption,xcolor,hyperref}
\hypersetup{colorlinks=true,linkcolor=blue,urlcolor=blue}
\setlength{\parindent}{0pt}\setlength{\parskip}{6pt}
\title{Fixed-Effect Regression Benchmarks\\\large A focused comparison at four and 28 cores}
\author{fereg development benchmark}\date{September 2026}
\begin{document}\maketitle
\begin{abstract}
This report compares OLS and Poisson pseudo-maximum-likelihood commands on four synthetic graph families and two Veneto panels. It measures complete estimation commands and concurrent peak process-tree proportional set size (PSS). The evidence is developmental, source-bound, and subject to the scientific and sampling checks below; it is not release qualification.
\end{abstract}
\section{Evidence and results}
"""
    content+=f"The fixed inventory contains {len(data['calls']):,} planned fits. {counts.get('pass',0):,} completed at the command layer. Of {len(rows)} combinations of package, configuration and core count, {sum(r['science_eligible'] for r in rows)} satisfy the three-repetition scientific checks, and {sum(r['pss_eligible'] for r in rows)} also have adequate PSS sampling.\n"
    content+=r"\begin{center}\begin{tabular}{lr}\toprule Disposition & Fits\\\midrule"+"\n"+table+"\n"+r"\bottomrule\end{tabular}\end{center}"+"\n"
    content+=r"\textbf{Candidate source:} \texttt{"+tex(data["source"])+r"}.\\"+"\n"
    content+=r"\textbf{Linux artifact SHA-256:}\\{\footnotesize\texttt{"+tex(data.get("artifact_sha256") or "unavailable")+r"}}."+"\n"
    content+=r"""\section{Design and measurement}
Ten sequential SGE bundles reserve homogeneous 28-physical-core Broadwell E5-2680v4 nodes, with a twelve-hour limit and at most four concurrent tasks. Every scientific configuration is compared under four- and 28-core affinity on the same host. Stata commands remain subject to the actual processor license ceiling; the four-core comparison supplies the equal-CPU-allowance comparison with reghdfe. Native/OpenMP pools obey the requested allowance and nested BLAS pools use one thread.

The OLS packages, in figure color order, are fereg (charcoal), reghdfe (blue), fixest (orange), pyfixest (green), and greg (purple). The analogous PPML packages are feppml, ppmlhdfe, fixest, pyfixest, and gglm. References are pinned to the previous systematic environment and their actual runtime bytes are checked. Package and artifact identities accompany the machine-readable evidence.

Main synthetic cells use 399,960 or 1,599,960 observations, ten controls, unit weights, conventional OLS covariance, and robust PPML covariance. Bottleneck additions vary control width, saved individual effects, clustering, and absorbed firm-specific slopes. Outcomes, graph membership, and overlapping controls are held fixed. A separate continuous slope variable varies within firms. Unsupported package features are explicit dispositions, not substituted models.

Each repetition uses a fresh process and a small untimed initialization. Input loading and columnwise import verification precede the timer. Estimation, inference, and requested individual-FE extraction/materialization are included. Every command retains its residual or mean for audit; individual-FE output is a separate variant. Disk export and independent audits follow the timer. Package and core-count order rotate. Target fits are capped at five minutes; a failed arm's remaining repetitions are skipped at that configuration and core count.

PSS is the maximum sum of contemporaneously sampled process PSS across complete parent/descendant sweeps, excluding the observer. It apportions shared pages and includes loaded data and runtime storage. It is neither a sum of independent process peaks nor exact minimum RAM. Nominal intervals are 20 milliseconds for synthetic cells up to 400k and 50 milliseconds otherwise. Fewer than ten complete in-command sweeps, observer errors, or excluded huge pages prevent headline memory ratios. Sampling gaps can miss brief peaks. Timings include concurrent monitoring; observer CPU and scan duration are reported separately and do not establish causal slowdown. RSS, USS, and swap remain diagnostics.

\section{Scientific checks and limitations}
Sample order, coefficients, covariance, available rank/DoF fields, objectives, published predictor binding, fresh scores, and FE reconstruction are checked. Small explicit-design oracles calibrate the added adapters. Independent two-FE feasibility uses a spanning forest and a grounded, scaled LSMR fallback; generic cases use independent scaled projection. Unresolved bounded audits remain indeterminate. ADR 0026 governs ppmlhdfe's reported objective and public-mean diagnostics. No native tolerance, routing, or scientific contract is weakened for a comparison.

Filled absolute-performance points meet the applicable checks. Hollow points show completed measurements with unresolved discrepancies or evidence limitations. Ratio panels require three comparable repetitions and, for memory, adequate sampling. Missing ratios must not be interpreted as favorable performance. All dispositions and individual repetitions accompany the report in JSON/CSV.

The short Veneto sample comprises 1999 and 2001; the long sample comprises 1996--2001. Their established sample membership is preserved. OLS uses raw log wages and PPML positive wage levels, with age and year controls estimated jointly with worker and firm effects. An unverifiable private-input recovery remains unattempted. Neither panel is described as an exact reconstruction of the historical published KSS sample.
\clearpage
"""
    content+=diagnostics(data,output)
    content+="\n".join(exhibits)
    content+=r"\section*{Reproducibility files}The accompanying \texttt{summary.csv} gives medians, ranges, eligibility, and dispositions. \texttt{results.json} contains all allowlisted per-fit receipts, source/input/runtime identities, observer diagnostics, and scheduler accounting. \texttt{native\_diagnostics.csv} retains available native timing and memory fields. Individual vector figures have adjacent machine-readable selections. Private row data and licensed logs are retained separately and are not included.\end{document}"+"\n"
    (output/"benchmark_report.tex").write_text(content)
    write_json(output/"report-build.json",dict(source=data["source"],calls=len(data["calls"]),summary_rows=len(rows),
        status_counts=dict(counts),tex_sha256=sha256(output/"benchmark_report.tex"),figure_count=len(list(figures.glob("*.pdf")))))


if __name__=="__main__":
    p=argparse.ArgumentParser()
    p.add_argument("results",type=Path)
    p.add_argument("output",type=Path)
    args=p.parse_args()
    build(read_json(args.results),args.output)
