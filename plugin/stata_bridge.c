#include "stata_bridge.h"

#include <stddef.h>
#include <stdint.h>
#include <string.h>

#if SYSTEM==STWIN32
#include <windows.h>
#else
#include <pthread.h>
#endif

static ST_int bridge_thread_bound = 0;
#if SYSTEM==STWIN32
static DWORD bridge_main_thread = 0;
#else
static pthread_t bridge_main_thread;
#endif

static ST_int fereg_bridge_require_main_thread(void)
{
    if (_stata_ == NULL || !bridge_thread_bound) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
#if SYSTEM==STWIN32
    if (GetCurrentThreadId() != bridge_main_thread) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
#else
    if (!pthread_equal(pthread_self(), bridge_main_thread)) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
#endif
    return 0;
}

static void fereg_bridge_bind_main_thread(void)
{
#if SYSTEM==STWIN32
    bridge_main_thread = GetCurrentThreadId();
#else
    bridge_main_thread = pthread_self();
#endif
    bridge_thread_bound = 1;
}

ST_int fereg_spi_is_main_thread(void)
{
    return fereg_bridge_require_main_thread() == 0 ? 1 : 0;
}

ST_int fereg_spi_display(const char *message)
{
    if (message == NULL) return FEREG_SPI_RC_SYNTAX;
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    return SF_display((char *) message);
}

ST_int fereg_spi_error(const char *message)
{
    if (message == NULL) return FEREG_SPI_RC_SYNTAX;
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    return SF_error((char *) message);
}

ST_int fereg_spi_poll(void)
{
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    return SF_poll();
}

ST_int fereg_spi_stop_requested(void)
{
    if (fereg_bridge_require_main_thread() != 0) return 1;
    return SW_stopflag != 0 ? 1 : 0;
}

ST_int fereg_spi_nobs(void)
{
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_VALUE_UNAVAILABLE;
    }
    return SF_nobs();
}

ST_int fereg_spi_nvars(void)
{
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_VALUE_UNAVAILABLE;
    }
    return SF_nvars();
}

ST_int fereg_spi_in1(void)
{
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_VALUE_UNAVAILABLE;
    }
    return SF_in1();
}

ST_int fereg_spi_in2(void)
{
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_VALUE_UNAVAILABLE;
    }
    return SF_in2();
}

ST_int fereg_spi_ifobs(ST_int observation)
{
    if (fereg_bridge_require_main_thread() != 0) return 0;
    return SF_ifobs(observation) ? 1 : 0;
}

ST_int fereg_spi_is_missing(ST_double value)
{
    if (fereg_bridge_require_main_thread() != 0) return 1;
    return SF_is_missing(value) ? 1 : 0;
}

ST_int fereg_spi_var_is_string(ST_int variable)
{
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_VALUE_UNAVAILABLE;
    }
    return SF_var_is_string(variable) ? 1 : 0;
}

ST_int fereg_spi_var_is_strl(ST_int variable)
{
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_VALUE_UNAVAILABLE;
    }
    return SF_var_is_strl(variable) ? 1 : 0;
}

ST_int fereg_spi_sdatalen(ST_int variable, ST_int observation)
{
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_VALUE_UNAVAILABLE;
    }
    return SF_sdatalen(variable, observation);
}

ST_int fereg_spi_vdata(ST_int variable, ST_int observation, ST_double *value)
{
    if (value == NULL) return FEREG_SPI_RC_SYNTAX;
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    return SF_vdata(variable, observation, value);
}

ST_int fereg_spi_vdata_with_missing(
    ST_int variable,
    ST_int observation,
    ST_double *value,
    ST_int *missing
)
{
    ST_int rc;
    if (value == NULL || missing == NULL) return FEREG_SPI_RC_SYNTAX;
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    rc = SF_vdata(variable, observation, value);
    if (rc != 0) return rc;
    *missing = SF_is_missing(*value) ? 1 : 0;
    return 0;
}

