*! fereg e(indepvars) reconstruction shared by OLS and PPML 0.5.0 09oct2026
program define _fereg_indepvars
    version 15.0
    // Pinned reghdfe's e(indepvars) excludes syntax/base coordinates,
    // retains numerically omitted coordinates, and renders every retained
    // factor component with Stata's nonbase `bn' marker. The posted b/V
    // stripe remains the full omission-marked axis. The result is returned
    // in the caller's local `target', so r() is left untouched.
    syntax , TARGET(name) COUNT(integer) STATUS(string) [NAMES(string asis)]
    local intended_indepvars
    // Stata 15 cannot subscript e() matrices directly; read a copy.
    tempname status_matrix
    if `count' > 0 matrix `status_matrix' = `status'
    if `count' > 0 {
        forvalues slot = 1/`count' {
            if `status_matrix'[`slot',2] == 2 continue
            local name : word `slot' of `names'
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
    c_local `target' `"`intended_indepvars'"'
end
