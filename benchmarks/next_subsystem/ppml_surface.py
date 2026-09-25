"""Additive positive-outcome PPML comparisons, separate from frozen definitions."""
import datetime
import hashlib
import json
import math

import numpy as np
import pandas as pd

from surface import configure as configure_ols, replace


def configure(cell):
    if cell['model'] != 'ppml' or cell.get('terms', 'numeric') not in ('numeric', 'factor', 'factor_cross'):
        raise ValueError('Unsupported PPML generated-term surface')
    outputs = cell.get('outputs', 'mu')
    if outputs not in ('mu', 'all'):
        raise ValueError('PPML surface requires mu or all outputs')
    if outputs == 'all' and cell.get('slope_partitions'):
        raise ValueError('Saved slope channels require a separate comparison')
    vce = cell.get('vce', 'robust')
    clusters = vce.split()[1:] if vce.startswith('cluster ') else []
    if vce != 'robust' and (not clusters or len(clusters) > 2 or len(set(clusters)) != len(clusters)
                           or any(name not in cell['effects'] + ['cl1', 'cl2'] for name in clusters)):
        raise ValueError('PPML surface supports robust or one/two declared cluster dimensions')
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
    result.update(model='ppml', outputs=outputs, vce=vce, clusters=clusters,
                  output_names=['saved_xb', 'saved_d', 'saved_eta', 'saved_resid'] +
                               ['saved_' + name for name in cell['effects']],
                  saved_absorb=' '.join('saved_' + name + '=' + name for name in cell['effects']))
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
    # Added terms have their own stream and positive multiplicative outcome
    # contribution; existing numeric/slope designs remain byte-for-byte unchanged.
    rng = np.random.default_rng(np.random.SeedSequence([info['seed'], 903]))
    if cell['terms'] in ('factor', 'factor_cross'):
        levels = cell.get('factor_levels', 4)
        probabilities = np.ones(levels)
        if cell.get('factor_balance') == 'unbalanced':
            probabilities = 1 / np.arange(1, levels + 1)
        elif cell.get('factor_balance', 'balanced') != 'balanced':
            raise ValueError('Unknown factor balance')
        frame['f'] = rng.choice(np.arange(1, levels + 1), len(frame), p=probabilities/probabilities.sum())
        frame.y *= np.exp(.04 * (frame.f - 1))
        if cell['terms'] == 'factor_cross':
            frame['h'] = rng.integers(1, cell.get('cross_levels', 3) + 1, len(frame))
            frame.y *= np.exp(-.03 * (frame.h - 1) + .01 * (frame.f - 1) * (frame.h - 1))
    rng = np.random.default_rng(np.random.SeedSequence([info['seed'], 905]))
    for name, groups in [('cl1', 37), ('cl2', 53)]:
        if name in cell['clusters']:
            frame[name] = rng.integers(1, groups + 1, len(frame))
    if not (np.isfinite(frame.y) & (frame.y > 0)).all():
        raise ValueError('This surface requires positive finite outcomes; separation needs a separate oracle')
    frame.to_stata(path, write_index=False, version=118, time_stamp=datetime.datetime(2000,1,1))
    assert np.array_equal(frame.to_numpy(),pd.read_stata(path,convert_categoricals=False).to_numpy())
    info.update(ppml_surface=dict(outputs=cell['outputs'],terms=cell['terms'],vce=cell['vce'],
                                 slope_partitions=cell.get('slope_partitions',{})),
                sha256=hashlib.sha256(path.read_bytes()).hexdigest(),columns=list(frame.columns),
                column_sha256={name:hashlib.sha256(np.asarray(frame[name],dtype='<f8').tobytes()).hexdigest() for name in frame})
    path.with_suffix('.json').write_text(json.dumps(info,indent=2)+'\n')


