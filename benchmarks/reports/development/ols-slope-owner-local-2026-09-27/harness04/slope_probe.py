"""Public OLS slope probe for the bounded Mac/Linux rotation.

Call run_case with an immutable input directory and an unused output directory.
All arms consume literally identical IEEE values; Stata logs remain outside Git.
"""
from pathlib import Path
import hashlib
import json
import os
import re
import signal
import subprocess
import sys
import shutil
import time

import numpy as np
import pandas as pd


def stata_threads(cell):
    threads = cell.get("stata_processors", cell["threads"])
    if not 1 <= threads <= cell["threads"]:
        raise ValueError("invalid explicit Stata control-plane capacity")
    return threads


def run_case(cell, data, output, arm, source, stock_library, diagnostic=False, monitor=None):
    output.mkdir(parents=True, exist_ok=False)
    env = {k: v for k, v in os.environ.items() if not k.startswith("FEREG_")}
    env.update(OPENBLAS_NUM_THREADS="1", VECLIB_MAXIMUM_THREADS="1", OMP_NUM_THREADS=str(cell["threads"]))
    if diagnostic:
        env["FEREG_PHASE_TIMING"] = "1"
    linux = sys.platform.startswith("linux")
    if linux:
        allocated = sorted(os.sched_getaffinity(0))
        if len(allocated) < cell["threads"] or int(env.get("NSLOTS", 0)) < cell["threads"]:
            raise ValueError("requested threads exceed allocated cores")
        prefix = ["taskset", "-c", ",".join(map(str, allocated[:cell["threads"]]))]
        timing = ["/usr/bin/time", "-v"]
    else:
        prefix, timing = [], ["/usr/bin/time", "-l"]
    env.update(MKL_NUM_THREADS="1", NUMEXPR_NUM_THREADS="1")
    k = cell["k"]
    if arm == "native":
        control_threads = stata_threads(cell)
        absorption = " ".join(e + ("##c." + cell["slopes"][e][0] if e in cell["slopes"] else "") for e in cell["effects"])
        command = f"fereg y x1-x{k}, absorb({absorption}) threads({cell['threads']}) memory({cell['memory']}) residuals(error)"
        script = f'''version 19.0
clear all
set more off
assert c(processors_lic)>={control_threads} & c(processors_mach)>={control_threads}
set processors {control_threads}
sysdir set PLUS "{output}"
sysdir set PERSONAL "{output}"
capture adopath - SITE
capture adopath - "."
adopath ++ "{source}/stata"
adopath ++ "{source}/dist"
use "{data}/input.dta", clear
{command}
drop error
timer on 91
{command}
generate double fitted = y-error
timer off 91
quietly timer list 91
scalar elapsed = r(t91)
scalar control_processors = c(processors)
scalar licensed_processors = c(processors_lic)
assert e(fereg_threads)=={cell['threads']}
generate byte sample=e(sample)
mata:
b=st_matrix("e(b)")[1,1..{k}]
V=st_matrix("e(V)")[1..{k},1..{k}]
fh=fopen("{output}/b.csv","w")
for (j=1;j<={k};j++) fput(fh,strofreal(b[j],"%24.17g"))
fclose(fh)
fh=fopen("{output}/V.csv","w")
for (j=1;j<={k};j++) fput(fh,invtokens(strofreal(V[j,.],"%24.17g"),","))
fclose(fh)
fh=fopen("{output}/meta.csv","w")
fput(fh,"seconds,N,rss,df_r,iterations,converged,estimator_threads,stata_processors,processors_lic")
m=(st_numscalar("elapsed"),st_numscalar("e(N)"),st_numscalar("e(rss)"),st_numscalar("e(df_r)"),st_numscalar("e(ic)"),st_numscalar("e(converged)"),st_numscalar("e(fereg_threads)"),st_numscalar("control_processors"),st_numscalar("licensed_processors"))
fput(fh,invtokens(strofreal(m,"%24.17g"),","))
fclose(fh)
end
keep obs_id sample fitted error
export delimited using "{output}/rows.csv", replace
display "OLS_SLOPE_PROBE_SUCCESS"
exit 0
'''
        (output / "fit.do").write_text(script)
        command = [shutil.which("stata-mp") if linux else "/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp", "-b", "do", str(output / "fit.do")]
    else:
        effects = " + ".join(e + ("[" + cell["slopes"][e][0] + "]" if e in cell["slopes"] else "") for e in cell["effects"])
        strict = ", fixef.tol=1e-11" if arm == "strict" else ""
        script = f'''.libPaths(c("{stock_library}", .libPaths()))
library(fixest)
names <- readLines("{data}/columns.txt")
values <- readBin("{data}/input.bin",double(),n=file.info("{data}/input.bin")$size/8,size=8,endian="little")
d <- as.data.frame(matrix(values,ncol=length(names))); names(d) <- names
f <- as.formula("y ~ {' + '.join('x'+str(j) for j in range(1,k+1))} | {effects}")
run <- function() {{ fit <- feols(f,d,nthreads={cell['threads']},notes=FALSE{strict}); rows <- data.frame(obs_id=d$obs_id[obs(fit)],sample=1,fitted=fitted(fit),error=resid(fit)); list(fit=fit,rows=rows) }}
warm <- run()
elapsed <- system.time(answer <- run())[["elapsed"]]
fit <- answer$fit
write.table(coef(fit),"{output}/b.csv",sep=",",row.names=FALSE,col.names=FALSE)
write.table(unname(vcov(fit)),"{output}/V.csv",sep=",",row.names=FALSE,col.names=FALSE)
write.csv(data.frame(seconds=elapsed,N=nobs(fit),rss=sum(resid(fit)^2),df_r=nobs(fit)-attr(vcov(fit,attr=TRUE),"df.K"),iterations=max(fit$iterations),converged=is.null(fit$convStatus)||fit$convStatus),"{output}/meta.csv",row.names=FALSE)
write.csv(answer$rows,"{output}/rows.csv",row.names=FALSE)
cat("OLS_SLOPE_PROBE_SUCCESS\\n")
'''
        (output / "fit.R").write_text(script)
        command = [shutil.which("Rscript") if linux else "/opt/homebrew/bin/Rscript", str(output / "fit.R")]
    started = time.monotonic()
    timed_out = False
    competing = []
    with (output / "stdout.txt").open("w") as out, (output / "stderr.txt").open("w") as err:
        process = subprocess.Popen([*timing, *prefix, *command], cwd=output, env=env,
                                   stdout=out, stderr=err, start_new_session=True)
        while True:
            try:
                process.wait(timeout=.25 if monitor else 180)
                break
            except subprocess.TimeoutExpired:
                if monitor:
                    competing.extend(monitor(process.pid))
                if time.monotonic()-started >= 180:
                    timed_out = True
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
                    break
    log_path = output / ("fit.log" if arm == "native" else "stdout.txt")
    log = log_path.read_text(errors="replace") if log_path.exists() else ""
    result = dict(arm=arm, diagnostic=diagnostic, process_seconds=time.monotonic()-started,
                  process_rc=process.returncode, timed_out=timed_out, competing_work=sorted(set(competing)),
                  sentinel=any(line.strip()=="OLS_SLOPE_PROBE_SUCCESS" for line in log.splitlines()))
    if result["sentinel"]:
        result["metadata"] = pd.read_csv(output / "meta.csv").iloc[0].to_dict()
    timing_text = (output / "stderr.txt").read_text()
    peak = re.search(r"Maximum resident set size \(kbytes\):\s*(\d+)", timing_text) if linux else re.search(r"(\d+)\s+maximum resident set size", timing_text)
    result["process_peak_rss_bytes"] = int(peak.group(1)) * (1024 if linux else 1) if peak else None
    result["controller_sha256"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    result["files"] = {p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in output.iterdir() if p.is_file()}
    (output / "receipt.json").write_text(json.dumps(result, indent=2, allow_nan=False)+"\n")
    return result
