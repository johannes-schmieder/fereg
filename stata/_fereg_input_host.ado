*! fereg public host facts for the parallel input policy 0.5.0 01nov2026
// Reports public c() facts only; the plugin decides whether its parallel
// numeric reader is qualified for them. Alias variables (Stata 18 and later)
// resolve through another frame and are reported for every variable read.
program define _fereg_input_host, rclass
    version 15.0
    syntax [varlist(default=none)]
    local edition `"`c(edition_real)'"'
    if !inlist(`"`edition'"', "BE", "SE", "MP") {
        local edition = cond(c(MP), "MP", cond(c(SE), "SE", "BE"))
    }
    local aliases 0
    if c(stata_version) >= 18 {
        foreach variable of local varlist {
            version 18: mata: st_local("alias", strofreal(st_isalias("`variable'")))
            if `alias' {
                local aliases 1
                continue, break
            }
        }
    }
    return local edition `"`edition'"'
    return local interface = cond(`"`c(console)'"' == "console", "console", "gui")
    return local aliases "`aliases'"
end
