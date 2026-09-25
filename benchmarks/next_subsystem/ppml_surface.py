"""Bounded positive-outcome PPML slopes and matched saved means, outside frozen cells."""
import datetime
import hashlib
import json
import math

import numpy as np
import pandas as pd

from surface import configure as configure_ols, replace


def configure(cell):
    if cell['model'] != 'ppml' or cell.get('terms', 'numeric') != 'numeric':
        raise ValueError('PPML surface requires numeric PPML terms')
    if cell.get('outputs', 'mu') != 'mu' or cell.get('vce', 'robust') != 'robust':
        raise ValueError('PPML surface currently requires saved means and robust inference')
    if not isinstance(cell.get('fixest_strict', False), bool):
        raise ValueError('Strict stock diagnostic flag must be Boolean')
    if 'fixest_glm_tol' in cell:
        tolerance = cell['fixest_glm_tol']
        if (not cell.get('fixest_strict') or isinstance(tolerance, bool)
                or not isinstance(tolerance, (int, float)) or not math.isfinite(tolerance)
                or not 0 < tolerance <= 1e-10):
            raise ValueError('Explicit stock outer tolerance may only strengthen the strict comparison')
    # Share only validated syntax construction, not OLS outcome or output policy.
    result = configure_ols(dict(cell, model='ols', outputs='none', vce='robust'))
    result.update(model='ppml', outputs='mu')
    return result


def augment(cell, path):
    frame = pd.read_stata(path, convert_categoricals=False)
    info = json.loads(path.with_suffix('.json').read_text())
    rng = np.random.default_rng(np.random.SeedSequence([info['seed'], 904]))
    for name in sorted({s for slopes in cell.get('slope_partitions', {}).values() for s in slopes}):
        frame[name] = rng.normal(size=len(frame))
    for effect, slopes in cell.get('slope_partitions', {}).items():
        for name in slopes:
            frame.y *= np.exp(.2 * np.sin(frame[effect]) * frame[name])
    if not (np.isfinite(frame.y) & (frame.y > 0)).all():
        raise ValueError('This surface requires positive finite outcomes; separation needs a separate oracle')
    frame.to_stata(path, write_index=False, version=118, time_stamp=datetime.datetime(2000,1,1))
    assert np.array_equal(frame.to_numpy(),pd.read_stata(path,convert_categoricals=False).to_numpy())
    info.update(ppml_surface=dict(outputs='mu',slope_partitions=cell.get('slope_partitions',{})),
                sha256=hashlib.sha256(path.read_bytes()).hexdigest(),columns=list(frame.columns),
                column_sha256={name:hashlib.sha256(np.asarray(frame[name],dtype='<f8').tobytes()).hexdigest() for name in frame})
    path.with_suffix('.json').write_text(json.dumps(info,indent=2)+'\n')


def stage(folder):
    path=folder/'measure.py';text=path.read_text()
    text=replace(text,'    maximum_rss = 0','''    if arm != "fixest":
        lines=[]
        for key in ("slope_absorb", "memory"):
            value=str(cell.get(key,""))
            if any(char in value for char in ('"', "\\n", "\\r")):
                raise ValueError("Unsafe PPML surface metadata")
            lines.append(f'local {key} "{value}"')
        (output / "surface-args.do").write_text("\\n".join(lines)+"\\n")
    maximum_rss = 0''');path.write_text(text)
    path=folder/'stata_worker.do';text=path.read_text()
    text=replace(text,'fit_options string_columns\n','fit_options string_columns\ninclude `"`output\'/surface-args.do"\'\n')
    text=replace(text,'local command `"`arm\' y','''if "`slope_absorb'" != "" local absorption absorb(`slope_absorb')
local fit_options `fit_options' memory(`memory')
local fit_options `fit_options' mu(saved_mu)
local command `"`arm' y''')
    text=replace(text,'    tempname values\n','''    mata: __sb_export_input(st_local("output") + "/means.bin", ("saved_mu"))
    tempname values
''');path.write_text(text)
    path=folder/'r_worker.R';text=path.read_text()
    text=replace(text,'paste(cell$effects, collapse=" + ")','cell$effects_r')
    text=replace(text,'fit_model <- function(data) {','''fit_model <- function(data) {
    if (isTRUE(cell$fixest_strict)) {
        return(fepois(formula, data=data, nthreads=cell$threads, vcov="hetero",
                      ssc=ssc(K.adj=FALSE), notes=FALSE,
                      glm.tol=if (is.null(cell$fixest_glm_tol)) 1e-10 else cell$fixest_glm_tol,
                      fixef.tol=1e-11))
    }''')
    text=replace(text,'invisible(fit_model(warm))','invisible(predict(fit_model(warm), type="response"))')
    text=replace(text,'seconds <- proc.time()[["elapsed"]] - start','''fit_seconds <- proc.time()[["elapsed"]] - start
saved_mu <- tryCatch(if (is.null(fit)) NULL else predict(fit, type="response"),
                     error=function(e) { error <<- conditionMessage(e); fit <<- NULL; NULL })
seconds <- proc.time()[["elapsed"]] - start''')
    text=replace(text,'R_version=as.character(getRversion())','fit_seconds=fit_seconds, R_version=as.character(getRversion())')
    text=replace(text,'    b <- coef(fit)','''    stopifnot(length(saved_mu)==nobs(fit), all(is.finite(saved_mu)), all(saved_mu>0))
    con <- file(file.path(output,"means.bin"),"wb")
    writeBin(as.double(saved_mu),con,size=8,endian="little")
    close(con)
    b <- coef(fit)''');path.write_text(text)
