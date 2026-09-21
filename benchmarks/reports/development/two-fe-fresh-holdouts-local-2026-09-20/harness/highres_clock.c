/* Benchmark-only stopwatch. Never linked into the estimator or distribution. */
#include "stplugin.h"
#include <math.h>
#include <string.h>
#include <time.h>

static struct timespec started;
static int active;

STDLL stata_call(int argc, char *argv[]);

STDLL stata_call(int argc, char *argv[])
{
    struct timespec ended;
    double seconds;
    if (argc == 1 && strcmp(argv[0], "begin") == 0) {
        if (active) return 198;
        if (clock_gettime(CLOCK_MONOTONIC_RAW, &started) != 0) return 610;
        active = 1;
        return 0;
    }
    if (argc == 2 && strcmp(argv[0], "end") == 0) {
        if (!active) return 198;
        active = 0;
        if (clock_gettime(CLOCK_MONOTONIC_RAW, &ended) != 0) return 610;
        seconds = (double)(ended.tv_sec - started.tv_sec) +
                  (double)(ended.tv_nsec - started.tv_nsec) * 1e-9;
        if (!isfinite(seconds) || seconds < 0.0) return 610;
        return SF_scal_save(argv[1], &seconds);
    }
    return 198;
}
