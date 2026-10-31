*! fereg shared memory shadow validation 0.5.0 18sep2026
program define _fereg_memory_validate
    version 15.0
    args report
    if rowsof(`report') != 11 | colsof(`report') != 2 exit 459
    forvalues row = 1/11 {
        if !inlist(`report'[`row',1], 0, 1) | ///
            missing(`report'[`row',2]) | `report'[`row',2] < 0 | ///
            `report'[`row',2] != floor(`report'[`row',2]) | ///
            `report'[`row',2] > 2^53 exit 459
        if `report'[`row',1] == 0 & `report'[`row',2] != 0 exit 459
    }
    if `report'[1,1] != 1 | !inlist(`report'[1,2], 1, 2) | ///
        `report'[2,1] != 1 | !inlist(`report'[2,2], 0, 1, 2) | ///
        `report'[11,1] != 1 exit 459
    if `report'[2,2] < 2 & (`report'[8,1] != 0 | `report'[9,1] != 0) exit 459
    if `report'[2,2] == 2 {
        if `report'[4,1] != 1 | `report'[5,1] != 1 | ///
            `report'[6,1] != 1 | `report'[7,1] != 1 | ///
            `report'[8,1] != 1 | `report'[9,1] != 1 exit 459
        // Schema 2 separates practical calibration from the low-16 strict gaps.
        if `report'[1,2] == 1 & `report'[11,2] != 0 exit 459
        if `report'[1,2] == 2 & `report'[11,2] >= 2^16 exit 459
        if `report'[1,2] == 2 {
            if `report'[3,1] != 1 | `report'[3,2] > `report'[4,2] exit 459
            local increment = max(`report'[6,2], `report'[7,2])
            if `report'[9,2] != `increment' | ///
                `report'[8,2] < `report'[4,2] + `report'[5,2] + `increment' exit 459
        }
    }
end
