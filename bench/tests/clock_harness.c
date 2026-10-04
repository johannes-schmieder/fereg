/* Exercise the benchmark clock's SPI scalar boundary and failure state. */
#include "stplugin.h"
#include <assert.h>
#include <string.h>

STDLL stata_call(int argc, char *argv[]);
static double elapsed;
static double resolution;
static int save_failure;

static int save_scalar(char *name, double *value)
{
    if (save_failure) return 321;
    if (strcmp(name, "elapsed") == 0) elapsed = *value;
    else if (strcmp(name, "resolution") == 0) resolution = *value;
    else return 198;
    return 0;
}

int main(void)
{
    ST_plugin spi = {0};
    char *start[] = {"start"};
    char *stop[] = {"stop", "elapsed", "resolution"};
    char *unknown[] = {"unknown"};
    spi.scalarsave = save_scalar;
    assert(pginit(&spi) == SD_PLUGINVER);
    assert(stata_call(3, stop) == 198);
    assert(stata_call(1, unknown) == 198);
    assert(stata_call(1, start) == 0);
    assert(stata_call(1, start) == 198);
    assert(stata_call(3, stop) == 0);
    assert(elapsed >= 0 && resolution > 0 && resolution <= 0.0000005);
    assert(stata_call(3, stop) == 198);
    assert(stata_call(1, start) == 0);
    save_failure = 1;
    assert(stata_call(3, stop) == 321);
    save_failure = 0;
    assert(stata_call(1, start) == 0);
    assert(stata_call(3, stop) == 0);
    return 0;
}