ST_int fereg_spi_vdata_columns(
    const ST_int *variables,
    ST_int variable_count,
    ST_int first_observation,
    ST_int observation_count,
    ST_double **columns,
    ST_double *missing_value
)
{
    ST_int observation;
    ST_int variable;
    ST_int rc;
#if SYSTEM==APPLEMAC
    ST_int (*reader)(ST_int, ST_int, ST_double *);
    ST_int observations;
    ST_int variables_available;
#endif
    if (variables == NULL || columns == NULL || missing_value == NULL || variable_count <= 0 ||
        first_observation <= 0 || observation_count <= 0) {
        return FEREG_SPI_RC_SYNTAX;
    }
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
#if SYSTEM==APPLEMAC
    /* Numeric batch addresses are checked once before using the SDK's direct
       accessor. Scalar reads and other platforms retain SF_vdata checks. */
    observations = SF_nobs();
    variables_available = SF_nvars();
    if (first_observation > observations ||
        observation_count > observations - first_observation + 1) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    for (variable = 0; variable < variable_count; ++variable) {
        if (columns[variable] == NULL) return FEREG_SPI_RC_SYNTAX;
        if (variables[variable] <= 0 || variables[variable] > variables_available ||
            SF_var_is_string(variables[variable])) {
            return FEREG_SPI_RC_UNAVAILABLE;
        }
    }
    reader = _stata_->vdata;
    if (reader == NULL) return FEREG_SPI_RC_UNAVAILABLE;
#endif
    *missing_value = SV_missval;
    for (observation = 0; observation < observation_count; ++observation) {
        for (variable = 0; variable < variable_count; ++variable) {
#if SYSTEM==APPLEMAC
            rc = reader(variables[variable], first_observation + observation,
                        &columns[variable][observation]);
#else
            if (columns[variable] == NULL) return FEREG_SPI_RC_SYNTAX;
            rc = SF_vdata(
                variables[variable],
                first_observation + observation,
                &columns[variable][observation]
            );
#endif
            if (rc != 0) return rc;
        }
    }
    return 0;
}

ST_int fereg_spi_vdata_rows(
    const ST_int *variables,
    ST_int variable_count,
    ST_int first_observation,
    ST_int observation_count,
    ST_double *values,
    ST_double *missing_value
)
{
#if SYSTEM==APPLEMAC
    ST_int observation, variable, rc;
    ST_int observations, variables_available;
    ST_int (*reader)(ST_int, ST_int, ST_double *);
    if (variables == NULL || values == NULL || missing_value == NULL ||
        variable_count <= 0 || variable_count > 64 || first_observation <= 0 ||
        observation_count <= 0 || observation_count > 4096) {
        return FEREG_SPI_RC_SYNTAX;
    }
    if (fereg_bridge_require_main_thread() != 0) return FEREG_SPI_RC_UNAVAILABLE;
    observations = SF_nobs();
    variables_available = SF_nvars();
    if (first_observation > observations ||
        observation_count > observations - first_observation + 1) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    for (variable = 0; variable < variable_count; ++variable) {
        if (variables[variable] <= 0 || variables[variable] > variables_available ||
            SF_var_is_string(variables[variable])) return FEREG_SPI_RC_UNAVAILABLE;
    }
    reader = _stata_->vdata;
    if (reader == NULL) return FEREG_SPI_RC_UNAVAILABLE;
    *missing_value = SV_missval;
    for (observation = 0; observation < observation_count; ++observation) {
        /* The bounded main loop needs one width check per four SDK calls.
           Each call still returns immediately on failure, in scalar order. */
        for (variable = 0; variable + 3 < variable_count; variable += 4) {
            rc = reader(variables[variable], first_observation + observation, values++);
            if (rc != 0) return rc;
            rc = reader(variables[variable + 1], first_observation + observation, values++);
            if (rc != 0) return rc;
            rc = reader(variables[variable + 2], first_observation + observation, values++);
            if (rc != 0) return rc;
            rc = reader(variables[variable + 3], first_observation + observation, values++);
            if (rc != 0) return rc;
        }
        for (; variable < variable_count; ++variable) {
            rc = reader(variables[variable], first_observation + observation, values++);
            if (rc != 0) return rc;
        }
    }
    return 0;
#else
    (void)variables; (void)variable_count; (void)first_observation;
    (void)observation_count; (void)values; (void)missing_value;
    return FEREG_SPI_RC_UNAVAILABLE;
#endif
}

ST_int fereg_spi_vstore(ST_int variable, ST_int observation, ST_double value)
{
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    return SF_vstore(variable, observation, value);
}

