#define WIN32_LEAN_AND_MEAN
#include <windows.h>
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
    size_t current = strlen(display_buffer);
    size_t available = sizeof(display_buffer) - current - 1U;
    size_t amount = strlen(message);
    if (amount > available) amount = available;
    memcpy(display_buffer + current, message, amount);
    display_buffer[current + amount] = '\0';
    return 0;
}

static ST_int append_error(char *message)
{
    size_t current = strlen(error_buffer);
    size_t available = sizeof(error_buffer) - current - 1U;
    size_t amount = strlen(message);
    if (amount > available) amount = available;
    memcpy(error_buffer + current, message, amount);
    error_buffer[current + amount] = '\0';
    return 0;
}

static int load_symbol(HMODULE library, const char *name, void *target, size_t target_size)
{
    FARPROC symbol = GetProcAddress(library, name);
    if (symbol == NULL || target_size != sizeof(symbol)) return 1;
    memcpy(target, &symbol, target_size);
    return 0;
}

int main(int argc, char *argv[])
{
    HMODULE library;
    pginit_function initialize = NULL;
    stata_call_function call = NULL;
    ST_plugin plugin;
    char *arguments[] = {"doctor", "threads=32"};

    if (argc != 2) {
        fprintf(stderr, "usage: plugin_loader_test_windows PATH\n");
        return 2;
    }
    library = LoadLibraryA(argv[1]);
    if (library == NULL) {
        fprintf(stderr, "LoadLibrary failed: %lu\n", (unsigned long) GetLastError());
        return 3;
    }
    if (load_symbol(library, "pginit", &initialize, sizeof(initialize)) != 0 ||
        load_symbol(library, "stata_call", &call, sizeof(call)) != 0) {
        fprintf(stderr, "required export missing or has incompatible representation\n");
        FreeLibrary(library);
        return 4;
    }
    memset(&plugin, 0, sizeof(plugin));
    plugin.spoutsml = append_display;
    plugin.spouterr = append_error;
    plugin.stopflag = &stop_flag;
    if (initialize(&plugin) != SD_PLUGINVER) {
        fprintf(stderr, "pginit ABI mismatch\n");
        FreeLibrary(library);
        return 5;
    }
    if (call(2, arguments) != 0) {
        fprintf(stderr, "stata_call returned nonzero\n");
        FreeLibrary(library);
        return 6;
    }
    if (strcmp(display_buffer, "loaded:doctor,threads=32\n") != 0 ||
        error_buffer[0] != '\0') {
        fprintf(stderr, "unexpected plugin output: %s / %s\n", display_buffer, error_buffer);
        FreeLibrary(library);
        return 7;
    }
    if (GetProcAddress(library, "fereg_dispatch") != NULL) {
        fprintf(stderr, "internal Rust dispatch leaked from exports\n");
        FreeLibrary(library);
        return 8;
    }
    FreeLibrary(library);
    puts("mock Windows plugin load/audit test: PASS");
    return 0;
}
