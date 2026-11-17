/* Invocation ownership uses real host threads and joins, never sleeps or
   assumptions about OS thread-ID reuse. No live Stata SPI is called here. */
#include "stata_bridge.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

#if SYSTEM == STWIN32
#include <windows.h>
typedef HANDLE test_thread;
#else
#include <pthread.h>
typedef pthread_t test_thread;
#endif

static int dispatches, displays, errors, reads;
static ST_int leaked_generation;
static const ST_int variables[] = {1};

static ST_int nobs(void) { return 2; }
static ST_int nvars(void) { return 1; }
static ST_boolean isstr(ST_int variable)
{
    assert(variable == 1);
    return (ST_boolean) 0;
}
static ST_int display(char *message)
{
    assert(message != NULL && fereg_spi_is_main_thread() == 1);
    ++displays;
    return 0;
}
static ST_int error(char *message)
{
    char *arguments[] = {"probe"};
    assert(message != NULL && strstr(message, "numeric input window") != NULL);
    assert(fereg_spi_is_main_thread() == 1 && fereg_spi_nobs() == 2);
    /* Cleanup diagnostics must still hold the invocation lock. */
    assert(stata_call(1, arguments) == FEREG_SPI_RC_UNAVAILABLE);
    ++errors;
    return 0;
}
static ST_int read_value(ST_int variable, ST_int observation, ST_double *value)
{
    assert(variable == 1 && observation == 1 && value != NULL);
    ++reads;
    *value = 37.0;
    return 0;
}

struct invocation {
    const char *action;
    ST_retcode expected;
    ST_int generation;
};

static void outside_owner_checks(void)
{
    assert(fereg_spi_is_main_thread() == 0);
    assert(fereg_spi_nobs() == FEREG_SPI_VALUE_UNAVAILABLE);
    assert(fereg_spi_display("outside owner") == FEREG_SPI_RC_UNAVAILABLE);
    assert(fereg_spi_error("outside owner") == FEREG_SPI_RC_UNAVAILABLE);
    assert(fereg_spi_stop_requested() == 1);
}

static void call_from_thread(struct invocation *call)
{
    char *arguments[] = {(char *) call->action};
    outside_owner_checks();
    assert(stata_call(1, arguments) == call->expected);
    outside_owner_checks();
    if (call->generation != 0) {
        double value = 0, *values[] = {&value};
        assert(fereg_spi_numeric_window_close(call->generation) == FEREG_SPI_RC_UNAVAILABLE);
        assert(fereg_spi_numeric_window_read(call->generation, 0, 1, 1, 1,
                   NULL, NULL, NULL, values, NULL) == 0);
        assert(value == 37.0);
    }
}

#if SYSTEM == STWIN32
static DWORD WINAPI thread_entry(LPVOID opaque)
{
    call_from_thread((struct invocation *) opaque);
    return 0;
}
static test_thread start_thread(struct invocation *call)
{
    HANDLE thread = CreateThread(NULL, 0, thread_entry, call, 0, NULL);
    assert(thread != NULL);
    return thread;
}
static void join_thread(test_thread thread)
{
    assert(WaitForSingleObject(thread, INFINITE) == WAIT_OBJECT_0);
    assert(CloseHandle(thread));
}
#else
static void *thread_entry(void *opaque)
{
    call_from_thread((struct invocation *) opaque);
    return NULL;
}
static test_thread start_thread(struct invocation *call)
{
    pthread_t thread;
    assert(pthread_create(&thread, NULL, thread_entry, call) == 0);
    return thread;
}
static void join_thread(test_thread thread)
{
    assert(pthread_join(thread, NULL) == 0);
}
#endif

