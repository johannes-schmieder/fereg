"""Additive matched OLS generated-term/output surface, separate from frozen cells."""
import datetime
import hashlib
import json

import numpy as np
import pandas as pd


def replace(text, before, after):
    assert text.count(before) == 1, before
    return text.replace(before, after)


def configure(cell):
    if cell['model'] != 'ols' or any(cell.get(k) for k in ('weight', 'absorb', 'restriction', 'fit_options', 'string_columns')):
        raise ValueError('Surface worker supports unweighted unrestricted OLS only')
    if cell.get('vce', '') not in ('', 'iid', 'robust'):
        raise ValueError('Surface worker supports IID/robust inference only')
    mode = cell.get('terms', 'numeric')
    if mode not in ('numeric', 'factor', 'product', 'factor_cross'):
        raise ValueError('Unknown generated-term surface')
    if cell.get('outputs', 'none') not in ('none', 'all'):
        raise ValueError('Unknown output request')
    if cell.get('memory', 'fast') not in ('fast', 'low'):
        raise ValueError('Surface worker supports explicit fast/low memory only')
    if cell.get('outputs') == 'all' and len(cell['effects']) != 1:
        raise ValueError('Matched normalized saved outputs currently require one FE')
    names = [f'x{i}' for i in range(1, cell['k'] + 1)]
    r_names = list(names)
    stata_rhs, r_rhs = list(names), list(names)
    if mode == 'product':
        if cell['k'] < 2:
            raise ValueError('Product requires two numeric sources')
        stata_rhs.append('c.x1#c.x2'); r_rhs.append('I(x1*x2)')
        names.append('c.x1#c.x2'); r_names.append('I(x1 * x2)')
    if mode in ('factor', 'factor_cross'):
        levels = cell.get('factor_levels', 4)
        if not 2 <= levels <= 16:
            raise ValueError('Factor width outside bounded surface')
        stata_rhs.append('i.f' if mode == 'factor' else 'i.f##i.h')
        r_rhs.append('factor(f)' if mode == 'factor' else 'factor(f)*factor(h)')
        names += [f'{v}.f' for v in range(2, levels + 1)]
        r_names += [f'factor(f){v}' for v in range(2, levels + 1)]
        if mode == 'factor_cross':
            other = cell.get('cross_levels', 3)
            if not 2 <= other <= 8:
                raise ValueError('Interaction width outside bounded surface')
            names += [f'{v}.h' for v in range(2, other + 1)]
            r_names += [f'factor(h){v}' for v in range(2, other + 1)]
            names += [f'{f}.f#{h}.h' for f in range(2, levels + 1) for h in range(2, other + 1)]
            r_names += [f'factor(f){f}:factor(h){h}' for f in range(2, levels + 1) for h in range(2, other + 1)]
    cell.update(rhs=' '.join(stata_rhs), rhs_r=' + '.join(r_rhs),
                coefficient_names=names, r_coefficient_names=r_names,
                export_names=' '.join(names), outputs=cell.get('outputs', 'none'),
                vce=cell.get('vce') or 'iid', memory=cell.get('memory', 'fast'))
    return cell


def augment(cell, path):
    frame = pd.read_stata(path, convert_categoricals=False)
    info = json.loads(path.with_suffix('.json').read_text())
    rng = np.random.default_rng(np.random.SeedSequence([info['seed'], 903]))
    if cell.get('terms') in ('factor', 'factor_cross'):
        levels = cell.get('factor_levels', 4)
        probabilities = np.ones(levels)
        if cell.get('factor_balance') == 'unbalanced':
            probabilities = 1 / np.arange(1, levels + 1)
        elif cell.get('factor_balance', 'balanced') != 'balanced':
            raise ValueError('Unknown factor balance')
        frame['f'] = rng.choice(np.arange(1, levels + 1), len(frame), p=probabilities/probabilities.sum())
        if cell['terms'] == 'factor_cross':
            frame['h'] = rng.integers(1, cell.get('cross_levels', 3) + 1, len(frame))
    frame.to_stata(path, write_index=False, version=118, time_stamp=datetime.datetime(2000, 1, 1))
    assert np.array_equal(frame.to_numpy(), pd.read_stata(path, convert_categoricals=False).to_numpy())
    info.update(surface={k:cell[k] for k in ('terms','outputs','rhs','coefficient_names') if k in cell},
        sha256=hashlib.sha256(path.read_bytes()).hexdigest(), columns=list(frame.columns),
        column_sha256={name:hashlib.sha256(np.asarray(frame[name],dtype='<f8').tobytes()).hexdigest() for name in frame})
    path.with_suffix('.json').write_text(json.dumps(info,indent=2)+'\n')


def columns(frame, cell):
    values = [frame[f'x{i}'].to_numpy() for i in range(1, cell['k'] + 1)]
    if cell.get('terms') == 'product':
        values.append(frame.x1.to_numpy()*frame.x2.to_numpy())
    if cell.get('terms') in ('factor','factor_cross'):
        levels = cell.get('factor_levels',4)
        values += [(frame.f.to_numpy()==v).astype(float) for v in range(2,levels+1)]
        if cell['terms']=='factor_cross':
            other=cell.get('cross_levels',3)
            values += [(frame.h.to_numpy()==v).astype(float) for v in range(2,other+1)]
            values += [((frame.f.to_numpy()==f)&(frame.h.to_numpy()==h)).astype(float)
                       for f in range(2,levels+1) for h in range(2,other+1)]
    return np.column_stack(values)


