#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "stata_bridge.h"

#if SYSTEM==STWIN32
#include <windows.h>
#else
#include <pthread.h>
#endif

static char display_buffer[4096];
static char error_buffer[4096];
static char string_slot[128];
static char macro_slot[128];
static ST_int stop_flag = 0;
static ST_double scalar_slot = 0.0;
static ST_double matrix_slot = 0.0;
static ST_double data_slot = 0.0;

static ST_int append_display(char *message)
{
    size_t remaining = sizeof(display_buffer) - strlen(display_buffer) - 1U;
    strncat(display_buffer, message, remaining);
    return 0;
}

static ST_int append_error(char *message)
{
    size_t remaining = sizeof(error_buffer) - strlen(error_buffer) - 1U;
    strncat(error_buffer, message, remaining);
    return 0;
}

static ST_int mock_poll(void) { return 17; }
static ST_int mock_nobs(void) { return 42; }
static ST_int mock_nvars(void) { return 7; }
static ST_int mock_in1(void) { return 5; }
static ST_int mock_in2(void) { return 40; }
static ST_boolean mock_selobs(ST_int observation) { return (ST_boolean) (observation % 2 == 0); }
static ST_boolean mock_isstr(ST_int variable) { return (ST_boolean) (variable == 4 || variable == 5); }
static ST_boolean mock_isstrl(ST_int variable) { return (ST_boolean) (variable == 5); }
static ST_boolean mock_isbinary(ST_int variable, ST_int observation)
{
    return (ST_boolean) (variable == 5 && observation == 99);
}
static ST_int mock_sdatalen(ST_int variable, ST_int observation)
{
    if ((variable != 4 && variable != 5) || observation != 12) return -111;
    return (ST_int) strlen(string_slot);
}
static ST_int mock_strldata(ST_int variable, ST_int observation, char *buffer, ST_int length)
{
    size_t needed;
    if (variable != 5 || observation != 12 || buffer == NULL || length <= 0) return 111;
    needed = strlen(string_slot) + 1U;
    if ((size_t) length < needed) return 920;
    memcpy(buffer, string_slot, needed);
    return 0;
}
static ST_int mock_rowsof(char *name) { return strcmp(name, "mock_matrix") == 0 ? 4 : -1; }
static ST_int mock_colsof(char *name) { return strcmp(name, "mock_matrix") == 0 ? 5 : -1; }
static ST_boolean mock_missing(ST_double value)
{
    if (isnan(value)) return (ST_boolean) 1U;
    return (ST_boolean) 0U;
}

static ST_int mock_vdata(ST_int variable, ST_int observation, ST_double *value)
{
    if (variable != 3 || observation != 11 || value == NULL) return 111;
    *value = data_slot;
    return 0;
}

static ST_int mock_vstore(ST_int variable, ST_int observation, ST_double value)
{
    if (variable != 3 || observation != 11) return 111;
    data_slot = value;
    return 0;
}

static ST_int mock_sdata(ST_int variable, ST_int observation, char *buffer)
{
    if (variable != 4 || observation != 12 || buffer == NULL) return 111;
    strcpy(buffer, string_slot);
    return 0;
}

static ST_int mock_sstore(ST_int variable, ST_int observation, char *value)
{
    if (variable != 4 || observation != 12 || value == NULL) return 111;
    if (strlen(value) >= sizeof(string_slot)) return 920;
    strcpy(string_slot, value);
    return 0;
}

static ST_int mock_mat_el(char *name, ST_int row, ST_int column, ST_double *value)
{
    if (strcmp(name, "mock_matrix") != 0 || row != 2 || column != 3 || value == NULL) {
        return 111;
    }
    *value = matrix_slot;
    return 0;
}

static ST_int mock_mat_store(char *name, ST_int row, ST_int column, ST_double value)
{
    if (strcmp(name, "mock_matrix") != 0 || row != 2 || column != 3) return 111;
    matrix_slot = value;
    return 0;
}

static ST_int mock_scalar_use(char *name, ST_double *value)
{
    if (strcmp(name, "mock_scalar") != 0 || value == NULL) return 111;
    *value = scalar_slot;
    return 0;
}

static ST_int mock_scalar_save(char *name, ST_double *value)
{
    if (strcmp(name, "mock_scalar") != 0 || value == NULL) return 112;
    scalar_slot = *value;
    return 0;
}

