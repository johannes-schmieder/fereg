#include "../../plugin/stata_bridge.h"

ST_retcode fereg_dispatch(ST_int argc, const char *const *argv)
{
    ST_int index;
    if (fereg_spi_display("loaded:") != 0) return 3499;
    for (index = 0; index < argc; ++index) {
        if (fereg_spi_display(argv[index]) != 0) return 3499;
        if (fereg_spi_display(index + 1 == argc ? "\n" : ",") != 0) return 3499;
    }
    return 0;
}
