version 15.0

// Bounded transport validation stays independent of the native producer.
// Local matrix copies avoid repeated Stata expression parsing and SPI reads;
// observation outputs are still checked by the surrounding transaction.
capture mata: mata drop _fereg_ppml_validation_error()
capture mata: mata drop _fereg_ppml_validate_result()
capture mata: mata drop _fereg_ppml_post_scalars()
mata:
real scalar _fereg_ppml_validation_error(string scalar message, real scalar code)
{
    errprintf("%s\n", message)
    return(code)
}

real scalar _fereg_ppml_validate_result()
{
    real matrix b, V, status, fit, dof
    real matrix fe, cluster, subset, separation, inference
    real matrix drop_counts, separation_sequence, certificate, solver, runtime
    real matrix receipt
    real scalar capacity, clusters, ferows, subsets, separations
    real scalar residual, saved, adjustment, vcecode, requestedvcecode
    real scalar dropsingletons, weightcode, memorycode, requestedthreads, feterms
    real scalar sources, requestedsolvercode, absorbed_rank, action_event_values, aggregate_index
    real scalar aggregate_row, allocation_certificate_passed, audit_values, bits, cardinality
    real scalar checkpoint_family, cluster_dimension, coefficients, column, covariance_scale
    real scalar dimension, drop_row, drop_total, effective_engine, effective_family
    real scalar engine_observed, executions, expected_aggregate, expected_correction, expected_method
    real scalar expected_sign, failed_attempt, finite_result, frame_count, identified_coefficients
    real scalar kind, linear_capacity_stable, list_index_0, list_index_1, mask
    real scalar maximum_generation, maximum_trace_row, method_code, method_code_count, method_requested
    real scalar minimum_clusters, minimum_trace_row, normalized_constants, other, ppml_capacity_stable
    real scalar previous_term, receipt_column, registered_order, row, selected_engine
    real scalar selected_family, separated_drop_total, sequence_column, sequence_direct_drops, sequence_executions
    real scalar sequence_row, sequence_singleton_drops, slot, source, statistical_drops
    real scalar status_code, status_mask, subset_row, term, term_has_slope
    real scalar unresolved_status, worst_status
    real rowvector list_values_0, list_values_1
    string rowvector methodcodes

    b = st_matrix(st_local("b"))
    V = st_matrix(st_local("V"))
    status = st_matrix(st_local("status"))
    fit = st_matrix(st_local("fit"))
    dof = st_matrix(st_local("dof"))
    fe = st_matrix(st_local("fe"))
    cluster = st_matrix(st_local("cluster"))
    subset = st_matrix(st_local("subset"))
    separation = st_matrix(st_local("separation"))
    inference = st_matrix(st_local("inference"))
    drop_counts = st_matrix(st_local("drop_counts"))
    separation_sequence = st_matrix(st_local("separation_sequence"))
    certificate = st_matrix(st_local("certificate"))
    solver = st_matrix(st_local("solver"))
    runtime = st_matrix(st_local("runtime"))
    receipt = st_matrix(st_local("receipt"))
    capacity = strtoreal(st_local("capacity"))
    clusters = strtoreal(st_local("clusters"))
    ferows = strtoreal(st_local("ferows"))
    subsets = strtoreal(st_local("subsets"))
    separations = strtoreal(st_local("separations"))
    residual = strtoreal(st_local("residual"))
    saved = strtoreal(st_local("saved"))
    adjustment = strtoreal(st_local("adjustment"))
    vcecode = strtoreal(st_local("vcecode"))
    requestedvcecode = strtoreal(st_local("requestedvcecode"))
    dropsingletons = strtoreal(st_local("dropsingletons"))
    weightcode = strtoreal(st_local("weightcode"))
    memorycode = strtoreal(st_local("memorycode"))
    requestedthreads = strtoreal(st_local("requestedthreads"))
    feterms = strtoreal(st_local("feterms"))
    sources = strtoreal(st_local("sources"))
    requestedsolvercode = strtoreal(st_local("requestedsolvercode"))
    methodcodes = tokens(st_local("methodcodes"))

    dimension = max((capacity, 1))
    if (rows(b) != 1 | cols(b) != dimension |
        rows(V) != dimension | cols(V) != dimension |
        rows(status) != dimension | cols(status) != 4 |
        rows(fit) != 27 | cols(fit) != 1 |
        rows(dof) != 12 | cols(dof) != 1 |
        rows(fe) != max((ferows, 1)) | cols(fe) != 8 |
        rows(cluster) != max((clusters, 1)) | cols(cluster) != 4 |
        rows(subset) != max((subsets, 1)) | cols(subset) != 4 |
        rows(separation) != max((separations, 1)) | cols(separation) != 14 |
        rows(inference) != 16 | cols(inference) != 1 |
        rows(drop_counts) != 14 | cols(drop_counts) != 2 |
        rows(separation_sequence) != 5 | cols(separation_sequence) != 13 |
        rows(certificate) != 16 | cols(certificate) != 1 |
        rows(solver) != 4 | cols(solver) != 22 |
        rows(runtime) != 108 | cols(runtime) != 1 |
        rows(receipt) != 1 | cols(receipt) != 33) {
        return(_fereg_ppml_validation_error("PPML result layout failed exact structural validation", 499))
    }
    coefficients = receipt[1,5]
    if (receipt[1,1] != 5354602 | receipt[1,2] != 1 |
        receipt[1,3] != 5 | receipt[1,20] != 16 |
        receipt[1,21] != 14 | receipt[1,22] != 5 |
        receipt[1,33] != 1) {
        return(_fereg_ppml_validation_error("PPML receipt wire header or COMPLETE marker is invalid", 499))
    }
    if (receipt[1,4] != vcecode |
        coefficients != trunc(coefficients) | coefficients != capacity |
        receipt[1,6] != coefficients |
        receipt[1,7] != st_nobs() | receipt[1,8] < receipt[1,9] |
        receipt[1,8] > st_nobs() | receipt[1,9] < 1 |
        receipt[1,10] != clusters | receipt[1,11] != ferows |
        receipt[1,12] != subsets | receipt[1,13] != 1 |
        receipt[1,14] != residual | receipt[1,15] != 1 |
        receipt[1,16] < 0 | receipt[1,16] != trunc(receipt[1,16]) |
        receipt[1,16] > receipt[1,8] |
        receipt[1,17] != separations | receipt[1,18] != saved |
        receipt[1,19] != adjustment) {
        return(_fereg_ppml_validation_error("PPML receipt failed semantic identity validation", 3499))
    }
    for (receipt_column = 23; receipt_column <= 32; ++receipt_column) {
        if (receipt[1,receipt_column] != trunc(receipt[1,receipt_column]) |  receipt[1,receipt_column] < 0) {
            return(_fereg_ppml_validation_error("PPML bounded execution evidence is not an exact count", 3499))
        }
    }
    for (receipt_column = 25; receipt_column <= 32; ++receipt_column) {
        if (receipt[1,receipt_column] > 4294967295) {
            return(_fereg_ppml_validation_error("PPML bounded hash evidence is outside uint32", 3499))
        }
    }

    selected_family = solver[1,2]
    selected_engine = solver[1,3]
    effective_family = solver[1,4]
    effective_engine = solver[1,5]
    checkpoint_family = solver[1,6]
    if (solver[1,1] != requestedsolvercode |
        !any(( 1, 2, 3, 7) :== (requestedsolvercode)) |
        !any(( 2, 3, 7) :== (selected_family)) |
        !any(( 2, 3, 7) :== (effective_family)) |
        !any(( 1, 2, 3, 4) :== (selected_engine)) |
        !any(( 1, 2, 3, 4) :== (effective_engine)) |
        !any(( 2, 3) :== (checkpoint_family))) {
        return(_fereg_ppml_validation_error("PPML solver report has an invalid route identity", 3499))
    }
    if ((requestedsolvercode == 1 & !any(( 2, 3) :== (selected_family))) |
        (requestedsolvercode != 1 & selected_family != requestedsolvercode) |
        (selected_family == 2 & selected_engine != 1) |
        (selected_family == 3 & !any(( 2, 3) :== (selected_engine))) |
        (selected_family == 7 & selected_engine != 4) |
        (effective_family == 2 & effective_engine != 1) |
        (effective_family == 3 &
        (!any(( 2, 3) :== (effective_engine)) | effective_engine != selected_engine)) |
        (effective_family == 7 & effective_engine != 4) |
        (selected_family == 2 & effective_family != 2) |
        (selected_family == 3 & effective_family != 3) |
        (selected_family == 7 & !any(( 2, 7) :== (effective_family)))) {
        return(_fereg_ppml_validation_error("PPML requested, selected, effective, or checkpoint solver disagrees", 3499))
    }
    for (row = 1; row <= 4; ++row) {
        if (solver[row,1] != requestedsolvercode |
            solver[row,2] != selected_family |
            solver[row,3] != selected_engine |
            solver[row,4] != effective_family |
            solver[row,5] != effective_engine |
            solver[row,6] != checkpoint_family |
            solver[row,7] != row) {
            return(_fereg_ppml_validation_error("PPML solver report identities are not repeated exactly", 3499))
        }
        list_values_0 = (8, 9, 10, 11, 13)
        for (list_index_0 = 1; list_index_0 <= cols(list_values_0); ++list_index_0) {
            column = list_values_0[list_index_0]
            if (solver[row,column] >= . |  solver[row,column] != trunc(solver[row,column]) |  solver[row,column] < 0) {
                return(_fereg_ppml_validation_error("PPML solver report count is invalid", 3499))
            }
        }
        if (!any(( 0, 1, 2, 3) :== (solver[row,12])) |
            (solver[row,11] == 0) != (solver[row,12] == 0) |
            solver[row,14] >= . | solver[row,14] < 0 |
            solver[row,15] >= . | solver[row,15] < 0 |
            solver[row,16] >= . | solver[row,16] < 0) {
            return(_fereg_ppml_validation_error("PPML solver retry, fallback, or timing evidence is invalid", 3499))
        }
        for (column = 17; column <= 22; ++column) {
            if (solver[row,column] < . & solver[row,column] < 0) {
                return(_fereg_ppml_validation_error("PPML solver certificate maximum is invalid", 3499))
            }
        }
        frame_count = solver[row,8] + solver[row,9]
        engine_observed = (row == 1 | row == selected_engine |  (row == 2 & checkpoint_family == 3))
        if (!engine_observed) {
            for (column = 8; column <= 16; ++column) {
                if (solver[row,column] != 0) {
                    return(_fereg_ppml_validation_error("PPML unobserved solver engine has nonzero work", 3499))
                }
            }
        }
        if (frame_count == 0) {
            for (column = 17; column <= 22; ++column) {
                if (solver[row,column] < .) {
                    return(_fereg_ppml_validation_error("PPML unobserved solver engine has certificate evidence", 3499))
                }
            }
        }
        failed_attempt = (solver[row,11] > 0)
        if (frame_count > 0 & !failed_attempt &  (solver[row,17] >= . | solver[row,18] >= . |  solver[row,19] >= .)) {
            return(_fereg_ppml_validation_error("PPML observed solver engine lacks certificate maxima", 3499))
        }
    }
    if ((solver[1,12] == 1) | solver[4,12] == 2 |
        solver[2,11] != 0 | solver[2,12] != 0 |
        solver[3,11] != 0 | solver[3,12] != 0 |
        solver[4,11] > 1 |
        (solver[4,12] == 1 &
        (selected_family != 7 | effective_family != 2 | effective_engine != 1)) |
        (solver[1,12] == 2 &
        (checkpoint_family != 3 | effective_family != selected_family |
        effective_engine != selected_engine)) |
        (solver[1,12] == 3 &
        (checkpoint_family != 3 | selected_family != 7 |
        effective_family != 2 | effective_engine != 1)) |
        (solver[1,12] == 3 & solver[4,12] != 3) |
        (solver[4,12] == 3 & solver[1,12] != 3)) {
        return(_fereg_ppml_validation_error("PPML solver fallback evidence is inconsistent", 3499))
    }
    if ((selected_family == 7 & effective_family == 2 &  (solver[4,11] != 1 | solver[4,12] != 1))) {
        return(_fereg_ppml_validation_error("PPML CMG-to-MAP route lacks its numerical fallback evidence", 3499))
    }
    if (receipt[1,24] > receipt[1,8] |  receipt[1,23] > 5 * (receipt[1,8] + 1)) {
        return(_fereg_ppml_validation_error("PPML bounded execution evidence exceeds its candidate-sample bound", 3499))
    }

    if (inference[1,1] != requestedvcecode |
        inference[2,1] != vcecode | inference[3,1] <= 0 |
        inference[4,1] != ((vcecode == 3) ? ( 2) : ( 1)) |
        inference[5,1] != 1 | !any(( 1, 2, 3) :== (inference[6,1])) |
        !any(( 0, 1) :== (inference[7,1])) |
        inference[8,1] != trunc(inference[8,1]) | inference[8,1] < 0 |
        inference[8,1] > coefficients |
        inference[11,1] != trunc(inference[11,1]) |
        inference[11,1] < 0 | inference[11,1] > coefficients |
        inference[12,1] < 0 | inference[13,1] < 0 |
        inference[14,1] < 0 | inference[15,1] <= 0 |
        inference[15,1] >= 1 | inference[16,1] != 1) {
        return(_fereg_ppml_validation_error("PPML inference evidence failed semantic validation", 3499))
    }
    expected_correction = dof[1,1] / (dof[1,1] - 1)
    if (vcecode == 3) expected_correction =  dof[7,1] / (dof[7,1] - 1)
    if (reldif(inference[3,1], expected_correction) > 1e-12) {
        return(_fereg_ppml_validation_error("PPML VCE correction evidence disagrees with DoF", 3499))
    }
    if (inference[7,1] != (inference[6,1] == 3) |
        inference[7,1] != (inference[8,1] > 0) |
        (inference[6,1] == 1 & inference[8,1] != 0) |
        (inference[6,1] == 2 &
        (inference[9,1] < 0 | inference[8,1] != 0)) |
        (inference[6,1] == 3 &
        (inference[9,1] >= 0 | inference[8,1] == 0)) |
        inference[12,1] > inference[15,1] |
        inference[13,1] > inference[15,1] |
        inference[14,1] > inference[15,1]) {
        return(_fereg_ppml_validation_error("PPML PSD evidence is internally inconsistent", 3499))
    }
    if (((vcecode == 2 | clusters <= 1) & inference[6,1] != 1) |
        (vcecode == 3 & clusters > 1 &
        !any(( 2, 3) :== (inference[6,1])))) {
        return(_fereg_ppml_validation_error("PPML PSD state disagrees with the covariance route", 3499))
    }

    drop_total = 0
    for (drop_row = 1; drop_row <= 14; ++drop_row) {
        if (drop_counts[drop_row,1] != drop_row - 1 |
            drop_counts[drop_row,2] != trunc(drop_counts[drop_row,2]) |
            drop_counts[drop_row,2] < 0) {
            return(_fereg_ppml_validation_error("PPML drop-count evidence is invalid", 3499))
        }
        drop_total = drop_total + drop_counts[drop_row,2]
    }
    separated_drop_total = 0
    for (drop_row = 10; drop_row <= 13; ++drop_row) {
        separated_drop_total = separated_drop_total +  drop_counts[drop_row,2]
    }
    if (drop_total != receipt[1,7] |
        drop_counts[1,2] != receipt[1,9] |
        drop_counts[2,2] != receipt[1,7] - receipt[1,8] |
        separated_drop_total != receipt[1,16]) {
        return(_fereg_ppml_validation_error("PPML drop counts disagree with the receipt", 3499))
    }

    sequence_executions = 0
    sequence_direct_drops = 0
    sequence_singleton_drops = 0
    minimum_trace_row = .
    maximum_trace_row = 0
    maximum_generation = 0
    for (sequence_row = 1; sequence_row <= 5; ++sequence_row) {
        method_code = sequence_row
        method_requested = any(methodcodes :== strofreal(method_code))
        if (sequence_row == 1) method_requested = dropsingletons
        registered_order = ((sequence_row == 1 |  !method_requested) ? ( 0) : ( sequence_row - 1))
        executions = separation_sequence[sequence_row,4]
        if (separation_sequence[sequence_row,1] != method_code |
            separation_sequence[sequence_row,2] != method_requested |
            separation_sequence[sequence_row,3] != registered_order |
            executions != trunc(executions) | executions < 0 |
            separation_sequence[sequence_row,7] !=
            trunc(separation_sequence[sequence_row,7]) |
            separation_sequence[sequence_row,7] < 0 |
            separation_sequence[sequence_row,7] > 127 |
            !any(( 0, 1) :== (separation_sequence[sequence_row,13]))) {
            return(_fereg_ppml_validation_error("PPML separation sequence identity is invalid", 3499))
        }
        for (sequence_column = 5; sequence_column <= 12; ++sequence_column) {
            if (separation_sequence[sequence_row,sequence_column] !=
                trunc(separation_sequence[sequence_row,sequence_column]) |
                separation_sequence[sequence_row,sequence_column] < 0) {
                return(_fereg_ppml_validation_error("PPML separation sequence contains a nonexact count", 3499))
            }
        }
        if ((executions == 0 &
            (separation_sequence[sequence_row,5] != 0 |
            separation_sequence[sequence_row,6] != 0 |
            separation_sequence[sequence_row,8] != 0 |
            separation_sequence[sequence_row,9] != 0 |
            separation_sequence[sequence_row,10] != 0 |
            separation_sequence[sequence_row,11] != 0 |
            separation_sequence[sequence_row,12] != 0 |
            separation_sequence[sequence_row,13] != 1)) |
            (executions > 0 &
            (!method_requested |
            separation_sequence[sequence_row,5] < 1 |
            separation_sequence[sequence_row,5] >
            separation_sequence[sequence_row,6] |
            separation_sequence[sequence_row,6] > receipt[1,23] |
            separation_sequence[sequence_row,7] < 1 |
            separation_sequence[sequence_row,11] >
            separation_sequence[sequence_row,12] |
            separation_sequence[sequence_row,12] > receipt[1,24]))) {
            return(_fereg_ppml_validation_error("PPML separation execution bounds are inconsistent", 3499))
        }
        // ADR 0017 permits unresolved simplex attempts followed by certified
        // comprehensive IR. The per-method flag honestly remains false; the
        // native seal validates the complete same-generation execution chain.
        status_mask = separation_sequence[sequence_row,7]
        unresolved_status = mod(floor(status_mask/8),2) |  mod(floor(status_mask/16),2) | mod(floor(status_mask/64),2)
        if ((executions == 0 & status_mask != 0) |
            mod(floor(status_mask/32),2) |
            (unresolved_status & sequence_row != 3) |
            separation_sequence[sequence_row,13] != !unresolved_status) {
            return(_fereg_ppml_validation_error("PPML separation certificate states are inconsistent", 3499))
        }
        if (unresolved_status &
            (separation_sequence[4,2] != 1 |
            separation_sequence[4,4] < 1 |
            separation_sequence[4,6] <= separation_sequence[3,6] |
            separation_sequence[4,12] != receipt[1,24] |
            separation_sequence[4,13] != 1)) {
            return(_fereg_ppml_validation_error("PPML unresolved simplex lacks terminal comprehensive IR evidence", 3499))
        }
        if (executions > 0) {
            if (executions > separation_sequence[sequence_row,6] -  separation_sequence[sequence_row,5] + 1) {
                return(_fereg_ppml_validation_error("PPML separation execution count exceeds its trace span", 3499))
            }
            if (missing(minimum_trace_row) |  separation_sequence[sequence_row,5] < minimum_trace_row) {
                minimum_trace_row = separation_sequence[sequence_row,5]
            }
            if (separation_sequence[sequence_row,6] > maximum_trace_row ) maximum_trace_row = separation_sequence[sequence_row,6]
            if (separation_sequence[sequence_row,12] > maximum_generation ) maximum_generation = separation_sequence[sequence_row,12]
        }
        aggregate_row = 0
        for (aggregate_index = 1; aggregate_index <= separations; ++aggregate_index) {
            if (separation[aggregate_index,1] == method_code) {
                aggregate_row = aggregate_index
            }
        }
        expected_aggregate = (sequence_row == 1 | method_requested)
        if ((aggregate_row > 0) != expected_aggregate) {
            return(_fereg_ppml_validation_error("PPML separation sequence and aggregate inventories disagree", 3499))
        }
        if (aggregate_row > 0) {
            worst_status = 1
            if (separation_sequence[sequence_row,7] > 0) {
                worst_status = 0
                for (status_code = 1; status_code <= 7; ++status_code) {
                    if (mod(floor(separation_sequence[sequence_row,7] /  2^(status_code - 1)), 2)) {
                        worst_status = status_code
                    }
                }
            }
            if (separation[aggregate_row,2] != worst_status |
                separation[aggregate_row,3] != executions |
                separation[aggregate_row,4] !=
                separation_sequence[sequence_row,12] |
                separation[aggregate_row,5] !=
                separation_sequence[sequence_row,8] |
                separation[aggregate_row,6] !=
                separation_sequence[sequence_row,9] |
                separation[aggregate_row,7] !=
                separation_sequence[sequence_row,10] |
                separation[aggregate_row,8] !=
                separation_sequence[sequence_row,13]) {
                return(_fereg_ppml_validation_error("PPML separation sequence disagrees with its aggregate", 3499))
            }
        }
        if ((sequence_row == 1 &
            separation_sequence[sequence_row,9] != 0) |
            (sequence_row > 1 &
            separation_sequence[sequence_row,9] !=
            drop_counts[(sequence_row + 8),2])) {
            return(_fereg_ppml_validation_error("PPML method-specific separation drops disagree with drop reasons", 3499))
        }
        sequence_executions = sequence_executions + executions
        sequence_direct_drops = sequence_direct_drops +  separation_sequence[sequence_row,9]
        sequence_singleton_drops = sequence_singleton_drops +  separation_sequence[sequence_row,10]
    }
    if (sequence_executions != receipt[1,23] |
        sequence_direct_drops - separation_sequence[1,9] !=
        receipt[1,16] |
        sequence_singleton_drops != drop_counts[9,2]) {
        return(_fereg_ppml_validation_error("PPML separation sequence disagrees with receipt drop evidence", 3499))
    }
    if ((receipt[1,23] == 0 & receipt[1,24] != 0) |
        (receipt[1,23] > 0 &
        (minimum_trace_row != 1 | maximum_trace_row != receipt[1,23])) |
        maximum_generation != receipt[1,24] |
        (receipt[1,24] == 0 &
        (receipt[1,25] != receipt[1,27] |
        receipt[1,26] != receipt[1,28]))) {
        return(_fereg_ppml_validation_error("PPML bounded chronology or generation hashes are inconsistent", 3499))
    }
    if (receipt[1,7] != fit[23,1] |
        receipt[1,8] != fit[22,1] |
        receipt[1,9] != fit[1,1] |
        fit[17,1] != dof[1,1] |
        fit[2,1] != dof[2,1] | fit[3,1] != dof[3,1] |
        dof[4,1] != dof[2,1] + dof[3,1] |
        fit[4,1] != dof[5,1] | fit[5,1] != dof[6,1] |
        dof[5,1] != dof[1,1] - dof[4,1] |
        fit[24,1] != 1 | fit[27,1] != 1 |
        dof[10,1] != fit[1,1] |
        dof[11,1] != clusters |
        dof[12,1] != dof[5,1] - dof[6,1]) {
        return(_fereg_ppml_validation_error("PPML fit, DoF, and receipt identities disagree", 3499))
    }
    if (fit[1,1] != trunc(fit[1,1]) | fit[1,1] < 1 |
        fit[2,1] != trunc(fit[2,1]) | fit[2,1] < 1 |
        fit[3,1] != trunc(fit[3,1]) | fit[3,1] < 0 |
        fit[4,1] != trunc(fit[4,1]) | fit[4,1] < 1 |
        fit[5,1] != trunc(fit[5,1]) | fit[5,1] < 1 |
        fit[6,1] < 0 | fit[10,1] <= 0 |
        (fit[11,1] < . & fit[11,1] < 0) |
        fit[12,1] != trunc(fit[12,1]) | fit[12,1] < 0 |
        fit[12,1] > fit[2,1] | fit[13,1] <= 0 |
        fit[14,1] != trunc(fit[14,1]) | fit[14,1] < 0 |
        fit[15,1] != trunc(fit[15,1]) | fit[15,1] < 0 |
        fit[16,1] != trunc(fit[16,1]) | fit[16,1] < 0 |
        fit[17,1] != trunc(fit[17,1]) | fit[17,1] < 1 |
        fit[18,1] <= 0 | (fit[20,1] < . & fit[20,1] <= 0) |
        fit[21,1] != weightcode |
        fit[22,1] != trunc(fit[22,1]) | fit[22,1] < fit[1,1] |
        fit[23,1] != trunc(fit[23,1]) | fit[23,1] != st_nobs() |
        fit[26,1] >= . | fit[27,1] != 1) {
        return(_fereg_ppml_validation_error("PPML fit report failed semantic validation", 3499))
    }
    if ((weightcode == 0 & (fit[19,1] < . | fit[20,1] != 1 |
        fit[25,1] < . | fit[26,1] != 0 |
        reldif(fit[17,1], fit[1,1]) > 1e-12 |
        reldif(fit[18,1], fit[1,1]) > 1e-12)) |
        (weightcode == 1 & (fit[19,1] >= . | fit[19,1] <= 0 |
        fit[20,1] != 1 | fit[25,1] >= . | fit[26,1] != 0 |
        reldif(fit[19,1], fit[18,1]) > 1e-12 |
        reldif(fit[18,1], fit[17,1]) > 1e-12)) |
        (weightcode == 2 & ((fit[19,1] < . & fit[19,1] <= 0) |
        (fit[20,1] < . & fit[20,1] <= 0) |
        fit[25,1] >= . | fit[26,1] >= . |
        reldif(fit[17,1], fit[1,1]) > 1e-12 |
        reldif(fit[18,1], fit[1,1]) > 1e-12))) {
        return(_fereg_ppml_validation_error("PPML prepared-weight report is inconsistent", 3499))
    }
    if (weightcode != 0) {
        if (abs(ln(fit[18,1]) - fit[25,1] - fit[26,1]) >
            1e-10 * (1 + abs(ln(fit[18,1]))) |
            (fit[19,1] < . &
            abs(ln(fit[19,1]) - fit[25,1]) >
            1e-10 * (1 + abs(fit[25,1]))) |
            (fit[20,1] < . &
            abs(ln(fit[20,1]) - fit[26,1]) >
            1e-10 * (1 + abs(fit[26,1])))) {
            return(_fereg_ppml_validation_error("PPML linear and logarithmic weight metadata disagree", 3499))
        }
    }
    if (dof[1,1] != trunc(dof[1,1]) | dof[1,1] < 1 |
        dof[2,1] != trunc(dof[2,1]) | dof[2,1] < 1 |
        dof[3,1] != trunc(dof[3,1]) | dof[3,1] < 0 |
        dof[4,1] != trunc(dof[4,1]) | dof[4,1] < 1 |
        dof[5,1] != trunc(dof[5,1]) | dof[5,1] < 1 |
        dof[6,1] != trunc(dof[6,1]) | dof[6,1] < 1 |
        dof[6,1] > dof[5,1] |
        dof[8,1] != trunc(dof[8,1]) | dof[8,1] < 0 |
        !any(( 0, 1) :== (dof[9,1])) |
        dof[10,1] != trunc(dof[10,1]) | dof[10,1] < 1 |
        dof[12,1] != trunc(dof[12,1]) | dof[12,1] < 0) {
        return(_fereg_ppml_validation_error("PPML degrees-of-freedom report failed semantic validation", 3499))
    }
    finite_result =
        all(b :< .) & all(V :< .) &
        all(status :< .) &
        all(fit[|1,1 \ 10,1|] :< .) &
        all(fit[|12,1 \ 18,1|] :< .) &
        all(fit[|21,1 \ 24,1|] :< .) &
        all(fit[|26,1 \ 27,1|] :< .) &
        all(dof[|1,1 \ 6,1|] :< .) &
        all(dof[|8,1 \ 12,1|] :< .) &
        all(inference :< .) &
        all(drop_counts :< .) &
        all(separation_sequence :< .) &
        all(certificate :< .) &
        all(runtime[|1,1 \ 16,1|] :< .) &
        all(runtime[|24,1 \ 27,1|] :< .) &
        runtime[41,1] < . &
        all(runtime[|77,1 \ 78,1|] :< .) &
        all(runtime[|83,1 \ 95,1|] :< .)
    if (finite_result != 1) {
        return(_fereg_ppml_validation_error("PPML result contains nonfinite mandatory values", 3499))
    }
    covariance_scale =  max((max(vec(abs(V))), 2.2250738585072014e-308))
    if (inference[10,1] <  -inference[15,1] * covariance_scale) {
        return(_fereg_ppml_validation_error("PPML covariance eigenvalue evidence exceeds its certified scale", 3499))
    }
    for (row = 1; row <= 15; ++row) {
        if (certificate[row,1] < 0) {
            return(_fereg_ppml_validation_error("PPML certificate contains a negative error norm", 3499))
        }
    }
    if (certificate[16,1] != 1) {
        return(_fereg_ppml_validation_error("PPML composite certificate did not pass", 3499))
    }
    identified_coefficients = 0
    normalized_constants = 0
    for (slot = 1; slot <= coefficients; ++slot) {
        status_code = status[slot,2]
        if (status[slot,1] != slot |
            !any(( 1, 2, 3, 4, 5, 6, 7) :== (status_code)) |
            !any(( 0, 1) :== (status[slot,3])) |
            !any(( 0, 1) :== (status[slot,4])) |
            status[slot,3] != (status_code == 1) |
            status[slot,4] != (status_code != 1)) {
            return(_fereg_ppml_validation_error("PPML coefficient-status table is invalid", 3499))
        }
        if (status_code == 1) ++identified_coefficients
        if (status_code == 7) {
            ++normalized_constants
            if (slot != coefficients) {
                return(_fereg_ppml_validation_error("PPML normalized structural constant is outside the final slot", 3499))
            }
        }
        if (status_code != 1) {
            if (b[1,slot] != 0) {
                return(_fereg_ppml_validation_error("PPML omitted coefficient was not exactly zero-filled", 3499))
            }
            for (other = 1; other <= coefficients; ++other) {
                if (V[slot,other] != 0 | V[other,slot] != 0) {
                    return(_fereg_ppml_validation_error("PPML omitted covariance row was not exactly zero-filled", 3499))
                }
            }
        }
    }
    if (normalized_constants > 1 |  identified_coefficients + normalized_constants != dof[2,1]) {
        return(_fereg_ppml_validation_error("PPML coefficient statuses disagree with finite rank", 3499))
    }
    if (inference[8,1] > identified_coefficients |  inference[11,1] >  identified_coefficients - inference[8,1]) {
        return(_fereg_ppml_validation_error("PPML PSD evidence exceeds the identified covariance rank", 3499))
    }

    absorbed_rank = 0
    if (ferows == 0) {
        if (fe[1,1] != 2 | dof[3,1] != 0) {
            return(_fereg_ppml_validation_error("PPML no-FE sentinel has the wrong identity", 3499))
        }
        for (column = 2; column <= 8; ++column) {
            if (fe[1,column] != 0) {
                return(_fereg_ppml_validation_error("PPML no-FE sentinel contains nonzero metadata", 3499))
            }
        }
    }
    else {
        previous_term = 0
        term_has_slope = 0
        for (row = 1; row <= ferows; ++row) {
            kind = fe[row,1]
            term = fe[row,2]
            source = fe[row,3]
            if (!any(( 1, 3, 4) :== (kind)) | term != trunc(term) |
                term < 1 | term > feterms |
                (row == 1 & term != 1) |
                (row > 1 & (term < previous_term | term > previous_term + 1)) |
                source != trunc(source) | source < 0 | source > sources |
                (kind == 1 & source == 0) |
                (kind == 3 & source != 0) |
                (kind == 4 & source == 0) |
                fe[row,4] != trunc(fe[row,4]) | fe[row,4] < 1 |
                fe[row,5] != trunc(fe[row,5]) | fe[row,5] < 0 |
                fe[row,5] > fe[row,4] |
                fe[row,6] != fe[row,4] - fe[row,5] |
                !any(( 0, 1) :== (fe[row,7])) | !any(( 0, 1) :== (fe[row,8]))) {
                return(_fereg_ppml_validation_error("PPML fixed-effect report failed semantic validation", 3499))
            }
            if (term != previous_term) term_has_slope = 0
            if (kind == 4) term_has_slope = 1
            else if (term_has_slope) {
                return(_fereg_ppml_validation_error("PPML fixed-effect channels are outside declaration order", 3499))
            }
            absorbed_rank = absorbed_rank + fe[row,6]
            previous_term = term
        }
        if (previous_term != feterms | absorbed_rank != dof[3,1]) {
            return(_fereg_ppml_validation_error("PPML fixed-effect rows disagree with absorbed rank or terms", 3499))
        }
    }

    minimum_clusters = .
    if (clusters == 0) {
        for (column = 1; column <= 4; ++column) {
            if (cluster[1,column] != 0) {
                return(_fereg_ppml_validation_error("PPML no-cluster sentinel contains nonzero metadata", 3499))
            }
        }
        if (dof[7,1] < . | dof[12,1] != 0) {
            return(_fereg_ppml_validation_error("PPML nonclustered degrees of freedom carry cluster metadata", 3499))
        }
    }
    else {
        for (row = 1; row <= clusters; ++row) {
            if (cluster[row,1] != row |
                cluster[row,2] != trunc(cluster[row,2]) |
                cluster[row,2] < 1 | cluster[row,2] > sources |
                cluster[row,3] != trunc(cluster[row,3]) |
                cluster[row,3] < 2 | cluster[row,4] != 0) {
                return(_fereg_ppml_validation_error("PPML cluster report failed semantic validation", 3499))
            }
            if (missing(minimum_clusters) | cluster[row,3] < minimum_clusters) {
                minimum_clusters = cluster[row,3]
            }
        }
        if (dof[7,1] != minimum_clusters |  dof[6,1] != min((dof[5,1], minimum_clusters - 1))) {
            return(_fereg_ppml_validation_error("PPML cluster counts disagree with reported degrees of freedom", 3499))
        }
    }

    if (clusters == 0) {
        for (column = 1; column <= 4; ++column) {
            if (subset[1,column] != 0) {
                return(_fereg_ppml_validation_error("PPML no-subset sentinel contains nonzero metadata", 3499))
            }
        }
    }
    else {
        subset_row = 0
        for (cardinality = 1; cardinality <= clusters; ++cardinality) {
            for (mask = 1; mask <= subsets; ++mask) {
                bits = 0
                for (dimension = 0; dimension <= (clusters - 1); ++dimension) {
                    bits = bits + mod(floor(mask / 2^dimension), 2)
                }
                if (bits != cardinality) continue
                ++subset_row
                expected_sign = ((mod(cardinality, 2)) ? ( 1) : ( -1))
                if (subset[subset_row,1] != mask |
                    subset[subset_row,2] != trunc(subset[subset_row,2]) |
                    subset[subset_row,2] < 2 |
                    subset[subset_row,3] != expected_sign |
                    subset[subset_row,4] != cardinality) {
                    return(_fereg_ppml_validation_error("PPML cluster-subset report is not in canonical order", 3499))
                }
                if (cardinality == 1) {
                    cluster_dimension = round(ln(mask) / ln(2),1) + 1
                    if (subset[subset_row,2] != cluster[cluster_dimension,3]) {
                        return(_fereg_ppml_validation_error("PPML singleton subset count disagrees with its cluster dimension", 3499))
                    }
                }
            }
        }
        if (subset_row != subsets) {
            return(_fereg_ppml_validation_error("PPML cluster-subset inventory is incomplete", 3499))
        }
    }

    method_code_count = cols(methodcodes)
    if (method_code_count != separations) {
        return(_fereg_ppml_validation_error("PPML expected separation-method inventory is inconsistent", 3499))
    }
    statistical_drops = 0
    for (row = 1; row <= separations; ++row) {
        expected_method = strtoreal(methodcodes[row])
        if (separation[row,1] != expected_method |
            !any(( 1, 2, 3, 4, 5, 6, 7) :== (separation[row,2])) |
            separation[row,3] != trunc(separation[row,3]) |
            separation[row,3] < 0 |
            separation[row,4] != trunc(separation[row,4]) |
            separation[row,4] < 0 |
            separation[row,5] != trunc(separation[row,5]) |
            separation[row,5] < 0 |
            separation[row,6] != trunc(separation[row,6]) |
            separation[row,6] < 0 |
            separation[row,7] != trunc(separation[row,7]) |
            separation[row,7] < 0 |
            !any(( 0, 1) :== (separation[row,8])) |
            (separation[row,9] < . & separation[row,9] < 0) |
            (separation[row,10] < . & separation[row,10] < 0) |
            (separation[row,11] < . & separation[row,11] < 0) |
            (separation[row,12] < . & separation[row,12] < 0) |
            separation[row,13] != trunc(separation[row,13]) |
            separation[row,13] < 0 |
            separation[row,14] != trunc(separation[row,14]) |
            separation[row,14] < 0) {
            return(_fereg_ppml_validation_error("PPML separation aggregate failed semantic validation", 3499))
        }
        if (separation[row,3] == 0) {
            if (separation[row,2] != 1 |
                separation[row,4] != 0 | separation[row,5] != 0 |
                separation[row,6] != 0 | separation[row,7] != 0 |
                separation[row,8] != 1 | separation[row,9] < . |
                separation[row,10] < . | separation[row,11] < . |
                separation[row,12] < . | separation[row,13] != 0 |
                separation[row,14] != 0) {
                return(_fereg_ppml_validation_error("PPML unexecuted separation row is not a bounded sentinel", 3499))
            }
        }
        if (expected_method != 1) {
            statistical_drops = statistical_drops + separation[row,6]
        }
    }
    if (statistical_drops != receipt[1,16]) {
        return(_fereg_ppml_validation_error("PPML separation aggregate disagrees with the receipt", 3499))
    }

    if (runtime[1,1] < 0 |
        runtime[2,1] != trunc(runtime[2,1]) | runtime[2,1] < 0 |
        runtime[3,1] != trunc(runtime[3,1]) | runtime[3,1] < 0 |
        runtime[4,1] != trunc(runtime[4,1]) | runtime[4,1] < 0 |
        runtime[2,1] != fit[14,1] | runtime[3,1] != fit[15,1] |
        runtime[4,1] != fit[16,1] |
        runtime[5,1] != memorycode | !any(( 1, 2) :== (runtime[6,1])) |
        (checkpoint_family == 2 & runtime[7,1] != 2) |
        (checkpoint_family == 3 & !any(( 2, 3) :== (runtime[7,1]))) |
        runtime[8,1] != trunc(runtime[8,1]) | runtime[8,1] < 0 |
        runtime[9,1] != trunc(runtime[9,1]) | runtime[9,1] < 0 |
        runtime[10,1] != requestedthreads |
        runtime[10,1] < 0 | runtime[10,1] != trunc(runtime[10,1]) |
        runtime[11,1] < 1 | runtime[11,1] != trunc(runtime[11,1]) |
        runtime[12,1] < 0 | runtime[13,1] < 0 | runtime[14,1] < 0 |
        runtime[15,1] < 0 | runtime[16,1] < 0 |
        !any(( 0, 1) :== (runtime[24,1])) | !any(( 0, 1) :== (runtime[25,1])) |
        runtime[26,1] != trunc(runtime[26,1]) | runtime[26,1] < 0 |
        runtime[27,1] != trunc(runtime[27,1]) | runtime[27,1] < 0) {
        return(_fereg_ppml_validation_error("PPML runtime report failed semantic validation", 3499))
    }
    for (row = 17; row <= 23; ++row) {
        // Row 22 is signed allocator growth (current minus start).  All other
        // observed allocator byte counts and ratios are nonnegative.
        if (row != 22 & runtime[row,1] < . &  runtime[row,1] < 0) {
            return(_fereg_ppml_validation_error("PPML allocator telemetry is invalid", 3499))
        }
    }
    for (row = 28; row <= 40; ++row) {
        if (runtime[row,1] < . & runtime[row,1] < 0) {
            return(_fereg_ppml_validation_error("PPML phase timing is invalid", 3499))
        }
    }
    if (!any(( 0, 1) :== (runtime[41,1])) | runtime[77,1] != 1 |  runtime[78,1] != 1) {
        return(_fereg_ppml_validation_error("PPML convergence or allocation-audit flags are invalid", 3499))
    }
    audit_values = 0
    for (row = 42; row <= 76; ++row) {
        if (runtime[row,1] < .) ++audit_values
    }
    if ((runtime[41,1] == 0 & audit_values != 0) |
        (runtime[41,1] == 1 & audit_values != 35) |
        (runtime[41,1] == 0 & runtime[25,1] != 0)) {
        return(_fereg_ppml_validation_error("PPML allocation-audit telemetry is only partially present", 3499))
    }
    if (runtime[41,1] == 1) {
        list_values_1 = (42, 43, 44, 45, 46, 47, 49, 50, 51, 52, 53, 54, 55, 56, 57, 59, 60, 61, 62, 63, 64, 65, 66, 68, 69, 75)
        for (list_index_1 = 1; list_index_1 <= cols(list_values_1); ++list_index_1) {
            row = list_values_1[list_index_1]
            if (runtime[row,1] != trunc(runtime[row,1]) |  runtime[row,1] < 0) {
                return(_fereg_ppml_validation_error("PPML allocation-audit count or byte field is invalid", 3499))
            }
        }
        if (runtime[42,1] < 16 |
            runtime[44,1] < runtime[43,1] |
            runtime[45,1] != runtime[44,1] - runtime[43,1] |
            runtime[48,1] != trunc(runtime[48,1]) |
            runtime[48,1] != runtime[47,1] - runtime[46,1] |
            runtime[50,1] < runtime[49,1] |
            runtime[51,1] != runtime[50,1] - runtime[49,1] |
            !any(( 0, 1) :== (runtime[58,1])) |
            !any(( 0, 1) :== (runtime[67,1])) |
            !any(( 0, 1) :== (runtime[70,1])) |
            !any(( 0, 1) :== (runtime[72,1])) |
            !any(( 0, 1) :== (runtime[73,1])) |
            !any(( 0, 1) :== (runtime[74,1])) |
            !any(( 0, 1) :== (runtime[76,1])) |
            !any(( 1, 2, 3, 4, 5, 6, 7) :== (runtime[71,1]))) {
            return(_fereg_ppml_validation_error("PPML allocation certificate failed semantic validation", 3499))
        }
        ppml_capacity_stable =
            (runtime[52,1] == runtime[55,1] &
            runtime[53,1] == runtime[56,1] &
            runtime[54,1] == runtime[57,1])
        linear_capacity_stable =
            (runtime[59,1] == runtime[63,1] &
            runtime[60,1] == runtime[64,1] &
            runtime[61,1] == runtime[65,1] &
            runtime[62,1] == runtime[66,1])
        allocation_certificate_passed =
            (runtime[45,1] == 0 & runtime[48,1] == 0 &
            runtime[51,1] == 0 & runtime[58,1] == 1 &
            runtime[67,1] == 1 & runtime[70,1] == 1 &
            runtime[72,1] == 1 & runtime[73,1] == 1 &
            runtime[74,1] == 1)
        if (runtime[58,1] != ppml_capacity_stable |
            (runtime[67,1] == 1 & !linear_capacity_stable) |
            runtime[70,1] != (runtime[68,1] == runtime[69,1]) |
            runtime[69,1] != runtime[16,1] |
            (runtime[71,1] != runtime[7,1] &
            !(runtime[71,1] == 2 & runtime[7,1] == 3 &
            selected_family == 2 & effective_family == 2 &
            checkpoint_family == 3)) |
            runtime[72,1] != (runtime[71,1] == 2) |
            runtime[76,1] != allocation_certificate_passed |
            runtime[76,1] != runtime[25,1]) {
            return(_fereg_ppml_validation_error("PPML allocation certificate identities disagree", 3499))
        }
    }
    if (runtime[79,1] < . & runtime[79,1] != trunc(runtime[79,1])) {
        return(_fereg_ppml_validation_error("PPML repeated-call baseline growth is invalid", 3499))
    }
    if (runtime[79,1] < . &  runtime[79,1] != runtime[18,1] - runtime[17,1]) {
        return(_fereg_ppml_validation_error("PPML repeated-call baseline identity disagrees", 3499))
    }
    action_event_values = 0
    for (row = 80; row <= 82; ++row) {
        if (runtime[row,1] < .) ++action_event_values
    }
    if (!any(( 0, 3) :== (action_event_values))) {
        return(_fereg_ppml_validation_error("PPML action allocation-event telemetry is partial", 3499))
    }
    if (action_event_values == 3) {
        if (runtime[80,1] != trunc(runtime[80,1]) | runtime[80,1] < 0 |
            runtime[81,1] != trunc(runtime[81,1]) |
            runtime[81,1] < runtime[80,1] |
            runtime[82,1] != runtime[81,1] - runtime[80,1]) {
            return(_fereg_ppml_validation_error("PPML action allocation-event telemetry is invalid", 3499))
        }
    }
    for (row = 83; row <= 108; ++row) {
        if (runtime[row,1] < . &  (runtime[row,1] != trunc(runtime[row,1]) |  runtime[row,1] < 0)) {
            return(_fereg_ppml_validation_error("PPML phase-peak telemetry is invalid", 3499))
        }
        if (row <= 95 & runtime[row,1] >= .) {
            return(_fereg_ppml_validation_error("PPML modeled phase-peak telemetry is missing", 3499))
        }
    }
    return(0)
}