static ST_int mock_macro_save(char *name, char *value)
{
    if (strcmp(name, "_mock_macro") != 0 || value == NULL) return 111;
    if (strlen(value) >= sizeof(macro_slot)) return 920;
    strcpy(macro_slot, value);
    return 0;
}

static ST_int mock_macro_use(char *name, char *buffer, ST_int length)
{
    size_t needed;
    if (strcmp(name, "_mock_macro") != 0 || buffer == NULL || length < 0) return 111;
    needed = strlen(macro_slot) + 1U;
    if ((size_t) length < needed) return 920;
    memcpy(buffer, macro_slot, needed);
    return 0;
}

ST_retcode stfixest_dispatch(ST_int argc, const char *const *argv)
{
    ST_int index;
    stfixest_spi_display("dispatch:");
    for (index = 0; index < argc; ++index) {
        stfixest_spi_display(argv[index]);
        stfixest_spi_display(index + 1 == argc ? "\n" : ",");
    }
    return 0;
}

struct worker_results {
    ST_int is_main_thread;
    ST_int display_rc;
    ST_int nobs;
    ST_int ifobs;
    ST_int is_missing;
    ST_int stop_requested;
    ST_int vdata_rc;
    ST_retcode stata_call_rc;
};

static void run_worker_checks(struct worker_results *results)
{
    ST_double value = 0.0;
    char *arguments[] = {"version"};
    results->is_main_thread = stfixest_spi_is_main_thread();
    results->display_rc = stfixest_spi_display("worker must not reach SPI\n");
    results->nobs = stfixest_spi_nobs();
    results->ifobs = stfixest_spi_ifobs(12);
    results->is_missing = stfixest_spi_is_missing(1.0);
    results->stop_requested = stfixest_spi_stop_requested();
    results->vdata_rc = stfixest_spi_vdata(3, 11, &value);
    results->stata_call_rc = stata_call(1, arguments);
}

#if SYSTEM==STWIN32
static DWORD WINAPI worker_entry(LPVOID opaque)
{
    run_worker_checks((struct worker_results *) opaque);
    return 0;
}
#else
static void *worker_entry(void *opaque)
{
    run_worker_checks((struct worker_results *) opaque);
    return NULL;
}
#endif

static int check_worker_thread_refusal(void)
{
    struct worker_results results;
    size_t display_length = strlen(display_buffer);
    size_t error_length = strlen(error_buffer);
    memset(&results, 0, sizeof(results));
#if SYSTEM==STWIN32
    {
        HANDLE worker = CreateThread(NULL, 0, worker_entry, &results, 0, NULL);
        DWORD wait_result;
        if (worker == NULL) {
            fprintf(stderr, "CreateThread failed\n");
            return 1;
        }
        wait_result = WaitForSingleObject(worker, INFINITE);
        if (CloseHandle(worker) == 0 || wait_result != WAIT_OBJECT_0) {
            fprintf(stderr, "Windows worker join failed\n");
            return 2;
        }
    }
#else
    {
        pthread_t worker;
        int rc = pthread_create(&worker, NULL, worker_entry, &results);
        if (rc != 0) {
            fprintf(stderr, "pthread_create failed: %d\n", rc);
            return 1;
        }
        rc = pthread_join(worker, NULL);
        if (rc != 0) {
            fprintf(stderr, "pthread_join failed: %d\n", rc);
            return 2;
        }
    }
#endif
    if (results.is_main_thread != 0 ||
        results.display_rc != STFIXEST_SPI_RC_UNAVAILABLE ||
        results.nobs != STFIXEST_SPI_VALUE_UNAVAILABLE ||
        results.ifobs != 0 || results.is_missing != 1 ||
        results.stop_requested != 1 ||
        results.vdata_rc != STFIXEST_SPI_RC_UNAVAILABLE ||
        results.stata_call_rc != STFIXEST_SPI_RC_UNAVAILABLE) {
        fprintf(
            stderr,
            "worker wrapper refusal failed: %d %d %d %d %d %d %d %d\n",
            results.is_main_thread,
            results.display_rc,
            results.nobs,
            results.ifobs,
            results.is_missing,
            results.stop_requested,
            results.vdata_rc,
            results.stata_call_rc
        );
        return 3;
    }
    if (strlen(display_buffer) != display_length ||
        strlen(error_buffer) != error_length) {
        fprintf(stderr, "worker reached an SPI callback\n");
        return 4;
    }
    return 0;
}

