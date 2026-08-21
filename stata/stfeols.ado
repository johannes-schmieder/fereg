*! StFixest 0.1.0 Phase 0-1 20aug2026
program define stfeols, eclass sortpreserve
    version 18.0
    syntax varlist(fv ts numeric min=1) [if] [in] [fw aw pw iw], ///
        [ABSorb(string asis) VCE(string asis) DOF(string asis) ///
         KEEPsingletons RESIDuals(name) SAVEFEs(string asis) ///
         SOLVer(string) PREConditioner(string) THREADS(integer 0) ///
         MEMory(string) POOLSize(integer 10) TOLerance(real 1e-8) ///
         ITERate(integer 16000) DETERMINISTIC TIMEIT VERBose(integer 0) ///
         NOCONStant]

    marksample touse, novarlist
    gettoken depvar indepvars : varlist

    if `threads' < 0 | `poolsize' <= 0 | `tolerance' <= 0 | `iterate' <= 0 {
        di as error "threads(), poolsize(), tolerance(), or iterate() is invalid"
        exit 198
    }

    di as error "stfeols estimation is not enabled in StFixest 0.1.0 Phase 0-1"
    di as error "This package proves the Rust-first plugin, protocol, parser, and safety architecture only."
    di as error "No partial or uncertified OLS/WLS estimate has been posted."
    exit 498
end
