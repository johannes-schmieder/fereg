/* Exercise the guarded numeric batch with a callback that must never receive
   an invalid address; no invalid access is attempted against live Stata. */
#include "stata_bridge.h"
#include <assert.h>
#include <limits.h>
#if SYSTEM == STWIN32
#include <windows.h>
#else
#include <pthread.h>
#endif
#include <stdio.h>
#include <string.h>

static int calls, failure_at;
static int store_calls, store_failure_at;
static ST_double stored[12];
static ST_int store_value(ST_int v, ST_int o, ST_double value)
{
    const ST_int order[] = {4, 1, 2};
    assert(store_calls < 12 && v == order[store_calls % 3] && o == store_calls / 3 + 1);
    ++store_calls;
    if (store_calls == store_failure_at) return 918;
    stored[store_calls - 1] = value;
    return 0;
}
static ST_int nobs(void) { return 65540; }
static ST_int nvars(void) { return 4; }
static ST_int range_first = 17, range_end = 290;
static ST_int in1(void) { return range_first; }
static ST_int in2(void) { return range_end; }
static ST_boolean isstr(ST_int v) { assert(v >= 1 && v <= 4); return (ST_boolean)(v == 3); }
static ST_int read_value(ST_int v, ST_int o, ST_double *d)
{
    assert(v >= 1 && v <= 4 && v != 3 && o >= 1 && o <= 65540 && d != NULL);
    ++calls;
    if (failure_at == calls) return 919;
    *d = (double)(v * 100000 + o);
    return 0;
}

static ST_int checked_read(ST_int v, ST_int o, ST_double *d)
{
    (void)v; (void)o; (void)d;
    assert(0 && "Numeric batches must select the guarded direct accessor");
    return 498;
}

ST_int fereg_dispatch(ST_int argc, const char *const *argv)
{
    (void)argc; (void)argv; return 0;
}

static void worker_checks(void)
{
    int vars[] = {1};
    double data[1], *cols[] = {data}, missing;
    assert(fereg_spi_vdata_columns(vars, 1, 1, 1, cols, &missing) == 498);
    assert(fereg_spi_vdata_rows(vars, 1, 1, 1, data, &missing) == 498);
    const double *outputs[] = {data};
    assert(fereg_spi_vstore_columns(vars, 1, 1, outputs) == 498);
    int first, end;
    assert(fereg_spi_input_range(&first, &end) == 498);
}

#if SYSTEM == STWIN32
static DWORD WINAPI worker(LPVOID unused)
{
    (void)unused;
    worker_checks();
    return 0;
}
#else
static void *worker(void *unused)
{
    (void)unused;
    worker_checks();
    return NULL;
}
#endif

/* Numeric window: values carry a missing code on every fifth observation. */
static ST_double window_missval;
static int window_errors;
static ST_int window_error(char *message)
{
    assert(message != NULL && strstr(message, "numeric input window") != NULL);
    ++window_errors;
    return 0;
}
static ST_int window_value(ST_int v, ST_int o, ST_double *d)
{
    assert(v >= 1 && v <= 4 && v != 3 && o >= 1 && o <= 65540 && d != NULL);
    ++calls;
    if (failure_at == calls) return 919;
    *d = o % 5 == 0 ? window_missval * (1.0 + (double)v / 64.0) : (double)(v * 100000 + o);
    return 0;
}

struct window_worker {
    ST_int generation;
    ST_int read_rc, open_rc, close_rc, poll_rc, nobs, display_rc, main;
    double values[2][8];
    unsigned char missing[2][8];
};

