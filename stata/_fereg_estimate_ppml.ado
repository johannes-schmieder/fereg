*! fereg guarded PPML eclass transaction 0.5.0 31aug2026
program define _fereg_estimate_ppml, eclass sortpreserve
    version 18.0
    local hidden_cmdline `"_fereg_estimate_ppml `0'"'

    syntax varlist(fv ts numeric min=1) [if] [in], ///
        [FWEIGHT(varname numeric) PWEIGHT(varname numeric) ///
         ABSorb(string asis) CLUster(string asis) KEEPsingletons ///
         OFFset(varname numeric) EXPosure(varname numeric) ///
         VCE(string) SEParation(string asis) ///
         RESIDual(name) RESIDualtype(string) XB(name) D(name) ///
         ETA(name) XBD(name) MU(name) DROPreason(name) ///
         MEMory(string) POOLSize(integer 10) THREADS(integer 0) ///
         DETERMINISTIC FINITESOLVer(string) ///
         FESOLVer(string) FEPREConditioner(string) ///
         FELSMRMode(string) FELSMRPolicy(string) FECMGMode(string) SOLVERTRACE ///
         TOLerance(real 1e-8) ITERate(integer 10000) ///
         SEPTOLerance(real 1e-8) SEPITERate(integer 10000) ///
         SIMPLEXITERate(integer 100000) SIMPLEXCELLS(integer 1000000) ///
         ALLOCATIONAUDIT INJECT(string) PUBLICMode ///
         PUBLICCMDLINE(string) PUBLICTIMEIT PUBLICDISPlay ///
         PUBLICWEXP(string)]

    gettoken depvar_token : varlist
    capture confirm numeric variable `depvar_token'
    if _rc {
        di as error "PPML dependent variable must be a plain numeric variable"
        exit 198
    }

    if (`"`publicmode'"' == "" & ///
            (strtrim(`"`publiccmdline'"') != "" | `"`publictimeit'"' != "" | ///
             `"`publicdisplay'"' != "" | strtrim(`"`publicwexp'"') != "")) | ///
       (`"`publicmode'"' != "" & strtrim(`"`publiccmdline'"') == "") {
        di as error "invalid private PPML public-publication context"
        exit 198
    }

    local fe_solver = lower(strtrim(`"`fesolver'"'))
    if `"`fe_solver'"' == "" local fe_solver "auto"
    local fe_preconditioner = lower(strtrim(`"`fepreconditioner'"'))
    if `"`fe_preconditioner'"' == "" local fe_preconditioner "none"
    local fe_lsmr_mode = lower(strtrim(`"`felsmrmode'"'))
    if `"`fe_lsmr_mode'"' == "" local fe_lsmr_mode "none"
    local fe_lsmr_policy = lower(strtrim(`"`felsmrpolicy'"'))
    if `"`fe_lsmr_policy'"' == "" local fe_lsmr_policy "none"
    local fe_cmg_mode = lower(strtrim(`"`fecmgmode'"'))
    if `"`fe_cmg_mode'"' == "" local fe_cmg_mode "none"
    local solver_trace = (`"`solvertrace'"' != "")
    if `"`publicmode'"' != "" & ///
       (`"`fe_solver'"' != "auto" | `"`fe_preconditioner'"' != "none" | ///
        `"`fe_lsmr_mode'"' != "none" | `"`fe_lsmr_policy'"' != "none" | ///
        `"`fe_cmg_mode'"' != "none" | ///
        `solver_trace') {
        di as error "private PPML FE-solver controls are unavailable through feppml"
        exit 198
    }
    if !inlist(`"`fe_solver'"', "auto", "map", "cg", "lsmr", "cmg") {
        di as error "fesolver() must be auto, map, cg, lsmr, or cmg"
        exit 198
    }
    if `"`fe_preconditioner'"' != "none" & ///
       !(`"`fe_solver'"' == "lsmr" & `"`fe_preconditioner'"' == "schwarz") {
        di as error "fepreconditioner(schwarz) is valid only with fesolver(lsmr)"
        exit 198
    }
    if !inlist(`"`fe_lsmr_mode'"', "none", "legacy", "fused", "joint") {
        di as error "felsmrmode() must be legacy, fused, or joint"
        exit 198
    }
    if !inlist(`"`fe_lsmr_policy'"', "none", "diagonal", "reuse", "rebuild") {
        di as error "felsmrpolicy() must be diagonal, reuse, or rebuild"
        exit 198
    }
    if !inlist(`"`fe_cmg_mode'"', "none", "fused") {
        di as error "fecmgmode() must be fused"
        exit 198
    }
    if (`"`fe_lsmr_mode'"' == "none") != (`"`fe_lsmr_policy'"' == "none") {
        di as error "felsmrmode() and felsmrpolicy() must be specified together"
        exit 198
    }
    if `"`fe_lsmr_mode'"' != "none" & `"`fe_solver'"' != "lsmr" {
        di as error "felsmrmode() and felsmrpolicy() require fesolver(lsmr)"
        exit 198
    }
    if `"`fe_lsmr_mode'"' != "none" & `"`fe_preconditioner'"' != "none" {
        di as error "felsmrpolicy() cannot be combined with fepreconditioner()"
        exit 198
    }
    if `"`fe_lsmr_mode'"' == "legacy" & `"`fe_lsmr_policy'"' == "reuse" {
        di as error "legacy PPML LSMR cannot reuse Schwarz state across frames"
        exit 198
    }
    if `"`fe_cmg_mode'"' != "none" & `"`fe_solver'"' != "cmg" {
        di as error "fecmgmode() requires fesolver(cmg)"
        exit 198
    }
    local solver_experiment 0
    if !inlist(`"`fe_solver'"', "auto", "map") | `solver_trace' {
        local solver_gate : environment FEREG_PPML_EXPERIMENTAL_SOLVERS
        if strtrim(`"`solver_gate'"') != "1" {
            di as error "experimental PPML FE solvers require FEREG_PPML_EXPERIMENTAL_SOLVERS=1"
            exit 498
        }
        local solver_experiment 1
    }

    local requested_weights = (`"`fweight'"' != "") + (`"`pweight'"' != "")
    if `requested_weights' > 1 {
        di as error "fweight() and pweight() are mutually exclusive"
        exit 198
    }
    if strtrim(`"`publicwexp'"') != "" & `requested_weights' == 0 {
        di as error "private publicwexp() requires a PPML weight"
        exit 198
    }
    if `"`offset'"' != "" & `"`exposure'"' != "" {
        di as error "offset() and exposure() are mutually exclusive"
        exit 198
    }
    if `poolsize' <= 0 | `threads' < 0 | ///
       `tolerance' <= 0 | `tolerance' >= 1 | `iterate' <= 0 | ///
       `septolerance' <= 0 | `septolerance' >= 1 | `sepiterate' <= 0 | ///
       `simplexiterate' <= 0 | `simplexcells' <= 0 {
        di as error "PPML tolerances must be in (0,1) and iteration limits must be positive"
        exit 198
    }

    local memory_mode = lower(strtrim(`"`memory'"'))
    if `"`memory_mode'"' == "" local memory_mode "auto"
    if !inlist(`"`memory_mode'"', "auto", "fast", "low") {
        di as error "memory() must be auto, fast, or low"
        exit 198
    }
    if (`"`fe_solver'"' != "auto" | `solver_trace') & `"`memory_mode'"' != "fast" {
        di as error "private PPML FE-solver experiments require memory(fast)"
        exit 198
    }
    if (`"`fe_solver'"' != "auto" | `solver_trace') & `"`allocationaudit'"' != "" {
        di as error "private PPML FE-solver experiments do not support allocationaudit"
        exit 198
    }
    local finite_solver = lower(strtrim(`"`finitesolver'"'))
    if `"`finite_solver'"' == "" local finite_solver "auto"
    if `"`finite_solver'"' != "auto" {
        di as error "only finitesolver(auto) is implemented"
        exit 498
    }

    local separation_wire = lower(strtrim(`"`separation'"'))
    if `"`separation_wire'"' == "" local separation_wire "fe simplex ir"
    local separation_word_count : word count `separation_wire'
    local separation_count 0
    local seen_separation
    foreach method of local separation_wire {
        if !inlist(`"`method'"', "fe", "simplex", "ir", "mu", "none", "default") {
            di as error "unknown separation method `method'"
            exit 198
        }
        if inlist(`"`method'"', "none", "default") & `separation_word_count' != 1 {
            di as error "separation(`method') cannot be combined with another method"
            exit 198
        }
        local duplicate : list method in seen_separation
        if `duplicate' {
            di as error "separation methods may not be repeated"
            exit 198
        }
        local seen_separation `seen_separation' `method'
        if !inlist(`"`method'"', "none", "default") local ++separation_count
    }
    if `"`separation_wire'"' == "default" local separation_count 3
    // The bounded report always includes singleton pruning as row one.
    local separation_count = 1 + `separation_count'
    local separation_codes 1
    if `"`separation_wire'"' == "default" {
        local separation_codes 1 2 3 4
    }
    else if `"`separation_wire'"' != "none" {
        foreach method of local separation_wire {
            if `"`method'"' == "fe" local separation_codes `separation_codes' 2
            if `"`method'"' == "simplex" local separation_codes `separation_codes' 3
            if `"`method'"' == "ir" local separation_codes `separation_codes' 4
            if `"`method'"' == "mu" local separation_codes `separation_codes' 5
        }
        local separation_codes : list sort separation_codes
    }

    local vce_text = strtrim(`"`vce'"')
    if `"`vce_text'"' == "" local vce_text "robust"
    gettoken vce_kind vce_rest : vce_text
    local vce_kind = lower(`"`vce_kind'"')
    local vce_rest = strtrim(`"`vce_rest'"')
    if `"`vce_kind'"' == "unadjusted" & `"`vce_rest'"' == "" {
        local vce_request_wire "unadjusted"
        local vce_wire "robust"
    }
    else if inlist(`"`vce_kind'"', "ols", "iid") & `"`vce_rest'"' == "" {
        di as error "vce(`vce_kind') is not defined for public PPML inference"
        exit 198
    }
    else if `"`vce_kind'"' == "robust" & `"`vce_rest'"' == "" {
        local vce_request_wire "robust"
        local vce_wire "robust"
    }
    else if `"`vce_kind'"' == "cluster" {
        if `"`vce_rest'"' == "" & strtrim(`"`cluster'"') == "" {
            di as error "vce(cluster) requires at least one cluster dimension"
            exit 198
        }
        if `"`vce_rest'"' == "" local vce_rest = strtrim(`"`cluster'"')
        if strtrim(`"`cluster'"') != "" & strtrim(`"`cluster'"') != `"`vce_rest'"' {
            di as error "cluster() and vce(cluster ...) disagree"
            exit 198
        }
        local cluster `"`vce_rest'"'
        local vce_request_wire "cluster"
        local vce_wire "cluster"
    }
    else {
        di as error "vce() must be robust, unadjusted, or cluster varlist"
        exit 198
    }
    if `"`vce_wire'"' != "cluster" & strtrim(`"`cluster'"') != "" {
        local vce_request_wire "cluster"
        local vce_wire "cluster"
    }

    local inject = lower(strtrim(`"`inject'"'))
    local inject_known = inlist(`"`inject'"', "", "posthold", "postplugin", ///
        "poststripe", "postereturn", "postscalars", "postpublic", ///
        "postdiagnostics", "precommit")
    local inject_known = `inject_known' | inlist(`"`inject'"', ///
        "breakpostplugin", "breakpostpublic", "mutatewire", ///
        "mutatesemantic", "mutateoutput", "panic", "breaknative", ///
        "allocation")
    if !`inject_known' {
        di as error "unknown developer PPML transaction injection stage: `inject'"
        exit 198
    }

    local residual_wire = lower(strtrim(`"`residualtype'"'))
    if `"`residual_wire'"' == "" {
        if `"`residual'"' == "" local residual_wire "none"
        else local residual_wire "response"
    }
    if !inlist(`"`residual_wire'"', "none", "response", "score", ///
        "pearson", "working", "deviance", "anscombe") {
        di as error "residualtype() must be response, score, pearson, working, deviance, or anscombe"
        exit 198
    }
    if (`"`residual_wire'"' == "none") != (`"`residual'"' == "") {
        di as error "residualtype(none) and residual() must be specified consistently"
        exit 198
    }
    if `"`eta'"' != "" & `"`xbd'"' != "" {
        di as error "eta() and xbd() are aliases and may not be combined"
        exit 198
    }
    local eta_target `"`eta'`xbd'"'

    local requested_output_names `residual' `xb' `d' `eta_target' `mu' `dropreason'
    local requested_output_names = strtrim(`"`requested_output_names'"')
    local unique_output_names : list uniq requested_output_names
    local requested_output_count : word count `requested_output_names'
    local unique_output_count : word count `unique_output_names'
    if `requested_output_count' != `unique_output_count' {
        di as error "PPML output variable names must be distinct"
        exit 110
    }
    foreach output of local requested_output_names {
        capture confirm new variable `output'
        if _rc {
            di as error "PPML output variable `output' already exists"
            exit 198
        }
    }

    marksample touse, novarlist
    local candidate_all = (strtrim(`"`if'"') == "" & strtrim(`"`in'"') == "")
    gettoken depvar indepvars : varlist
    local absorb_option
    local cluster_option
    if strtrim(`"`absorb'"') != "" local absorb_option `"absorb(`absorb')"'
    if strtrim(`"`cluster'"') != "" local cluster_option `"cluster(`cluster')"'

    * PPML's categorical FE and cluster owners accept original sources only.
    * The shared fereg parser intentionally has broader legacy behavior, so
    * reject time-series aliases here before it can strip r(ts_op).
    local ppml_absorb_rest `"`absorb'"'
    while strtrim(`"`ppml_absorb_rest'"') != "" {
        gettoken ppml_absorb_term ppml_absorb_rest : ppml_absorb_rest, bind
        local ppml_equal = strpos(`"`ppml_absorb_term'"', "=")
        if `ppml_equal' > 0 local ppml_absorb_term = ///
            substr(`"`ppml_absorb_term'"', `ppml_equal' + 1, .)
        local ppml_absorb_term : subinstr local ppml_absorb_term "##c." "#", all
        local ppml_absorb_term : subinstr local ppml_absorb_term "#c." "#", all
        local ppml_absorb_term : subinstr local ppml_absorb_term "#" " ", all
        local ppml_absorb_term : subinstr local ppml_absorb_term "(" " ", all
        local ppml_absorb_term : subinstr local ppml_absorb_term ")" " ", all
        foreach ppml_absorb_source of local ppml_absorb_term {
            quietly _ms_parse_parts `ppml_absorb_source'
            if strtrim(`"`r(ts_op)'"') != "" {
                di as error "time-series operators are not allowed in PPML absorb()"
                exit 198
            }
        }
    }
    local ppml_cluster_terms : subinstr local cluster "#" " ", all
    foreach ppml_cluster_source of local ppml_cluster_terms {
        quietly _ms_parse_parts `ppml_cluster_source'
        if strtrim(`"`r(ts_op)'"') != "" {
            di as error "time-series operators are not allowed in PPML clusters"
            exit 198
        }
    }

    tempname model_sources model_terms model_components absorb_terms
    tempname absorb_components cluster_terms cluster_components input_bindings
    if strtrim(`"`indepvars'"') == "" {
        fereg_parse `touse', touse(`touse') emptymodel ///
            `absorb_option' `cluster_option'
    }
    else {
        fereg_parse `indepvars', touse(`touse') `absorb_option' `cluster_option'
    }

    local source_vars `"`r(source_vars)'"'
    local coefficient_names `"`r(coefficient_names)'"'
    local component_raw `"`r(component_raw_operators)'"'
    local component_ts `"`r(component_ts_operators)'"'
    local absorb_labels `"`r(absorb_labels)'"'
    local absorb_saved_names `"`r(absorb_saved_names)'"'
    local absorb_label_registry `"`r(absorb_label_registry)'"'
    local absorb_saved_registry `"`r(absorb_saved_name_registry)'"'
    local cluster_labels `"`r(cluster_labels)'"'
    local n_sources = r(n_sources)
    local n_terms = r(n_model_terms)
    local n_components = r(n_model_components)
    local n_absorb_terms = r(n_absorb_terms)
    local n_absorb_components = r(n_absorb_components)
    local n_cluster_terms = r(n_cluster_terms)
    local n_cluster_components = r(n_cluster_components)
    local model_major = r(model_wire_major)
    local model_minor = r(model_wire_minor)
    matrix `model_sources' = r(model_sources)
    matrix `model_terms' = r(model_terms)
    matrix `model_components' = r(model_components)
    matrix `absorb_terms' = r(absorb_terms)
    matrix `absorb_components' = r(absorb_components)
    matrix `cluster_terms' = r(cluster_terms)
    matrix `cluster_components' = r(cluster_components)

    if `n_cluster_terms' > 5 {
        di as error "feppml supports at most five cluster dimensions"
        exit 198
    }
    if `"`vce_wire'"' == "cluster" & `n_cluster_terms' == 0 {
        di as error "vce(cluster) requires at least one cluster dimension"
        exit 198
    }

    // Stata, not Rust, owns time-series indexing and panel-gap semantics.
    local original_source_vars `"`source_vars'"'
    tempname original_model_components
    matrix `original_model_components' = `model_components'
    if `n_components' > 0 {
        forvalues component_row = 1/`n_components' {
            if `original_model_components'[`component_row',9] != 2 continue
            local source_id = `original_model_components'[`component_row',3]
            local base_source : word `source_id' of `original_source_vars'
            local ts_piece : word `component_row' of `component_ts'
            local ts_colon = strpos(`"`ts_piece'"', ":")
            if `ts_colon' == 0 exit 198
            local ts_operator = substr(`"`ts_piece'"', `ts_colon' + 1, .)
            local ts_expression `"`ts_operator'"'
            if substr(`"`ts_expression'"', strlen(`"`ts_expression'"'), 1) != "." ///
                local ts_expression `"`ts_expression'."'
            tempvar materialized_ts
            capture quietly generate double `materialized_ts' = `ts_expression'`base_source'
            if _rc {
                local materialize_rc = _rc
                exit `materialize_rc'
            }

            local source_has_other_use 0
            forvalues check = 1/`n_components' {
                if `original_model_components'[`check',3] == `source_id' & ///
                   `original_model_components'[`check',9] != 2 local source_has_other_use 1
            }
            if `n_absorb_components' > 0 {
                forvalues check = 1/`n_absorb_components' {
                    if `absorb_components'[`check',4] == `source_id' local source_has_other_use 1
                }
            }
            if `n_cluster_components' > 0 {
                forvalues check = 1/`n_cluster_components' {
                    if `cluster_components'[`check',3] == `source_id' local source_has_other_use 1
                }
            }
            local source_already_reused 0
            if `component_row' > 1 {
                forvalues prior = 1/`=`component_row' - 1' {
                    if `original_model_components'[`prior',3] == `source_id' & ///
                       `original_model_components'[`prior',9] == 2 local source_already_reused 1
                }
            }
            if !`source_has_other_use' & !`source_already_reused' {
                local rebuilt_source_vars
                forvalues source_index = 1/`n_sources' {
                    local existing_source : word `source_index' of `source_vars'
                    if `source_index' == `source_id' ///
                        local rebuilt_source_vars `"`rebuilt_source_vars' `materialized_ts'"'
                    else local rebuilt_source_vars `"`rebuilt_source_vars' `existing_source'"'
                }
                local source_vars `"`rebuilt_source_vars'"'
                local materialized_source_id = `source_id'
            }
            else {
                local ++n_sources
                local source_vars `"`source_vars' `materialized_ts'"'
                matrix `model_sources' = `model_sources' \ (`n_sources', 0)
                local materialized_source_id = `n_sources'
            }
            matrix `model_components'[`component_row',3] = `materialized_source_id'
            matrix `model_components'[`component_row',9] = 3
        }
    }

    local source_registry
    foreach value of local source_vars {
        local piece `"`=strlen(`"`value'"')':`value'"'
        if `"`source_registry'"' == "" local source_registry `"`piece'"'
        else local source_registry `"`source_registry'|`piece'"'
    }
    if `n_sources' == 0 local source_registry "."
    local token_registry
    foreach value of local coefficient_names {
        local piece `"`=strlen(`"`value'"')':`value'"'
        if `"`token_registry'"' == "" local token_registry `"`piece'"'
        else local token_registry `"`token_registry'|`piece'"'
    }
    if `n_terms' == 0 local token_registry "."
    local raw_registry
    foreach piece of local component_raw {
        if `"`raw_registry'"' == "" local raw_registry `"`piece'"'
        else local raw_registry `"`raw_registry'|`piece'"'
    }
    local ts_registry
    foreach piece of local component_ts {
        if `"`ts_registry'"' == "" local ts_registry `"`piece'"'
        else local ts_registry `"`ts_registry'|`piece'"'
    }
    if `n_components' == 0 {
        local raw_registry "."
        local ts_registry "."
    }
    local cluster_label_registry
    foreach value of local cluster_labels {
        local piece `"`=strlen(`"`value'"')':`value'"'
        if `"`cluster_label_registry'"' == "" local cluster_label_registry `"`piece'"'
        else local cluster_label_registry `"`cluster_label_registry'|`piece'"'
    }
    if `n_cluster_terms' == 0 local cluster_label_registry "."

    local has_fweight = (`"`fweight'"' != "")
    local has_pweight = (`"`pweight'"' != "")
    local has_weight = `has_fweight' + `has_pweight'
    local has_adjustment = (`"`offset'`exposure'"' != "")
    local n_bindings = 2 + `n_sources' + `has_weight'
    matrix `input_bindings' = J(`n_bindings', 3, 0)
    matrix `input_bindings'[1,1] = 1
    matrix `input_bindings'[1,2] = 1
    matrix `input_bindings'[2,1] = 2
    matrix `input_bindings'[2,2] = 2
    if `n_sources' > 0 {
        forvalues source = 1/`n_sources' {
            local row = 2 + `source'
            matrix `input_bindings'[`row',1] = `row'
            matrix `input_bindings'[`row',2] = 3
            matrix `input_bindings'[`row',3] = `source'
        }
    }
    if `has_weight' {
        local row = 3 + `n_sources'
        matrix `input_bindings'[`row',1] = `row'
        if `has_fweight' matrix `input_bindings'[`row',2] = 5
        if `has_pweight' matrix `input_bindings'[`row',2] = 6
    }

    local adjustment_kind "none"
    local adjustment_variable
    local adjustment_position 0
    if `"`offset'"' != "" {
        local adjustment_kind "offset"
        local adjustment_variable `offset'
        local adjustment_position = `n_bindings' + 1
    }
    if `"`exposure'"' != "" {
        local adjustment_kind "exposure"
        local adjustment_variable `exposure'
        local adjustment_position = `n_bindings' + 1
    }
    local input_variable_count = `n_bindings' + `has_adjustment'

    local n_absorb_intercepts 0
    local n_absorb_slopes 0
    if `n_absorb_terms' > 0 {
        forvalues row = 1/`n_absorb_terms' {
            local n_absorb_intercepts = `n_absorb_intercepts' + `absorb_terms'[`row',3]
            local n_absorb_slopes = `n_absorb_slopes' + `absorb_terms'[`row',4]
        }
    }
    local output_saved_count = `n_absorb_intercepts' + `n_absorb_slopes'
    local saved_target_variables
    if `n_absorb_terms' > 0 {
        forvalues term = 1/`n_absorb_terms' {
            local saved_base : word `term' of `absorb_saved_names'
            local intercept = `absorb_terms'[`term',3]
            local slopes = `absorb_terms'[`term',4]
            local channels = `intercept' + `slopes'
            if `"`saved_base'"' == "." continue
            forvalues channel = 1/`channels' {
                local target `"`saved_base'"'
                if !`intercept' | `channel' > 1 {
                    local slope_index = `channel' - `intercept'
                    local target `"`saved_base'_Slope`slope_index'"'
                }
                local saved_target_variables `saved_target_variables' `target'
            }
        }
    }
    local all_output_names `requested_output_names' `saved_target_variables'
    local all_output_names = strtrim(`"`all_output_names'"')
    local unique_all_output_names : list uniq all_output_names
    local all_output_count : word count `all_output_names'
    local unique_all_output_count : word count `unique_all_output_names'
    if `all_output_count' != `unique_all_output_count' {
        di as error "PPML output and saved fixed-effect names must be distinct"
        exit 110
    }
    foreach output of local saved_target_variables {
        confirm new variable `output'
    }
    local requested_saved_count : word count `saved_target_variables'
    local coefficient_capacity = `n_terms' + 1
    local matrix_dimension = max(`coefficient_capacity', 1)
    local fe_rows = max(`n_absorb_intercepts' + `n_absorb_slopes', 1)
    local cluster_rows = max(`n_cluster_terms', 1)
    local subset_rows = max(2^`n_cluster_terms' - 1, 1)
    local separation_rows = max(`separation_count', 1)

    tempvar sample_shadow
    generate byte `sample_shadow' = `touse'
    local output_variables `sample_shadow'
    local output_sample = `input_variable_count' + 1
    local output_residual 0
    local output_xb 0
    local output_absorbed 0
    local output_eta 0
    local output_mu 0
    local output_dropreason 0
    if `"`residual'"' != "" {
        tempvar residual_shadow
        generate double `residual_shadow' = .
        label variable `residual_shadow' "PPML `residual_wire' residual"
        local output_variables `output_variables' `residual_shadow'
        local output_variable_count : word count `output_variables'
        local output_residual = `input_variable_count' + `output_variable_count'
    }
    if `"`xb'"' != "" {
        tempvar xb_shadow
        generate double `xb_shadow' = .
        label variable `xb_shadow' "Finite linear prediction including adjustment"
        local output_variables `output_variables' `xb_shadow'
        local output_variable_count : word count `output_variables'
        local output_xb = `input_variable_count' + `output_variable_count'
    }
    if `"`d'"' != "" {
        tempvar d_shadow
        generate double `d_shadow' = .
        label variable `d_shadow' "Absorbed contribution"
        local output_variables `output_variables' `d_shadow'
        local output_variable_count : word count `output_variables'
        local output_absorbed = `input_variable_count' + `output_variable_count'
    }
    if `"`eta_target'"' != "" {
        tempvar eta_shadow
        generate double `eta_shadow' = .
        label variable `eta_shadow' "Total PPML linear prediction"
        local output_variables `output_variables' `eta_shadow'
        local output_variable_count : word count `output_variables'
        local output_eta = `input_variable_count' + `output_variable_count'
    }
    if `"`mu'"' != "" {
        tempvar mu_shadow
        generate double `mu_shadow' = .
        label variable `mu_shadow' "PPML conditional mean"
        local output_variables `output_variables' `mu_shadow'
        local output_variable_count : word count `output_variables'
        local output_mu = `input_variable_count' + `output_variable_count'
    }
    if `"`dropreason'"' != "" {
        tempvar dropreason_shadow
        generate long `dropreason_shadow' = .
        label variable `dropreason_shadow' "fereg PPML final-sample drop reason"
        local output_variables `output_variables' `dropreason_shadow'
        local output_variable_count : word count `output_variables'
        local output_dropreason = `input_variable_count' + `output_variable_count'
    }

    local output_saved_registry
    local saved_shadow_variables
    local saved_target_cursor 0
    if `n_absorb_terms' > 0 {
        forvalues term = 1/`n_absorb_terms' {
            local saved_base : word `term' of `absorb_saved_names'
            local intercept = `absorb_terms'[`term',3]
            local slopes = `absorb_terms'[`term',4]
            local channels = `intercept' + `slopes'
            forvalues channel = 1/`channels' {
                local saved_position 0
                if `"`saved_base'"' != "." {
                    local ++saved_target_cursor
                    local target : word `saved_target_cursor' of `saved_target_variables'
                    tempvar saved_shadow
                    generate double `saved_shadow' = .
                    label variable `saved_shadow' "[FE] `target'"
                    local output_variables `output_variables' `saved_shadow'
                    local output_variable_count : word count `output_variables'
                    local saved_position = `input_variable_count' + `output_variable_count'
                    local saved_shadow_variables `saved_shadow_variables' `saved_shadow'
                }
                local saved_piece `"`=strlen("`saved_position'")':`saved_position'"'
                if `"`output_saved_registry'"' == "" local output_saved_registry `"`saved_piece'"'
                else local output_saved_registry `"`output_saved_registry'|`saved_piece'"'
            }
        }
    }
    if `output_saved_count' == 0 local output_saved_registry "."

    tempname b_shadow v_shadow status_shadow fit_shadow dof_shadow
    tempname fe_shadow cluster_shadow subset_shadow separation_shadow
    tempname inference_shadow drop_counts_shadow separation_sequence_shadow
    tempname certificate_shadow runtime_shadow receipt_shadow
    matrix `b_shadow' = J(1, `matrix_dimension', .)
    matrix `v_shadow' = J(`matrix_dimension', `matrix_dimension', .)
    matrix `status_shadow' = J(`matrix_dimension', 4, .)
    matrix `fit_shadow' = J(27, 1, .)
    matrix `dof_shadow' = J(12, 1, .)
    matrix `fe_shadow' = J(`fe_rows', 8, .)
    matrix `cluster_shadow' = J(`cluster_rows', 4, .)
    matrix `subset_shadow' = J(`subset_rows', 4, .)
    matrix `separation_shadow' = J(`separation_rows', 14, .)
    matrix `inference_shadow' = J(16, 1, 0)
    matrix `drop_counts_shadow' = J(14, 2, 0)
    matrix `separation_sequence_shadow' = J(5, 13, 0)
    matrix `certificate_shadow' = J(16, 1, .)
    matrix `runtime_shadow' = J(108, 1, .)
    matrix `receipt_shadow' = J(1, 33, 0)

    tempname held_estimates
    tempvar held_sample
    _estimates hold `held_estimates', restore nullok varname(`held_sample')
    if `"`inject'"' == "posthold" exit 3498

    local weight_variable `"`fweight'`pweight'"'
    local input_complete 0
    if `candidate_all' {
        tempvar complete_input
        quietly generate byte `complete_input' = `touse'
        quietly markout `complete_input' `depvar' `source_vars' `weight_variable' `adjustment_variable'
        quietly count if !`complete_input'
        local input_complete = (r(N) == 0)
    }

    capture quietly program list __fereg_estimate_plugin
    if _rc {
        _fereg_load, action(loadonly)
        local plugin_file `"`r(plugin_file)'"'
        capture program __fereg_estimate_plugin, plugin using(`"`plugin_file'"')
        local load_rc = _rc
        if `load_rc' != 0 & `load_rc' != 110 exit `load_rc'
    }
    local keep_singletons = (`"`keepsingletons'"' != "")
    local deterministic_wire = (`"`deterministic'"' != "")
    local allocation_audit_wire = (`"`allocationaudit'"' != "")
    local plugin_inject
    if `"`inject'"' == "panic" local plugin_inject "panic"
    if `"`inject'"' == "breaknative" local plugin_inject "breaknative"
    if `"`inject'"' == "allocation" local plugin_inject "allocation"
    local request_arguments `"ado=0.5.0;protocol_major=1;protocol_minor=5;stata=`c(stata_version)';confirm=feppml-estimation;plugin=0.5.0;result_major=1;result_minor=1;request_major=1;request_minor=4;model_major=`model_major';model_minor=`model_minor';source_matrix=`model_sources';source_count=`n_sources';term_matrix=`model_terms';term_count=`n_terms';component_matrix=`model_components';component_count=`n_components';source_names=`source_registry';canonical_tokens=`token_registry';raw_component_operators=`raw_registry';raw_time_series_operators=`ts_registry';absorb_term_matrix=`absorb_terms';absorb_term_count=`n_absorb_terms';absorb_component_matrix=`absorb_components';absorb_component_count=`n_absorb_components';absorb_labels=`absorb_label_registry';absorb_saved_names=`absorb_saved_registry';cluster_term_matrix=`cluster_terms';cluster_term_count=`n_cluster_terms';cluster_component_matrix=`cluster_components';cluster_component_count=`n_cluster_components';cluster_labels=`cluster_label_registry';binding_matrix=`input_bindings';binding_count=`n_bindings';include_intercept=1;candidate_all=`candidate_all';input_complete=`input_complete';keep_singletons=`keep_singletons';adjustment=`adjustment_kind';adjustment_position=`adjustment_position';residual=`residual_wire';vce=`vce_request_wire';memory=`memory_mode';poolsize=`poolsize';threads=`threads';deterministic=`deterministic_wire';allocation_audit=`allocation_audit_wire';finite_solver=`finite_solver';fe_solver=`fe_solver';fe_preconditioner=`fe_preconditioner';fe_lsmr_mode=`fe_lsmr_mode';fe_lsmr_policy=`fe_lsmr_policy';fe_cmg_mode=`fe_cmg_mode';solver_trace=`solver_trace';solver_experiment=`solver_experiment';tolerance=`tolerance';iterate=`iterate';separation=`separation_wire';separation_tolerance=`septolerance';separation_max_iterations=`sepiterate';simplex_max_iterations=`simplexiterate';simplex_max_tableau_cells=`simplexcells';output_sample=`output_sample';output_residual=`output_residual';output_xb=`output_xb';output_absorbed=`output_absorbed';output_eta=`output_eta';output_mean=`output_mu';output_drop_reason=`output_dropreason';output_saved_count=`output_saved_count';output_saved_channels=`output_saved_registry';output_b_matrix=`b_shadow';output_v_matrix=`v_shadow';output_status_matrix=`status_shadow';output_fit_matrix=`fit_shadow';output_dof_matrix=`dof_shadow';output_fe_matrix=`fe_shadow';output_cluster_matrix=`cluster_shadow';output_subset_matrix=`subset_shadow';output_separation_matrix=`separation_shadow';output_inference_matrix=`inference_shadow';output_drop_counts_matrix=`drop_counts_shadow';output_separation_sequence_matrix=`separation_sequence_shadow';output_certificate_matrix=`certificate_shadow';output_runtime_matrix=`runtime_shadow';output_receipt_matrix=`receipt_shadow';"'
    if `"`plugin_inject'"' != "" {
        local request_arguments `"`request_arguments'inject=`plugin_inject';"'
    }
    if `"`plugin_inject'"' == "breaknative" {
        local break_marker_token : environment FEREG_PPML_BREAK_MARKER_TOKEN
        if strtrim(`"`break_marker_token'"') == "" {
            di as error "hidden PPML breaknative requires FEREG_PPML_BREAK_MARKER_TOKEN"
            exit 198
        }
        local request_arguments ///
            `"`request_arguments'break_marker_token=`break_marker_token';"'
    }
    local request_bundle
    local request_count 0
    while `"`request_arguments'"' != "" {
        gettoken argument request_arguments : request_arguments, parse(";")
        if `"`argument'"' == ";" continue
        local argument = strtrim(`"`argument'"')
        local ++request_count
        local argument_length = strlen(`"`argument'"')
        local request_bundle `"`request_bundle'`argument_length':`argument'"'
    }
    local request_bundle `"`request_count':`request_bundle'"'
    plugin call __fereg_estimate_plugin `touse' `depvar' `source_vars' ///
        `weight_variable' `adjustment_variable' `output_variables', ///
        `"estimate-ppml"' `"bundle=`request_bundle'"'

    // Developer-only publication-boundary mutations prove that malformed
    // native products are classified and rolled back without a second call.
    if `"`inject'"' == "mutatewire" matrix `receipt_shadow'[1,1] = 0
    if `"`inject'"' == "mutatesemantic" matrix `certificate_shadow'[16,1] = 0
    if `"`inject'"' == "mutateoutput" replace `sample_shadow' = 2 in 1

    if `"`inject'"' == "breakpostplugin" {
        di as result "FEPPML_BREAK_ARMED"
        sleep 30000
        exit 3498
    }
    if `"`inject'"' == "postplugin" exit 3498

    capture noisily _fereg_eppml_validate `b_shadow' `v_shadow' `status_shadow' ///
        `fit_shadow' `dof_shadow' `fe_shadow' `cluster_shadow' ///
        `subset_shadow' `separation_shadow' `inference_shadow' ///
        `drop_counts_shadow' `separation_sequence_shadow' ///
        `certificate_shadow' `runtime_shadow' `receipt_shadow', ///
        capacity(`coefficient_capacity') ///
        clusters(`n_cluster_terms') ferows(`=`n_absorb_intercepts' + `n_absorb_slopes'') ///
        subsets(`=cond(`n_cluster_terms' == 0, 0, 2^`n_cluster_terms' - 1)') ///
        separations(`separation_count') residual(`=(`"`residual'"' != "")') ///
        saved(`requested_saved_count') ///
        adjustment(`=cond(`"`adjustment_kind'"' == "none", 0, cond(`"`adjustment_kind'"' == "offset", 1, 2))') ///
        vcecode(`=cond(`"`vce_wire'"' == "cluster", 3, 2)') ///
        requestedvcecode(`=cond(`"`vce_request_wire'"' == "unadjusted", 1, cond(`"`vce_request_wire'"' == "cluster", 3, 2))') ///
        weightcode(`=cond(`has_fweight', 1, cond(`has_pweight', 2, 0))') ///
        memorycode(`=cond(`"`memory_mode'"' == "auto", 0, cond(`"`memory_mode'"' == "fast", 1, 2))') ///
        requestedthreads(`threads') feterms(`n_absorb_terms') sources(`n_sources') ///
        methodcodes(`"`separation_codes'"') dropsingletons(`=1 - `keep_singletons'')
    local validation_rc = _rc
    if `validation_rc' {
        if `validation_rc' == 499 exit 499
        exit 3499
    }
    local coefficient_count = r(coefficients)

    // The native result tables carry enough identity to prove that the
    // rebased FE/cluster topology is still the parser-owned declaration.  Do
    // this comparison outside the generic layout validator because the input
    // metadata matrices remain private to this transaction.
    if `n_absorb_terms' > 0 {
        local fe_cursor 0
        forvalues term = 1/`n_absorb_terms' {
            local categorical_count = `absorb_terms'[`term',2]
            local intercept = `absorb_terms'[`term',3]
            local slopes = `absorb_terms'[`term',4]
            if `intercept' {
                local ++fe_cursor
                local expected_kind = cond(`categorical_count' == 1, 1, 3)
                local expected_source 0
                if `categorical_count' == 1 {
                    forvalues component = 1/`n_absorb_components' {
                        if `absorb_components'[`component',1] == `term' & ///
                           `absorb_components'[`component',2] == 1 & ///
                           `absorb_components'[`component',3] == 1 {
                            local expected_source = `absorb_components'[`component',4]
                        }
                    }
                }
                if `fe_shadow'[`fe_cursor',1] != `expected_kind' | ///
                   `fe_shadow'[`fe_cursor',2] != `term' | ///
                   `fe_shadow'[`fe_cursor',3] != `expected_source' {
                    di as error "PPML fixed-effect intercept identity disagrees with absorb()"
                    exit 3499
                }
            }
            if `slopes' > 0 {
                forvalues slope = 1/`slopes' {
                    local ++fe_cursor
                    local expected_source 0
                    forvalues component = 1/`n_absorb_components' {
                        if `absorb_components'[`component',1] == `term' & ///
                           `absorb_components'[`component',2] == 2 & ///
                           `absorb_components'[`component',3] == `slope' {
                            local expected_source = `absorb_components'[`component',4]
                        }
                    }
                    if `expected_source' == 0 | ///
                       `fe_shadow'[`fe_cursor',1] != 4 | ///
                       `fe_shadow'[`fe_cursor',2] != `term' | ///
                       `fe_shadow'[`fe_cursor',3] != `expected_source' {
                        di as error "PPML fixed-effect slope identity disagrees with absorb()"
                        exit 3499
                    }
                }
            }
        }
        if `fe_cursor' != `n_absorb_intercepts' + `n_absorb_slopes' {
            di as error "PPML fixed-effect channel count disagrees with absorb()"
            exit 3499
        }
    }
    if `n_cluster_terms' > 0 {
        forvalues cluster_row = 1/`n_cluster_terms' {
            if `cluster_shadow'[`cluster_row',2] != ///
               `cluster_terms'[`cluster_row',2] {
                di as error "PPML cluster arity disagrees with vce(cluster ...)"
                exit 3499
            }
        }
    }

    capture assert inlist(`sample_shadow', 0, 1) & !missing(`sample_shadow')
    if _rc {
        di as error "PPML sample output is not a total binary map"
        exit 3499
    }
    capture assert `sample_shadow' <= `touse'
    if _rc {
        di as error "PPML sample output escaped the Stata candidate sample"
        exit 3499
    }
    quietly count if `sample_shadow'
    if r(N) != `receipt_shadow'[1,9] {
        di as error "PPML sample output disagrees with the committed final row count"
        exit 3499
    }
    foreach shadow in `residual_shadow' `xb_shadow' `d_shadow' ///
        `eta_shadow' `mu_shadow' `saved_shadow_variables' {
        if `"`shadow'"' == "" continue
        capture assert `shadow' < . if `sample_shadow'
        if _rc {
            di as error "PPML observation output contains a missing final-sample value"
            exit 3499
        }
        capture assert missing(`shadow') if !`sample_shadow'
        if _rc {
            di as error "PPML observation output was published outside the final sample"
            exit 3499
        }
    }
    if `"`mu'"' != "" {
        capture assert `mu_shadow' > 0 if `sample_shadow'
        if _rc {
            di as error "PPML mean output is not strictly positive"
            exit 3499
        }
    }
    if `"`dropreason'"' != "" {
        capture assert `dropreason_shadow' == int(`dropreason_shadow') & ///
            inrange(`dropreason_shadow', 0, 13)
        if _rc {
            di as error "PPML drop-reason output is invalid"
            exit 3499
        }
        capture assert (`dropreason_shadow' == 0) == `sample_shadow'
        if _rc {
            di as error "PPML drop reasons disagree with the final sample"
            exit 3499
        }
        forvalues drop_reason = 0/13 {
            quietly count if `dropreason_shadow' == `drop_reason'
            if r(N) != `drop_counts_shadow'[`=`drop_reason' + 1',2] {
                di as error "PPML drop-reason output disagrees with its bounded count table"
                exit 3499
            }
        }
    }

    if `coefficient_count' < `matrix_dimension' {
        matrix `b_shadow' = `b_shadow'[1, 1..`coefficient_count']
        matrix `v_shadow' = `v_shadow'[1..`coefficient_count', 1..`coefficient_count']
        matrix `status_shadow' = `status_shadow'[1..`coefficient_count', 1..4]
    }

    // Coefficient identity belongs to the original parser-owned model axis.
    // A factor level may vanish during domain filtering or separation while
    // its base/omission slot remains part of the certified result.
    local posted_names `"`coefficient_names' _cons"'
    local posted_names = strtrim(`"`posted_names'"')
    local posted_name_count : word count `posted_names'
    if `posted_name_count' != `coefficient_count' {
        di as error "PPML coefficient-name count disagrees with receipt"
        exit 3499
    }
    matrix colnames `b_shadow' = `posted_names'
    matrix colnames `v_shadow' = `posted_names'
    matrix rownames `v_shadow' = `posted_names'
    tempname marker_b marker_v posted_b posted_v
    matrix `marker_b' = J(1, `coefficient_count', 1)
    matrix `marker_v' = I(`coefficient_count')
    forvalues slot = 1/`coefficient_count' {
        if `status_shadow'[`slot',2] != 1 {
            matrix `marker_b'[1,`slot'] = 0
            matrix `marker_v'[`slot',`slot'] = 0
        }
    }
    matrix colnames `marker_b' = `posted_names'
    matrix colnames `marker_v' = `posted_names'
    matrix rownames `marker_v' = `posted_names'
    _ms_findomitted `marker_b' `marker_v'
    local marked_names : colfullnames `marker_b'
    mata: st_matrix("`posted_b'", st_matrix("`b_shadow'"))
    mata: st_matrix("`posted_v'", st_matrix("`v_shadow'"))
    matrix colnames `posted_b' = `marked_names'
    matrix colnames `posted_v' = `marked_names'
    matrix rownames `posted_v' = `marked_names'
    if `"`inject'"' == "poststripe" exit 3498

    local N_effective = `fit_shadow'[17,1]
    local eweight
    if `has_fweight' local eweight "[fweight=`fweight']"
    if `has_pweight' local eweight "[pweight=`pweight']"
    ereturn post `posted_b' `posted_v' `eweight', depname(`depvar') ///
        obs(`N_effective') esample(`sample_shadow') buildfvinfo
    if `"`inject'"' == "postereturn" exit 3498

    ereturn scalar rank = `dof_shadow'[2,1]
    ereturn scalar feppml_model_rank = `dof_shadow'[4,1]
    ereturn scalar df_m = `fit_shadow'[12,1]
    ereturn scalar df_resid = `dof_shadow'[6,1]
    ereturn scalar df_structural = `dof_shadow'[5,1]
    ereturn scalar df_a = `dof_shadow'[3,1]
    ereturn scalar N_physical = `fit_shadow'[1,1]
    ereturn scalar N_effective = `fit_shadow'[17,1]
    ereturn scalar N_candidate = `fit_shadow'[22,1]
    ereturn scalar N_full = `fit_shadow'[23,1]
    ereturn scalar deviance = `fit_shadow'[6,1]
    ereturn scalar ll = `fit_shadow'[7,1]
    ereturn scalar ll_0 = `fit_shadow'[8,1]
    ereturn scalar r2_p = `fit_shadow'[9,1]
    ereturn scalar mu_0 = `fit_shadow'[10,1]
    if `fit_shadow'[11,1] < . {
        ereturn scalar chi2 = `fit_shadow'[11,1]
        ereturn scalar df_chi2 = `fit_shadow'[12,1]
    }
    ereturn scalar sum_y = `fit_shadow'[13,1]
    ereturn scalar ic = `fit_shadow'[14,1]
    ereturn scalar inner_iterations = `fit_shadow'[15,1]
    ereturn scalar step_halvings = `fit_shadow'[16,1]
    ereturn scalar sumweights = `fit_shadow'[18,1]
    if `fit_shadow'[19,1] < . ereturn scalar raw_sumweights = `fit_shadow'[19,1]
    if `fit_shadow'[20,1] < . ereturn scalar weight_multiplier = `fit_shadow'[20,1]
    ereturn scalar feppml_weight_code = `fit_shadow'[21,1]
    ereturn scalar converged = `fit_shadow'[24,1]
    if `fit_shadow'[25,1] < . ereturn scalar log_raw_sumweights = `fit_shadow'[25,1]
    ereturn scalar log_weight_multiplier = `fit_shadow'[26,1]
    ereturn scalar feppml_objective_scale_code = `fit_shadow'[27,1]
    ereturn scalar separated = `receipt_shadow'[1,16]
    ereturn scalar feppml_vce_requested = `inference_shadow'[1,1]
    ereturn scalar feppml_vce_effective = `inference_shadow'[2,1]
    ereturn scalar feppml_vce_correction = `inference_shadow'[3,1]
    ereturn scalar feppml_vce_correction_policy = `inference_shadow'[4,1]
    ereturn scalar feppml_coef_distribution = `inference_shadow'[5,1]
    ereturn scalar feppml_psd_state = `inference_shadow'[6,1]
    ereturn scalar feppml_psd_adjusted = `inference_shadow'[7,1]
    ereturn scalar feppml_psd_clipped = `inference_shadow'[8,1]
    ereturn scalar feppml_psd_raw_min = `inference_shadow'[9,1]
    ereturn scalar feppml_psd_final_min = `inference_shadow'[10,1]
    ereturn scalar feppml_psd_final_rank = `inference_shadow'[11,1]
    ereturn scalar feppml_psd_eigen_relinf = `inference_shadow'[12,1]
    ereturn scalar feppml_psd_orthog_relinf = `inference_shadow'[13,1]
    ereturn scalar feppml_psd_clip_relinf = `inference_shadow'[14,1]
    ereturn scalar feppml_cert_tolerance = `inference_shadow'[15,1]
    ereturn scalar feppml_psd_passed = `inference_shadow'[16,1]
    ereturn scalar feppml_sep_executions = `receipt_shadow'[1,23]
    ereturn scalar feppml_sep_generations = `receipt_shadow'[1,24]
    ereturn scalar feppml_initial_hash_hi = `receipt_shadow'[1,25]
    ereturn scalar feppml_initial_hash_lo = `receipt_shadow'[1,26]
    ereturn scalar feppml_final_hash_hi = `receipt_shadow'[1,27]
    ereturn scalar feppml_final_hash_lo = `receipt_shadow'[1,28]
    ereturn scalar feppml_trace_digest_hi = `receipt_shadow'[1,29]
    ereturn scalar feppml_trace_digest_lo = `receipt_shadow'[1,30]
    ereturn scalar feppml_hash_digest_hi = `receipt_shadow'[1,31]
    ereturn scalar feppml_hash_digest_lo = `receipt_shadow'[1,32]
    ereturn scalar num_singletons = `drop_counts_shadow'[9,2]
    ereturn scalar drop_singletons = 1 - `keep_singletons'
    ereturn scalar report_constant = 1
    ereturn scalar N_hdfe = `n_absorb_terms'
    ereturn scalar N_hdfe_extended = `=`n_absorb_intercepts' + `n_absorb_slopes''
    ereturn scalar feppml_rank_is_approximate = `dof_shadow'[9,1]
    ereturn scalar feppml_connected_components = `dof_shadow'[8,1]
    ereturn scalar feppml_cluster_df_cap = `dof_shadow'[12,1]
    if `dof_shadow'[7,1] < . ereturn scalar G_min = `dof_shadow'[7,1]
    ereturn scalar feppml_memory_requested_code = `runtime_shadow'[5,1]
    ereturn scalar feppml_memory_selected_code = `runtime_shadow'[6,1]
    ereturn scalar feppml_linear_route = `runtime_shadow'[7,1]
    ereturn scalar feppml_fallback_mask = `runtime_shadow'[8,1]
    ereturn scalar feppml_fallback_count = `runtime_shadow'[9,1]
    ereturn scalar feppml_requested_threads = `runtime_shadow'[10,1]
    ereturn scalar feppml_threads = `runtime_shadow'[11,1]
    ereturn scalar feppml_memory_fast_bytes = `runtime_shadow'[12,1]
    ereturn scalar feppml_memory_low_bytes = `runtime_shadow'[13,1]
    ereturn scalar feppml_memory_selected_bytes = `runtime_shadow'[14,1]
    ereturn scalar feppml_modeled_scratch_bytes = `runtime_shadow'[15,1]
    ereturn scalar feppml_scratch_bytes = `runtime_shadow'[16,1]
    local allocator_names allocation_previous allocation_start allocation_current ///
        allocation_peak allocation_peak_delta allocation_growth allocation_model_ratio
    forvalues allocator_row = 17/23 {
        if `runtime_shadow'[`allocator_row',1] < . {
            local allocator_name : word `=`allocator_row' - 16' of `allocator_names'
            ereturn scalar feppml_`allocator_name' = `runtime_shadow'[`allocator_row',1]
        }
    }
    ereturn scalar feppml_workspace_allocation_free = `runtime_shadow'[24,1]
    ereturn scalar feppml_iteration_allocation_free = `runtime_shadow'[25,1]
    ereturn scalar feppml_diagnostic_records = `runtime_shadow'[26,1]
    ereturn scalar feppml_strict_profile_rejections = `runtime_shadow'[27,1]
    local phase_names preparation separation_fe separation_simplex separation_ir ///
        separation_mu weight_refresh fe_projection finite_solve ///
        line_search strict_final_solve inference fe_recovery publication
    forvalues phase_row = 28/40 {
        local phase_name : word `=`phase_row' - 27' of `phase_names'
        if `runtime_shadow'[`phase_row',1] < . {
            ereturn scalar feppml_time_`phase_name' = `runtime_shadow'[`phase_row',1]
        }
    }
    local audit_scalar_names allocation_audit_present audit_iterations ///
        audit_events_start audit_events_end audit_events_delta ///
        audit_current_start audit_current_end audit_current_growth ///
        audit_peak_start audit_peak_end audit_peak_growth ///
        ppml_obs_cap_start ppml_coef_cap_start ppml_diag_cap_start ///
        ppml_obs_cap_end ppml_coef_cap_end ppml_diag_cap_end ///
        ppml_capacity_stable linear_elem_cap_start linear_res_elem_start ///
        linear_scratch_start linear_fe_bytes_start linear_elem_cap_end ///
        linear_res_elem_end linear_scratch_end linear_fe_bytes_end ///
        linear_capacity_stable audit_scratch_start audit_scratch_end ///
        audit_scratch_stable audit_linear_route audit_route_qualified ///
        linear_session_alloc_free fresh_warm_equivalent steady_median_ns ///
        allocation_cert_passed two_consecutive rank_frozen ///
        repeated_baseline_growth action_events_start action_events_end ///
        action_events_delta
    forvalues runtime_row = 41/82 {
        if `runtime_shadow'[`runtime_row',1] < . {
            local scalar_name : word `=`runtime_row' - 40' of `audit_scalar_names'
            ereturn scalar feppml_`scalar_name' = `runtime_shadow'[`runtime_row',1]
        }
    }
    local peak_phase_names prep sep_fe sep_simplex sep_ir sep_mu ///
        weight feproj finite line_search final_solve ///
        inference fe_recovery publication
    forvalues peak_index = 1/13 {
        local peak_name : word `peak_index' of `peak_phase_names'
        ereturn scalar feppml_modeled_peak_`peak_name' = ///
            `runtime_shadow'[`=82 + `peak_index'',1]
        if `runtime_shadow'[`=95 + `peak_index'',1] < . {
            ereturn scalar feppml_observed_peak_`peak_name' = ///
                `runtime_shadow'[`=95 + `peak_index'',1]
        }
    }
    if `"`inject'"' == "postscalars" exit 3498

    matrix colnames `status_shadow' = slot status kept omitted
    matrix rownames `status_shadow' = `marked_names'
    matrix rownames `fit_shadow' = N_physical rank_finite rank_absorbed ///
        df_structural df_reported deviance ll ll_0 r2_p mu_0 chi2 ///
        df_chi2 sum_y outer_iterations inner_iterations step_halvings ///
        N_effective sumweights raw_sumweights weight_multiplier ///
        weight_code N_candidate N_original converged log_raw_sumweights ///
        log_weight_multiplier objective_scale_code
    matrix colnames `fit_shadow' = value
    matrix rownames `dof_shadow' = N_effective rank_finite rank_absorbed ///
        rank_model df_structural df_reported N_clust components ///
        rank_approximate N_physical N_clustervars df_cluster_cap
    matrix colnames `dof_shadow' = value
    matrix colnames `fe_shadow' = kind term source categories redundant rank ///
        approximate nested
    local fe_row_names
    forvalues row = 1/`fe_rows' {
        local fe_row_names `fe_row_names' fe`row'
    }
    matrix rownames `fe_shadow' = `fe_row_names'
    matrix colnames `cluster_shadow' = dimension source_count groups reserved
    local cluster_row_names
    forvalues row = 1/`cluster_rows' {
        local cluster_row_names `cluster_row_names' cluster`row'
    }
    matrix rownames `cluster_shadow' = `cluster_row_names'
    matrix colnames `subset_shadow' = mask groups sign cardinality
    local subset_row_names
    forvalues row = 1/`subset_rows' {
        local subset_mask = `subset_shadow'[`row',1]
        local subset_row_names `subset_row_names' subset`subset_mask'
    }
    matrix rownames `subset_shadow' = `subset_row_names'
    matrix colnames `separation_shadow' = method status executions generation ///
        iterations direct_drops singleton_drops certificates interior_error ///
        boundary_violation span_residual negative_margin checked_effects zero_levels
    local separation_row_names
    forvalues row = 1/`separation_rows' {
        local method_code = `separation_shadow'[`row',1]
        if `method_code' == 1 local method_name "singleton"
        else if `method_code' == 2 local method_name "fe"
        else if `method_code' == 3 local method_name "simplex"
        else if `method_code' == 4 local method_name "ir"
        else local method_name "mu"
        local separation_row_names `separation_row_names' `method_name'
    }
    matrix rownames `separation_shadow' = `separation_row_names'
    matrix rownames `inference_shadow' = requested_vce_code ///
        effective_vce_code correction correction_policy_code ///
        coefficient_distribution_code psd_state psd_adjusted ///
        psd_clipped_eigenvalues psd_raw_minimum_eigenvalue ///
        psd_final_minimum_eigenvalue psd_final_rank ///
        psd_eigensystem_recon_relinf psd_orthogonality_relinf ///
        psd_clipped_recon_relinf certificate_tolerance psd_passed
    matrix colnames `inference_shadow' = value
    matrix rownames `drop_counts_shadow' = retained excluded_if_in ///
        missing_outcome missing_regressor invalid_or_missing_weight ///
        missing_fixed_effect missing_cluster missing_adjustment singleton ///
        fixed_effect_separation simplex_separation ir_separation ///
        mu_separation other_model_exclusion
    matrix colnames `drop_counts_shadow' = reason count
    matrix rownames `separation_sequence_shadow' = singleton fe simplex ir mu
    matrix colnames `separation_sequence_shadow' = method requested ///
        registered_order executions first_trace_row last_trace_row ///
        status_mask total_iterations direct_drops singleton_drops ///
        first_generation last_generation all_certificates_passed
    matrix rownames `certificate_shadow' = score_finite_absinf score_fe_absinf ///
        score_relinf score_rel2 linear_within_relinf linear_reconstruct_relinf ///
        predictor_reconstruct_relinf mean_reconstruct_relinf ///
        saved_total_relinf saved_channels_relinf bread_symmetry_relinf ///
        meat_relinf covariance_relinf covariance_symmetry_relinf ///
        wald_solve_relinf composite_passed
    matrix colnames `certificate_shadow' = value
    matrix rownames `runtime_shadow' = total_seconds outer_iterations ///
        inner_iterations step_halvings memory_requested memory_selected ///
        linear_route fallback_mask fallback_count threads_requested ///
        threads_effective modeled_fast_bytes modeled_low_bytes ///
        modeled_selected_bytes modeled_scratch_bytes observed_scratch_bytes ///
        allocation_previous allocation_start allocation_current allocation_peak ///
        allocation_peak_delta allocation_growth allocation_model_ratio ///
        workspace_allocation_free iteration_allocation_free diagnostic_records ///
        strict_profile_rejections time_preparation time_separation_fe ///
        time_separation_simplex time_separation_ir time_separation_mu ///
        time_weight_response_refresh time_fe_projection time_finite_solve ///
        time_objective_line_search time_strict_final_solve time_inference ///
        time_fe_recovery time_publication allocation_audit_present ///
        audit_iterations audit_events_start audit_events_end ///
        audit_events_delta audit_current_start audit_current_end ///
        audit_current_growth audit_peak_start audit_peak_end ///
        audit_peak_growth ppml_obs_capacity_start ppml_coef_capacity_start ///
        ppml_diag_capacity_start ppml_obs_capacity_end ppml_coef_capacity_end ///
        ppml_diag_capacity_end ppml_capacity_stable ///
        linear_element_capacity_start linear_resident_elements_start ///
        linear_scratch_bytes_start linear_fe_topology_bytes_start ///
        linear_element_capacity_end linear_resident_elements_end ///
        linear_scratch_bytes_end linear_fe_topology_bytes_end ///
        linear_capacity_stable audit_scratch_bytes_start ///
        audit_scratch_bytes_end audit_scratch_stable audit_linear_route ///
        audit_route_qualified linear_session_allocation_free ///
        fresh_warm_equivalent steady_iteration_median_ns ///
        allocation_certificate_passed two_consecutive_convergence ///
        rank_frozen repeated_baseline_growth action_events_start ///
        action_events_end action_events_delta modeled_peak_prep ///
        modeled_peak_sep_fe modeled_peak_sep_simplex modeled_peak_sep_ir ///
        modeled_peak_sep_mu modeled_peak_weight_refresh ///
        modeled_peak_fe_projection modeled_peak_finite_solve ///
        modeled_peak_line_search modeled_peak_final_solve ///
        modeled_peak_inference modeled_peak_fe_recovery ///
        modeled_peak_publication observed_peak_prep observed_peak_sep_fe ///
        observed_peak_sep_simplex observed_peak_sep_ir observed_peak_sep_mu ///
        observed_peak_weight_refresh observed_peak_fe_projection ///
        observed_peak_finite_solve observed_peak_line_search ///
        observed_peak_final_solve observed_peak_inference ///
        observed_peak_fe_recovery observed_peak_publication
    matrix colnames `runtime_shadow' = value
    matrix colnames `receipt_shadow' = magic major minor vce coefficients ///
        covariance_dimension N_original N_candidate N_final cluster_dimensions ///
        fixed_effect_rows subset_rows sample_present residual_present ///
        certificates_passed separated separation_rows saved_channels adjustment ///
        inference_rows drop_count_rows sequence_rows separation_executions ///
        separation_generations initial_hash_hi initial_hash_lo final_hash_hi ///
        final_hash_lo trace_digest_hi trace_digest_lo hash_digest_hi ///
        hash_digest_lo complete
    matrix rownames `receipt_shadow' = receipt

    ereturn matrix feppml_status = `status_shadow', copy
    ereturn matrix feppml_fit = `fit_shadow', copy
    ereturn matrix feppml_dof = `dof_shadow', copy
    ereturn matrix feppml_fe = `fe_shadow', copy
    ereturn matrix feppml_cluster = `cluster_shadow', copy
    ereturn matrix feppml_subset = `subset_shadow', copy
    ereturn matrix feppml_separation = `separation_shadow', copy
    ereturn matrix feppml_inference = `inference_shadow', copy
    ereturn matrix feppml_drop_counts = `drop_counts_shadow', copy
    ereturn matrix feppml_separation_sequence = `separation_sequence_shadow', copy
    ereturn matrix feppml_certificate = `certificate_shadow', copy
    ereturn matrix feppml_runtime = `runtime_shadow', copy
    ereturn matrix feppml_receipt = `receipt_shadow', copy
    if `n_absorb_intercepts' + `n_absorb_slopes' > 0 {
        tempname posted_dof_table
        local actual_fe_rows = `n_absorb_intercepts' + `n_absorb_slopes'
        matrix `posted_dof_table' = J(`actual_fe_rows', 5, 0)
        local dof_row_names
        forvalues row = 1/`actual_fe_rows' {
            matrix `posted_dof_table'[`row',1] = `fe_shadow'[`row',4]
            matrix `posted_dof_table'[`row',2] = `fe_shadow'[`row',5]
            matrix `posted_dof_table'[`row',3] = `fe_shadow'[`row',6]
            matrix `posted_dof_table'[`row',4] = `fe_shadow'[`row',7]
            matrix `posted_dof_table'[`row',5] = `fe_shadow'[`row',8]
            local term = `fe_shadow'[`row',2]
            local dof_row_name
            forvalues component = 1/`n_absorb_components' {
                if `absorb_components'[`component',1] == `term' & ///
                   `absorb_components'[`component',2] == 1 {
                    local source_id = `absorb_components'[`component',4]
                    local source : word `source_id' of `source_vars'
                    if `"`dof_row_name'"' == "" local dof_row_name `"1.`source'"'
                    else local dof_row_name `"`dof_row_name'#1.`source'"'
                }
            }
            if `fe_shadow'[`row',1] == 4 {
                local slope_source_id = `fe_shadow'[`row',3]
                local slope_source : word `slope_source_id' of `source_vars'
                local dof_row_name `"`dof_row_name'#c.`slope_source'"'
            }
            local dof_row_names `"`dof_row_names' `dof_row_name'"'
        }
        matrix rownames `posted_dof_table' = `dof_row_names'
        mata: st_matrixcolstripe("`posted_dof_table'", ///
            (J(5,1,""), ("Categories" \ "Redundant" \ "Num Coefs" \ ///
            "Inexact?" \ "Nested?")))
        ereturn matrix dof_table = `posted_dof_table', copy
    }
    local intended_indepvars
    if `n_terms' > 0 {
        forvalues slot = 1/`n_terms' {
            if `status_shadow'[`slot',2] == 2 continue
            local name : word `slot' of `coefficient_names'
            local remaining `"`name'"'
            local intended_name
            while `"`remaining'"' != "" {
                gettoken component remaining : remaining, parse("#")
                if `"`component'"' == "#" {
                    local intended_name `"`intended_name'#"'
                    continue
                }
                local intended_component `"`component'"'
                if substr(`"`component'"', 1, 2) == "o." {
                    local intended_component = substr(`"`component'"', 3, .)
                }
                else if regexm(`"`component'"', "^(-?[0-9]+)(b|bn|o)?[.](.+)$") {
                    local factor_level = regexs(1)
                    local factor_variable = regexs(3)
                    local intended_component `"`factor_level'bn.`factor_variable'"'
                }
                local intended_name `"`intended_name'`intended_component'"'
            }
            local intended_indepvars `"`intended_indepvars' `intended_name'"'
        }
    }
    local intended_indepvars = strtrim(`"`intended_indepvars'"')
    if `"`publicmode'"' == "" {
        ereturn local cmd "_fereg_estimate_ppml"
        ereturn local cmdline `"`hidden_cmdline'"'
    }
    else {
        ereturn local cmd "feppml"
        ereturn local cmdline `"`publiccmdline'"'
        ereturn local feppml_gate "public_ppml_v1"
        ereturn local feppml_memory_requested "`memory_mode'"
        ereturn scalar feppml_poolsize = `poolsize'
        ereturn scalar feppml_requested_tolerance = `tolerance'
        ereturn scalar feppml_iterate = `iterate'
        ereturn scalar feppml_timeit = (`"`publictimeit'"' != "")
    }
    ereturn local model "poisson"
    ereturn local depvar "`depvar'"
    ereturn local indepvars `"`intended_indepvars'"'
    ereturn local vce `"`vce_wire'"'
    ereturn local vcetype "Robust"
    ereturn local absvars `"`absorb_labels'"'
    ereturn local extended_absvars `"`absorb_labels'"'
    ereturn local separation `"`separation_wire'"'
    ereturn local memory `"`memory_mode'"'
    ereturn local chi2type "Wald"
    ereturn local title "Poisson regression with high-dimensional fixed effects"
    if `n_absorb_terms' > 0 {
        ereturn local title2 "Absorbing `n_absorb_terms' HDFE group"
    }
    ereturn scalar poolsize = `poolsize'
    ereturn scalar requested_threads = `threads'
    ereturn scalar deterministic = `deterministic_wire'
    ereturn local adjustment `"`adjustment_kind'"'
    if `"`offset'"' != "" ereturn local offset "`offset'"
    if `"`exposure'"' != "" ereturn local exposure "`exposure'"
    if `has_fweight' {
        ereturn local wtype "fweight"
        if `"`publicmode'"' != "" & strtrim(`"`publicwexp'"') != "" {
            ereturn local wexp `"`publicwexp'"'
        }
        else ereturn local wexp "= `fweight'"
    }
    if `has_pweight' {
        ereturn local wtype "pweight"
        if `"`publicmode'"' != "" & strtrim(`"`publicwexp'"') != "" {
            ereturn local wexp `"`publicwexp'"'
        }
        else ereturn local wexp "= `pweight'"
    }
    if `"`vce_wire'"' == "cluster" {
        local posted_cluster_labels = strtrim(`"`cluster_labels'"')
        local minimum_clusters .
        forvalues row = 1/`n_cluster_terms' {
            local groups = `cluster_shadow'[`row',3]
            if missing(`minimum_clusters') | `groups' < `minimum_clusters' {
                local minimum_clusters = `groups'
            }
            local cluster_label : word `row' of `cluster_labels'
            ereturn scalar N_clust`row' = `groups'
            ereturn local clustvar`row' `"`cluster_label'"'
        }
        ereturn scalar N_clust = `minimum_clusters'
        ereturn scalar N_clustervars = `n_cluster_terms'
        ereturn local clustvar `"`posted_cluster_labels'"'
        ereturn local title3 "Statistics robust to heteroskedasticity and clustering"
    }
    if `"`residual'"' != "" {
        ereturn local resid "`residual'"
        ereturn local residual_type "`residual_wire'"
    }
    if `"`xb'"' != "" ereturn local xb_var "`xb'"
    if `"`d'"' != "" ereturn local d_var "`d'"
    if `"`eta_target'"' != "" ereturn local eta_var "`eta_target'"
    if `"`mu'"' != "" ereturn local mu_var "`mu'"
    if `"`dropreason'"' != "" ereturn local drop_reason_var "`dropreason'"
    ereturn local predict "feppml_p"
    ereturn local feppml_action_protocol "1.5"
    ereturn local feppml_model_wire "2.1"
    ereturn local feppml_request_wire "1.4"
    ereturn local feppml_result_wire "1.1"
    ereturn local feppml_plugin "0.5.0"
    if `"`inject'"' == "breakpostpublic" {
        di as result "FEPPML_BREAK_ARMED"
        sleep 30000
        exit 3498
    }
    if `"`inject'"' == "postpublic" exit 3498
    if `"`inject'"' == "postdiagnostics" exit 3498
    if `"`inject'"' == "precommit" exit 3498

    nobreak {
        if `"`residual'"' != "" rename `residual_shadow' `residual'
        if `"`xb'"' != "" rename `xb_shadow' `xb'
        if `"`d'"' != "" rename `d_shadow' `d'
        if `"`eta_target'"' != "" rename `eta_shadow' `eta_target'
        if `"`mu'"' != "" rename `mu_shadow' `mu'
        if `"`dropreason'"' != "" rename `dropreason_shadow' `dropreason'
        local n_saved_targets : word count `saved_target_variables'
        if `n_saved_targets' > 0 {
            forvalues saved_index = 1/`n_saved_targets' {
                local saved_shadow : word `saved_index' of `saved_shadow_variables'
                local saved_target : word `saved_index' of `saved_target_variables'
                rename `saved_shadow' `saved_target'
            }
        }
        _estimates unhold `held_estimates', not
        if `"`publictimeit'"' != "" {
            di as txt "feppml native total-to-commit (seconds): " ///
                as result %9.4f `runtime_shadow'[1,1]
        }
        if `"`publicdisplay'"' != "" ereturn display
    }
