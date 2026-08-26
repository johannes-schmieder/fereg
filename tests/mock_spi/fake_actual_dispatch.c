#include <string.h>
#include "../../plugin/stata_bridge.h"

static int has_argument(ST_int argc, const char *const *argv, const char *expected)
{
    ST_int index;
    for (index = 0; index < argc; ++index) {
        if (strcmp(argv[index], expected) == 0) return 1;
    }
    return 0;
}

ST_retcode fereg_dispatch(ST_int argc, const char *const *argv)
{
    if (argc <= 0 || argv == NULL) return 198;
    if (strcmp(argv[0], "version") == 0) {
        fereg_spi_display("fereg.plugin_version=0.5.0\n");
        fereg_spi_display("fereg.action_protocol=1.1\n");
        fereg_spi_display("fereg.model_wire=2.1\n");
        fereg_spi_display("fereg.ols_result_wire=1.1\n");
        return 0;
    }
    if (strcmp(argv[0], "doctor") == 0) {
        if (has_argument(argc, argv, "protocol_major=99")) {
            fereg_spi_error("fereg: protocol_mismatch: mock\n");
            return 499;
        }
        fereg_spi_display("fereg.native_selftest=pass\n");
        fereg_spi_display("fereg.ado_plugin_compatible=yes\n");
        return 0;
    }
    if (strcmp(argv[0], "panic-test") == 0) {
        fereg_spi_error("fereg: panic: caught Rust panic at the Stata ABI boundary: mock\n");
        return 3499;
    }
    return 198;
}