static int check_numeric_round_trips(void)
{
    ST_double recovered = 0.0;
    if (stfixest_spi_vstore(3, 11, 8.5) != 0 ||
        stfixest_spi_vdata(3, 11, &recovered) != 0 || recovered != 8.5) {
        fprintf(stderr, "data round trip failed: %.17g\n", recovered);
        return 1;
    }
    if (stfixest_spi_mat_store("mock_matrix", 2, 3, -4.75) != 0 ||
        stfixest_spi_mat_el("mock_matrix", 2, 3, &recovered) != 0 || recovered != -4.75) {
        fprintf(stderr, "matrix round trip failed: %.17g\n", recovered);
        return 2;
    }
    if (stfixest_spi_scal_save("mock_scalar", 3.25) != 0 ||
        stfixest_spi_scal_use("mock_scalar", &recovered) != 0 || recovered != 3.25) {
        fprintf(stderr, "scalar round trip failed: %.17g\n", recovered);
        return 3;
    }
    return 0;
}

static int check_string_round_trips(void)
{
    char buffer[128];
    memset(buffer, 0, sizeof(buffer));
    if (stfixest_spi_sstore(4, 12, "firm alpha") != 0 ||
        stfixest_spi_sdata(4, 12, buffer) != 0 || strcmp(buffer, "firm alpha") != 0) {
        fprintf(stderr, "string round trip failed: %s\n", buffer);
        return 1;
    }
    memset(buffer, 0, sizeof(buffer));
    if (stfixest_spi_macro_save("_mock_macro", "protocol=1.0") != 0 ||
        stfixest_spi_macro_use("_mock_macro", buffer, (ST_int) sizeof(buffer)) != 0 ||
        strcmp(buffer, "protocol=1.0") != 0) {
        fprintf(stderr, "macro round trip failed: %s\n", buffer);
        return 2;
    }
    if (stfixest_spi_sdatalen(4, 12) != (ST_int) strlen(string_slot)) {
        fprintf(stderr, "string length wrapper failed\n");
        return 3;
    }
    memset(buffer, 0, sizeof(buffer));
    if (stfixest_spi_read_string(4, 12, buffer, (ST_int) sizeof(buffer)) != 0 ||
        strcmp(buffer, "firm alpha") != 0) {
        fprintf(stderr, "bounded fixed-string read failed: %s\n", buffer);
        return 4;
    }
    memset(buffer, 0, sizeof(buffer));
    if (stfixest_spi_read_string(5, 12, buffer, (ST_int) sizeof(buffer)) != 0 ||
        strcmp(buffer, "firm alpha") != 0) {
        fprintf(stderr, "bounded strL read failed: %s\n", buffer);
        return 5;
    }
    return 0;
}

