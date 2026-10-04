/* Benchmark-only command clock. It owns no estimator state or data. */
#if !defined(_WIN32)
#define _POSIX_C_SOURCE 200809L
#endif
#include "stplugin.h"
#include <stdint.h>
#include <string.h>
#if defined(_WIN32)
#include <windows.h>
#elif defined(__APPLE__)
#include <mach/mach_time.h>
#else
#include <time.h>
#endif

STDLL stata_call(int argc, char *argv[]);

static uint64_t started_ticks;
static double tick_seconds;
static double resolution_seconds;
static int running;

static int clock_info(void)
{
#if defined(_WIN32)
    LARGE_INTEGER frequency;
    if (!QueryPerformanceFrequency(&frequency) || frequency.QuadPart <= 0) return 498;
    tick_seconds = 1.0 / (double)frequency.QuadPart;
    resolution_seconds = tick_seconds;
#elif defined(__APPLE__)
    mach_timebase_info_data_t timebase;
    if (mach_timebase_info(&timebase) != KERN_SUCCESS || timebase.denom == 0) return 498;
    tick_seconds = 1e-9 * (double)timebase.numer / (double)timebase.denom;
    resolution_seconds = tick_seconds;
#else
    struct timespec resolution;
    if (clock_getres(CLOCK_MONOTONIC, &resolution) != 0) return 498;
    tick_seconds = 1e-9;
    resolution_seconds = (double)resolution.tv_sec + (double)resolution.tv_nsec * tick_seconds;
#endif
    return 0;
}

static int clock_ticks(uint64_t *ticks)
{
#if defined(_WIN32)
    LARGE_INTEGER counter;
    if (!QueryPerformanceCounter(&counter) || counter.QuadPart < 0) return 498;
    *ticks = (uint64_t)counter.QuadPart;
#elif defined(__APPLE__)
    *ticks = mach_absolute_time();
#else
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now) != 0 || now.tv_sec < 0) return 498;
    *ticks = (uint64_t)now.tv_sec * UINT64_C(1000000000) + (uint64_t)now.tv_nsec;
#endif
    return 0;
}

STDLL stata_call(int argc, char *argv[])
{
    uint64_t ticks;
    ST_double elapsed;
    int rc;
    if (argc == 1 && strcmp(argv[0], "start") == 0) {
        if (running) return 198;
        rc = clock_info();
        if (rc != 0) return rc;
        rc = clock_ticks(&started_ticks);
        if (rc != 0) return rc;
        running = 1;
        return 0;
    }
    if (argc != 3 || strcmp(argv[0], "stop") != 0 || !running) return 198;
    rc = clock_ticks(&ticks);
    running = 0;
    if (rc != 0 || ticks < started_ticks) return 498;
    elapsed = (double)(ticks - started_ticks) * tick_seconds;
    rc = SF_scal_save(argv[1], &elapsed);
    if (rc != 0) return rc;
    return SF_scal_save(argv[2], &resolution_seconds);
}
