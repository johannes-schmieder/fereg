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

ST_retcode stfixest_dispatch(ST_int argc, const char *const *argv)
{
    if (argc <= 0 || argv == NULL) return 198;
    if (strcmp(argv[0], "version") == 0) {
        stfixest_spi_display("stfixest.plugin_version=0.1.0\n");
        stfixest_spi_display("stfixest.action_protocol=1.1\n");
        stfixest_spi_display("stfixest.model_wire=2.1\n");
        stfixest_spi_display("stfixest.ols_result_wire=1.0\n");
        return 0;
    }
    if (strcmp(argv[0], "doctor") == 0) {
        if (has_argument(argc, argv, "protocol_major=99")) {
            stfixest_spi_error("stfixest: protocol_mismatch: mock\n");
            return 499;
        }
        stfixest_spi_display("stfixest.native_selftest=pass\n");
        stfixest_spi_display("stfixest.ado_plugin_compatible=yes\n");
        return 0;
    }
    if (strcmp(argv[0], "panic-test") == 0) {
        stfixest_spi_error("stfixest: panic: caught Rust panic at the Stata ABI boundary: mock\n");
        return 3499;
    }
    return 198;
}