ST_int fereg_spi_vstore_columns(
    const ST_int *variables,
    ST_int variable_count,
    ST_int observation_count,
    const ST_double *const *columns
)
{
    ST_int variable, observation, rc;
    ST_int variables_available;
    ST_int (*store)(ST_int, ST_int, ST_double);
    if (variables == NULL || columns == NULL || variable_count <= 0 ||
        variable_count > 64 || observation_count <= 0) return FEREG_SPI_RC_SYNTAX;
    if (fereg_bridge_require_main_thread() != 0) return FEREG_SPI_RC_UNAVAILABLE;
#if defined(SD_SAFEMODE)
    store = _stata_->safestore;
#else
    store = _stata_->store;
#endif
    if (store == NULL) return FEREG_SPI_RC_UNAVAILABLE;
    if (observation_count > SF_nobs()) return FEREG_SPI_RC_UNAVAILABLE;
    variables_available = SF_nvars();
    for (variable = 0; variable < variable_count; ++variable) {
        if (columns[variable] == NULL) return FEREG_SPI_RC_SYNTAX;
        if (variables[variable] <= 0 || variables[variable] > variables_available ||
            SF_var_is_string(variables[variable])) return FEREG_SPI_RC_UNAVAILABLE;
    }
    /* Keep the checked SDK accessor and the scalar row/column error order.
       Only the per-value Rust/C crossing and thread check move out of the loop. */
    for (observation = 0; observation < observation_count; ++observation) {
        for (variable = 0; variable < variable_count; ++variable) {
            rc = store(variables[variable], observation + 1, columns[variable][observation]);
            if (rc != 0) return rc;
        }
    }
    return 0;
}

ST_int fereg_spi_sdata(ST_int variable, ST_int observation, char *buffer)
{
    if (buffer == NULL) return FEREG_SPI_RC_SYNTAX;
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    return SF_sdata(variable, observation, buffer);
}

ST_int fereg_spi_read_string(
    ST_int variable,
    ST_int observation,
    char *buffer,
    ST_int length
)
{
    ST_int rc;
    ST_int needed;
    if (buffer == NULL || length <= 0) return FEREG_SPI_RC_SYNTAX;
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    needed = SF_sdatalen(variable, observation);
    if (needed < 0) return needed;
    if (needed >= length) return 920;
    if (SF_var_is_strl(variable)) {
        if (SF_var_is_binary(variable, observation)) return 109;
        rc = SF_strldata(variable, observation, buffer, length);
    }
    else {
        rc = SF_sdata(variable, observation, buffer);
    }
    if (rc != 0) return rc;
    buffer[length - 1] = '\0';
    return 0;
}

ST_int fereg_spi_sstore(ST_int variable, ST_int observation, const char *value)
{
    if (value == NULL) return FEREG_SPI_RC_SYNTAX;
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    return SF_sstore(variable, observation, (char *) value);
}

ST_int fereg_spi_matrix_rows(const char *name)
{
    if (name == NULL) return FEREG_SPI_VALUE_UNAVAILABLE;
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_VALUE_UNAVAILABLE;
    }
    return SF_row((char *) name);
}

ST_int fereg_spi_matrix_cols(const char *name)
{
    if (name == NULL) return FEREG_SPI_VALUE_UNAVAILABLE;
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_VALUE_UNAVAILABLE;
    }
    return SF_col((char *) name);
}

ST_int fereg_spi_mat_el(const char *name, ST_int row, ST_int column, ST_double *value)
{
    if (name == NULL || value == NULL) return FEREG_SPI_RC_SYNTAX;
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    return SF_mat_el((char *) name, row, column, value);
}

ST_int fereg_spi_mat_store(const char *name, ST_int row, ST_int column, ST_double value)
{
    if (name == NULL) return FEREG_SPI_RC_SYNTAX;
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    return SF_mat_store((char *) name, row, column, value);
}

ST_int fereg_spi_scal_use(const char *name, ST_double *value)
{
    if (name == NULL || value == NULL) return FEREG_SPI_RC_SYNTAX;
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    return SF_scal_use((char *) name, value);
}

ST_int fereg_spi_scal_save(const char *name, ST_double value)
{
    if (name == NULL) return FEREG_SPI_RC_SYNTAX;
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    return SF_scal_save((char *) name, &value);
}

ST_int fereg_spi_macro_save(const char *name, const char *value)
{
    if (name == NULL || value == NULL) return FEREG_SPI_RC_SYNTAX;
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    return SF_macro_save((char *) name, (char *) value);
}

ST_int fereg_spi_macro_use(const char *name, char *buffer, ST_int length)
{
    if (name == NULL || buffer == NULL || length <= 0) {
        return FEREG_SPI_RC_SYNTAX;
    }
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    return SF_macro_use((char *) name, buffer, length);
}

STDLL stata_call(int argc, char *argv[])
{
    const char *const *arguments;
    if (_stata_ == NULL) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    if (argc < 0 || (argc > 0 && argv == NULL)) {
        return FEREG_SPI_RC_SYNTAX;
    }
    if (!bridge_thread_bound) {
        fereg_bridge_bind_main_thread();
    }
    else if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    arguments = (const char *const *) argv;
    return fereg_dispatch((ST_int) argc, arguments);
}
