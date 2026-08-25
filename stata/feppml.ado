*! fereg feppml fail-closed stub 0.9.0 24aug2026
program define feppml, eclass sortpreserve
    version 18.0
    syntax varlist(fv ts numeric min=1) [if] [in] [fw aw pw iw], ///
        [ABSorb(string asis) VCE(string asis) DOF(string asis) ///
         OFFset(varname numeric) EXPosure(varname numeric) ///
         SEParation(string asis) KEEPsingletons ///
         RESIDuals(name) SAVEFEs(string asis) ///
         SOLVer(string) PREConditioner(string) THREADS(integer 0) ///
         MEMory(string) POOLSize(integer 10) TOLerance(real 1e-8) ///
         ITERate(integer 100) DETERMINISTIC TIMEIT VERBose(integer 0)]

    marksample touse, novarlist
    gettoken depvar indepvars : varlist

    if `"`offset'"' != "" & `"`exposure'"' != "" {
        di as error "offset() and exposure() are mutually exclusive"
        exit 198
    }
    if `threads' < 0 | `poolsize' <= 0 | `tolerance' <= 0 | `iterate' <= 0 {
        di as error "threads(), poolsize(), tolerance(), or iterate() is invalid"
        exit 198
    }

    di as error "feppml estimation is not implemented in fereg 0.9.0"
    di as error "PPML IRLS and separation are outside the 0.9.0 OLS release-candidate scope."
    di as error "No partial or uncertified PPML estimate has been posted."
    exit 498
end
