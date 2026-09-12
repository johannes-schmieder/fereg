#ifndef FEREG_STATA_BRIDGE_H
#define FEREG_STATA_BRIDGE_H

#include "stplugin.h"

#if defined(_WIN32)
#define FEREG_BRIDGE_INTERNAL
#elif defined(__GNUC__) || defined(__clang__)
#define FEREG_BRIDGE_INTERNAL __attribute__((visibility("hidden")))
#else
#define FEREG_BRIDGE_INTERNAL
#endif

#ifdef __cplusplus
extern "C" {
#endif

enum {
    FEREG_SPI_RC_SYNTAX = 198,
    FEREG_SPI_RC_UNAVAILABLE = 498,
    FEREG_SPI_VALUE_UNAVAILABLE = -1
};

/*
 * These wrappers are internal to the one-file plugin and may only call SPI
 * functions after the official pginit() has initialized _stata_ and the first
 * valid stata_call() has bound the bridge to Stata's calling thread.
 * Functions returning an SPI return code use 498 for an unavailable session.
 * Scalar metadata functions use -1. Predicate fallbacks are deliberately
 * fail-closed: stop/missing return true, while selected-observation returns
 * false. fereg_spi_is_main_thread() is safe to call from any thread.
 */
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_display(const char *message);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_error(const char *message);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_poll(void);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_stop_requested(void);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_nobs(void);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_nvars(void);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_in1(void);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_in2(void);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_ifobs(ST_int observation);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_is_missing(ST_double value);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_var_is_string(ST_int variable);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_var_is_strl(ST_int variable);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_sdatalen(
    ST_int variable,
    ST_int observation
);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_vdata(
    ST_int variable,
    ST_int observation,
    ST_double *value
);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_vdata_with_missing(
    ST_int variable,
    ST_int observation,
    ST_double *value,
    ST_int *missing
);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_vdata_columns(
    const ST_int *variables,
    ST_int variable_count,
    ST_int first_observation,
    ST_int observation_count,
    ST_double **columns,
    ST_double *missing_value
);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_vstore(
    ST_int variable,
    ST_int observation,
    ST_double value
);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_sdata(
    ST_int variable,
    ST_int observation,
    char *buffer
);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_read_string(
    ST_int variable,
    ST_int observation,
    char *buffer,
    ST_int length
);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_sstore(
    ST_int variable,
    ST_int observation,
    const char *value
);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_matrix_rows(const char *name);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_matrix_cols(const char *name);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_mat_el(
    const char *name,
    ST_int row,
    ST_int column,
    ST_double *value
);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_mat_store(
    const char *name,
    ST_int row,
    ST_int column,
    ST_double value
);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_scal_use(
    const char *name,
    ST_double *value
);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_scal_save(
    const char *name,
    ST_double value
);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_macro_save(
    const char *name,
    const char *value
);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_macro_use(
    const char *name,
    char *buffer,
    ST_int length
);
FEREG_BRIDGE_INTERNAL ST_int fereg_spi_is_main_thread(void);

/* Implemented by Rust. Its exported function contains the panic boundary. */
FEREG_BRIDGE_INTERNAL ST_retcode fereg_dispatch(
    ST_int argc,
    const char *const *argv
);

/* Stata ABI entry point. pginit() is declared by the official stplugin.h. */
STDLL stata_call(int argc, char *argv[]);

#ifdef __cplusplus
}
#endif

#endif
