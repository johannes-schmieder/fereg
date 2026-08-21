#ifndef STFIXEST_STATA_BRIDGE_H
#define STFIXEST_STATA_BRIDGE_H

#include "stplugin.h"

#ifdef __cplusplus
extern "C" {
#endif

/* Implemented by the Rust static library. */
ST_retcode stfixest_dispatch(ST_int argc, const char *const *argv);

/* Main-thread-only wrappers around the Stata Plugin Interface. */
ST_int stfixest_spi_display(const char *message);
ST_int stfixest_spi_error(const char *message);
ST_int stfixest_spi_poll(void);
ST_int stfixest_spi_stop_requested(void);
ST_int stfixest_spi_nobs(void);
ST_int stfixest_spi_nvars(void);
ST_int stfixest_spi_vdata(ST_int variable, ST_int observation, ST_double *value);
ST_int stfixest_spi_vstore(ST_int variable, ST_int observation, ST_double value);
ST_int stfixest_spi_sdata(ST_int variable, ST_int observation, char *buffer);
ST_int stfixest_spi_sstore(ST_int variable, ST_int observation, const char *value);
ST_int stfixest_spi_mat_el(
    const char *name,
    ST_int row,
    ST_int column,
    ST_double *value
);
ST_int stfixest_spi_mat_store(
    const char *name,
    ST_int row,
    ST_int column,
    ST_double value
);
ST_int stfixest_spi_scal_use(const char *name, ST_double *value);
ST_int stfixest_spi_scal_save(const char *name, ST_double value);
ST_int stfixest_spi_macro_save(const char *name, const char *value);
ST_int stfixest_spi_macro_use(const char *name, char *buffer, ST_int length);
ST_int stfixest_spi_is_missing(ST_double value);

STDLL stata_call(int argc, char *argv[]);

#ifdef __cplusplus
}
#endif

#endif