def stage(folder):
    path=folder/'measure.py'; text=path.read_text()
    text=replace(text,'("weight", "vce", "absorb", "restriction", "fit_options", "string_columns"))',
                 '("weight", "vce", "absorb", "restriction", "fit_options", "string_columns", "rhs", "export_names", "outputs", "memory"))')
    path.write_text(text)
    path=folder/'stata_worker.do'; text=path.read_text()
    text=replace(text,'fit_options string_columns\n','fit_options string_columns rhs export_names outputs memory\n')
    text=replace(text,'local effects\n','if "`rhs\'" != "" local xvars `"`rhs\'"\'\nlocal effects\n')
    text=replace(text,'local command `"`arm\' y', '''if "`memory'" == "low" local fit_options `fit_options' memory(low)
if "`outputs'" == "all" {
    local absorption absorb(saved_fe=g1)
    local fit_options `fit_options' residuals(saved_resid)
}
local command `"`arm' y''')
    text=replace(text,'    local warm_rc = _rc', '''    local warm_rc = _rc
    if !`warm_rc' & "`outputs'" == "all" {
        capture quietly predict double saved_xb, xb
        local warm_rc = _rc
        if !`warm_rc' {
            capture quietly predict double saved_xbd, xbd
            local warm_rc = _rc
        }
    }''')
    text=replace(text,'local seconds = r(t91)', '''local fit_seconds = r(t91)
local seconds = `fit_seconds'
if !`rc' & "`outputs'" == "all" {
    timer clear 92
    timer on 92
    capture quietly predict double saved_xb, xb
    local rc = _rc
    if !`rc' {
        capture quietly predict double saved_xbd, xbd
        local rc = _rc
    }
    timer off 92
    quietly timer list 92
    local seconds = `fit_seconds' + r(t92)
}''')
    text=replace(text,'file write `meta\' "stata_processors"', 'file write `meta\' "fit_seconds" _tab %24.17g (`fit_seconds\') _n\nfile write `meta\' "stata_processors"')
    text=replace(text,'    tempname values\n', '''    if "`outputs'" == "all" {
        mata: __sb_export_input(st_local("output") + "/outputs.bin", ("saved_fe", "saved_resid", "saved_xb", "saved_xbd"))
    }
    local k : word count `export_names'
    tempname values
''')
    text=replace(text,'        local b = __sb_b[1,`j\']', '''        local name : word `j' of `export_names'
        local bj = colnumb(__sb_b, "`name'")
        assert `bj' < .
        local b = __sb_b[1,`bj']''')
    text=replace(text,"sqrt(__sb_V[`j',`j'])","sqrt(__sb_V[`bj',`bj'])")
    text=replace(text,'"x`j\'," %24.17g (`b\')','"`name\'," %24.17g (`b\')')
    text=replace(text,'''                file write `values' "x`j',x`h'," %24.17g (__sb_V[`j',`h']) _n''', '''                local jname : word `j' of `export_names'
                local hname : word `h' of `export_names'
                local bj = colnumb(__sb_b, "`jname'")
                local bh = colnumb(__sb_b, "`hname'")
                file write `values' "`jname',`hname'," %24.17g (__sb_V[`bj',`bh']) _n''')
    path.write_text(text)
    path=folder/'r_worker.R'; text=path.read_text()
    text=replace(text,'paste(paste0("x", seq_len(cell$k)), collapse=" + ")','cell$rhs_r')
    text=replace(text,'vcov="iid", notes=FALSE','vcov=if(cell$vce == "robust") "hetero" else "iid",\n              ssc=ssc(K.adj=TRUE, K.fixef="nonnested", G.adj=TRUE, G.df="min", t.df="min"), notes=FALSE')
    text=replace(text,'if (c$phase == "warm") {','''recover_outputs <- function(fit) {
    if (is.null(fit) || cell$outputs != "all") return(NULL)
    effects <- fixef(fit, sorted=FALSE, notes=FALSE)$g1
    contribution <- unname(effects[fit$fixef_id$g1])
    intercept <- mean(contribution)
    list(contribution-intercept, resid(fit), fitted(fit)-contribution+intercept, fitted(fit))
}
if (c$phase == "warm") {''')
    text=replace(text,'invisible(fit_model(warm))','invisible(recover_outputs(fit_model(warm)))')
    text=replace(text,'seconds <- proc.time()[["elapsed"]] - start','''fit_seconds <- proc.time()[["elapsed"]] - start
saved_outputs <- tryCatch(recover_outputs(fit), error=function(e) { error <<- conditionMessage(e); fit <<- NULL; NULL })
seconds <- proc.time()[["elapsed"]] - start''')
    text=replace(text,'R_version=as.character(getRversion())','fit_seconds=fit_seconds, R_version=as.character(getRversion())')
    text=replace(text,'    b <- coef(fit)','''    b <- coef(fit)[cell$r_coefficient_names]
    stopifnot(all(is.finite(b)))
    names(b) <- cell$coefficient_names''')
    text=replace(text,'    standard_errors <- se(fit)','    standard_errors <- se(fit)[cell$r_coefficient_names]')
    text=replace(text,'    V <- vcov(fit) * correction','''    V <- vcov(fit)[cell$r_coefficient_names,cell$r_coefficient_names,drop=FALSE] * correction
    dimnames(V) <- list(cell$coefficient_names,cell$coefficient_names)
    if (!is.null(saved_outputs)) {
        con <- file(file.path(output,"outputs.bin"),"wb")
        for (values in saved_outputs) writeBin(as.double(values),con,size=8,endian="little")
        close(con)
    }''')
    text=replace(text,'else "native IID"','else paste("native", cell$vce, "explicit nonnested-K/min-G SSC")')
    path.write_text(text)
