"""Bounded Mac public-command probe; diagnostic clocks are kept separate.

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
import time

import numpy as np
import pandas as pd

REPO = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO / "benchmarks/systematic"))
from generate import generate

STATA = "/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp"
R_STOCK = REPO / "target/performance-repair-20260927/ols-adjudication/R-stock"


def prepare(case, seed, directory):
    directory.mkdir(parents=True, exist_ok=False)
    manifest = json.loads((REPO / "benchmarks/systematic/manifest-light-v2.json").read_text())
    cell = dict(case, model="ols", section="slopes",
                effects=manifest["designs"][case["design"]]["effects"],
                slopes={"g1": ["z1"], "g2": ["z2"]})
    info = generate(cell, seed, directory / "input.dta", manifest)
    frame = pd.read_stata(directory / "input.dta", convert_categoricals=False)
    (directory / "input.bin").write_bytes(b"".join(np.asarray(frame[c], dtype="<f8").tobytes() for c in frame))
    (directory / "columns.txt").write_text("\n".join(frame.columns) + "\n")
    return cell, info


def run_case(cell, data, output, arm, plugin_directory=None, diagnostic=False, monitor=None):
    output.mkdir(parents=True, exist_ok=False)
    env = {k: v for k, v in os.environ.items() if not k.startswith("FEREG_")}
    env.update(OPENBLAS_NUM_THREADS="1", VECLIB_MAXIMUM_THREADS="1", OMP_NUM_THREADS=str(cell["threads"]))
    if diagnostic:
        env["FEREG_PHASE_TIMING"] = "1"
    k = cell["k"]
    if arm == "native":
        absorption = " ".join(e + ("##c." + cell["slopes"][e][0] if e in cell["slopes"] else "") for e in cell["effects"])
        command = f"fereg y x1-x{k}, absorb({absorption}) threads({cell['threads']}) memory({cell['memory']}) residuals(error)"
        script = f'''version 19.0
clear all
set more off
set processors `=min({cell['threads']},c(processors_lic),c(processors_mach))'
sysdir set PLUS "{output}"
sysdir set PERSONAL "{output}"
capture adopath - SITE
capture adopath - "."
adopath ++ "{REPO}/stata"
adopath ++ "{plugin_directory}"
use "{data}/input.dta", clear
{command}
drop error
timer on 91
{command}
generate double fitted = y-error
timer off 91
quietly timer list 91
scalar elapsed = r(t91)
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
fput(fh,"seconds,N,rss,df_r,iterations,converged")
m=(st_numscalar("elapsed"),st_numscalar("e(N)"),st_numscalar("e(rss)"),st_numscalar("e(df_r)"),st_numscalar("e(ic)"),st_numscalar("e(converged)"))
fput(fh,invtokens(strofreal(m,"%24.17g"),","))
fclose(fh)
end
keep obs_id sample fitted error
export delimited using "{output}/rows.csv", replace
display "OLS_SLOPE_PROBE_SUCCESS"
exit 0
'''
        (output / "fit.do").write_text(script)
        command = [STATA, "-b", "do", str(output / "fit.do")]
    else:
        effects = " + ".join(e + ("[" + cell["slopes"][e][0] + "]" if e in cell["slopes"] else "") for e in cell["effects"])
        strict = ", fixef.tol=1e-11" if arm == "strict" else ""
        script = f'''.libPaths(c("{R_STOCK}", .libPaths()))
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
        command = ["/opt/homebrew/bin/Rscript", str(output / "fit.R")]
    started = time.monotonic()
    timed_out = False
    competing = []
    with (output / "stdout.txt").open("w") as out, (output / "stderr.txt").open("w") as err:
        process = subprocess.Popen(["/usr/bin/time", "-l", *command], cwd=output, env=env,
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
    peak = re.search(r"(\d+)\s+maximum resident set size", (output / "stderr.txt").read_text())
    result["process_peak_rss_bytes"] = int(peak.group(1)) if peak else None
    result["controller_sha256"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    result["files"] = {p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in output.iterdir() if p.is_file()}
    (output / "receipt.json").write_text(json.dumps(result, indent=2, allow_nan=False)+"\n")
    return result


if __name__ == "__main__":
    registration = json.loads((Path(__file__).parent / "registration.json").read_text())
    root = Path(sys.argv[1]).resolve()
    for case in registration["cases"]:
        data = root / "inputs" / case["id"]
        cell, _ = prepare(case, registration["pilot_seed"], data)
        (data / "cell.json").write_text(json.dumps(cell,indent=2)+"\n")
        print(case["id"], run_case(cell, data, root / "baseline-diagnostic" / case["id"],
              "native", root / "baseline/dist", True), flush=True)
