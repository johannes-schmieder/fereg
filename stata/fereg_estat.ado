*! fereg estat adapter 0.5.0 24aug2026
program define fereg_estat, rclass
    version 15.0
    if !inlist(`"`e(cmd)'"', "fereg", "_fereg_estimate_ols_test", ///
        "_fereg_ols_hidden_numeric_v1") {
        error 301
    }

    gettoken key 0 : 0, parse(", ")
    local key = lower(strtrim(`"`key'"'))
    local key_length = strlen(`"`key'"')
    if `"`key'"' == substr("summarize", 1, max(2, `key_length')) {
        syntax [anything] [, * noheader]
        estat_summ `anything', `options' noheader
        return add
        exit
    }

    di as error `"invalid subcommand `key'"'
    exit 321
end
