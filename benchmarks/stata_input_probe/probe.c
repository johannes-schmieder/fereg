/* Diagnostic only: compare SDK accessors on validated numeric input. */
#include "stplugin.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static double seconds(void)
{
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t) != 0) abort();
    return (double)t.tv_sec + (double)t.tv_nsec * 1e-9;
}

/* All calls, including validation, execute on the calling Stata thread. */
static int read_values(int mode, int n, int k, double **columns, double *rows)
{
    int i, j, rc;
    ST_IIIDp reader = mode >= 2 ? _stata_->vdata : _stata_->safevdata;
    if (n < 1 || k < 1 || n > SF_nobs() || k > SF_nvars() || reader == NULL)
        return 498;
    for (j = 0; j < k; ++j) {
        if (SF_var_is_string(j + 1) || columns[j] == NULL) return 498;
    }
    if (mode == 4) {
        /* Charge both contiguous reads and a bounded transpose to the
           existing column representation; mode 3 omits that latter cost. */
        for (int first = 0; first < n; first += 256) {
            int count = n - first < 256 ? n - first : 256;
            for (i = 0; i < count; ++i) for (j = 0; j < k; ++j) {
                rc = reader(j + 1, first + i + 1, &rows[(size_t)i * (size_t)k + (size_t)j]);
                if (rc != 0) return rc;
            }
            for (j = 0; j < k; ++j) for (i = 0; i < count; ++i)
                columns[j][first + i] = rows[(size_t)i * (size_t)k + (size_t)j];
        }
        return 0;
    }
    for (i = 0; i < n; ++i) {
        for (j = 0; j < k; ++j) {
            if (mode == 0) {
                /* Mirror the current bridge's per-cell pointer check/load. */
                if (columns[j] == NULL) return 198;
                rc = SF_vdata(j + 1, i + 1, &columns[j][i]);
            } else if (mode == 3) {
                rc = reader(j + 1, i + 1, &rows[(size_t)i * (size_t)k + (size_t)j]);
            } else {
                rc = reader(j + 1, i + 1, &columns[j][i]);
            }
            if (rc != 0) return rc;
        }
    }
    return 0;
}

static uint64_t hash_values(int mode, int n, int k, double **columns, double *rows)
{
    uint64_t hash = UINT64_C(14695981039346656037);
    int i, j;
    for (i = 0; i < n; ++i) for (j = 0; j < k; ++j) {
        uint64_t bits;
        const double *value = mode == 3 ? &rows[(size_t)i * (size_t)k + (size_t)j] : &columns[j][i];
        memcpy(&bits, value, sizeof(bits));
        hash = (hash ^ bits) * UINT64_C(1099511628211);
    }
    return hash;
}

STDLL stata_call(int argc, char *argv[])
{
    int n = SF_nobs(), k = SF_nvars(), i, j, rep, slot, rc = 0;
    uint64_t expected = 0;
    double **columns = NULL, *rows = NULL;
    FILE *out = NULL;
    if (argc != 2 || n < 1 || k < 1 || k > 1024 || n > 4000000) return 198;
    /* Values, converted storage types and every missing code are compared
       individually in semantic mode, without relying on a hash. */
    if (strcmp(argv[0], "semantic") == 0) {
        size_t compared = 0;
        for (j = 1; j <= k; ++j) {
            if (SF_var_is_string(j)) return 498;
            for (i = 1; i <= n; ++i) {
                double a, b;
                int ar = _stata_->safevdata(j, i, &a);
                int br = _stata_->vdata(j, i, &b);
                if (ar != br || ar != 0 || memcmp(&a, &b, sizeof(a)) != 0) return 9;
                ++compared;
            }
        }
        out = fopen(argv[1], "w");
        if (out == NULL) return 603;
        fprintf(out, "status\tcells\npass\t%zu\n", compared);
        fclose(out);
        return 0;
    }
    if (strcmp(argv[0], "timing") != 0) return 198;
    columns = calloc((size_t)k, sizeof(*columns));
    rows = malloc((size_t)n * (size_t)k * sizeof(*rows));
    if (columns == NULL || rows == NULL) { rc = 909; goto cleanup; }
    for (j = 0; j < k; ++j) {
        columns[j] = malloc((size_t)n * sizeof(**columns));
        if (columns[j] == NULL) { rc = 909; goto cleanup; }
    }
    out = fopen(argv[1], "w");
    if (out == NULL) { rc = 603; goto cleanup; }
    fprintf(out, "repeat\tposition\tmode\tseconds\thash\n");
    for (rep = 0; rep < 11; ++rep) for (slot = 0; slot < 5; ++slot) {
        int mode = (slot + rep) % 5;
        double start, elapsed;
        uint64_t hash;
        /* Page commitment and hashing are deliberately outside read timing. */
        if (mode == 3) memset(rows, 0, (size_t)n * (size_t)k * sizeof(*rows));
        else for (j = 0; j < k; ++j) memset(columns[j], 0, (size_t)n * sizeof(**columns));
        if (mode == 4) memset(rows, 0, (size_t)(n < 256 ? n : 256) * (size_t)k * sizeof(*rows));
        start = seconds();
        rc = read_values(mode, n, k, columns, rows);
        elapsed = seconds() - start;
        if (rc != 0) goto cleanup;
        hash = hash_values(mode, n, k, columns, rows);
        if (rep == 0 && slot == 0) expected = hash;
        if (hash != expected) { rc = 9; goto cleanup; }
        fprintf(out, "%d\t%d\t%d\t%.9f\t%llu\n", rep, slot, mode, elapsed, (unsigned long long)hash);
        fflush(out);
    }
cleanup:
    if (out != NULL) fclose(out);
    if (columns != NULL) for (j = 0; j < k; ++j) free(columns[j]);
    free(columns);
    free(rows);
    return rc;
}