static void window_worker_checks(struct window_worker *w)
{
    double *values[] = {w->values[0], w->values[1]};
    unsigned char *missing[] = {w->missing[0], w->missing[1]};
    int vars[] = {1};
    double fill;
    ST_int generation;
    w->read_rc = fereg_spi_numeric_window_read(w->generation, 0, 2, 3, 8, NULL, NULL, NULL,
                                               values, missing);
    w->open_rc = fereg_spi_numeric_window_open(vars, 1, 65540, &fill, &generation);
    w->close_rc = fereg_spi_numeric_window_close(w->generation);
    w->poll_rc = fereg_spi_poll();
    w->nobs = fereg_spi_nobs();
    w->display_rc = fereg_spi_display("worker");
    w->main = fereg_spi_is_main_thread();
}

#if SYSTEM == STWIN32
static DWORD WINAPI window_worker_entry(LPVOID opaque)
{
    window_worker_checks((struct window_worker *)opaque);
    return 0;
}
#else
static void *window_worker_entry(void *opaque)
{
    window_worker_checks((struct window_worker *)opaque);
    return NULL;
}
#endif

static void run_window_worker(struct window_worker *w)
{
#if SYSTEM == STWIN32
    HANDLE thread = CreateThread(NULL, 0, window_worker_entry, w, 0, NULL);
    assert(thread != NULL);
    assert(WaitForSingleObject(thread, INFINITE) == WAIT_OBJECT_0);
    assert(CloseHandle(thread));
#else
    pthread_t thread;
    assert(pthread_create(&thread, NULL, window_worker_entry, w) == 0);
    assert(pthread_join(thread, NULL) == 0);
#endif
}

