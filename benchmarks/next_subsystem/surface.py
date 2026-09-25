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
    if mode not in ('numeric', 'factor', 'product', 'factor_cross', 'time_series'):
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
    cell['ts_setup'] = ''
    if mode == 'time_series':
        operators = cell.get('ts_operators', ['L'])
        if not operators or any(op not in ('L', 'F', 'D') for op in operators):
            raise ValueError('Only first lag, lead and difference are screened')
        cell['ts_operators'] = operators
        cell['ts_setup'] = 'yes'
        names = [f'{operators[i % len(operators)]}.{name}' for i, name in enumerate(names)]
        r_names = [f'{name[0].lower()}({name[2:]}, 1)' for name in names]
        stata_rhs, r_rhs = list(names), list(r_names)
    slopes = cell.get('slope_partitions', {})
    if slopes and (mode != 'numeric' or cell.get('outputs') == 'all'):
        raise ValueError('Slope screen currently requires numeric terms and no saved outputs')
    if any(effect not in cell['effects'] or not channels or
           any(name not in ('z1', 'z2', 'z3', 'z4') for name in channels)
           for effect, channels in slopes.items()):
        raise ValueError('Unknown slope source or partition')
    cell['slope_absorb'] = ' '.join(
        f'{effect}##c.({" ".join(slopes[effect])})' if effect in slopes else effect
        for effect in cell['effects']) if slopes else ''
    cell['effects_r'] = ' + '.join(
        f'{effect}[{", ".join(slopes[effect])}]' if effect in slopes else effect
        for effect in cell['effects'])
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
    cell.update(terms=mode, rhs=' '.join(stata_rhs), rhs_r=' + '.join(r_rhs),
                coefficient_names=names, r_coefficient_names=r_names,
                export_names=' '.join(names), outputs=cell.get('outputs', 'none'),
                vce=cell.get('vce') or 'iid', memory=cell.get('memory', 'fast'))
    return cell


def augment(cell, path):
    frame = pd.read_stata(path, convert_categoricals=False)
    info = json.loads(path.with_suffix('.json').read_text())
    rng = np.random.default_rng(np.random.SeedSequence([info['seed'], 903]))
    if cell.get('terms') == 'time_series':
        row = np.arange(len(frame))
        frame['ts_panel'] = row // 30 + 1
        time = row % 30 + 1
        if cell.get('time_gaps', False):
            time = time + (time >= 9) + 2 * (time >= 21)
        frame['ts_time'] = time
    for name in sorted({name for names in cell.get('slope_partitions', {}).values() for name in names}):
        frame[name] = rng.normal(size=len(frame))
    for effect, names in cell.get('slope_partitions', {}).items():
        for name in names:
            frame['y'] += .3 * np.sin(frame[effect]) * frame[name]
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
    if cell.get('terms') == 'time_series':
        # A keyed lookup on the original axis is independent of Stata and fixest
        # indexing. Generate before availability/singleton pruning, never after.
        keys = list(zip(frame.ts_panel, frame.ts_time))
        if len(set(keys)) != len(keys):
            raise ValueError('Duplicate panel-time key')
        lookup = {key: i for i, key in enumerate(keys)}
        operators = cell['ts_operators']
        for j, current in enumerate(values):
            op = operators[j % len(operators)]
            previous = np.array([current[lookup[(p, t + (1 if op == 'F' else -1))]]
                                 if (p, t + (1 if op == 'F' else -1)) in lookup else np.nan
                                 for p, t in keys])
            values[j] = current - previous if op == 'D' else previous
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
    # Wide generated-token lists can exceed Stata's startup argument buffer.
    # A captured include binds metadata before timing without a longer argv.
    text=replace(text,'    maximum_rss = 0', '''    if arm != "fixest":
        lines = []
        for key in ("rhs", "export_names", "outputs", "memory", "ts_setup", "slope_absorb"):
            value = str(cell.get(key, ""))
            if any(char in value for char in ('"', "\\n", "\\r")):
                raise ValueError("Unsafe surface metadata")
            lines.append(f'local {key} "{value}"')
        (output / "surface-args.do").write_text("\\n".join(lines) + "\\n")
    maximum_rss = 0''')
    path.write_text(text)
    path=folder/'stata_worker.do'; text=path.read_text()
    text=replace(text,'fit_options string_columns\n','fit_options string_columns\ninclude `"`output\'/surface-args.do"\'\n')
    text=replace(text,'    quietly use `"`warm\'"\', clear', '    quietly use `"`warm\'"\', clear\n    if "`ts_setup\'" == "yes" quietly xtset ts_panel ts_time')
    text=replace(text,'quietly use `"`data\'"\', clear', 'quietly use `"`data\'"\', clear\nif "`ts_setup\'" == "yes" quietly xtset ts_panel ts_time')
    text=replace(text,'local effects\n','if "`rhs\'" != "" local xvars `"`rhs\'"\'\nlocal effects\n')
    text=replace(text,'local command `"`arm\' y', '''if "`slope_absorb'" != "" local absorption absorb(`slope_absorb')
if "`memory'" == "low" local fit_options `fit_options' memory(low)
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
    text=replace(text,'    as.data.frame(cols)', '''    result <- as.data.frame(cols)
    if (cell$terms == "time_series") result <- panel(result, ~ts_panel+ts_time, time.step=1)
    result''')
    text=replace(text,'paste(paste0("x", seq_len(cell$k)), collapse=" + ")','cell$rhs_r')
    text=replace(text,'paste(cell$effects, collapse=" + ")','cell$effects_r')
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
