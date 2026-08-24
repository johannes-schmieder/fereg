#include <dlfcn.h>
#include <stdio.h>
#include <string.h>
#include "../../plugin/stplugin.h"

typedef ST_retcode (*pginit_function)(ST_plugin *plugin);
typedef ST_retcode (*stata_call_function)(int argc, char *argv[]);

static char display_buffer[1024];
static char error_buffer[1024];
static ST_int stop_flag = 0;

static ST_int append_display(char *message)
{
    size_t remaining = sizeof(display_buffer) - strlen(display_buffer) - 1U;
    strncat(display_buffer, message, remaining);
    return 0;
}

static ST_int append_error(char *message)
{
    size_t remaining = sizeof(error_buffer) - strlen(error_buffer) - 1U;
    strncat(error_buffer, message, remaining);
    return 0;
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

int main(int argc, char *argv[])
{
    void *library;
    pginit_function initialize;
    stata_call_function call;
    ST_plugin plugin;
    char *arguments[] = {"doctor", "threads=32"};

    if (argc != 2) {
        fprintf(stderr, "usage: plugin_loader_test PATH\n");
        return 2;
    }
    library = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
    if (library == NULL) {
        fprintf(stderr, "dlopen failed: %s\n", dlerror());
        return 3;
    }
    if (load_symbol(library, "pginit", &initialize, sizeof(initialize)) != 0 ||
        load_symbol(library, "stata_call", &call, sizeof(call)) != 0) {
        fprintf(stderr, "required export missing or has incompatible representation\n");
        dlclose(library);
        return 4;
    }
    memset(&plugin, 0, sizeof(plugin));
    plugin.spoutsml = append_display;
    plugin.spouterr = append_error;
    plugin.stopflag = &stop_flag;
    if (initialize(&plugin) != SD_PLUGINVER) {
        fprintf(stderr, "pginit ABI mismatch\n");
        dlclose(library);
        return 5;
    }
    if (call(2, arguments) != 0) {
        fprintf(stderr, "stata_call returned nonzero\n");
        dlclose(library);
        return 6;
    }
    if (strcmp(display_buffer, "loaded:doctor,threads=32\n") != 0 || error_buffer[0] != '\0') {
        fprintf(stderr, "unexpected plugin output: %s / %s\n", display_buffer, error_buffer);
        dlclose(library);
        return 7;
    }
    if (dlsym(library, "fereg_dispatch") != NULL) {
        fprintf(stderr, "internal Rust dispatch leaked from export map\n");
        dlclose(library);
        return 8;
    }
    dlclose(library);
    puts("mock Linux plugin load/audit test: PASS");
    return 0;
}