end


program define _fereg_eppml_validate, rclass
    version 18.0
    syntax namelist(min=15 max=15), CAPACITY(integer) CLUSTERS(integer) ///
        FEROWS(integer) SUBSETS(integer) SEPARATIONS(integer) ///
        RESIDUAL(integer) SAVED(integer) ADJUSTMENT(integer) VCECODE(integer) ///
        REQUESTEDVCECODE(integer) DROPSINGLETONS(integer) ///
        WEIGHTCODE(integer) MEMORYCODE(integer) REQUESTEDTHREADS(integer) ///
        FETERMS(integer) SOURCES(integer) METHODCodes(string)
    local b : word 1 of `namelist'
    local V : word 2 of `namelist'
    local status : word 3 of `namelist'
    local fit : word 4 of `namelist'
    local dof : word 5 of `namelist'
    local fe : word 6 of `namelist'
    local cluster : word 7 of `namelist'
    local subset : word 8 of `namelist'
    local separation : word 9 of `namelist'
    local inference : word 10 of `namelist'
    local drop_counts : word 11 of `namelist'
    local separation_sequence : word 12 of `namelist'
    local certificate : word 13 of `namelist'
    local runtime : word 14 of `namelist'
    local receipt : word 15 of `namelist'
    local dimension = max(`capacity', 1)
    if rowsof(`b') != 1 | colsof(`b') != `dimension' | ///
       rowsof(`V') != `dimension' | colsof(`V') != `dimension' | ///
       rowsof(`status') != `dimension' | colsof(`status') != 4 | ///
       rowsof(`fit') != 27 | colsof(`fit') != 1 | ///
       rowsof(`dof') != 12 | colsof(`dof') != 1 | ///
       rowsof(`fe') != max(`ferows', 1) | colsof(`fe') != 8 | ///
       rowsof(`cluster') != max(`clusters', 1) | colsof(`cluster') != 4 | ///
       rowsof(`subset') != max(`subsets', 1) | colsof(`subset') != 4 | ///
       rowsof(`separation') != max(`separations', 1) | colsof(`separation') != 14 | ///
       rowsof(`inference') != 16 | colsof(`inference') != 1 | ///
       rowsof(`drop_counts') != 14 | colsof(`drop_counts') != 2 | ///
       rowsof(`separation_sequence') != 5 | colsof(`separation_sequence') != 13 | ///
       rowsof(`certificate') != 16 | colsof(`certificate') != 1 | ///
       rowsof(`runtime') != 108 | colsof(`runtime') != 1 | ///
       rowsof(`receipt') != 1 | colsof(`receipt') != 33 {
        di as error "PPML result layout failed exact structural validation"
        exit 499
    }
    local coefficients = `receipt'[1,5]
    if `receipt'[1,1] != 5354602 | `receipt'[1,2] != 1 | ///
       `receipt'[1,3] != 1 | `receipt'[1,20] != 16 | ///
       `receipt'[1,21] != 14 | `receipt'[1,22] != 5 | ///
       `receipt'[1,33] != 1 {
        di as error "PPML receipt wire header or COMPLETE marker is invalid"
        exit 499
    }
    if `receipt'[1,4] != `vcecode' | ///
       `coefficients' != int(`coefficients') | `coefficients' != `capacity' | ///
       `receipt'[1,6] != `coefficients' | ///
       `receipt'[1,7] != _N | `receipt'[1,8] < `receipt'[1,9] | ///
       `receipt'[1,8] > _N | `receipt'[1,9] < 1 | ///
       `receipt'[1,10] != `clusters' | `receipt'[1,11] != `ferows' | ///
       `receipt'[1,12] != `subsets' | `receipt'[1,13] != 1 | ///
       `receipt'[1,14] != `residual' | `receipt'[1,15] != 1 | ///
       `receipt'[1,16] < 0 | `receipt'[1,16] != int(`receipt'[1,16]) | ///
       `receipt'[1,16] > `receipt'[1,8] | ///
       `receipt'[1,17] != `separations' | `receipt'[1,18] != `saved' | ///
       `receipt'[1,19] != `adjustment' {
        di as error "PPML receipt failed semantic identity validation"
        exit 3499
    }
    forvalues receipt_column = 23/32 {
        if `receipt'[1,`receipt_column'] != int(`receipt'[1,`receipt_column']) | ///
           `receipt'[1,`receipt_column'] < 0 {
            di as error "PPML bounded execution evidence is not an exact count"
            exit 3499
        }
    }
    forvalues receipt_column = 25/32 {
        if `receipt'[1,`receipt_column'] > 4294967295 {
            di as error "PPML bounded hash evidence is outside uint32"
            exit 3499
        }
    }
    if `receipt'[1,24] > `receipt'[1,8] | ///
       `receipt'[1,23] > 5 * (`receipt'[1,8] + 1) {
        di as error "PPML bounded execution evidence exceeds its candidate-sample bound"
        exit 3499
    }

    if `inference'[1,1] != `requestedvcecode' | ///
       `inference'[2,1] != `vcecode' | `inference'[3,1] <= 0 | ///
       `inference'[4,1] != cond(`vcecode' == 3, 2, 1) | ///
       `inference'[5,1] != 1 | !inlist(`inference'[6,1], 1, 2, 3) | ///
       !inlist(`inference'[7,1], 0, 1) | ///
       `inference'[8,1] != int(`inference'[8,1]) | `inference'[8,1] < 0 | ///
       `inference'[8,1] > `coefficients' | ///
       `inference'[11,1] != int(`inference'[11,1]) | ///
       `inference'[11,1] < 0 | `inference'[11,1] > `coefficients' | ///
       `inference'[12,1] < 0 | `inference'[13,1] < 0 | ///
       `inference'[14,1] < 0 | `inference'[15,1] <= 0 | ///
       `inference'[15,1] >= 1 | `inference'[16,1] != 1 {
        di as error "PPML inference evidence failed semantic validation"
        exit 3499
    }
    local expected_correction = `dof'[1,1] / (`dof'[1,1] - 1)
    if `vcecode' == 3 local expected_correction = ///
        `dof'[7,1] / (`dof'[7,1] - 1)
    if reldif(`inference'[3,1], `expected_correction') > 1e-12 {
        di as error "PPML VCE correction evidence disagrees with DoF"
        exit 3499
    }
    if `inference'[7,1] != (`inference'[6,1] == 3) | ///
       `inference'[7,1] != (`inference'[8,1] > 0) | ///
       (`inference'[6,1] == 1 & `inference'[8,1] != 0) | ///
       (`inference'[6,1] == 2 & ///
            (`inference'[9,1] < 0 | `inference'[8,1] != 0)) | ///
       (`inference'[6,1] == 3 & ///
            (`inference'[9,1] >= 0 | `inference'[8,1] == 0)) | ///
       `inference'[12,1] > `inference'[15,1] | ///
       `inference'[13,1] > `inference'[15,1] | ///
       `inference'[14,1] > `inference'[15,1] {
        di as error "PPML PSD evidence is internally inconsistent"
        exit 3499
    }
    if ((`vcecode' == 2 | `clusters' <= 1) & `inference'[6,1] != 1) | ///
       (`vcecode' == 3 & `clusters' > 1 & ///
            !inlist(`inference'[6,1], 2, 3)) {
        di as error "PPML PSD state disagrees with the covariance route"
        exit 3499
    }

    local drop_total 0
    forvalues drop_row = 1/14 {
        if `drop_counts'[`drop_row',1] != `drop_row' - 1 | ///
           `drop_counts'[`drop_row',2] != int(`drop_counts'[`drop_row',2]) | ///
           `drop_counts'[`drop_row',2] < 0 {
            di as error "PPML drop-count evidence is invalid"
            exit 3499
        }
        local drop_total = `drop_total' + `drop_counts'[`drop_row',2]
    }
    local separated_drop_total 0
    forvalues drop_row = 10/13 {
        local separated_drop_total = `separated_drop_total' + ///
            `drop_counts'[`drop_row',2]
    }
    if `drop_total' != `receipt'[1,7] | ///
       `drop_counts'[1,2] != `receipt'[1,9] | ///
       `drop_counts'[2,2] != `receipt'[1,7] - `receipt'[1,8] | ///
       `separated_drop_total' != `receipt'[1,16] {
        di as error "PPML drop counts disagree with the receipt"
        exit 3499
    }

    local sequence_executions 0
    local sequence_direct_drops 0
    local sequence_singleton_drops 0
    local minimum_trace_row .
    local maximum_trace_row 0
    local maximum_generation 0
    forvalues sequence_row = 1/5 {
        local method_code = `sequence_row'
        local method_requested : list method_code in methodcodes
        if `sequence_row' == 1 local method_requested = `dropsingletons'
        local registered_order = cond(`sequence_row' == 1 | ///
            !`method_requested', 0, `sequence_row' - 1)
        local executions = `separation_sequence'[`sequence_row',4]
        if `separation_sequence'[`sequence_row',1] != `method_code' | ///
           `separation_sequence'[`sequence_row',2] != `method_requested' | ///
           `separation_sequence'[`sequence_row',3] != `registered_order' | ///
           `executions' != int(`executions') | `executions' < 0 | ///
           `separation_sequence'[`sequence_row',7] != ///
                int(`separation_sequence'[`sequence_row',7]) | ///
           `separation_sequence'[`sequence_row',7] < 0 | ///
           `separation_sequence'[`sequence_row',7] > 127 | ///
           !inlist(`separation_sequence'[`sequence_row',13], 0, 1) {
            di as error "PPML separation sequence identity is invalid"
            exit 3499
        }
        forvalues sequence_column = 5/12 {
            if `separation_sequence'[`sequence_row',`sequence_column'] != ///
                    int(`separation_sequence'[`sequence_row',`sequence_column']) | ///
               `separation_sequence'[`sequence_row',`sequence_column'] < 0 {
                di as error "PPML separation sequence contains a nonexact count"
                exit 3499
            }
        }
        if (`executions' == 0 & ///
                (`separation_sequence'[`sequence_row',5] != 0 | ///
                 `separation_sequence'[`sequence_row',6] != 0 | ///
                 `separation_sequence'[`sequence_row',8] != 0 | ///
                 `separation_sequence'[`sequence_row',9] != 0 | ///
                 `separation_sequence'[`sequence_row',10] != 0 | ///
                 `separation_sequence'[`sequence_row',11] != 0 | ///
                 `separation_sequence'[`sequence_row',12] != 0 | ///
                 `separation_sequence'[`sequence_row',13] != 1)) | ///
           (`executions' > 0 & ///
                (!`method_requested' | ///
                 `separation_sequence'[`sequence_row',5] < 1 | ///
                 `separation_sequence'[`sequence_row',5] > ///
                    `separation_sequence'[`sequence_row',6] | ///
                 `separation_sequence'[`sequence_row',6] > `receipt'[1,23] | ///
                 `separation_sequence'[`sequence_row',7] < 1 | ///
                 `separation_sequence'[`sequence_row',11] > ///
                    `separation_sequence'[`sequence_row',12] | ///
                 `separation_sequence'[`sequence_row',12] > `receipt'[1,24] | ///
                 `separation_sequence'[`sequence_row',13] != 1)) {
            di as error "PPML separation execution bounds are inconsistent"
            exit 3499
        }
        if `executions' > 0 {
            if `executions' > `separation_sequence'[`sequence_row',6] - ///
                    `separation_sequence'[`sequence_row',5] + 1 {
                di as error "PPML separation execution count exceeds its trace span"
                exit 3499
            }
            if missing(`minimum_trace_row') | ///
                    `separation_sequence'[`sequence_row',5] < `minimum_trace_row' {
                local minimum_trace_row = `separation_sequence'[`sequence_row',5]
            }
            if `separation_sequence'[`sequence_row',6] > `maximum_trace_row' ///
                local maximum_trace_row = `separation_sequence'[`sequence_row',6]
            if `separation_sequence'[`sequence_row',12] > `maximum_generation' ///
                local maximum_generation = `separation_sequence'[`sequence_row',12]
        }
        local aggregate_row 0
        forvalues aggregate_index = 1/`separations' {
            if `separation'[`aggregate_index',1] == `method_code' {
                local aggregate_row = `aggregate_index'
            }
        }
        local expected_aggregate = (`sequence_row' == 1 | `method_requested')
        if (`aggregate_row' > 0) != `expected_aggregate' {
            di as error "PPML separation sequence and aggregate inventories disagree"
            exit 3499
        }
        if `aggregate_row' > 0 {
            local worst_status 1
            if `separation_sequence'[`sequence_row',7] > 0 {
                local worst_status 0
                forvalues status_code = 1/7 {
                    if mod(floor(`separation_sequence'[`sequence_row',7] / ///
                            2^(`status_code' - 1)), 2) {
                        local worst_status = `status_code'
                    }
                }
            }
            if `separation'[`aggregate_row',2] != `worst_status' | ///
               `separation'[`aggregate_row',3] != `executions' | ///
               `separation'[`aggregate_row',4] != ///
                    `separation_sequence'[`sequence_row',12] | ///
               `separation'[`aggregate_row',5] != ///
                    `separation_sequence'[`sequence_row',8] | ///
               `separation'[`aggregate_row',6] != ///
                    `separation_sequence'[`sequence_row',9] | ///
               `separation'[`aggregate_row',7] != ///
                    `separation_sequence'[`sequence_row',10] | ///
               `separation'[`aggregate_row',8] != ///
                    `separation_sequence'[`sequence_row',13] {
                di as error "PPML separation sequence disagrees with its aggregate"
                exit 3499
            }
        }
        if (`sequence_row' == 1 & ///
                `separation_sequence'[`sequence_row',9] != 0) | ///
           (`sequence_row' > 1 & ///
                `separation_sequence'[`sequence_row',9] != ///
                    `drop_counts'[`=`sequence_row' + 8',2]) {
            di as error "PPML method-specific separation drops disagree with drop reasons"
            exit 3499
        }
        local sequence_executions = `sequence_executions' + `executions'
        local sequence_direct_drops = `sequence_direct_drops' + ///
            `separation_sequence'[`sequence_row',9]
        local sequence_singleton_drops = `sequence_singleton_drops' + ///
            `separation_sequence'[`sequence_row',10]
    }
    if `sequence_executions' != `receipt'[1,23] | ///
       `sequence_direct_drops' - `separation_sequence'[1,9] != ///
            `receipt'[1,16] | ///
       `sequence_singleton_drops' != `drop_counts'[9,2] {
        di as error "PPML separation sequence disagrees with receipt drop evidence"
        exit 3499
    }
    if (`receipt'[1,23] == 0 & `receipt'[1,24] != 0) | ///
       (`receipt'[1,23] > 0 & ///
            (`minimum_trace_row' != 1 | `maximum_trace_row' != `receipt'[1,23])) | ///
       `maximum_generation' != `receipt'[1,24] | ///
       (`receipt'[1,24] == 0 & ///
            (`receipt'[1,25] != `receipt'[1,27] | ///
             `receipt'[1,26] != `receipt'[1,28])) {
        di as error "PPML bounded chronology or generation hashes are inconsistent"
        exit 3499
    }
    if `receipt'[1,7] != `fit'[23,1] | ///
       `receipt'[1,8] != `fit'[22,1] | ///
       `receipt'[1,9] != `fit'[1,1] | ///
       `fit'[17,1] != `dof'[1,1] | ///
       `fit'[2,1] != `dof'[2,1] | `fit'[3,1] != `dof'[3,1] | ///
       `dof'[4,1] != `dof'[2,1] + `dof'[3,1] | ///
       `fit'[4,1] != `dof'[5,1] | `fit'[5,1] != `dof'[6,1] | ///
       `dof'[5,1] != `dof'[1,1] - `dof'[4,1] | ///
       `fit'[24,1] != 1 | `fit'[27,1] != 1 | ///
       `dof'[10,1] != `fit'[1,1] | ///
       `dof'[11,1] != `clusters' | ///
       `dof'[12,1] != `dof'[5,1] - `dof'[6,1] {
        di as error "PPML fit, DoF, and receipt identities disagree"
        exit 3499
    }
    if `fit'[1,1] != int(`fit'[1,1]) | `fit'[1,1] < 1 | ///
       `fit'[2,1] != int(`fit'[2,1]) | `fit'[2,1] < 1 | ///
       `fit'[3,1] != int(`fit'[3,1]) | `fit'[3,1] < 0 | ///
       `fit'[4,1] != int(`fit'[4,1]) | `fit'[4,1] < 1 | ///
       `fit'[5,1] != int(`fit'[5,1]) | `fit'[5,1] < 1 | ///
       `fit'[6,1] < 0 | `fit'[10,1] <= 0 | ///
       (`fit'[11,1] < . & `fit'[11,1] < 0) | ///
       `fit'[12,1] != int(`fit'[12,1]) | `fit'[12,1] < 0 | ///
       `fit'[12,1] > `fit'[2,1] | `fit'[13,1] <= 0 | ///
       `fit'[14,1] != int(`fit'[14,1]) | `fit'[14,1] < 0 | ///
       `fit'[15,1] != int(`fit'[15,1]) | `fit'[15,1] < 0 | ///
       `fit'[16,1] != int(`fit'[16,1]) | `fit'[16,1] < 0 | ///
       `fit'[17,1] != int(`fit'[17,1]) | `fit'[17,1] < 1 | ///
       `fit'[18,1] <= 0 | (`fit'[20,1] < . & `fit'[20,1] <= 0) | ///
       `fit'[21,1] != `weightcode' | ///
       `fit'[22,1] != int(`fit'[22,1]) | `fit'[22,1] < `fit'[1,1] | ///
       `fit'[23,1] != int(`fit'[23,1]) | `fit'[23,1] != _N | ///
       `fit'[26,1] >= . | `fit'[27,1] != 1 {
        di as error "PPML fit report failed semantic validation"
        exit 3499
    }
    if (`weightcode' == 0 & (`fit'[19,1] < . | `fit'[20,1] != 1 | ///
            `fit'[25,1] < . | `fit'[26,1] != 0 | ///
            reldif(`fit'[17,1], `fit'[1,1]) > 1e-12 | ///
            reldif(`fit'[18,1], `fit'[1,1]) > 1e-12)) | ///
       (`weightcode' == 1 & (`fit'[19,1] >= . | `fit'[19,1] <= 0 | ///
            `fit'[20,1] != 1 | `fit'[25,1] >= . | `fit'[26,1] != 0 | ///
            reldif(`fit'[19,1], `fit'[18,1]) > 1e-12 | ///
            reldif(`fit'[18,1], `fit'[17,1]) > 1e-12)) | ///
       (`weightcode' == 2 & ((`fit'[19,1] < . & `fit'[19,1] <= 0) | ///
            (`fit'[20,1] < . & `fit'[20,1] <= 0) | ///
            `fit'[25,1] >= . | `fit'[26,1] >= . | ///
            reldif(`fit'[17,1], `fit'[1,1]) > 1e-12 | ///
            reldif(`fit'[18,1], `fit'[1,1]) > 1e-12)) {
        di as error "PPML prepared-weight report is inconsistent"
        exit 3499
    }
    if `weightcode' != 0 {
        if abs(ln(`fit'[18,1]) - `fit'[25,1] - `fit'[26,1]) > ///
                1e-10 * (1 + abs(ln(`fit'[18,1]))) | ///
           (`fit'[19,1] < . & ///
                abs(ln(`fit'[19,1]) - `fit'[25,1]) > ///
                    1e-10 * (1 + abs(`fit'[25,1]))) | ///
           (`fit'[20,1] < . & ///
                abs(ln(`fit'[20,1]) - `fit'[26,1]) > ///
                    1e-10 * (1 + abs(`fit'[26,1]))) {
            di as error "PPML linear and logarithmic weight metadata disagree"
            exit 3499
        }
    }
    if `dof'[1,1] != int(`dof'[1,1]) | `dof'[1,1] < 1 | ///
       `dof'[2,1] != int(`dof'[2,1]) | `dof'[2,1] < 1 | ///
       `dof'[3,1] != int(`dof'[3,1]) | `dof'[3,1] < 0 | ///
       `dof'[4,1] != int(`dof'[4,1]) | `dof'[4,1] < 1 | ///
       `dof'[5,1] != int(`dof'[5,1]) | `dof'[5,1] < 1 | ///
       `dof'[6,1] != int(`dof'[6,1]) | `dof'[6,1] < 1 | ///
       `dof'[6,1] > `dof'[5,1] | ///
       `dof'[8,1] != int(`dof'[8,1]) | `dof'[8,1] < 0 | ///
       !inlist(`dof'[9,1], 0, 1) | ///
       `dof'[10,1] != int(`dof'[10,1]) | `dof'[10,1] < 1 | ///
       `dof'[12,1] != int(`dof'[12,1]) | `dof'[12,1] < 0 {
        di as error "PPML degrees-of-freedom report failed semantic validation"
        exit 3499
    }
    tempname finite_result covariance_scale
    mata: st_numscalar("`finite_result'", ///
        all(st_matrix("`b'") :< .) & all(st_matrix("`V'") :< .) & ///
        all(st_matrix("`status'") :< .) & ///
        all(st_matrix("`fit'")[|1,1 \ 10,1|] :< .) & ///
        all(st_matrix("`fit'")[|12,1 \ 18,1|] :< .) & ///
        all(st_matrix("`fit'")[|21,1 \ 24,1|] :< .) & ///
        all(st_matrix("`fit'")[|26,1 \ 27,1|] :< .) & ///
        all(st_matrix("`dof'")[|1,1 \ 6,1|] :< .) & ///
        all(st_matrix("`dof'")[|8,1 \ 12,1|] :< .) & ///
        all(st_matrix("`inference'") :< .) & ///
        all(st_matrix("`drop_counts'") :< .) & ///
        all(st_matrix("`separation_sequence'") :< .) & ///
        all(st_matrix("`certificate'") :< .) & ///
        all(st_matrix("`runtime'")[|1,1 \ 16,1|] :< .) & ///
        all(st_matrix("`runtime'")[|24,1 \ 27,1|] :< .) & ///
        st_matrix("`runtime'")[41,1] < . & ///
        all(st_matrix("`runtime'")[|77,1 \ 78,1|] :< .) & ///
        all(st_matrix("`runtime'")[|83,1 \ 95,1|] :< .))
    if scalar(`finite_result') != 1 {
        di as error "PPML result contains nonfinite mandatory values"
        exit 3499
    }
    mata: st_numscalar("`covariance_scale'", ///
        max((max(vec(abs(st_matrix("`V'")))), 2.2250738585072014e-308)))
    if `inference'[10,1] < ///
            -`inference'[15,1] * scalar(`covariance_scale') {
        di as error "PPML covariance eigenvalue evidence exceeds its certified scale"
        exit 3499
    }
    forvalues row = 1/15 {
        if `certificate'[`row',1] < 0 {
            di as error "PPML certificate contains a negative error norm"
            exit 3499
        }
    }
    if `certificate'[16,1] != 1 {
        di as error "PPML composite certificate did not pass"
        exit 3499
    }
    local identified_coefficients 0
    local normalized_constants 0
    forvalues slot = 1/`coefficients' {
        local status_code = `status'[`slot',2]
        if `status'[`slot',1] != `slot' | ///
           !inlist(`status_code', 1, 2, 3, 4, 5, 6, 7) | ///
           !inlist(`status'[`slot',3], 0, 1) | ///
           !inlist(`status'[`slot',4], 0, 1) | ///
           `status'[`slot',3] != (`status_code' == 1) | ///
           `status'[`slot',4] != (`status_code' != 1) {
            di as error "PPML coefficient-status table is invalid"
            exit 3499
        }
        if `status_code' == 1 local ++identified_coefficients
        if `status_code' == 7 {
            local ++normalized_constants
            if `slot' != `coefficients' {
                di as error "PPML normalized structural constant is outside the final slot"
                exit 3499
            }
        }
        if `status_code' != 1 {
            if `b'[1,`slot'] != 0 {
                di as error "PPML omitted coefficient was not exactly zero-filled"
                exit 3499
            }
            forvalues other = 1/`coefficients' {
                if `V'[`slot',`other'] != 0 | `V'[`other',`slot'] != 0 {
                    di as error "PPML omitted covariance row was not exactly zero-filled"
                    exit 3499
                }
            }
        }
    }
    if `normalized_constants' > 1 | ///
       `identified_coefficients' + `normalized_constants' != `dof'[2,1] {
        di as error "PPML coefficient statuses disagree with finite rank"
        exit 3499
    }
    if `inference'[8,1] > `identified_coefficients' | ///
       `inference'[11,1] > ///
            `identified_coefficients' - `inference'[8,1] {
        di as error "PPML PSD evidence exceeds the identified covariance rank"
        exit 3499
    }

    local absorbed_rank 0
    if `ferows' == 0 {
        if `fe'[1,1] != 2 | `dof'[3,1] != 0 {
            di as error "PPML no-FE sentinel has the wrong identity"
            exit 3499
        }
        forvalues column = 2/8 {
            if `fe'[1,`column'] != 0 {
                di as error "PPML no-FE sentinel contains nonzero metadata"
                exit 3499
            }
        }
    }
    else {
        local previous_term 0
        local term_has_slope 0
        forvalues row = 1/`ferows' {
            local kind = `fe'[`row',1]
            local term = `fe'[`row',2]
            local source = `fe'[`row',3]
            if !inlist(`kind', 1, 3, 4) | `term' != int(`term') | ///
               `term' < 1 | `term' > `feterms' | ///
               (`row' == 1 & `term' != 1) | ///
               (`row' > 1 & (`term' < `previous_term' | `term' > `previous_term' + 1)) | ///
               `source' != int(`source') | `source' < 0 | `source' > `sources' | ///
               (`kind' == 1 & `source' == 0) | ///
               (`kind' == 3 & `source' != 0) | ///
               (`kind' == 4 & `source' == 0) | ///
               `fe'[`row',4] != int(`fe'[`row',4]) | `fe'[`row',4] < 1 | ///
               `fe'[`row',5] != int(`fe'[`row',5]) | `fe'[`row',5] < 0 | ///
               `fe'[`row',5] > `fe'[`row',4] | ///
               `fe'[`row',6] != `fe'[`row',4] - `fe'[`row',5] | ///
               !inlist(`fe'[`row',7], 0, 1) | !inlist(`fe'[`row',8], 0, 1) {
                di as error "PPML fixed-effect report failed semantic validation"
                exit 3499
            }
            if `term' != `previous_term' local term_has_slope 0
            if `kind' == 4 local term_has_slope 1
            else if `term_has_slope' {
                di as error "PPML fixed-effect channels are outside declaration order"
                exit 3499
            }
            local absorbed_rank = `absorbed_rank' + `fe'[`row',6]
            local previous_term = `term'
        }
        if `previous_term' != `feterms' | `absorbed_rank' != `dof'[3,1] {
            di as error "PPML fixed-effect rows disagree with absorbed rank or terms"
            exit 3499
        }
    }

    local minimum_clusters .
    if `clusters' == 0 {
        forvalues column = 1/4 {
            if `cluster'[1,`column'] != 0 {
                di as error "PPML no-cluster sentinel contains nonzero metadata"
                exit 3499
            }
        }
        if `dof'[7,1] < . | `dof'[12,1] != 0 {
            di as error "PPML nonclustered degrees of freedom carry cluster metadata"
            exit 3499
        }
    }
    else {
        forvalues row = 1/`clusters' {
            if `cluster'[`row',1] != `row' | ///
               `cluster'[`row',2] != int(`cluster'[`row',2]) | ///
               `cluster'[`row',2] < 1 | `cluster'[`row',2] > `sources' | ///
               `cluster'[`row',3] != int(`cluster'[`row',3]) | ///
               `cluster'[`row',3] < 2 | `cluster'[`row',4] != 0 {
                di as error "PPML cluster report failed semantic validation"
                exit 3499
            }
            if missing(`minimum_clusters') | `cluster'[`row',3] < `minimum_clusters' {
                local minimum_clusters = `cluster'[`row',3]
            }
        }
        if `dof'[7,1] != `minimum_clusters' | ///
           `dof'[6,1] != min(`dof'[5,1], `minimum_clusters' - 1) {
            di as error "PPML cluster counts disagree with reported degrees of freedom"
            exit 3499
        }
    }

    if `clusters' == 0 {
        forvalues column = 1/4 {
            if `subset'[1,`column'] != 0 {
                di as error "PPML no-subset sentinel contains nonzero metadata"
                exit 3499
            }
        }
    }
    else {
        local subset_row 0
        forvalues cardinality = 1/`clusters' {
            forvalues mask = 1/`subsets' {
                local bits 0
                forvalues dimension = 0/`=`clusters' - 1' {
                    local bits = `bits' + mod(floor(`mask' / 2^`dimension'), 2)
                }
                if `bits' != `cardinality' continue
                local ++subset_row
                local expected_sign = cond(mod(`cardinality', 2), 1, -1)
                if `subset'[`subset_row',1] != `mask' | ///
                   `subset'[`subset_row',2] != int(`subset'[`subset_row',2]) | ///
                   `subset'[`subset_row',2] < 2 | ///
                   `subset'[`subset_row',3] != `expected_sign' | ///
                   `subset'[`subset_row',4] != `cardinality' {
                    di as error "PPML cluster-subset report is not in canonical order"
                    exit 3499
                }
                if `cardinality' == 1 {
                    local cluster_dimension = round(ln(`mask') / ln(2)) + 1
                    if `subset'[`subset_row',2] != `cluster'[`cluster_dimension',3] {
                        di as error "PPML singleton subset count disagrees with its cluster dimension"
                        exit 3499
                    }
                }
            }
        }
        if `subset_row' != `subsets' {
            di as error "PPML cluster-subset inventory is incomplete"
            exit 3499
        }
    }

    local method_code_count : word count `methodcodes'
    if `method_code_count' != `separations' {
        di as error "PPML expected separation-method inventory is inconsistent"
        exit 3499
    }
    local statistical_drops 0
    forvalues row = 1/`separations' {
        local expected_method : word `row' of `methodcodes'
        if `separation'[`row',1] != `expected_method' | ///
           !inlist(`separation'[`row',2], 1, 2, 3, 4, 5, 6, 7) | ///
           `separation'[`row',3] != int(`separation'[`row',3]) | ///
           `separation'[`row',3] < 0 | ///
           `separation'[`row',4] != int(`separation'[`row',4]) | ///
           `separation'[`row',4] < 0 | ///
           `separation'[`row',5] != int(`separation'[`row',5]) | ///
           `separation'[`row',5] < 0 | ///
           `separation'[`row',6] != int(`separation'[`row',6]) | ///
           `separation'[`row',6] < 0 | ///
           `separation'[`row',7] != int(`separation'[`row',7]) | ///
           `separation'[`row',7] < 0 | ///
           !inlist(`separation'[`row',8], 0, 1) | ///
           (`separation'[`row',9] < . & `separation'[`row',9] < 0) | ///
           (`separation'[`row',10] < . & `separation'[`row',10] < 0) | ///
           (`separation'[`row',11] < . & `separation'[`row',11] < 0) | ///
           (`separation'[`row',12] < . & `separation'[`row',12] < 0) | ///
           `separation'[`row',13] != int(`separation'[`row',13]) | ///
           `separation'[`row',13] < 0 | ///
           `separation'[`row',14] != int(`separation'[`row',14]) | ///
           `separation'[`row',14] < 0 {
            di as error "PPML separation aggregate failed semantic validation"
            exit 3499
        }
        if `separation'[`row',3] == 0 {
            if `separation'[`row',2] != 1 | ///
               `separation'[`row',4] != 0 | `separation'[`row',5] != 0 | ///
               `separation'[`row',6] != 0 | `separation'[`row',7] != 0 | ///
               `separation'[`row',8] != 1 | `separation'[`row',9] < . | ///
               `separation'[`row',10] < . | `separation'[`row',11] < . | ///
               `separation'[`row',12] < . | `separation'[`row',13] != 0 | ///
               `separation'[`row',14] != 0 {
                di as error "PPML unexecuted separation row is not a bounded sentinel"
                exit 3499
            }
        }
        if `expected_method' != 1 {
            local statistical_drops = `statistical_drops' + `separation'[`row',6]
        }
    }
    if `statistical_drops' != `receipt'[1,16] {
        di as error "PPML separation aggregate disagrees with the receipt"
        exit 3499
    }

    if `runtime'[1,1] < 0 | ///
       `runtime'[2,1] != int(`runtime'[2,1]) | `runtime'[2,1] < 0 | ///
       `runtime'[3,1] != int(`runtime'[3,1]) | `runtime'[3,1] < 0 | ///
       `runtime'[4,1] != int(`runtime'[4,1]) | `runtime'[4,1] < 0 | ///
       `runtime'[2,1] != `fit'[14,1] | `runtime'[3,1] != `fit'[15,1] | ///
       `runtime'[4,1] != `fit'[16,1] | ///
       `runtime'[5,1] != `memorycode' | !inlist(`runtime'[6,1], 1, 2) | ///
       `runtime'[7,1] != int(`runtime'[7,1]) | `runtime'[7,1] < 0 | ///
       `runtime'[8,1] != int(`runtime'[8,1]) | `runtime'[8,1] < 0 | ///
       `runtime'[9,1] != int(`runtime'[9,1]) | `runtime'[9,1] < 0 | ///
       `runtime'[10,1] != `requestedthreads' | ///
       `runtime'[10,1] < 0 | `runtime'[10,1] != int(`runtime'[10,1]) | ///
       `runtime'[11,1] < 1 | `runtime'[11,1] != int(`runtime'[11,1]) | ///
       `runtime'[12,1] < 0 | `runtime'[13,1] < 0 | `runtime'[14,1] < 0 | ///
       `runtime'[15,1] < 0 | `runtime'[16,1] < 0 | ///
       !inlist(`runtime'[24,1], 0, 1) | !inlist(`runtime'[25,1], 0, 1) | ///
       `runtime'[26,1] != int(`runtime'[26,1]) | `runtime'[26,1] < 0 | ///
       `runtime'[27,1] != int(`runtime'[27,1]) | `runtime'[27,1] < 0 {
        di as error "PPML runtime report failed semantic validation"
        exit 3499
    }
    forvalues row = 17/23 {
        // Row 22 is signed allocator growth (current minus start).  All other
        // observed allocator byte counts and ratios are nonnegative.
        if `row' != 22 & `runtime'[`row',1] < . & ///
           `runtime'[`row',1] < 0 {
            di as error "PPML allocator telemetry is invalid"
            exit 3499
        }
    }
    forvalues row = 28/40 {
        if `runtime'[`row',1] < . & `runtime'[`row',1] < 0 {
            di as error "PPML phase timing is invalid"
            exit 3499
        }
    }
    if !inlist(`runtime'[41,1], 0, 1) | `runtime'[77,1] != 1 | ///
       `runtime'[78,1] != 1 {
        di as error "PPML convergence or allocation-audit flags are invalid"
        exit 3499
    }
    local audit_values 0
    forvalues row = 42/76 {
        if `runtime'[`row',1] < . local ++audit_values
    }
    if (`runtime'[41,1] == 0 & `audit_values' != 0) | ///
       (`runtime'[41,1] == 1 & `audit_values' != 35) | ///
       (`runtime'[41,1] == 0 & `runtime'[25,1] != 0) {
        di as error "PPML allocation-audit telemetry is only partially present"
        exit 3499
    }
    if `runtime'[41,1] == 1 {
        foreach row in 42 43 44 45 46 47 49 50 51 52 53 54 55 56 57 ///
            59 60 61 62 63 64 65 66 68 69 75 {
            if `runtime'[`row',1] != int(`runtime'[`row',1]) | ///
               `runtime'[`row',1] < 0 {
                di as error "PPML allocation-audit count or byte field is invalid"
                exit 3499
            }
        }
        if `runtime'[42,1] < 16 | ///
           `runtime'[44,1] < `runtime'[43,1] | ///
           `runtime'[45,1] != `runtime'[44,1] - `runtime'[43,1] | ///
           `runtime'[48,1] != int(`runtime'[48,1]) | ///
           `runtime'[48,1] != `runtime'[47,1] - `runtime'[46,1] | ///
           `runtime'[50,1] < `runtime'[49,1] | ///
           `runtime'[51,1] != `runtime'[50,1] - `runtime'[49,1] | ///
           !inlist(`runtime'[58,1], 0, 1) | ///
           !inlist(`runtime'[67,1], 0, 1) | ///
           !inlist(`runtime'[70,1], 0, 1) | ///
           !inlist(`runtime'[72,1], 0, 1) | ///
           !inlist(`runtime'[73,1], 0, 1) | ///
           !inlist(`runtime'[74,1], 0, 1) | ///
           !inlist(`runtime'[76,1], 0, 1) | ///
           !inlist(`runtime'[71,1], 1, 2, 3, 4, 5, 6, 7) {
            di as error "PPML allocation certificate failed semantic validation"
            exit 3499
        }
        local ppml_capacity_stable = ///
            (`runtime'[52,1] == `runtime'[55,1] & ///
             `runtime'[53,1] == `runtime'[56,1] & ///
             `runtime'[54,1] == `runtime'[57,1])
        local linear_capacity_stable = ///
            (`runtime'[59,1] == `runtime'[63,1] & ///
             `runtime'[60,1] == `runtime'[64,1] & ///
             `runtime'[61,1] == `runtime'[65,1] & ///
             `runtime'[62,1] == `runtime'[66,1])
        local allocation_certificate_passed = ///
            (`runtime'[45,1] == 0 & `runtime'[48,1] == 0 & ///
             `runtime'[51,1] == 0 & `runtime'[58,1] == 1 & ///
             `runtime'[67,1] == 1 & `runtime'[70,1] == 1 & ///
             `runtime'[72,1] == 1 & `runtime'[73,1] == 1 & ///
             `runtime'[74,1] == 1)
        if `runtime'[58,1] != `ppml_capacity_stable' | ///
           (`runtime'[67,1] == 1 & !`linear_capacity_stable') | ///
           `runtime'[70,1] != (`runtime'[68,1] == `runtime'[69,1]) | ///
           `runtime'[69,1] != `runtime'[16,1] | ///
           `runtime'[71,1] != `runtime'[7,1] | ///
           `runtime'[72,1] != (`runtime'[71,1] == 2) | ///
           `runtime'[76,1] != `allocation_certificate_passed' | ///
           `runtime'[76,1] != `runtime'[25,1] {
            di as error "PPML allocation certificate identities disagree"
            exit 3499
        }
    }
    if `runtime'[79,1] < . & `runtime'[79,1] != int(`runtime'[79,1]) {
        di as error "PPML repeated-call baseline growth is invalid"
        exit 3499
    }
    if `runtime'[79,1] < . & ///
       `runtime'[79,1] != `runtime'[18,1] - `runtime'[17,1] {
        di as error "PPML repeated-call baseline identity disagrees"
        exit 3499
    }
    local action_event_values 0
    forvalues row = 80/82 {
        if `runtime'[`row',1] < . local ++action_event_values
    }
    if !inlist(`action_event_values', 0, 3) {
        di as error "PPML action allocation-event telemetry is partial"
        exit 3499
    }
    if `action_event_values' == 3 {
        if `runtime'[80,1] != int(`runtime'[80,1]) | `runtime'[80,1] < 0 | ///
           `runtime'[81,1] != int(`runtime'[81,1]) | ///
           `runtime'[81,1] < `runtime'[80,1] | ///
           `runtime'[82,1] != `runtime'[81,1] - `runtime'[80,1] {
            di as error "PPML action allocation-event telemetry is invalid"
            exit 3499
        }
    }
    forvalues row = 83/108 {
        if `runtime'[`row',1] < . & ///
           (`runtime'[`row',1] != int(`runtime'[`row',1]) | ///
            `runtime'[`row',1] < 0) {
            di as error "PPML phase-peak telemetry is invalid"
            exit 3499
        }
        if `row' <= 95 & `runtime'[`row',1] >= . {
            di as error "PPML modeled phase-peak telemetry is missing"
            exit 3499
        }
    }
    return scalar coefficients = `coefficients'
end