def stage(folder):
    path=folder/'measure.py';text=path.read_text()
    text=replace(text,'    maximum_rss = 0','''    if arm != "fixest":
        lines=[]
        for key in ("slope_absorb", "memory", "rhs", "export_names", "outputs", "saved_absorb"):
            value=str(cell.get(key,""))
            if any(char in value for char in ('"', "\\n", "\\r")):
                raise ValueError("Unsafe PPML surface metadata")
            lines.append(f'local {key} "{value}"')
        (output / "surface-args.do").write_text("\\n".join(lines)+"\\n")
    maximum_rss = 0''');path.write_text(text)
    path=folder/'stata_worker.do';text=path.read_text()
    text=replace(text,'fit_options string_columns\n','fit_options string_columns\ninclude `"`output\'/surface-args.do"\'\n')
    text=replace(text,'local effects\n','local xvars `"`rhs\'"\'\nlocal effects\n')
    text=replace(text,'local command `"`arm\' y','''if "`slope_absorb'" != "" local absorption absorb(`slope_absorb')
local fit_options `fit_options' memory(`memory')
local fit_options `fit_options' mu(saved_mu)
if "`outputs'" == "all" {
    local absorption absorb(`saved_absorb')
    local fit_options `fit_options' xb(saved_xb) d(saved_d) eta(saved_eta) residuals(saved_resid)
}
local command `"`arm' y''')
    text=replace(text,'    tempname values\n','''    mata: __sb_export_input(st_local("output") + "/means.bin", ("saved_mu"))
    if "`outputs'" == "all" {
        local saved_channels saved_xb saved_d saved_eta saved_resid
        foreach effect of local effects {
            local saved_channels `saved_channels' saved_`effect'
        }
        mata: __sb_export_input(st_local("output") + "/outputs.bin", tokens(st_local("saved_channels")))
    }
    local k : word count `export_names'
    tempname values
''')
    text=replace(text,'        local b = __sb_b[1,`j\']','''        local name : word `j' of `export_names'
        local bj = colnumb(__sb_b, "`name'")
        assert `bj' < .
        local b = __sb_b[1,`bj']''')
    text=replace(text,"sqrt(__sb_V[`j',`j'])","sqrt(__sb_V[`bj',`bj'])")
    text=replace(text,'"x`j\'," %24.17g (`b\')','"`name\'," %24.17g (`b\')')
    text=replace(text,'''                file write `values' "x`j',x`h'," %24.17g (__sb_V[`j',`h']) _n''','''                local jname : word `j' of `export_names'
                local hname : word `h' of `export_names'
                local bj = colnumb(__sb_b, "`jname'")
                local bh = colnumb(__sb_b, "`hname'")
                file write `values' "`jname',`hname'," %24.17g (__sb_V[`bj',`bh']) _n''')
    path.write_text(text)
    path=folder/'r_worker.R';text=path.read_text()
    text=replace(text,'paste(paste0("x", seq_len(cell$k)), collapse=" + ")','cell$rhs_r')
    text=replace(text,'paste(cell$effects, collapse=" + ")','cell$effects_r')
    text=replace(text,'fit_model <- function(data) {','''vce <- if (length(cell$clusters)) as.formula(paste("~", paste(cell$clusters, collapse=" + "))) else "hetero"
correction_spec <- ssc(K.adj=FALSE, G.adj=TRUE, G.df="min")
fit_model <- function(data) {
    if (isTRUE(cell$fixest_strict)) {
        return(fepois(formula, data=data, nthreads=cell$threads, vcov=vce,
                      ssc=correction_spec, notes=FALSE,
                      glm.tol=if (is.null(cell$fixest_glm_tol)) 1e-10 else cell$fixest_glm_tol,
                      fixef.tol=1e-11))
    }''')
    text=replace(text,'vcov="hetero", ssc=ssc(K.adj=FALSE), notes=FALSE','vcov=vce, ssc=correction_spec, notes=FALSE')
    text=replace(text,'if (c$phase == "warm") {','''recover_outputs <- function(fit, data) {
    mu <- predict(fit, type="response")
    if (cell$outputs != "all") return(list(mu=mu, channels=NULL))
    eta <- predict(fit, type="link")
    X <- model.matrix(fit, type="rhs")[,cell$r_coefficient_names,drop=FALSE]
    xb <- as.vector(X %*% coef(fit)[cell$r_coefficient_names])
    effects <- fixef(fit, sorted=FALSE, notes=FALSE, nthreads=cell$threads, fixef.tol=1e-11)
    contributions <- lapply(cell$effects, function(name) unname(effects[[name]][fit$fixef_id[[name]]]))
    channels <- c(list(xb, eta-xb, eta, data$y[obs(fit)]-mu), contributions)
    list(mu=mu, channels=channels)
}
if (c$phase == "warm") {''')
    text=replace(text,'invisible(fit_model(warm))','invisible(recover_outputs(fit_model(warm), warm))')
    text=replace(text,'seconds <- proc.time()[["elapsed"]] - start','''fit_seconds <- proc.time()[["elapsed"]] - start
saved <- tryCatch(if (is.null(fit)) NULL else recover_outputs(fit, data),
                     error=function(e) { error <<- conditionMessage(e); fit <<- NULL; NULL })
saved_mu <- saved$mu
seconds <- proc.time()[["elapsed"]] - start''')
    text=replace(text,'R_version=as.character(getRversion())','fit_seconds=fit_seconds, R_version=as.character(getRversion())')
    text=replace(text,'    b <- coef(fit)','''    stopifnot(length(saved_mu)==nobs(fit), all(is.finite(saved_mu)), all(saved_mu>0))
    con <- file(file.path(output,"means.bin"),"wb")
    writeBin(as.double(saved_mu),con,size=8,endian="little")
    close(con)
    if (!is.null(saved$channels)) {
        con <- file(file.path(output,"outputs.bin"),"wb")
        for (values in saved$channels) writeBin(as.double(values),con,size=8,endian="little")
        close(con)
    }
    b <- coef(fit)[cell$r_coefficient_names]
    stopifnot(all(is.finite(b)))
    names(b) <- cell$coefficient_names''')
    text=replace(text,'    standard_errors <- se(fit)','    standard_errors <- se(fit)[cell$r_coefficient_names]')
    text=replace(text,'if (cell$model == "ppml") nobs(fit)/(nobs(fit)-1) else 1','if (length(cell$clusters)) 1 else nobs(fit)/(nobs(fit)-1)')
    text=replace(text,'if (cell$model == "ppml") "HC0 multiplied by N/(N-1)" else "native IID"','if (length(cell$clusters)) "CGM common Gmin/(Gmin-1), no K adjustment" else "HC0 multiplied by N/(N-1)"')
    text=replace(text,'    V <- vcov(fit) * correction','''    V <- vcov(fit)[cell$r_coefficient_names,cell$r_coefficient_names,drop=FALSE] * correction
    dimnames(V) <- list(cell$coefficient_names,cell$coefficient_names)''')
    path.write_text(text)
