*! fereg Stata memory adapter 0.5.0 18sep2026
program define _fereg_memory_baseline, rclass
    version 18.0
    // These are hidden Stata fields. Unknown versions/layouts fail to missing,
    // never to zero. No observations, strings or matrices are materialized.
    local used .
    local allocated .
    if inlist(floor(c(stata_version)), 18, 19) {
        quietly frame dir
        local frames `"`r(frames)'"'
        local frame_count : word count `frames'
        capture quietly memory
        if !_rc {
            local valid 1
            local allocated 0
            foreach field in data_data_a data_strl_a data_etc_a data_oh_a ///
                stata_matrices_a stata_ado_a stata_sr_a mata_matrices_a ///
                mata_functions_a maxvar_a other_a {
                local value = r(`field')
                if missing(`value') | `value' < 0 | `value' != floor(`value') local valid 0
                local allocated = `allocated' + `value'
            }
            // data_data_u is frame-local, whereas allocated segments and strL
            // pools include other frames. Do not add global pools per frame.
            if `frame_count' == 1 {
                local used = `allocated'
                foreach category in data_data data_strl data_etc data_oh {
                    local value = r(`category'_u)
                    if missing(`value') | `value' < 0 | `value' > r(`category'_a) local valid 0
                    local used = `used' - r(`category'_a) + `value'
                }
            }
            if !`valid' | `allocated' > 2^53 {
                local allocated .
                local used .
            }
        }
    }
    return scalar used = `used'
    return scalar allocated = `allocated'
end
