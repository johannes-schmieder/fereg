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

/*
 * One read-only numeric window (see fereg_spi_numeric_window_open). The main
 * thread writes these fields only while no worker can call the window: Rust
 * opens it before dispatching a phase and closes it after joining every
 * worker of that phase, so workers only read them.
 */
static struct {
    ST_int open;
    ST_int generation;
    const ST_int *variables;
    ST_int variable_count;
    ST_int observations;
    ST_double missing_value;
    ST_int (*reader)(ST_int, ST_int, ST_double *);
} numeric_window;

static ST_int fereg_bridge_on_main_thread(void)
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

/* Every SPI wrapper except the window's own read/close requires the bound
   main thread and a closed numeric window: while workers read, the main thread
   makes no SPI call (no polling, display, metadata or stores). */
static ST_int fereg_bridge_require_main_thread(void)
{
    if (fereg_bridge_on_main_thread() != 0 || numeric_window.open) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
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
    return fereg_bridge_on_main_thread() == 0 ? 1 : 0;
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
    ST_int (*reader)(ST_int, ST_int, ST_double *);
    ST_int observations;
    ST_int variables_available;
    if (variables == NULL || columns == NULL || missing_value == NULL || variable_count <= 0 ||
        first_observation <= 0 || observation_count <= 0) {
        return FEREG_SPI_RC_SYNTAX;
    }
    if (fereg_bridge_require_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    /* Numeric batch addresses are checked once before using the SDK's direct
       accessor. Scalar reads retain SF_vdata checks on every platform. */
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
    *missing_value = SV_missval;
    for (observation = 0; observation < observation_count; ++observation) {
        for (variable = 0; variable < variable_count; ++variable) {
            rc = reader(variables[variable], first_observation + observation,
                        &columns[variable][observation]);
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
}

ST_int fereg_spi_numeric_window_open(
    const ST_int *variables,
    ST_int variable_count,
    ST_int observations,
    ST_double *missing_value,
    ST_int *generation
)
{
    ST_int variable;
    ST_int variables_available;
    ST_int (*reader)(ST_int, ST_int, ST_double *);
    if (variables == NULL || missing_value == NULL || generation == NULL ||
        variable_count <= 0 || observations <= 0) {
        return FEREG_SPI_RC_SYNTAX;
    }
    if (fereg_bridge_require_main_thread() != 0) return FEREG_SPI_RC_UNAVAILABLE;
    if (SF_nobs() != observations) return FEREG_SPI_RC_UNAVAILABLE;
    variables_available = SF_nvars();
    for (variable = 0; variable < variable_count; ++variable) {
        if (variables[variable] <= 0 || variables[variable] > variables_available ||
            SF_var_is_string(variables[variable])) {
            return FEREG_SPI_RC_UNAVAILABLE;
        }
    }
    reader = _stata_->vdata;
    if (reader == NULL) return FEREG_SPI_RC_UNAVAILABLE;
    numeric_window.variables = variables;
    numeric_window.variable_count = variable_count;
    numeric_window.observations = observations;
    numeric_window.missing_value = SV_missval;
    numeric_window.reader = reader;
    numeric_window.generation =
        numeric_window.generation >= 0x7fffffff ? 1 : numeric_window.generation + 1;
    numeric_window.open = 1;
    *missing_value = numeric_window.missing_value;
    *generation = numeric_window.generation;
    return 0;
}

ST_int fereg_spi_numeric_window_read(
    ST_int generation,
    ST_int first_slot,
    ST_int slot_count,
    ST_int first_observation,
    ST_int observation_count,
    const unsigned char *retained,
    const ST_double *excluded_values,
    const unsigned char *excluded_missing,
    ST_double *const *values,
    unsigned char *const *missing
)
{
    ST_int observation, slot, rc;
    ST_double value, boundary;
    const ST_int *variables;
    ST_int (*reader)(ST_int, ST_int, ST_double *);
    /* Callable from any thread, but only inside the main thread's open window.
       It reads the window state without SPI metadata calls. */
    if (!numeric_window.open || generation != numeric_window.generation) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    if (values == NULL || slot_count <= 0 || slot_count > 64 || first_slot < 0 ||
        first_slot > numeric_window.variable_count - slot_count ||
        first_observation <= 0 || observation_count <= 0 ||
        first_observation > numeric_window.observations ||
        observation_count > numeric_window.observations - first_observation + 1 ||
        (retained != NULL &&
         (excluded_values == NULL || (missing != NULL && excluded_missing == NULL)))) {
        return FEREG_SPI_RC_SYNTAX;
    }
    for (slot = 0; slot < slot_count; ++slot) {
        if (values[slot] == NULL || (missing != NULL && missing[slot] == NULL)) {
            return FEREG_SPI_RC_SYNTAX;
        }
    }
    reader = numeric_window.reader;
    variables = numeric_window.variables + first_slot;
    boundary = numeric_window.missing_value;
    for (observation = 0; observation < observation_count; ++observation) {
        if (retained != NULL && retained[observation] == 0) {
            for (slot = 0; slot < slot_count; ++slot) {
                values[slot][observation] = excluded_values[slot];
                if (missing != NULL) missing[slot][observation] = excluded_missing[slot];
            }
            continue;
        }
        /* Stata stores every numeric missing code at or above `.`; the SDK
           converts each storage type to this double, as in the serial batch. */
        for (slot = 0; slot < slot_count; ++slot) {
            rc = reader(variables[slot], first_observation + observation, &value);
            if (rc != 0) return rc;
            values[slot][observation] = value;
            if (missing != NULL) {
                missing[slot][observation] = (unsigned char) (value >= boundary ? 1 : 0);
            }
        }
    }
    return 0;
}

ST_int fereg_spi_numeric_window_close(ST_int generation)
{
    if (fereg_bridge_on_main_thread() != 0 || !numeric_window.open ||
        generation != numeric_window.generation) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    numeric_window.open = 0;
    numeric_window.variables = NULL;
    numeric_window.variable_count = 0;
    numeric_window.observations = 0;
    numeric_window.reader = NULL;
    return 0;
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
    else if (fereg_bridge_on_main_thread() != 0) {
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    if (numeric_window.open) {
        /* No action is running and every worker joined before the previous
           call returned, so a window still open here is a leaked guard. Close
           it and fail this call visibly instead of refusing every later one. */
        numeric_window.open = 0;
        numeric_window.variables = NULL;
        numeric_window.variable_count = 0;
        numeric_window.observations = 0;
        numeric_window.reader = NULL;
        SF_error((char *) "fereg: internal: a numeric input window was left open and has been closed\n");
        return FEREG_SPI_RC_UNAVAILABLE;
    }
    arguments = (const char *const *) argv;
    return fereg_dispatch((ST_int) argc, arguments);
}
