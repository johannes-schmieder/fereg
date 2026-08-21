#ifndef STFIXEST_STATA_BRIDGE_H
#define STFIXEST_STATA_BRIDGE_H

#include "stplugin.h"

#if defined(_WIN32)
#define STFIXEST_BRIDGE_EXPORT __declspec(dllexport)
#else
#define STFIXEST_BRIDGE_EXPORT __attribute__(visibility("default"))
#endif

#ifdef __cplusplus
extern "C" {
'#endif

STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_display(const char *message);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_error(const char *message);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_poll(void);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_stop_requested(void);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_nobs(void);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_nvars(void);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_in1(void);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_in2(void);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_ifobs(ST_int observation);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_is_missing(ST_double value);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_var_is_string(ST_int variable);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_var_is_strl(ST_int variable);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_sdatalen(ST_int variable, ST_int observation);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_vdata(ST_int variable, ST_int observation, ST_double *value);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_vstore(ST_int variable, ST_int observation, ST_double value);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_sdata(ST_int variable, ST_int observation, char *buffer);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_read_string(
    ST_int variable,
    ST_int observation,
    char *buffer,
    ST_int length
);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_sstore(ST_int variable, ST_int observation, const char *value);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_matrix_rows(const char *name);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_matrix_cols(const char *name);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_mat_el(const char *name, ST_int row, ST_int column, ST_double *value);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_mat_store(const char *name, ST_int row, ST_int column, ST_double value);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_scal_use(const char *name, ST_double *value);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_scal_save(const char *name, ST_double value);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_macro_save(const char *name, const char *value);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_macro_use(const char *name, char *buffer, ST_int length);
STFIXEST_BRIDGE_EXPORT ST_int stfixest_spi_is_main_thread(void);

STFIXEST_BRIDGE_EXPORT ST_retcode stfixest_dispatch(ST_int argc, const char *const *argv);
STDLL stata_call(int argc, char *argv[]);

#ifdef __cplusplus
}
#endif

#endif