ST_retcode fereg_dispatch(ST_int argc, const char *const *argv)
{
    ST_int generation;
    ST_double boundary;
    char *nested[] = {"probe"};
    assert(argc == 1 && argv != NULL);
    assert(fereg_spi_is_main_thread() == 1 && fereg_spi_nobs() == 2);
    assert(fereg_spi_display("active owner") == 0);
    ++dispatches;
    if (strcmp(argv[0], "reentry") == 0) {
        int before = dispatches;
        assert(stata_call(1, nested) == FEREG_SPI_RC_UNAVAILABLE);
        assert(dispatches == before && fereg_spi_nobs() == 2);
    } else if (strcmp(argv[0], "overlap") == 0) {
        struct invocation call = {"probe", FEREG_SPI_RC_UNAVAILABLE, 0};
        test_thread threads[16];
        int before = dispatches, before_displays = displays;
        for (int i = 0; i < 16; ++i) threads[i] = start_thread(&call);
        for (int i = 0; i < 16; ++i) join_thread(threads[i]);
        assert(dispatches == before && displays == before_displays);
        assert(fereg_spi_nobs() == 2);
    } else if (strcmp(argv[0], "window") == 0) {
        struct invocation call = {"probe", FEREG_SPI_RC_UNAVAILABLE, 0};
        assert(fereg_spi_numeric_window_open(variables, 1, 2, &boundary, &generation) == 0);
        assert(stata_call(1, nested) == FEREG_SPI_RC_UNAVAILABLE);
        assert(fereg_spi_nobs() == FEREG_SPI_VALUE_UNAVAILABLE);
        call.generation = generation;
        join_thread(start_thread(&call));
        assert(fereg_spi_numeric_window_close(generation) == 0);
        assert(fereg_spi_nobs() == 2);
    } else if (strncmp(argv[0], "leak", 4) == 0) {
        assert(fereg_spi_numeric_window_open(variables, 1, 2, &boundary,
                   &leaked_generation) == 0);
    }
    if (strstr(argv[0], "error") != NULL) return 198;
    if (strstr(argv[0], "break") != NULL) return 1;
    /* Rust catches actual panics before C sees this return code. */
    if (strstr(argv[0], "panic") != NULL) return 3499;
    return 0;
}

int main(void)
{
    ST_plugin plugin;
    const struct invocation cases[] = {
        {"probe", 0, 0}, {"reentry", 0, 0}, {"overlap", 0, 0},
        {"window", 0, 0}, {"error", 198, 0}, {"break", 1, 0},
        {"panic", 3499, 0}, {"leak", 498, 0}, {"leak_error", 198, 0},
        {"leak_break", 1, 0}, {"leak_panic", 3499, 0}
    };
    struct invocation probe = {"probe", 0, 0};
    char *arguments[] = {"probe"};
    double value, *values[] = {&value};
    memset(&plugin, 0, sizeof(plugin));
    plugin.nobs = nobs; plugin.nvars = nvars; plugin.isstr = isstr;
    plugin.spoutsml = display; plugin.spouterr = error; plugin.vdata = read_value;
    plugin.missval = 0x1p+1023;
    assert(pginit(&plugin) == SD_PLUGINVER);
    outside_owner_checks();
    /* The first call belongs to another live host thread. */
    join_thread(start_thread(&probe));
    for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); ++i) {
        struct invocation call = cases[i];
        int before = dispatches;
        assert(stata_call(-1, NULL) == 198 && stata_call(1, NULL) == 198);
        assert(dispatches == before);
        join_thread(start_thread(&call));
        outside_owner_checks();
        assert(fereg_spi_numeric_window_read(leaked_generation, 0, 1, 1, 1,
                   NULL, NULL, NULL, values, NULL) == 498);
        /* Every success/error/Break/panic/leak permits an immediate different
           thread and then another worker to invoke the plugin successfully. */
        assert(stata_call(1, arguments) == 0);
        outside_owner_checks();
        join_thread(start_thread(&probe));
    }
    assert(errors == 4 && reads == 1);
    puts("INVOCATION_GUARD_PASS");
    return 0;
}