int main(void)
{
    ST_plugin plugin;
    char *arguments[] = {"version", "threads=32"};
    int rc;
    memset(&plugin, 0, sizeof(plugin));
    plugin.spoutsml = append_display;
    plugin.spouterr = append_error;
    plugin.pollstd = mock_poll;
    plugin.nobs = mock_nobs;
    plugin.nvars = mock_nvars;
    plugin.nvar = mock_nvars;
    plugin.nobs1 = mock_in1;
    plugin.nobs2 = mock_in2;
    plugin.selobs = mock_selobs;
    plugin.isstr = mock_isstr;
    plugin.isstrl = mock_isstrl;
    plugin.isbinary = mock_isbinary;
    plugin.sdatalen = mock_sdatalen;
    plugin.strldata = mock_strldata;
    plugin.rowsof = mock_rowsof;
    plugin.colsof = mock_colsof;
    plugin.ismissing = mock_missing;
    plugin.safevdata = mock_vdata;
    plugin.safestore = mock_vstore;
    plugin.sdata = mock_sdata;
    plugin.sstore = mock_sstore;
    plugin.safematel = mock_mat_el;
    plugin.safematstore = mock_mat_store;
    plugin.scalaruse = mock_scalar_use;
    plugin.scalarsave = mock_scalar_save;
    plugin.macresave = mock_macro_save;
    plugin.macuse = mock_macro_use;
    plugin.stopflag = &stop_flag;

    if (stfixest_spi_is_main_thread() != 0 ||
        stfixest_spi_display("uninitialized\n") != STFIXEST_SPI_RC_UNAVAILABLE ||
        stfixest_spi_nobs() != STFIXEST_SPI_VALUE_UNAVAILABLE ||
        stata_call(0, NULL) != STFIXEST_SPI_RC_UNAVAILABLE) {
        fprintf(stderr, "uninitialized bridge did not fail closed\n");
        return 1;
    }
    if (pginit(&plugin) != SD_PLUGINVER) {
        fprintf(stderr, "pginit returned the wrong SPI version\n");
        return 2;
    }
    if (_stata_ != &plugin || stfixest_spi_is_main_thread() != 0 ||
        stfixest_spi_display("not bound\n") != STFIXEST_SPI_RC_UNAVAILABLE ||
        stfixest_spi_nobs() != STFIXEST_SPI_VALUE_UNAVAILABLE) {
        fprintf(stderr, "pginit did not initialize the SPI session\n");
        return 3;
    }
    if (stata_call(-1, NULL) != STFIXEST_SPI_RC_SYNTAX ||
        stfixest_spi_is_main_thread() != 0) {
        fprintf(stderr, "invalid stata_call bound the bridge thread\n");
        return 4;
    }
    if (stata_call(2, arguments) != 0 || stfixest_spi_is_main_thread() != 1) {
        fprintf(stderr, "first valid stata_call did not bind and dispatch\n");
        return 5;
    }
    rc = check_worker_thread_refusal();
    if (rc != 0) {
        return 6 + rc;
    }
    if (stfixest_spi_nobs() != 42 || stfixest_spi_nvars() != 7 ||
        stfixest_spi_in1() != 5 || stfixest_spi_in2() != 40 ||
        stfixest_spi_ifobs(12) != 1 || stfixest_spi_ifobs(13) != 0) {
        fprintf(stderr, "dimension wrappers failed\n");
        return 10;
    }
    if (stfixest_spi_poll() != 17 || stfixest_spi_stop_requested() != 0) {
        fprintf(stderr, "poll/stop wrappers failed\n");
        return 11;
    }
    stop_flag = 1;
    if (stfixest_spi_stop_requested() != 1) {
        fprintf(stderr, "stop flag transition failed\n");
        return 12;
    }
    stop_flag = 0;
    if (!stfixest_spi_is_missing(NAN) || stfixest_spi_is_missing(1.0)) {
        fprintf(stderr, "missing-value wrapper failed\n");
        return 13;
    }
    if (stfixest_spi_matrix_rows("mock_matrix") != 4 ||
        stfixest_spi_matrix_cols("mock_matrix") != 5 ||
        stfixest_spi_var_is_string(4) != 1 ||
        stfixest_spi_var_is_string(3) != 0 ||
        stfixest_spi_var_is_strl(5) != 1) {
        fprintf(stderr, "metadata wrappers failed\n");
        return 14;
    }
    rc = check_numeric_round_trips();
    if (rc != 0) return 10 + rc;
    rc = check_string_round_trips();
    if (rc != 0) return 20 + rc;
    if (stfixest_spi_display("hello\n") != 0 || stfixest_spi_error("warning\n") != 0) {
        fprintf(stderr, "display/error wrappers failed\n");
        return 30;
    }
    if (strcmp(display_buffer, "dispatch:version,threads=32\nhello\n") != 0) {
        fprintf(stderr, "unexpected display buffer: %s\n", display_buffer);
        return 32;
    }
    if (strcmp(error_buffer, "warning\n") != 0) {
        fprintf(stderr, "unexpected error buffer: %s\n", error_buffer);
        return 33;
    }
    if (stfixest_spi_vdata(3, 11, NULL) != STFIXEST_SPI_RC_SYNTAX ||
        stfixest_spi_mat_el(NULL, 1, 1, &scalar_slot) != STFIXEST_SPI_RC_SYNTAX ||
        stfixest_spi_macro_use("_mock_macro", NULL, 1) != STFIXEST_SPI_RC_SYNTAX ||
        stfixest_spi_macro_use("_mock_macro", macro_slot, 0) !=
            STFIXEST_SPI_RC_SYNTAX) {
        fprintf(stderr, "null-pointer guard failed\n");
        return 34;
    }
    puts("mock-spi bridge test: PASS");
    return 0;
}