static void window_checks(ST_plugin *plugin)
{
    int vars[] = {4, 1, 2};
    double boundary = 0, a[6], b[6], c[6], *cols[] = {a, b, c};
    unsigned char ma[6], mb[6], mc[6], *flags[] = {ma, mb, mc};
    const unsigned char keep[] = {1, 0, 1, 1, 0, 1};
    const double fills[] = {-1.0, -2.0, -3.0};
    const unsigned char fill_flags[] = {0, 1, 0};
    ST_int generation = 0, stale;
    struct window_worker w;
    window_missval = plugin->missval;
    plugin->vdata = window_value;
    calls = 0; failure_at = 0;
    /* Open validates on the main thread, exactly as the serial batch does. */
    assert(fereg_spi_numeric_window_open(NULL, 3, 65540, &boundary, &generation) == 198);
    assert(fereg_spi_numeric_window_open(vars, 0, 65540, &boundary, &generation) == 198);
    assert(fereg_spi_numeric_window_open(vars, 3, 0, &boundary, &generation) == 198);
    assert(fereg_spi_numeric_window_open(vars, 3, 65540, NULL, &generation) == 198);
    assert(fereg_spi_numeric_window_open(vars, 3, 65540, &boundary, NULL) == 198);
    assert(fereg_spi_numeric_window_open(vars, 3, 65539, &boundary, &generation) == 498);
    for (int bad = -1; bad <= 5; ++bad) {
        if (bad == 1 || bad == 2 || bad == 4) continue;
        vars[2] = bad;
        assert(fereg_spi_numeric_window_open(vars, 3, 65540, &boundary, &generation) == 498);
    }
    vars[2] = 2;
    plugin->vdata = NULL;
    assert(fereg_spi_numeric_window_open(vars, 3, 65540, &boundary, &generation) == 498);
    plugin->vdata = window_value;
    /* A closed window refuses reads and closes from every thread. */
    assert(fereg_spi_numeric_window_read(generation, 0, 3, 1, 6, NULL, NULL, NULL, cols, flags) == 498);
    assert(fereg_spi_numeric_window_close(generation) == 498);
    assert(calls == 0);
    assert(fereg_spi_numeric_window_open(vars, 3, 65540, &boundary, &generation) == 0);
    assert(boundary == plugin->missval && generation > 0);
    /* While open, the main thread may make no other SPI call. */
    assert(fereg_spi_nobs() == FEREG_SPI_VALUE_UNAVAILABLE);
    assert(fereg_spi_poll() == 498 && fereg_spi_stop_requested() == 1);
    assert(fereg_spi_display("open") == 498);
    assert(fereg_spi_vdata_columns(vars, 3, 1, 1, cols, &boundary) == 498);
    assert(fereg_spi_numeric_window_open(vars, 3, 65540, &boundary, &stale) == 498);
    assert(fereg_spi_is_main_thread() == 1);
    /* Shape and range checks; nothing is read. */
    assert(fereg_spi_numeric_window_read(generation + 1, 0, 3, 1, 6, NULL, NULL, NULL, cols, flags) == 498);
    assert(fereg_spi_numeric_window_read(generation, 0, 0, 1, 6, NULL, NULL, NULL, cols, flags) == 198);
    assert(fereg_spi_numeric_window_read(generation, 0, 65, 1, 6, NULL, NULL, NULL, cols, flags) == 198);
    assert(fereg_spi_numeric_window_read(generation, -1, 3, 1, 6, NULL, NULL, NULL, cols, flags) == 198);
    assert(fereg_spi_numeric_window_read(generation, 1, 3, 1, 6, NULL, NULL, NULL, cols, flags) == 198);
    assert(fereg_spi_numeric_window_read(generation, 0, 3, 0, 6, NULL, NULL, NULL, cols, flags) == 198);
    assert(fereg_spi_numeric_window_read(generation, 0, 3, 65536, 6, NULL, NULL, NULL, cols, flags) == 198);
    assert(fereg_spi_numeric_window_read(generation, 0, 3, INT_MAX, INT_MAX, NULL, NULL, NULL, cols, flags) == 198);
    assert(fereg_spi_numeric_window_read(generation, 0, 3, 1, 6, NULL, NULL, NULL, NULL, flags) == 198);
    assert(fereg_spi_numeric_window_read(generation, 0, 3, 1, 6, keep, NULL, fill_flags, cols, flags) == 198);
    assert(fereg_spi_numeric_window_read(generation, 0, 3, 1, 6, keep, fills, NULL, cols, flags) == 198);
    cols[1] = NULL;
    assert(fereg_spi_numeric_window_read(generation, 0, 3, 1, 6, NULL, NULL, NULL, cols, flags) == 198);
    cols[1] = b; flags[2] = NULL;
    assert(fereg_spi_numeric_window_read(generation, 0, 3, 1, 6, NULL, NULL, NULL, cols, flags) == 198);
    flags[2] = mc;
    assert(calls == 0);
    /* All rows: SDK doubles and `value >= missval` flags in row/slot order. */
    assert(fereg_spi_numeric_window_read(generation, 0, 3, 65535, 6, NULL, NULL, NULL, cols, flags) == 0);
    assert(calls == 18);
    for (int o = 0; o < 6; ++o) {
        const int observation = 65535 + o;
        for (int s = 0; s < 3; ++s) {
            double expected;
            assert(window_value(vars[s], observation, &expected) == 0);
            assert(memcmp(&cols[s][o], &expected, sizeof(double)) == 0);
            assert(flags[s][o] == (observation % 5 == 0));
        }
    }
    /* Retained mask: excluded rows receive the slot fills and are never read. */
    calls = 0;
    assert(fereg_spi_numeric_window_read(generation, 0, 3, 11, 6, keep, fills, fill_flags, cols, flags) == 0);
    assert(calls == 12);
    for (int o = 0; o < 6; ++o)
        for (int s = 0; s < 3; ++s)
            if (!keep[o]) assert(cols[s][o] == fills[s] && flags[s][o] == fill_flags[s]);
            else assert(cols[s][o] == (double)(vars[s] * 100000 + 11 + o) && flags[s][o] == 0);
    /* Slot offsets, flagless reads and the first failing SDK call in order. */
    calls = 0;
    assert(fereg_spi_numeric_window_read(generation, 2, 1, 1, 6, NULL, NULL, NULL, cols, NULL) == 0);
    assert(calls == 6 && a[0] == 200001 && a[5] == 200006);
    for (int failure = 1; failure <= 12; ++failure) {
        calls = 0; failure_at = failure;
        assert(fereg_spi_numeric_window_read(generation, 0, 3, 11, 6, keep, fills, fill_flags, cols, flags) == 919);
        assert(calls == failure);
    }
    failure_at = 0;
    /* Another thread may read inside the open window, and only read. */
    memset(&w, 0, sizeof(w));
    w.generation = generation;
    calls = 0;
    run_window_worker(&w);
    assert(w.read_rc == 0 && calls == 16);
    assert(w.values[0][0] == 400003 && w.values[1][6] == 100009 && w.missing[1][2] == 1 && w.missing[1][6] == 0);
    assert(w.open_rc == 498 && w.close_rc == 498 && w.poll_rc == 498);
    assert(w.nobs == FEREG_SPI_VALUE_UNAVAILABLE && w.display_rc == 498 && w.main == 0);
    stale = generation;
    assert(fereg_spi_numeric_window_close(generation + 1) == 498);
    assert(fereg_spi_numeric_window_close(generation) == 0);
    assert(fereg_spi_numeric_window_close(generation) == 498);
    /* After close: the stale generation and every worker read are refused. */
    memset(&w, 0, sizeof(w));
    w.generation = stale;
    calls = 0;
    run_window_worker(&w);
    assert(w.read_rc == 498 && calls == 0 && w.main == 0);
    assert(fereg_spi_numeric_window_read(stale, 0, 3, 1, 6, NULL, NULL, NULL, cols, flags) == 498);
    assert(fereg_spi_nobs() == 65540 && stata_call(0, NULL) == 0);
    assert(fereg_spi_numeric_window_open(vars, 3, 65540, &boundary, &generation) == 0);
    assert(generation != stale);
    assert(fereg_spi_numeric_window_close(generation) == 0);
    /* A leaked window cannot brick the plugin: the next entry closes it, fails
       visibly once, and the following call proceeds. */
    assert(fereg_spi_numeric_window_open(vars, 3, 65540, &boundary, &generation) == 0);
    window_errors = 0;
    assert(stata_call(0, NULL) == 498 && window_errors == 1);
    assert(fereg_spi_numeric_window_read(generation, 0, 3, 1, 6, NULL, NULL, NULL, cols, flags) == 498);
    assert(fereg_spi_numeric_window_close(generation) == 498);
    assert(stata_call(0, NULL) == 0 && window_errors == 1);
    assert(fereg_spi_nobs() == 65540);
    plugin->vdata = read_value;
    calls = 0;
    puts("NUMERIC_WINDOW_GUARD_PASS");
}

