/* Lossless, bounded replay transport. A malformed case is a harness error. */
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Corpus bounds. Local large-case profiling may raise them at compile time,
   for example -DREPLAY_MAX_ROWS=4000000 -DREPLAY_MAX_CELLS=100000000U. */
#ifndef REPLAY_MAX_ROWS
#define REPLAY_MAX_ROWS 1000000
#endif
#ifndef REPLAY_MAX_CELLS
#define REPLAY_MAX_CELLS 10000000U
#endif

typedef struct { char *name; int rows, cols; double *values; } ReplayMatrix;
typedef struct { char *name, *value; } ReplayMacro;
static int replay_rows, replay_columns, replay_argc, replay_matrix_count, replay_macro_count;
static char **replay_arguments;
static double **replay_data;
static char ***replay_strings;
static ReplayMatrix *replay_matrices;
static ReplayMacro *replay_macros;
static int replay_fault;

static void replay_die(const char *message)
{
    fprintf(stderr, "replay harness error: %s\n", message);
    exit(90);
}

static void *replay_allocate(size_t count, size_t size)
{
    void *value;
    if (count > REPLAY_MAX_CELLS || size > 1000000U) replay_die("allocation bound");
    value = calloc(count == 0U ? 1U : count, size);
    if (value == NULL) replay_die("allocation failed");
    return value;
}

static int replay_integer(FILE *file, int maximum)
{
    int value;
    if (fscanf(file, "%d", &value) != 1 || value < 0 || value > maximum)
        replay_die("invalid bounded integer");
    return value;
}

static char *replay_text(FILE *file)
{
    int length = replay_integer(file, 1000000);
    char *value = replay_allocate((size_t) length + 1U, 1U);
    if (fgetc(file) != '\n' || fread(value, 1U, (size_t) length, file) != (size_t) length ||
        fgetc(file) != '\n' || strlen(value) != (size_t) length) replay_die("invalid string");
    return value;
}

static double replay_number(FILE *file)
{
    uint64_t bits;
    double value;
    if (fscanf(file, "%16" SCNx64, &bits) != 1) replay_die("invalid float bits");
    memcpy(&value, &bits, sizeof(value));
    return value;
}

static void replay_read(const char *path)
{
    FILE *file = fopen(path, "rb");
    char magic[32];
    int i, j;
    if (file == NULL || fgets(magic, sizeof(magic), file) == NULL ||
        strcmp(magic, "FEREG_REPLAY_V1\n") != 0) replay_die("input version");
    replay_rows = replay_integer(file, REPLAY_MAX_ROWS);
    replay_columns = replay_integer(file, 10000);
    replay_argc = replay_integer(file, 1000);
    replay_matrix_count = replay_integer(file, 200);
    replay_macro_count = replay_integer(file, 100);
    if ((size_t) replay_rows * (size_t) replay_columns > REPLAY_MAX_CELLS) replay_die("data bound");
    replay_arguments = replay_allocate((size_t) replay_argc, sizeof(char *));
    for (i = 0; i < replay_argc; ++i) replay_arguments[i] = replay_text(file);
    replay_data = replay_allocate((size_t) replay_columns, sizeof(double *));
    replay_strings = replay_allocate((size_t) replay_columns, sizeof(char **));
    for (i = 0; i < replay_columns; ++i) {
        int string_kind = replay_integer(file, 1);
        replay_data[i] = replay_allocate((size_t) replay_rows, sizeof(double));
        if (string_kind) replay_strings[i] = replay_allocate((size_t) replay_rows, sizeof(char *));
        for (j = 0; j < replay_rows; ++j) {
            if (string_kind) replay_strings[i][j] = replay_text(file);
            else replay_data[i][j] = replay_number(file);
        }
    }
    replay_matrices = replay_allocate((size_t) replay_matrix_count, sizeof(ReplayMatrix));
    for (i = 0; i < replay_matrix_count; ++i) {
        ReplayMatrix *matrix = &replay_matrices[i];
        size_t cell, count;
        matrix->name = replay_text(file);
        matrix->rows = replay_integer(file, 10000);
        matrix->cols = replay_integer(file, 10000);
        count = (size_t) matrix->rows * (size_t) matrix->cols;
        matrix->values = replay_allocate(count, sizeof(double));
        for (cell = 0; cell < count; ++cell) matrix->values[cell] = replay_number(file);
    }
    replay_macros = replay_allocate((size_t) replay_macro_count, sizeof(ReplayMacro));
    for (i = 0; i < replay_macro_count; ++i) {
        replay_macros[i].name = replay_text(file);
        replay_macros[i].value = replay_text(file);
    }
    while ((j = fgetc(file)) != EOF) if (j != '\n' && j != ' ' && j != '\r') replay_die("trailing input");
    if (ferror(file) || fclose(file) != 0) replay_die("input read failure");
}

