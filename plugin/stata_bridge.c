/* Minimal Stata-thread bridge. Scientific logic is implemented in Rust. */
#include <stddef.h>
#include "stata_bridge.h"

ST_int stfixest_spi_display(const char *message)
{
    if (message == NULL) return 198;
    return SF_display((char *) message);
}

ST_int stfixest_spi_error(const char *message)
{
    if (message == NULL) return 198;
    return SF_error((char *) message);
}

ST_int stfixest_spi_poll(void)
{
    return SF_poll();
}

ST_int stfixest_spi_stop_requested(void)
{
    return SW_stopflag != 0;
}

ST_int stfixest_spi_nobs(void)
{
    return SF_nobs();
}

ST_int stfixest_spi_nvars(void)
{
    return SF_nvars();
}

ST_int stfixest_spi_vdata(ST_int variable, ST_int observation, ST_double *value)
{
    if (value == NULL) return 198;
    return SF_vdata(variable, observation, value);
}

ST_int stfixest_spi_vstore(ST_int variable, ST_int observation, ST_double value)
{
    return SF_vstore(variable, observation, value);
}

ST_int stfixest_spi_sdata(ST_int variable, ST_int observation, char *buffer)
{
    if (buffer == NULL) return 198;
    return SF_sdata(variable, observation, buffer);
}

ST_int stfixest_spi_sstore(ST_int variable, ST_int observation, const char *value)
{
    if (value == NULL) return 198;
    return SF_sstore(variable, observation, (char *) value);
}

ST_int stfixest_spi_mat_el(const char *name, ST_int row, ST_int column, ST_double *value)
{
    if (name == NULL || value == NULL) return 198;
    return SF_mat_el((char *) name, row, column, value);
}

ST_int stfixest_spi_mat_store(const char *name, ST_int row, ST_int column, ST_double value)
{
    if (name == NULL) return 198;
    return SF_mat_store((char *) name, row, column, value);
}

ST_int stfixest_spi_scal_use(const char *name, ST_double *value)
{
    if (name == NULL || value == NULL) return 198;
    return SF_scal_use((char *) name, value);
}

ST_int stfixest_spi_scal_save(const char *name, ST_double value)
{
    if (name == NULL) return 198;
    return SF_scal_save((char *) name, &value);
}

ST_int stfixest_spi_macro_save(const char *name, const char *value)
{
    if (name == NULL || value == NULL) return 198;
    return SF_macro_save((char *) name, (char *) value);
}

ST_int stfixest_spi_macro_use(const char *name, char *buffer, ST_int length)
{
    if (name == NULL || buffer == NULL || length < 0) return 198;
    return SF_macro_use((char *) name, buffer, length);
}

ST_int stfixest_spi_is_missing(ST_double value)
{
    return SF_is_missing(value) != 0;
}

STDLL stata_call(int argc, char *argv[])
{
    return stfixest_dispatch(argc, (const char *const *) argv);
}