int main(void)
{
    ST_plugin plugin;
    int vars[] = {4, 1, 2};
    double a[4] = {0}, b[4] = {0}, c[4] = {0}, *cols[] = {a, b, c}, missing;
    memset(&plugin, 0, sizeof(plugin));
    plugin.nobs = nobs; plugin.nvars = nvars; plugin.isstr = isstr;
    plugin.nobs1 = in1; plugin.nobs2 = in2;
    plugin.safevdata = checked_read; plugin.vdata = read_value;
    plugin.safestore = store_value; plugin.store = store_value;
    plugin.missval = 0x1p+1023;
    plugin.spouterr = window_error;
    assert(fereg_spi_vdata_columns(vars, 3, 1, 4, cols, &missing) == 498);
    assert(pginit(&plugin) == SD_PLUGINVER);
    assert(fereg_spi_vdata_columns(vars, 3, 1, 4, cols, &missing) == 498);
    assert(stata_call(0, NULL) == 0);
    int observed_first, observed_end;
    assert(fereg_spi_input_range(&observed_first, &observed_end) == 0 && observed_first == 16 && observed_end == 290);
    range_first = 0;
    assert(fereg_spi_input_range(&observed_first, &observed_end) == 198);
    range_first = 291;
    assert(fereg_spi_input_range(&observed_first, &observed_end) == 198);
    range_first = 17; range_end = 65541;
    assert(fereg_spi_input_range(&observed_first, &observed_end) == 198);
    range_end = 290;
#if SYSTEM == STWIN32
    HANDLE thread = CreateThread(NULL, 0, worker, NULL, 0, NULL);
    assert(thread != NULL);
    assert(WaitForSingleObject(thread, INFINITE) == WAIT_OBJECT_0);
    assert(CloseHandle(thread));
#else
    pthread_t thread;
    assert(pthread_create(&thread, NULL, worker, NULL) == 0);
    assert(pthread_join(thread, NULL) == 0);
#endif
    assert(calls == 0);
    assert(fereg_spi_vdata_columns(vars, 3, 65537, 4, cols, &missing) == 0);
    assert(calls == 12 && a[0] == 465537 && b[3] == 165540 && c[3] == 265540);
    assert(missing == plugin.missval);
    calls = 0;
    assert(fereg_spi_vdata_columns(NULL, 3, 1, 1, cols, &missing) == 198);
    assert(fereg_spi_vdata_columns(vars, 0, 1, 1, cols, &missing) == 198);
    assert(fereg_spi_vdata_columns(vars, 3, 0, 1, cols, &missing) == 198);
    assert(fereg_spi_vdata_columns(vars, 3, 1, -1, cols, &missing) == 198);
    assert(fereg_spi_vdata_columns(vars, 3, 1, 1, NULL, &missing) == 198);
    assert(fereg_spi_vdata_columns(vars, 3, 1, 1, cols, NULL) == 198);
    cols[2] = NULL;
    assert(fereg_spi_vdata_columns(vars, 3, 1, 1, cols, &missing) == 198);
    cols[2] = c;
    assert(fereg_spi_vdata_columns(vars, 3, 65541, 1, cols, &missing) == 498);
    assert(fereg_spi_vdata_columns(vars, 3, INT_MAX, INT_MAX, cols, &missing) == 498);
    assert(fereg_spi_vdata_columns(vars, 3, 65540, 2, cols, &missing) == 498);
    for (int bad = -1; bad <= 5; ++bad) {
        if (bad == 1 || bad == 2 || bad == 4) continue;
        vars[2] = bad;
        assert(fereg_spi_vdata_columns(vars, 3, 1, 1, cols, &missing) == 498);
    }
    vars[2] = 2;
    plugin.vdata = NULL;
    assert(fereg_spi_vdata_columns(vars, 3, 1, 1, cols, &missing) == 498);
    plugin.vdata = read_value;
    assert(calls == 0);
    failure_at = 5;
    assert(fereg_spi_vdata_columns(vars, 3, 1, 4, cols, &missing) == 919);
    assert(calls == 5 && c[0] == 200001 && a[1] == 400002);
    calls = 0;
    failure_at = 0;
    double rows[13] = {0};
    rows[12] = -77;
    assert(fereg_spi_vdata_rows(vars, 3, 65537, 4, rows, &missing) == 0);
    assert(calls == 12 && rows[0] == 465537 && rows[11] == 265540 && rows[12] == -77);
    calls = 0;
    assert(fereg_spi_vdata_rows(NULL, 3, 1, 1, rows, &missing) == 198);
    assert(fereg_spi_vdata_rows(vars, 0, 1, 1, rows, &missing) == 198);
    assert(fereg_spi_vdata_rows(vars, 65, 1, 1, rows, &missing) == 198);
    assert(fereg_spi_vdata_rows(vars, 3, 1, 4097, rows, &missing) == 198);
    assert(fereg_spi_vdata_rows(vars, 3, 1, 1, NULL, &missing) == 198);
    assert(fereg_spi_vdata_rows(vars, 3, 1, 1, rows, NULL) == 198);
    assert(fereg_spi_vdata_rows(vars, 3, INT_MAX, 2, rows, &missing) == 498);
    assert(fereg_spi_vdata_rows(vars, 3, 65540, 2, rows, &missing) == 498);
    vars[2] = 3;
    assert(fereg_spi_vdata_rows(vars, 3, 1, 1, rows, &missing) == 498);
    vars[2] = 2;
    plugin.vdata = NULL;
    assert(fereg_spi_vdata_rows(vars, 3, 1, 1, rows, &missing) == 498);
    plugin.vdata = read_value;
    assert(calls == 0);
    failure_at = 5;
    assert(fereg_spi_vdata_rows(vars, 3, 1, 4, rows, &missing) == 919);
    assert(calls == 5 && rows[0] == 400001 && rows[3] == 400002 && rows[12] == -77);
    for (int width = 1; width <= 64; ++width) {
        const int order[] = {4, 1, 2};
        int positions[64];
        double batch[3 * 64 + 1];
        for (int j = 0; j < width; ++j) positions[j] = order[j % 3];
        for (int edge = 0; edge < 2; ++edge) {
            const int first = edge == 0 ? 1 : 65538;
            calls = 0; failure_at = 0;
            for (int j = 0; j <= 3 * width; ++j) batch[j] = -77;
            assert(fereg_spi_vdata_rows(positions, width, first, 3, batch, &missing) == 0);
            assert(calls == 3 * width && batch[3 * width] == -77);
            for (int j = 0; j < 3 * width; ++j)
                assert(batch[j] == positions[j % width] * 100000 + first + j / width);
            for (int failure = 1; failure <= 3 * width; ++failure) {
                calls = 0; failure_at = failure;
                for (int j = 0; j <= 3 * width; ++j) batch[j] = -77;
                assert(fereg_spi_vdata_rows(positions, width, first, 3, batch, &missing) == 919);
                assert(calls == failure);
                for (int j = 0; j < failure - 1; ++j)
                    assert(batch[j] == positions[j % width] * 100000 + first + j / width);
                for (int j = failure - 1; j <= 3 * width; ++j) assert(batch[j] == -77);
            }
        }
    }
    puts("INPUT_GUARD_PASS");
    window_checks(&plugin);
    const double *outputs[] = {a, b, c};
    assert(store_calls == 0);
    assert(fereg_spi_vstore_columns(NULL, 3, 4, outputs) == 198);
    assert(fereg_spi_vstore_columns(vars, 0, 4, outputs) == 198);
    assert(fereg_spi_vstore_columns(vars, 65, 4, outputs) == 198);
    assert(fereg_spi_vstore_columns(vars, 3, 0, outputs) == 198);
    assert(fereg_spi_vstore_columns(vars, 3, 65541, outputs) == 498);
    assert(fereg_spi_vstore_columns(vars, 3, 4, NULL) == 198);
    outputs[2] = NULL;
    assert(fereg_spi_vstore_columns(vars, 3, 4, outputs) == 198);
    outputs[2] = c;
    for (int bad = -1; bad <= 5; ++bad) {
        if (bad == 1 || bad == 2 || bad == 4) continue;
        vars[2] = bad;
        assert(fereg_spi_vstore_columns(vars, 3, 4, outputs) == 498);
    }
    vars[2] = 2;
    plugin.safestore = NULL; plugin.store = NULL;
    assert(fereg_spi_vstore_columns(vars, 3, 4, outputs) == 498);
    plugin.safestore = store_value; plugin.store = store_value;
    assert(store_calls == 0);
    a[0] = -0.0;
    assert(fereg_spi_vstore_columns(vars, 3, 4, outputs) == 0);
    assert(store_calls == 12);
    for (int i = 0; i < 12; ++i) assert(memcmp(&stored[i], &outputs[i % 3][i / 3], sizeof(double)) == 0);
    for (int failure = 1; failure <= 12; ++failure) {
        store_calls = 0; store_failure_at = failure;
        assert(fereg_spi_vstore_columns(vars, 3, 4, outputs) == 918);
        assert(store_calls == failure);
    }
    puts("OUTPUT_GUARD_PASS");
    return 0;
}
