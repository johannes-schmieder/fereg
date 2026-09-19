/* Exercise the Mac numeric batch with a callback that must never receive
   an invalid address; no invalid access is attempted against live Stata. */
#include "stata_bridge.h"
#include <assert.h>
#include <limits.h>
#include <pthread.h>
#include <stdio.h>
#include <string.h>

static int calls, failure_at;
static ST_int nobs(void) { return 65540; }
static ST_int nvars(void) { return 4; }
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
    assert(0 && "Mac numeric batches must select the guarded direct accessor");
    return 498;
}

ST_int fereg_dispatch(ST_int argc, const char *const *argv)
{
    (void)argc; (void)argv; return 0;
}

static void *worker(void *unused)
{
    int vars[] = {1};
    double data[1], *cols[] = {data}, missing;
    (void)unused;
    assert(fereg_spi_vdata_columns(vars, 1, 1, 1, cols, &missing) == 498);
    assert(fereg_spi_vdata_rows(vars, 1, 1, 1, data, &missing) == 498);
    return NULL;
}

int main(void)
{
    ST_plugin plugin;
    int vars[] = {4, 1, 2};
    double a[4] = {0}, b[4] = {0}, c[4] = {0}, *cols[] = {a, b, c}, missing;
    pthread_t thread;
    memset(&plugin, 0, sizeof(plugin));
    plugin.nobs = nobs; plugin.nvars = nvars; plugin.isstr = isstr;
    plugin.safevdata = checked_read; plugin.vdata = read_value;
    plugin.missval = 0x1p+1023;
    assert(fereg_spi_vdata_columns(vars, 3, 1, 4, cols, &missing) == 498);
    assert(pginit(&plugin) == SD_PLUGINVER);
    assert(fereg_spi_vdata_columns(vars, 3, 1, 4, cols, &missing) == 498);
    assert(stata_call(0, NULL) == 0);
    assert(pthread_create(&thread, NULL, worker, NULL) == 0);
    assert(pthread_join(thread, NULL) == 0);
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
    puts("INPUT_GUARD_PASS");
    return 0;
}