void _fereg_ppml_post_scalars()
{
    real matrix dof_shadow, fit_shadow, receipt_shadow, inference_shadow
    real matrix drop_counts_shadow, runtime_shadow, solver_shadow
    real scalar keep_singletons, n_absorb_terms, n_absorb_intercepts, n_absorb_slopes
    real scalar i, value
    string rowvector names

    dof_shadow = st_matrix(st_local("dof_shadow"))
    fit_shadow = st_matrix(st_local("fit_shadow"))
    receipt_shadow = st_matrix(st_local("receipt_shadow"))
    inference_shadow = st_matrix(st_local("inference_shadow"))
    drop_counts_shadow = st_matrix(st_local("drop_counts_shadow"))
    runtime_shadow = st_matrix(st_local("runtime_shadow"))
    solver_shadow = st_matrix(st_local("solver_shadow"))
    keep_singletons = strtoreal(st_local("keep_singletons"))
    n_absorb_terms = strtoreal(st_local("n_absorb_terms"))
    n_absorb_intercepts = strtoreal(st_local("n_absorb_intercepts"))
    n_absorb_slopes = strtoreal(st_local("n_absorb_slopes"))

    st_numscalar("e(rank)", dof_shadow[2,1])
    st_numscalar("e(feppml_model_rank)", dof_shadow[4,1])
    st_numscalar("e(df_m)", fit_shadow[12,1])
    st_numscalar("e(df_resid)", dof_shadow[6,1])
    st_numscalar("e(df_structural)", dof_shadow[5,1])
    st_numscalar("e(df_a)", dof_shadow[3,1])
    st_numscalar("e(N_physical)", fit_shadow[1,1])
    st_numscalar("e(N_effective)", fit_shadow[17,1])
    st_numscalar("e(N_candidate)", fit_shadow[22,1])
    st_numscalar("e(N_full)", fit_shadow[23,1])
    st_numscalar("e(deviance)", fit_shadow[6,1])
    st_numscalar("e(ll)", fit_shadow[7,1])
    st_numscalar("e(ll_0)", fit_shadow[8,1])
    st_numscalar("e(r2_p)", fit_shadow[9,1])
    st_numscalar("e(mu_0)", fit_shadow[10,1])
    if (fit_shadow[11,1] < .) {
        st_numscalar("e(chi2)", fit_shadow[11,1])
        st_numscalar("e(df_chi2)", fit_shadow[12,1])
    }
    st_numscalar("e(sum_y)", fit_shadow[13,1])
    st_numscalar("e(ic)", fit_shadow[14,1])
    st_numscalar("e(inner_iterations)", fit_shadow[15,1])
    st_numscalar("e(step_halvings)", fit_shadow[16,1])
    st_numscalar("e(sumweights)", fit_shadow[18,1])
    if (fit_shadow[19,1] < .) st_numscalar("e(raw_sumweights)", fit_shadow[19,1])
    if (fit_shadow[20,1] < .) st_numscalar("e(weight_multiplier)", fit_shadow[20,1])
    st_numscalar("e(feppml_weight_code)", fit_shadow[21,1])
    st_numscalar("e(converged)", fit_shadow[24,1])
    if (fit_shadow[25,1] < .) st_numscalar("e(log_raw_sumweights)", fit_shadow[25,1])
    st_numscalar("e(log_weight_multiplier)", fit_shadow[26,1])
    st_numscalar("e(feppml_objective_scale_code)", fit_shadow[27,1])
    st_numscalar("e(separated)", receipt_shadow[1,16])
    st_numscalar("e(feppml_vce_requested)", inference_shadow[1,1])
    st_numscalar("e(feppml_vce_effective)", inference_shadow[2,1])
    st_numscalar("e(feppml_vce_correction)", inference_shadow[3,1])
    st_numscalar("e(feppml_vce_correction_policy)", inference_shadow[4,1])
    st_numscalar("e(feppml_coef_distribution)", inference_shadow[5,1])
    st_numscalar("e(feppml_psd_state)", inference_shadow[6,1])
    st_numscalar("e(feppml_psd_adjusted)", inference_shadow[7,1])
    st_numscalar("e(feppml_psd_clipped)", inference_shadow[8,1])
    st_numscalar("e(feppml_psd_raw_min)", inference_shadow[9,1])
    st_numscalar("e(feppml_psd_final_min)", inference_shadow[10,1])
    st_numscalar("e(feppml_psd_final_rank)", inference_shadow[11,1])
    st_numscalar("e(feppml_psd_eigen_relinf)", inference_shadow[12,1])
    st_numscalar("e(feppml_psd_orthog_relinf)", inference_shadow[13,1])
    st_numscalar("e(feppml_psd_clip_relinf)", inference_shadow[14,1])
    st_numscalar("e(feppml_cert_tolerance)", inference_shadow[15,1])
    st_numscalar("e(feppml_psd_passed)", inference_shadow[16,1])
    st_numscalar("e(feppml_sep_executions)", receipt_shadow[1,23])
    st_numscalar("e(feppml_sep_generations)", receipt_shadow[1,24])
    st_numscalar("e(feppml_initial_hash_hi)", receipt_shadow[1,25])
    st_numscalar("e(feppml_initial_hash_lo)", receipt_shadow[1,26])
    st_numscalar("e(feppml_final_hash_hi)", receipt_shadow[1,27])
    st_numscalar("e(feppml_final_hash_lo)", receipt_shadow[1,28])
    st_numscalar("e(feppml_trace_digest_hi)", receipt_shadow[1,29])
    st_numscalar("e(feppml_trace_digest_lo)", receipt_shadow[1,30])
    st_numscalar("e(feppml_hash_digest_hi)", receipt_shadow[1,31])
    st_numscalar("e(feppml_hash_digest_lo)", receipt_shadow[1,32])
    st_numscalar("e(num_singletons)", drop_counts_shadow[9,2])
    st_numscalar("e(drop_singletons)", 1 - keep_singletons)
    st_numscalar("e(report_constant)", 1)
    st_numscalar("e(N_hdfe)", n_absorb_terms)
    st_numscalar("e(N_hdfe_extended)", (n_absorb_intercepts + n_absorb_slopes))
    st_numscalar("e(feppml_rank_is_approximate)", dof_shadow[9,1])
    st_numscalar("e(feppml_connected_components)", dof_shadow[8,1])
    st_numscalar("e(feppml_cluster_df_cap)", dof_shadow[12,1])
    if (dof_shadow[7,1] < .) st_numscalar("e(G_min)", dof_shadow[7,1])
    st_numscalar("e(feppml_memory_requested_code)", runtime_shadow[5,1])
    st_numscalar("e(feppml_memory_selected_code)", runtime_shadow[6,1])
    st_numscalar("e(feppml_linear_route)", runtime_shadow[7,1])
    st_numscalar("e(feppml_fallback_mask)", runtime_shadow[8,1])
    st_numscalar("e(feppml_fallback_count)", runtime_shadow[9,1])
    st_numscalar("e(feppml_solver_fallback_count)",
        solver_shadow[1,11] + solver_shadow[2,11] +
        solver_shadow[3,11] + solver_shadow[4,11])
    st_numscalar("e(feppml_requested_threads)", runtime_shadow[10,1])
    st_numscalar("e(feppml_threads)", runtime_shadow[11,1])
    st_numscalar("e(feppml_memory_fast_bytes)", runtime_shadow[12,1])
    st_numscalar("e(feppml_memory_low_bytes)", runtime_shadow[13,1])
    st_numscalar("e(feppml_memory_selected_bytes)", runtime_shadow[14,1])
    st_numscalar("e(feppml_modeled_scratch_bytes)", runtime_shadow[15,1])
    st_numscalar("e(feppml_scratch_bytes)", runtime_shadow[16,1])
    st_numscalar("e(feppml_workspace_allocation_free)", runtime_shadow[24,1])
    st_numscalar("e(feppml_iteration_allocation_free)", runtime_shadow[25,1])
    st_numscalar("e(feppml_diagnostic_records)", runtime_shadow[26,1])
    st_numscalar("e(feppml_strict_profile_rejections)", runtime_shadow[27,1])
    names = tokens("allocation_previous allocation_start allocation_current allocation_peak " +
        "allocation_peak_delta allocation_growth allocation_model_ratio")
    for (i = 1; i <= cols(names); ++i) {
        value = runtime_shadow[i + 16,1]
        if (value < .) st_numscalar("e(feppml_" + names[i] + ")", value)
    }
    names = tokens("preparation separation_fe separation_simplex separation_ir separation_mu " +
        "weight_refresh fe_projection finite_solve line_search strict_final_solve " +
        "inference fe_recovery publication")
    for (i = 1; i <= cols(names); ++i) {
        value = runtime_shadow[i + 27,1]
        if (value < .) st_numscalar("e(feppml_time_" + names[i] + ")", value)
    }
    names = tokens("allocation_audit_present audit_iterations audit_events_start " +
        "audit_events_end audit_events_delta audit_current_start audit_current_end " +
        "audit_current_growth audit_peak_start audit_peak_end audit_peak_growth " +
        "ppml_obs_cap_start ppml_coef_cap_start ppml_diag_cap_start ppml_obs_cap_end " +
        "ppml_coef_cap_end ppml_diag_cap_end ppml_capacity_stable " +
        "linear_elem_cap_start linear_res_elem_start linear_scratch_start " +
        "linear_fe_bytes_start linear_elem_cap_end linear_res_elem_end " +
        "linear_scratch_end linear_fe_bytes_end linear_capacity_stable " +
        "audit_scratch_start audit_scratch_end audit_scratch_stable " +
        "audit_linear_route audit_route_qualified linear_session_alloc_free " +
        "fresh_warm_equivalent steady_median_ns allocation_cert_passed " +
        "two_consecutive rank_frozen repeated_baseline_growth action_events_start " +
        "action_events_end action_events_delta")
    for (i = 1; i <= cols(names); ++i) {
        value = runtime_shadow[i + 40,1]
        if (value < .) st_numscalar("e(feppml_" + names[i] + ")", value)
    }
    names = tokens("prep sep_fe sep_simplex sep_ir sep_mu weight feproj finite line_search " +
        "final_solve inference fe_recovery publication")
    for (i = 1; i <= cols(names); ++i) {
        st_numscalar("e(feppml_modeled_peak_" + names[i] + ")", runtime_shadow[i + 82,1])
        value = runtime_shadow[i + 95,1]
        if (value < .) st_numscalar("e(feppml_observed_peak_" + names[i] + ")", value)
    }
}
end
