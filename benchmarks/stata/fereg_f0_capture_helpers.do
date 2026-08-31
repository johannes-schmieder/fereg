/* Machine-record helpers for one direct F0-versus-candidate fereg arm.

   This file is loaded by a rendered driver. It never resolves or calls an
   estimator and never creates benchmark data. All records use the closed
   `FEREG_F0_CAPTURE|1|...` prefix. */
version 18.0

capture program drop __fereg_f0_encode
program define __fereg_f0_encode, rclass
    version 18.0
    syntax , VALUE(string asis)
    local encoded `"`value'"'
    local encoded = subinstr(`"`encoded'"', "%", "%25", .)
    local encoded = subinstr(`"`encoded'"', "|", "%7C", .)
    local encoded = subinstr(`"`encoded'"', char(34), "%22", .)
    local encoded = subinstr(`"`encoded'"', char(13), "%0D", .)
    local encoded = subinstr(`"`encoded'"', char(10), "%0A", .)
    return local encoded `"`encoded'"'
end

capture program drop __fereg_f0_capture_matrix
program define __fereg_f0_capture_matrix
    version 18.0
    syntax name(name=matrix_name), FILEHANDLE(name) PHASE(string) ///
        ARM(string) CASEID(string)

    capture matrix __fereg_f0_matrix = e(`matrix_name')
    if _rc {
        file write `filehandle' ///
            "FEREG_F0_CAPTURE|1|absent|`phase'|matrix|`matrix_name'" _n
        exit
    }
    local rows = rowsof(__fereg_f0_matrix)
    local columns = colsof(__fereg_f0_matrix)
    file write `filehandle' ///
        "FEREG_F0_CAPTURE|1|matrix_dimensions|`phase'|`matrix_name'|`rows'|`columns'" _n
    if `rows' == 0 | `columns' == 0 exit

    local rownames : rowfullnames __fereg_f0_matrix
    local columnnames : colfullnames __fereg_f0_matrix
    forvalues row = 1/`rows' {
        local rowname : word `row' of `rownames'
        quietly __fereg_f0_encode, value(`"`rowname'"')
        local encoded_row `"`r(encoded)'"'
        forvalues column = 1/`columns' {
            local columnname : word `column' of `columnnames'
            quietly __fereg_f0_encode, value(`"`columnname'"')
            local encoded_column `"`r(encoded)'"'
            local value : display %21x __fereg_f0_matrix[`row',`column']
            file write `filehandle' ///
                "FEREG_F0_CAPTURE|1|matrix|`phase'|`matrix_name'|`row'|`column'|`encoded_row'|`encoded_column'|`value'" _n
        }
    }
end

capture program drop __fereg_f0_capture_e
program define __fereg_f0_capture_e
    version 18.0
    syntax , FILEHANDLE(name) PHASE(string) ARM(string) CASEID(string)

    local scalars : e(scalars)
    local position 0
    foreach name of local scalars {
        local ++position
        file write `filehandle' ///
            "FEREG_F0_CAPTURE|1|inventory|`phase'|scalar|`position'|`name'" _n
        local value : display %21x e(`name')
        file write `filehandle' ///
            "FEREG_F0_CAPTURE|1|scalar|`phase'|`name'|`value'" _n
    }

    local macros : e(macros)
    local position 0
    foreach name of local macros {
        local ++position
        file write `filehandle' ///
            "FEREG_F0_CAPTURE|1|inventory|`phase'|macro|`position'|`name'" _n
        local text `"`e(`name')'"'
        quietly __fereg_f0_encode, value(`"`text'"')
        local encoded `"`r(encoded)'"'
        file write `filehandle' ///
            "FEREG_F0_CAPTURE|1|macro|`phase'|`name'|`encoded'" _n
    }

    local matrices : e(matrices)
    local position 0
    foreach name of local matrices {
        local ++position
        file write `filehandle' ///
            "FEREG_F0_CAPTURE|1|inventory|`phase'|matrix|`position'|`name'" _n
        __fereg_f0_capture_matrix `name', filehandle(`filehandle') ///
            phase(`phase') arm(`arm') caseid(`caseid')
    }

    local functions : e(functions)
    local position 0
    foreach name of local functions {
        local ++position
        file write `filehandle' ///
            "FEREG_F0_CAPTURE|1|inventory|`phase'|function|`position'|`name'" _n
    }
end

capture program drop __fereg_f0_capture_sample
program define __fereg_f0_capture_sample
    version 18.0
    syntax varlist(min=2 max=2 numeric), FILEHANDLE(name) PHASE(string) ///
        ARM(string) CASEID(string)
    gettoken obs_id sample : varlist

    quietly count if `sample' == 1
    local included = r(N)
    quietly count if `sample' == 0
    local excluded = r(N)
    quietly _datasignature `obs_id' `sample'
    local mask_signature `"`r(datasignature)'"'
    quietly __fereg_f0_encode, value(`"`mask_signature'"')
    local encoded_mask `"`r(encoded)'"'

    preserve
    quietly keep if `sample' == 1
    if _N == 0 local included_signature "empty"
    else {
        quietly _datasignature `obs_id'
        local included_signature `"`r(datasignature)'"'
    }
    restore
    quietly __fereg_f0_encode, value(`"`included_signature'"')
    local encoded_included `"`r(encoded)'"'

    preserve
    quietly keep if `sample' == 0
    if _N == 0 local excluded_signature "empty"
    else {
        quietly _datasignature `obs_id'
        local excluded_signature `"`r(datasignature)'"'
    }
    restore
    quietly __fereg_f0_encode, value(`"`excluded_signature'"')
    local encoded_excluded `"`r(encoded)'"'

    file write `filehandle' ///
        "FEREG_F0_CAPTURE|1|sample|`phase'|`included'|`excluded'|`encoded_mask'|`encoded_included'|`encoded_excluded'" _n
end

capture program drop __fereg_f0_capture_saved
program define __fereg_f0_capture_saved
    version 18.0
    syntax varlist(min=2 max=2 numeric), FILEHANDLE(name) PHASE(string) ///
        ARM(string) CASEID(string) OUTPUT(string)
    gettoken obs_id saved : varlist

    quietly count if `saved' < .
    local nonmissing = r(N)
    quietly summarize `saved', meanonly
    local minimum : display %21x r(min)
    local maximum : display %21x r(max)
    local sum : display %21x r(sum)
    quietly _datasignature `obs_id' `saved'
    local signature `"`r(datasignature)'"'
    quietly __fereg_f0_encode, value(`"`signature'"')
    local encoded_signature `"`r(encoded)'"'
    file write `filehandle' ///
        "FEREG_F0_CAPTURE|1|saved_output|`phase'|`output'|`nonmissing'|`encoded_signature'|`minimum'|`maximum'|`sum'" _n
end