static void replay_json_string(FILE *file, const char *text)
{
    const unsigned char *p = (const unsigned char *) text;
    fputc('"', file);
    for (; *p; ++p) {
        if (*p == '"' || *p == '\\') fprintf(file, "\\%c", *p);
        else if (*p < 32U) fprintf(file, "\\u%04x", (unsigned int) *p);
        else fputc(*p, file);
    }
    fputc('"', file);
}

static void replay_json_number(FILE *file, double value)
{
    uint64_t bits;
    memcpy(&bits, &value, sizeof(bits));
    fprintf(file, "\"%016" PRIx64 "\"", bits);
}

static void replay_write(const char *path, int code)
{
    FILE *file = fopen(path, "wb");
    int i, j;
    if (file == NULL) replay_die("cannot open output");
    fprintf(file, "{\"schema\":1,\"return_code\":%d,\"matrices\":{", code);
    for (i = 0; i < replay_matrix_count; ++i) {
        ReplayMatrix *matrix = &replay_matrices[i];
        size_t cell, count = (size_t) matrix->rows * (size_t) matrix->cols;
        if (i) fputc(',', file);
        replay_json_string(file, matrix->name);
        fprintf(file, ":{\"rows\":%d,\"columns\":%d,\"bits\":[", matrix->rows, matrix->cols);
        for (cell = 0; cell < count; ++cell) {
            if (cell) fputc(',', file);
            replay_json_number(file, matrix->values[cell]);
        }
        fputs("]}", file);
    }
    fputs("},\"columns\":[", file);
#ifdef REPLAY_TIMING
    /* Timing builds stream numeric columns as raw binary beside the JSON so
     * large outputs stay hashable without hex text; timing inputs are numeric. */
    {
        char column_path[4096];
        FILE *columns;
        (void) j;
        if (snprintf(column_path, sizeof(column_path), "%s.columns", path) >= (int) sizeof(column_path))
            replay_die("column path too long");
        columns = fopen(column_path, "wb");
        if (columns == NULL) replay_die("cannot open column output");
        for (i = 0; i < replay_columns; ++i) {
            if (replay_strings[i]) replay_die("timing builds require numeric columns");
            if (fwrite(replay_data[i], sizeof(double), (size_t) replay_rows, columns) != (size_t) replay_rows)
                replay_die("column write failure");
        }
        if (fclose(columns) != 0) replay_die("column close failure");
    }
#else
    for (i = 0; i < replay_columns; ++i) {
        if (i) fputc(',', file);
        fputc('[', file);
        for (j = 0; j < replay_rows; ++j) {
            if (j) fputc(',', file);
            if (replay_strings[i]) replay_json_string(file, replay_strings[i][j]);
            else replay_json_number(file, replay_data[i][j]);
        }
        fputc(']', file);
    }
#endif
    fputs("],\"error\":", file);
    replay_json_string(file, error_buffer);
    fputs("}\n", file);
    if (ferror(file) || fclose(file) != 0) replay_die("output write failure");
}
