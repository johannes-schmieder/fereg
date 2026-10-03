/* One process, one real binary, one complete native estimation transaction. */
#include "replay_io.h"

static int replay_nobs(void) { return replay_rows; }
static int replay_nvars(void) { return replay_columns; }
static int replay_in1(void) { return 1; }
static ST_boolean replay_selected(int row) { return (ST_boolean) (row >= 1 && row <= replay_rows); }
static ST_boolean replay_isstr(int variable)
{
    return (ST_boolean) (variable > 0 && variable <= replay_columns && replay_strings[variable - 1] != NULL);
}
static ST_boolean replay_missing(double value)
{
    return (ST_boolean) (isnan(value) || value >= 0x1p+1023);
}
static int replay_location(int variable, int row)
{
    if (variable < 1 || variable > replay_columns || row < 1 || row > replay_rows) {
        replay_fault = 1;
        return 0;
    }
    return 1;
}
static int replay_vdata(int variable, int row, double *value)
{
    if (!replay_location(variable, row) || replay_isstr(variable)) return 111;
    *value = replay_data[variable - 1][row - 1];
    return 0;
}
static int replay_vstore(int variable, int row, double value)
{
    if (!replay_location(variable, row) || replay_isstr(variable)) return 111;
    replay_data[variable - 1][row - 1] = value;
    return 0;
}
static int replay_sdata(int variable, int row, char *value)
{
    if (!replay_location(variable, row) || !replay_isstr(variable)) return 111;
    strcpy(value, replay_strings[variable - 1][row - 1]);
    return 0;
}
static int replay_sdatalen(int variable, int row)
{
    if (!replay_location(variable, row) || !replay_isstr(variable)) return -1;
    return (int) strlen(replay_strings[variable - 1][row - 1]);
}
static ReplayMatrix *replay_matrix(const char *name)
{
    int i;
    for (i = 0; i < replay_matrix_count; ++i)
        if (strcmp(name, replay_matrices[i].name) == 0) return &replay_matrices[i];
    replay_fault = 1;
    return NULL;
}
static int replay_rowsof(char *name)
{
    ReplayMatrix *matrix = replay_matrix(name);
    return matrix == NULL ? 0 : matrix->rows;
}
static int replay_colsof(char *name)
{
    ReplayMatrix *matrix = replay_matrix(name);
    return matrix == NULL ? 0 : matrix->cols;
}
static double *replay_cell(char *name, int row, int column)
{
    ReplayMatrix *matrix = replay_matrix(name);
    if (matrix == NULL || row < 1 || column < 1 || row > matrix->rows || column > matrix->cols) {
        replay_fault = 1;
        return NULL;
    }
    return &matrix->values[(size_t) (row - 1) * (size_t) matrix->cols + (size_t) column - 1U];
}
static int replay_matel(char *name, int row, int column, double *value)
{
    double *cell = replay_cell(name, row, column);
    if (cell == NULL) return 503;
    *value = *cell;
    return 0;
}
static int replay_matstore(char *name, int row, int column, double value)
{
    double *cell = replay_cell(name, row, column);
    if (cell == NULL) return 503;
    *cell = value;
    return 0;
}
static int replay_macuse(char *name, char *buffer, int length)
{
    int i;
    for (i = 0; i < replay_macro_count; ++i) {
        const ReplayMacro *macro = &replay_macros[i];
        if (strcmp(name, macro->name) == 0) {
            if (length <= 0 || strlen(macro->value) >= (size_t) length) return 198;
            strcpy(buffer, macro->value);
            return 0;
        }
    }
    replay_fault = 1;
    return 111;
}
static int replay_run(pginit_function initialize, stata_call_function call,
                      const char *input, const char *output)
{
    ST_plugin plugin;
    int code;
    replay_read(input);
    memset(&plugin, 0, sizeof(plugin));
    plugin.spoutsml = append_display;
    plugin.spouterr = append_error;
    plugin.pollstd = mock_poll;
    plugin.nobs = replay_nobs;
    plugin.nvars = replay_nvars;
    plugin.nvar = replay_nvars;
    plugin.nobs1 = replay_in1;
    plugin.nobs2 = replay_nobs;
    plugin.selobs = replay_selected;
    plugin.isstr = replay_isstr;
    plugin.isstrl = mock_isstr;
    plugin.ismissing = replay_missing;
    plugin.missval = 0x1p+1023;
    plugin.safevdata = replay_vdata;
    plugin.vdata = replay_vdata;
    plugin.safestore = replay_vstore;
    plugin.sdata = replay_sdata;
    plugin.sdatalen = replay_sdatalen;
    plugin.rowsof = replay_rowsof;
    plugin.colsof = replay_colsof;
    plugin.safematel = replay_matel;
    plugin.safematstore = replay_matstore;
    plugin.macuse = replay_macuse;
    plugin.stopflag = &stop_flag;
    if (initialize(&plugin) != SD_PLUGINVER) replay_die("plugin ABI mismatch");
    code = call(replay_argc, replay_arguments);
    if (replay_fault) replay_die("SPI request outside the supplied case");
    replay_write(output, code);
    if (code != 0) fprintf(stderr, "%s\n", error_buffer);
    return 0;
}
