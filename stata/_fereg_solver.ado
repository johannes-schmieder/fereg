*! fereg solver() grammar 0.5.3 07nov2026
*! One vocabulary in and out. With spec(), parse solver(method [, reduce()
*! precondition() accelerate() cmg*()]) for fereg or feppml, validate the
*! combination and map it to the native route tokens of the request wire
*! (auto map lsmr cmg cmgschur it squarem). With route(), describe one native
*! route row (see crates/fereg-plugin/src/route_report.rs) in the same words.
program define _fereg_solver, rclass
    version 15.0
    syntax , COMMAND(name) [SPEC(string) ROUTE(name) ROW(integer 1) ///
        LABELS(string)]
    if !inlist("`command'", "fereg", "feppml") {
        di as error "_fereg_solver: unknown command `command'"
        exit 198
    }
    if "`route'" != "" {
        Describe `route' `row' `"`labels'"'
        return add
        exit
    }
    local spec = strtrim(`"`spec'"')
    if `"`spec'"' == "" local spec "auto"
    gettoken method suboptions : spec, parse(",")
    local method = lower(strtrim(`"`method'"'))
    local suboptions = strtrim(`"`suboptions'"')
    if substr(`"`suboptions'"', 1, 1) == "," {
        local suboptions = strtrim(substr(`"`suboptions'"', 2, .))
    }
    local words : word count `method'
    if `words' != 1 {
        di as error "solver() takes one method, then a comma and its sub-options, for example solver(lsmr, precondition(schwarz))"
        exit 198
    }

    local methods "auto, map, lsmr, or cg"
    if "`command'" == "feppml" local methods "auto, lsmr, or cg"
    if inlist(`"`method'"', "cmg", "cmgschur", "it", "squarem", "irons-tuck") {
        local hint "solver(cg)"
        if `"`method'"' == "cmgschur" local hint "solver(cg, reduce(eliminate))"
        if inlist(`"`method'"', "it", "irons-tuck") local hint "solver(map, accelerate(irons-tuck))"
        if `"`method'"' == "squarem" local hint "solver(map, accelerate(squarem))"
        if "`command'" == "feppml" & `"`method'"' != "cmg" local hint ""
        if `"`hint'"' != "" local hint "; request `hint'"
        di as error `"solver() must be `methods'`hint'"'
        exit 198
    }
    if "`command'" == "feppml" & `"`method'"' == "map" {
        di as error "feppml has no alternating-projections solver; solver() must be auto, lsmr, or cg"
        exit 198
    }
    if !inlist(`"`method'"', "auto", "map", "lsmr", "cg") {
        di as error "solver() must be `methods'"
        exit 198
    }

    local 0 `", `suboptions'"'
    capture syntax [, REDuce(string) PRECondition(string) ACCELerate(string) ///
        CMGDEGree(string) CMGSAMples(string) CMGSEed(string)]
    if _rc {
        di as error `"solver(`method', ...) accepts reduce(), precondition(), accelerate(), cmgdegree(), cmgsamples(), and cmgseed()"'
        exit 198
    }
    local reduce = lower(strtrim(`"`reduce'"'))
    local precondition = lower(strtrim(`"`precondition'"'))
    local accelerate = lower(strtrim(`"`accelerate'"'))
    if `"`reduce'"' != "" & !inlist(`"`reduce'"', "auto", "full", "eliminate") {
        di as error "solver(..., reduce()) must be auto, full, or eliminate"
        exit 198
    }
    if `"`precondition'"' != "" & !inlist(`"`precondition'"', "auto", "none", ///
        "jacobi", "block", "schwarz", "cmg") {
        di as error "solver(..., precondition()) must be auto, none, jacobi, block, schwarz, or cmg"
        exit 198
    }
    if `"`accelerate'"' != "" & !inlist(`"`accelerate'"', "none", "irons-tuck", ///
        "squarem", "cg") {
        di as error "solver(..., accelerate()) must be none, irons-tuck, or squarem"
        exit 198
    }
    local r = cond(`"`reduce'"' == "", "auto", `"`reduce'"')
    local p = cond(`"`precondition'"' == "", "auto", `"`precondition'"')
    local a = cond(`"`accelerate'"' == "", "none", `"`accelerate'"')
    local cmg_given = (`"`cmgdegree'`cmgsamples'`cmgseed'"' != "")

    if `"`a'"' != "none" & `"`method'"' != "map" {
        di as error "accelerate() applies only to solver(map)"
        exit 198
    }
    local route
    local wire_preconditioner "off"
    if `"`method'"' == "auto" {
        if `"`r'"' != "auto" {
            di as error "solver(auto) chooses the reduction; request solver(cg, reduce(`r')) or solver(lsmr) to fix it"
            exit 198
        }
        if "`command'" == "feppml" & `"`p'"' != "auto" {
            di as error "feppml's solver(auto) chooses its preconditioners; request solver(lsmr) or solver(cg) to fix one"
            exit 198
        }
        if !inlist(`"`p'"', "auto", "jacobi") {
            di as error "solver(auto) accepts precondition(jacobi), which keeps every Krylov solve Jacobi-preconditioned; it chooses block, Schwarz, and CMG preconditioners itself, and another fixed preconditioner needs an explicit method, for example solver(lsmr, precondition(none))"
            exit 198
        }
        local route "auto"
        local wire_preconditioner = cond(`"`p'"' == "jacobi", "diagonal", "auto")
    }
    else if `"`method'"' == "map" {
        if `"`r'"' == "eliminate" {
            di as error "solver(map) iterates over every effect; reduce(eliminate) is not available"
            exit 198
        }
        if `"`p'"' != "auto" {
            di as error "solver(map) has no preconditioner; it accepts accelerate(irons-tuck) or accelerate(squarem)"
            exit 198
        }
        if `"`a'"' == "cg" {
            di as error "accelerate(cg) is disabled because its residual certificate does not ensure projection accuracy; use solver(cg) or solver(lsmr)"
            exit 198
        }
        local route = cond(`"`a'"' == "irons-tuck", "it", ///
            cond(`"`a'"' == "squarem", "squarem", "map"))
    }
    else if `"`method'"' == "lsmr" {
        if `"`r'"' == "eliminate" {
            di as error "solver(lsmr) runs on the full system; reduce(eliminate) is available with solver(cg)"
            exit 198
        }
        if "`command'" == "feppml" {
            if !inlist(`"`p'"', "auto", "schwarz") {
                di as error "feppml's solver(lsmr) uses the Schwarz preconditioner; precondition() must be auto or schwarz"
                exit 198
            }
        }
        else if !inlist(`"`p'"', "auto", "none", "jacobi", "schwarz") {
            di as error "solver(lsmr) accepts precondition(none), precondition(jacobi), or precondition(schwarz)"
            exit 198
        }
        local route "lsmr"
        local wire_preconditioner = cond(`"`p'"' == "none", "off", ///
            cond(`"`p'"' == "jacobi", "diagonal", `"`p'"'))
        if "`command'" == "feppml" local wire_preconditioner "auto"
    }
    else {
        // Conjugate gradients: CMG on the full graph (the established
        // route) or, explicitly, on the eliminated two-FE Schur system.
        if "`command'" == "feppml" & `"`r'"' == "eliminate" {
            di as error "feppml's solver(cg) runs on the full system; reduce() must be auto or full"
            exit 198
        }
        if (inlist(`"`p'"', "jacobi", "block") & `"`r'"' != "full") | `"`p'"' == "block" {
            di as error "solver(cg, reduce(eliminate) precondition(`p')) is not available as an explicit request yet; solver(auto) runs the eliminated Jacobi and block-Jacobi CG automatically"
            exit 198
        }
        if !inlist(`"`p'"', "auto", "cmg") {
            di as error "solver(cg) accepts precondition(cmg); for precondition(`p') request solver(lsmr, precondition(`p'))"
            exit 198
        }
        local route = cond(`"`r'"' == "eliminate", "cmgschur", "cmg")
    }
    if `cmg_given' & "`route'" != "cmgschur" {
        di as error "cmgdegree(), cmgsamples(), and cmgseed() apply only to solver(cg, reduce(eliminate))"
        exit 198
    }

    // The normalized request, in the order the grammar lists sub-options.
    local normalized `"`method'"'
    local pieces
    foreach option in reduce precondition accelerate cmgdegree cmgsamples cmgseed {
        local value = strtrim(`"``option''"')
        if `"`value'"' != "" local pieces `"`pieces' `option'(`value')"'
    }
    local pieces = strtrim(`"`pieces'"')
    if `"`pieces'"' != "" local normalized `"`normalized', `pieces'"'

    return local spec `"`normalized'"'
    return local route "`route'"
    return local preconditioner "`wire_preconditioner'"
    return local cmgdegree = strtrim(`"`cmgdegree'"')
    return local cmgsamples = strtrim(`"`cmgsamples'"')
    return local cmgseed = strtrim(`"`cmgseed'"')
end

// Decode one route row: reduction, method, MAP acceleration, preconditioner,
// five per-effect local solves, escalations, iterations and fallback.
program define Describe, rclass
    args matrix row labels
    tempname R
    matrix `R' = `matrix'
    if colsof(`R') < 12 | `row' < 1 | `row' > rowsof(`R') {
        di as error "fixed-effect route report has an invalid layout"
        exit 3499
    }
    forvalues column = 1/12 {
        local value = `R'[`row',`column']
        if missing(`value') | `value' != int(`value') | `value' < 0 {
            di as error "fixed-effect route report failed semantic validation"
            exit 3499
        }
    }
    local reduction : word `=`R'[`row',1]+1' of none full eliminate
    local method : word `=`R'[`row',2]+1' of none direct map cg lsmr derived
    local acceleration : word `=`R'[`row',3]+1' of none cg irons-tuck squarem
    local preconditioner : word `=`R'[`row',4]+1' of none jacobi block schwarz cmg
    if `"`reduction'"' == "" | `"`method'"' == "" | `"`acceleration'"' == "" | ///
       `"`preconditioner'"' == "" | `R'[`row',12] > 1 | ///
       (`"`acceleration'"' != "none" & `"`method'"' != "map") {
        di as error "fixed-effect route report failed semantic validation"
        exit 3499
    }
    local blocks
    forvalues effect = 1/5 {
        local local_code = `R'[`row',`effect'+4]
        if `local_code' > 3 | (`local_code' > 0 & `"`preconditioner'"' != "block") {
            di as error "fixed-effect route report failed semantic validation"
            exit 3499
        }
        if `local_code' > 0 {
            local solve : word `local_code' of dense cmg jacobi
            local name : word `effect' of `labels'
            if `"`name'"' == "" local name "effect`effect'"
            local blocks `"`blocks' `name'=`solve'"'
        }
    }
    local blocks = strtrim(`"`blocks'"')
    if `"`preconditioner'"' == "block" & `"`blocks'"' == "" {
        di as error "fixed-effect route report failed semantic validation"
        exit 3499
    }
    // The used route in request syntax, for the display line.
    local text "`method'"
    if "`method'" == "map" & "`acceleration'" != "none" ///
        local text "map, accelerate(`acceleration')"
    if "`method'" == "lsmr" local text "lsmr, precondition(`preconditioner')"
    if "`method'" == "cg" local text "cg, reduce(`reduction') precondition(`preconditioner')"
    return local text "`text'"
    return local reduction "`reduction'"
    return local method "`method'"
    return local acceleration "`acceleration'"
    return local preconditioner "`preconditioner'"
    return local block_solvers `"`blocks'"'
    return scalar escalations = `R'[`row',10]
    return scalar iterations = `R'[`row',11]
    return scalar fallback = `R'[`row',12]
end
