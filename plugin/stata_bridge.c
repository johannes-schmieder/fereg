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

static ST_int stfixest_bridge_require_main_thread(void)
{
    if (_stata_ == NULL || !bridge_thread_bound) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
#if SYSTEM==STWIN32
    if (GetCurrentThreadId() != bridge_main_thread) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
#else
    if (!pthread_equal(pthread_self(), bridge_main_thread)) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
#endif
    return 0;
}

static void stfixest_bridge_bind_main_thread(void)
{
#if SYSTEM==STWIN32
    bridge_main_thread = GetCurrentThreadId();
#else
    bridge_main_thread = pthread_self();
#endif
    bridge_thread_bound = 1;
}

ST_int stfixest_spi_is_main_thread(void)
{
    return stfixest_bridge_require_main_thread() == 0 ? 1 : 0;
}

ST_int stfixest_spi_display(const char *message)
{
    if (message == NULL) return STFIXEST_SPI_RC_SYNTAX;
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
    return SF_display((char *) message);
}

ST_int stfixest_spi_error(const char *message)
{
    if (message == NULL) return STFIXEST_SPI_RC_SYNTAX;
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
    return SF_error((char *) message);
}

ST_int stfixest_spi_poll(void)
{
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
    return SF_poll();
}

ST_int stfixest_spi_stop_requested(void)
{
    if (stfixest_bridge_require_main_thread() != 0) return 1;
    return SW_stopflag != 0 ? 1 : 0;
}

ST_int stfixest_spi_nobs(void)
{
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_VALUE_UNAVAILABLE;
    }
    return SF_nobs();
}

ST_int stfixest_spi_nvars(void)
{
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_VALUE_UNAVAILABLE;
    }
    return SF_nvars();
}

ST_int stfixest_spi_in1(void)
{
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_VALUE_UNAVAILABLE;
    }
    return SF_in1();
}

ST_int stfixest_spi_in2(void)
{
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_VALUE_UNAVAILABLE;
    }
    return SF_in2();
}

ST_int stfixest_spi_ifobs(ST_int observation)
{
    if (stfixest_bridge_require_main_thread() != 0) return 0;
    return SF_ifobs(observation) ? 1 : 0;
}

ST_int stfixest_spi_is_missing(ST_double value)
{
    if (stfixest_bridge_require_main_thread() != 0) return 1;
    return SF_is_missing(value) ? 1 : 0;
}

ST_int stfixest_spi_var_is_string(ST_int variable)
{
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_VALUE_UNAVAILABLE;
    }
    return SF_var_is_string(variable) ? 1 : 0;
}

ST_int stfixest_spi_var_is_strl(ST_int variable)
{
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_VALUE_UNAVAILABLE;
    }
    return SF_var_is_strl(variable) ? 1 : 0;
}

ST_int stfixest_spi_sdatalen(ST_int variable, ST_int observation)
{
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_VALUE_UNAVAILABLE;
    }
    return SF_sdatalen(variable, observation);
}

ST_int stfixest_spi_vdata(ST_int variable, ST_int observation, ST_double *value)
{
    if (value == NULL) return STFIXEST_SPI_RC_SYNTAX;
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
    return SF_vdata(variable, observation, value);
}

ST_int stfixest_spi_vstore(ST_int variable, ST_int observation, ST_double value)
{
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
    return SF_vstore(variable, observation, value);
}

ST_int stfixest_spi_sdata(ST_int variable, ST_int observation, char *buffer)
{
    if (buffer == NULL) return STFIXEST_SPI_RC_SYNTAX;
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
    return SF_sdata(variable, observation, buffer);
}

ST_int stfixest_spi_read_string(
    ST_int variable,
    ST_int observation,
    char *buffer,
    ST_int length
)
{
    ST_int rc;
    ST_int needed;
    if (buffer == NULL || length <= 0) return STFIXEST_SPI_RC_SYNTAX;
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
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

ST_int stfixest_spi_sstore(ST_int variable, ST_int observation, const char *value)
{
    if (value == NULL) return STFIXEST_SPI_RC_SYNTAX;
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
    return SF_sstore(variable, observation, (char *) value);
}

ST_int stfixest_spi_matrix_rows(const char *name)
{
    if (name == NULL) return STFIXEST_SPI_VALUE_UNAVAILABLE;
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_VALUE_UNAVAILABLE;
    }
    return SF_row((char *) name);
}

ST_int stfixest_spi_matrix_cols(const char *name)
{
    if (name == NULL) return STFIXEST_SPI_VALUE_UNAVAILABLE;
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_VALUE_UNAVAILABLE;
    }
    return SF_col((char *) name);
}

ST_int stfixest_spi_mat_el(const char *name, ST_int row, ST_int column, ST_double *value)
{
    if (name == NULL || value == NULL) return STFIXEST_SPI_RC_SYNTAX;
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
    return SF_mat_el((char *) name, row, column, value);
}

ST_int stfixest_spi_mat_store(const char *name, ST_int row, ST_int column, ST_double value)
{
    if (name == NULL) return STFIXEST_SPI_RC_SYNTAX;
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
    return SF_mat_store((char *) name, row, column, value);
}

ST_int stfixest_spi_scal_use(const char *name, ST_double *value)
{
    if (name == NULL || value == NULL) return STFIXEST_SPI_RC_SYNTAX;
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
    return SF_scal_use((char *) name, value);
}

ST_int stfixest_spi_scal_save(const char *name, ST_double value)
{
    if (name == NULL) return STFIXEST_SPI_RC_SYNTAX;
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
    return SF_scal_save((char *) name, &value);
}

ST_int stfixest_spi_macro_save(const char *name, const char *value)
{
    if (name == NULL || value == NULL) return STFIXEST_SPI_RC_SYNTAX;
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
    return SF_macro_save((char *) name, (char *) value);
}

ST_int stfixest_spi_macro_use(const char *name, char *buffer, ST_int length)
{
    if (name == NULL || buffer == NULL || length <= 0) {
        return STFIXEST_SPI_RC_SYNTAX;
    }
    if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
    return SF_macro_use((char *) name, buffer, length);
}

STDLL stata_call(int argc, char *argv[])
{
    const char *const *arguments;
    if (_stata_ == NULL) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
    if (argc < 0 || (argc > 0 && argv == NULL)) {
        return STFIXEST_SPI_RC_SYNTAX;
    }
    if (!bridge_thread_bound) {
        stfixest_bridge_bind_main_thread();
    }
    else if (stfixest_bridge_require_main_thread() != 0) {
        return STFIXEST_SPI_RC_UNAVAILABLE;
    }
    arguments = (const char *const *) argv;
    return stfixest_dispatch((ST_int) argc, arguments);
}
