#include <dlfcn.h>
#include <stdio.h>
#include <string.h>
#include "../../plugin/stplugin.h"

typedef ST_retcode (*pginit_function)(ST_plugin *plugin);
typedef ST_retcode (*stata_call_function)(int argc, char *argv[]);

static char display_buffer[16384];
static char error_buffer[16384];
static ST_int stop_flag = 0;

static ST_int append_text(char *destination, size_t capacity, const char *message)
{
    size_t current = strlen(destination);
    size_t available = capacity - current - 1U;
    size_t amount = strlen(message);
    if (amount > available) amount = available;
    memcpy(destination + current, message, amount);
    destination[current + amount] = '\0';
    return 0;
}

static ST_int append_display(char *message)
{
    return append_text(display_buffer, sizeof(display_buffer), message);
}

static ST_int append_error(char *message)
{
    return append_text(error_buffer, sizeof(error_buffer), message);
}

static void clear_output(void)
{
    display_buffer[0] = '\0';
    error_buffer[0] = '\0';
}

static int load_symbol(void *library, const char *name, void *target, size_t target_size)
{
    void *symbol;
    if (target_size != sizeof(symbol)) return 1;
    dlerror();
    symbol = dlsym(library, name);
    if (symbol == NULL || dlerror() != NULL) return 1;
    memcpy(target, &symbol, target_size);
    return 0;
}

static int check_version(stata_call_function call)
{
    char *arguments[] = {"version"};
    clear_output();
    if (call(1, arguments) != 0) return 1;
    if (strstr(display_buffer, "stfixest.plugin_version=0.1.0") == NULL ||
        strstr(display_buffer, "stfixest.model_protocol=1.0") == NULL ||
        error_buffer[0] != '\0') {
        return 2;
    }
    return 0;
}

static int check_doctor(stata_call_function call)
{
    char *arguments[] = {
        "doctor",
        "ado=0.1.0-phase01",
        "protocol_major=1",
        "protocol_minor=0",
        "stata=18.0",
        "threads=32",
        "plugin_file=validation.plugin",
        "dependency_audit=target-native-pass"
    };
    clear_output();
    if (call((int) (sizeof(arguments) / sizeof(arguments[0])), arguments) != 0) return 1;
    if (strstr(display_buffer, "stfixest.native_selftest=pass") == NULL ||
        strstr(display_buffer, "stfixest.ado_plugin_compatible=yes") == NULL ||
        error_buffer[0] != '\0') {
        return 2;
    }
    return 0;
}

static int check_protocol_mismatch(stata_call_function call)
{
    char *arguments[] = {
        "doctor",
        "ado=0.1.0-phase01",
        "protocol_major=99",
        "protocol_minor=0",
        "stata=18.0"
    };
    clear_output();
    if (call((int) (sizeof(arguments) / sizeof(arguments[0])), arguments) != 499) return 1;
    if (strstr(error_buffer, "protocol_mismatch") == NULL) return 2;
    return 0;
}

static int check_panic_boundary(stata_call_function call)
{
    char *arguments[] = {"panic-test", "confirm=panic-boundary-test"};
    clear_output();
    if (call(2, arguments) != 3499) return 1;
    if (strstr(error_buffer, "caught Rust panic") == NULL) return 2;
    return 0;
}

int main(int argc, char *argv[])
{
    void *library;
    pginit_function initialize = NULL;
    stata_call_function call = NULL;
    ST_plugin plugin;
    int result;

    if (argc != 2) {
        fprintf(stderr, "usage: actual_plugin_loader_test PATH\n");
        return 2;
    }
    library = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
    if (library == NULL) {
        fprintf(stderr, "dlopen failed: %s\n", dlerror());
        return 3;
    }
    if (load_symbol(library, "pginit", &initialize, sizeof(initialize)) != 0 ||
        load_symbol(library, "stata_call", &call, sizeof(call)) != 0) {
        fprintf(stderr, "required export missing or incompatible\n");
        dlclose(library);
        return 4;
    }
    if (dlsym(library, "stfixest_dispatch") != NULL) {
        fprintf(stderr, "internal Rust dispatch leaked from exports\n");
        dlclose(library);
        return 5;
    }

    memset(&plugin, 0, sizeof(plugin));
    plugin.spoutsml = append_display;
    plugin.spouterr = append_error;
    plugin.stopflag = &stop_flag;
    if (initialize(&plugin) != SD_PLUGINVER) {
        fprintf(stderr, "pginit ABI mismatch\n");
        dlclose(library);
        return 6;
    }

    result = check_version(call);
    if (result != 0) {
        fprintf(stderr, "version path failed (%d): %s / %s\n", result, display_buffer, error_buffer);
        dlclose(library);
        return 10 + result;
    }
    result = check_doctor(call);
    if (result != 0) {
        fprintf(stderr, "doctor path failed (%d): %s / %s\n", result, display_buffer, error_buffer);
        dlclose(library);
        return 20 + result;
    }
    result = check_protocol_mismatch(call);
    if (result != 0) {
        fprintf(stderr, "protocol mismatch path failed (%d): %s / %s\n", result, display_buffer, error_buffer);
        dlclose(library);
        return 30 + result;
    }
    result = check_panic_boundary(call);
    if (result != 0) {
        fprintf(stderr, "panic path failed (%d): %s / %s\n", result, display_buffer, error_buffer);
        dlclose(library);
        return 40 + result;
    }

    dlclose(library);
    puts("actual-plugin ABI/self-test harness: PASS");
    return 0;
}
