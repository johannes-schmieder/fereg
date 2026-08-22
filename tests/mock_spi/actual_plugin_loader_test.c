#include <dlfcn.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
#include "../../plugin/stplugin.h"

typedef ST_retcode (*pginit_function)(ST_plugin *plugin);
typedef ST_retcode (*stata_call_function)(int argc, char *argv[]);

static char display_buffer[16384];
static char error_buffer[16384];
static ST_int stop_flag = 0;
static ST_double plugin_data[7][5];
static ST_int plugin_read_counts[7][5];
static ST_int plugin_nvars = 6;
static ST_int output_store_calls = 0;
static ST_int fail_output_store_at = 0;
static ST_int last_store_was_complete = 0;
static const char *prepared_store_name_pointers[11];
static const char *result_matrix_names[11] = {
    "RB", "RV", "RSTATUS", "RFIT", "RDOF", "RSOLVER",
    "RFE", "RCLUSTER", "RSUBSET", "RCERT", "RREC"
};

static ST_double result_b[1][2];
static ST_double result_v[2][2];
static ST_double result_status[2][4];
static ST_double result_fit[12][2];
static ST_double result_dof[20][2];
static ST_double result_solver[2][12];
static ST_double result_fe[1][8];
static ST_double result_cluster[1][4];
static ST_double result_subset[1][4];
static ST_double result_certificate[32][2];
static ST_double result_receipt[1][16];

static ST_int result_matrix_name_index(const char *name)
{
    ST_int index;
    for (index = 0; index < 11; ++index) {
        if (strcmp(name, result_matrix_names[index]) == 0) return index;
    }
    return -1;
}

static ST_int mock_poll(void) { return 0; }
static ST_int mock_nobs(void) { return 5; }
static ST_int mock_nvars(void) { return plugin_nvars; }
static ST_boolean mock_isstr(ST_int variable)
{
    (void) variable;
    return (ST_boolean) 0U;
}
static ST_boolean mock_missing(ST_double value)
{
    return (ST_boolean) (isnan(value) ? 1U : 0U);
}
static ST_int mock_vdata(ST_int variable, ST_int observation, ST_double *value)
{
    if (variable < 1 || variable > 7 || observation < 1 || observation > 5 || value == NULL) {
        return 111;
    }
    plugin_read_counts[variable - 1][observation - 1] += 1;
    *value = plugin_data[variable - 1][observation - 1];
    return 0;
}
static ST_int mock_vstore(ST_int variable, ST_int observation, ST_double value)
{
    if (variable < 1 || variable > 7 || observation < 1 || observation > 5) return 111;
    output_store_calls += 1;
    last_store_was_complete = 0;
    if (fail_output_store_at == output_store_calls) return 912;
    plugin_data[variable - 1][observation - 1] = value;
    return 0;
}

static ST_double *mock_result_matrix(char *name, ST_int *rows, ST_int *columns)
{
    if (strcmp(name, "RB") == 0) {
        *rows = 1; *columns = 2; return &result_b[0][0];
    }
    if (strcmp(name, "RV") == 0) {
        *rows = 2; *columns = 2; return &result_v[0][0];
    }
    if (strcmp(name, "RSTATUS") == 0) {
        *rows = 2; *columns = 4; return &result_status[0][0];
    }
    if (strcmp(name, "RFIT") == 0) {
        *rows = 12; *columns = 2; return &result_fit[0][0];
    }
    if (strcmp(name, "RDOF") == 0) {
        *rows = 20; *columns = 2; return &result_dof[0][0];
    }
    if (strcmp(name, "RSOLVER") == 0) {
        *rows = 2; *columns = 12; return &result_solver[0][0];
    }
    if (strcmp(name, "RFE") == 0) {
        *rows = 1; *columns = 8; return &result_fe[0][0];
    }
    if (strcmp(name, "RCLUSTER") == 0) {
        *rows = 1; *columns = 4; return &result_cluster[0][0];
    }
    if (strcmp(name, "RSUBSET") == 0) {
        *rows = 1; *columns = 4; return &result_subset[0][0];
    }
    if (strcmp(name, "RCERT") == 0) {
        *rows = 32; *columns = 2; return &result_certificate[0][0];
    }
    if (strcmp(name, "RREC") == 0) {
        *rows = 1; *columns = 16; return &result_receipt[0][0];
    }
    return NULL;
}

static const ST_double source_matrix[1][2] = {{1.0, 0.0}};
static const ST_double term_matrix[1][8] = {{1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0}};
static const ST_double component_matrix[1][13] = {
    {1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 2.0, 1.0, 1.0, 1.0, 0.0, 0.0}
};
static const ST_double empty_absorb_term_matrix[1][4] = {{0.0, 0.0, 0.0, 0.0}};
static const ST_double empty_absorb_component_matrix[1][4] = {{0.0, 0.0, 0.0, 0.0}};
static const ST_double factor_term_matrix[3][8] = {
    {1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 0.0, 0.0},
    {2.0, 2.0, 2.0, 2.0, 1.0, 0.0, 0.0, 0.0},
    {3.0, 3.0, 3.0, 2.0, 1.0, 0.0, 0.0, 0.0}
};
static const ST_double factor_component_matrix[3][13] = {
    {1.0, 1.0, 1.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0},
    {2.0, 1.0, 1.0, 2.0, 2.0, 2.0, 0.0, 0.0, 1.0, 2.0, 2.0, 0.0, 0.0},
    {3.0, 1.0, 1.0, 2.0, 2.0, 3.0, 0.0, 0.0, 1.0, 3.0, 3.0, 0.0, 0.0}
};
static const ST_double interaction_source_matrix[2][2] = {
    {1.0, 0.0}, {2.0, 0.0}
};
static const ST_double interaction_term_matrix[1][8] = {
    {1.0, 1.0, 1.0, 3.0, 2.0, 0.0, 0.0, 0.0}
};
static const ST_double interaction_component_matrix[2][13] = {
    {1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0},
    {1.0, 2.0, 2.0, 2.0, 2.0, 1.0, 0.0, 0.0, 1.0, 2.0, 2.0, 0.0, 0.0}
};
static const ST_double binding_matrix[3][3] = {
    {1.0, 1.0, 0.0}, {2.0, 2.0, 0.0}, {3.0, 3.0, 1.0}
};
static const ST_double analytic_weight_binding_matrix[4][3] = {
    {1.0, 1.0, 0.0}, {2.0, 2.0, 0.0}, {3.0, 3.0, 1.0}, {4.0, 4.0, 0.0}
};
static const ST_double frequency_weight_binding_matrix[4][3] = {
    {1.0, 1.0, 0.0}, {2.0, 2.0, 0.0}, {3.0, 3.0, 1.0}, {4.0, 5.0, 0.0}
};
static const ST_double probability_weight_binding_matrix[4][3] = {
    {1.0, 1.0, 0.0}, {2.0, 2.0, 0.0}, {3.0, 3.0, 1.0}, {4.0, 6.0, 0.0}
};
static const ST_double interaction_binding_matrix[4][3] = {
    {1.0, 1.0, 0.0}, {2.0, 2.0, 0.0},
    {3.0, 3.0, 1.0}, {4.0, 3.0, 2.0}
};

static ST_int mock_rowsof(char *name)
{
    ST_int rows = 0;
    ST_int columns = 0;
    if (mock_result_matrix(name, &rows, &columns) != NULL) return rows;
    if (strcmp(name, "S") == 0 || strcmp(name, "T") == 0 || strcmp(name, "C") == 0 ||
        strcmp(name, "A") == 0 || strcmp(name, "AC") == 0 ||
        strcmp(name, "CT") == 0 || strcmp(name, "CC") == 0) return 1;
    if (strcmp(name, "B") == 0) return 3;
    if (strcmp(name, "BW") == 0) return 4;
    if (strcmp(name, "BF") == 0) return 4;
    if (strcmp(name, "BP") == 0) return 4;
    if (strcmp(name, "TF") == 0 || strcmp(name, "CF") == 0) return 3;
    if (strcmp(name, "SI") == 0 || strcmp(name, "CI") == 0) return 2;
    if (strcmp(name, "TI") == 0) return 1;
    if (strcmp(name, "BI") == 0) return 4;
    return -1;
}
static ST_int mock_colsof(char *name)
{
    ST_int rows = 0;
    ST_int columns = 0;
    if (mock_result_matrix(name, &rows, &columns) != NULL) return columns;
    if (strcmp(name, "S") == 0) return 2;
    if (strcmp(name, "T") == 0) return 8;
    if (strcmp(name, "C") == 0) return 13;
    if (strcmp(name, "A") == 0 || strcmp(name, "AC") == 0) return 4;
    if (strcmp(name, "CT") == 0) return 2;
    if (strcmp(name, "CC") == 0) return 3;
    if (strcmp(name, "B") == 0) return 3;
    if (strcmp(name, "BW") == 0) return 3;
    if (strcmp(name, "BF") == 0) return 3;
    if (strcmp(name, "BP") == 0) return 3;
    if (strcmp(name, "TF") == 0) return 8;
    if (strcmp(name, "CF") == 0) return 13;
    if (strcmp(name, "SI") == 0) return 2;
    if (strcmp(name, "TI") == 0) return 8;
    if (strcmp(name, "CI") == 0) return 13;
    if (strcmp(name, "BI") == 0) return 3;
    return -1;
}
static ST_int mock_mat_el(char *name, ST_int row, ST_int column, ST_double *value)
{
    ST_int result_rows = 0;
    ST_int result_columns = 0;
    ST_double *result = mock_result_matrix(name, &result_rows, &result_columns);
    if (value == NULL || row < 1 || column < 1) return 111;
    if (result != NULL && row <= result_rows && column <= result_columns) {
        *value = result[(row - 1) * result_columns + column - 1];
        return 0;
    }
    if (strcmp(name, "S") == 0 && row == 1 && column <= 2) {
        *value = source_matrix[0][column - 1];
        return 0;
    }
    if (strcmp(name, "T") == 0 && row == 1 && column <= 8) {
        *value = term_matrix[0][column - 1];
        return 0;
    }
    if (strcmp(name, "C") == 0 && row == 1 && column <= 13) {
        *value = component_matrix[0][column - 1];
        return 0;
    }
    if (strcmp(name, "A") == 0 && row == 1 && column <= 4) {
        *value = empty_absorb_term_matrix[0][column - 1];
        return 0;
    }
    if (strcmp(name, "AC") == 0 && row == 1 && column <= 4) {
        *value = empty_absorb_component_matrix[0][column - 1];
        return 0;
    }
    if (strcmp(name, "CT") == 0 && row == 1 && column <= 2) {
        *value = 0.0;
        return 0;
    }
    if (strcmp(name, "CC") == 0 && row == 1 && column <= 3) {
        *value = 0.0;
        return 0;
    }
    if (strcmp(name, "TF") == 0 && row <= 3 && column <= 8) {
        *value = factor_term_matrix[row - 1][column - 1];
        return 0;
    }
    if (strcmp(name, "CF") == 0 && row <= 3 && column <= 13) {
        *value = factor_component_matrix[row - 1][column - 1];
        return 0;
    }
    if (strcmp(name, "SI") == 0 && row <= 2 && column <= 2) {
        *value = interaction_source_matrix[row - 1][column - 1];
        return 0;
    }
    if (strcmp(name, "TI") == 0 && row == 1 && column <= 8) {
        *value = interaction_term_matrix[0][column - 1];
        return 0;
    }
    if (strcmp(name, "CI") == 0 && row <= 2 && column <= 13) {
        *value = interaction_component_matrix[row - 1][column - 1];
        return 0;
    }
    if (strcmp(name, "B") == 0 && row <= 3 && column <= 3) {
        *value = binding_matrix[row - 1][column - 1];
        return 0;
    }
    if (strcmp(name, "BW") == 0 && row <= 4 && column <= 3) {
        *value = analytic_weight_binding_matrix[row - 1][column - 1];
        return 0;
    }
    if (strcmp(name, "BF") == 0 && row <= 4 && column <= 3) {
        *value = frequency_weight_binding_matrix[row - 1][column - 1];
        return 0;
    }
    if (strcmp(name, "BP") == 0 && row <= 4 && column <= 3) {
        *value = probability_weight_binding_matrix[row - 1][column - 1];
        return 0;
    }
    if (strcmp(name, "BI") == 0 && row <= 4 && column <= 3) {
        *value = interaction_binding_matrix[row - 1][column - 1];
        return 0;
    }
    return 111;
}

static ST_int mock_mat_store(
    char *name,
    ST_int row,
    ST_int column,
    ST_double value
)
{
    ST_int name_index = result_matrix_name_index(name);
    ST_int rows = 0;
    ST_int columns = 0;
    ST_double *matrix = mock_result_matrix(name, &rows, &columns);
    if (name_index < 0 || matrix == NULL || row < 1 || row > rows ||
        column < 1 || column > columns) {
        return 111;
    }
    if (prepared_store_name_pointers[name_index] == NULL) {
        prepared_store_name_pointers[name_index] = name;
    } else if (prepared_store_name_pointers[name_index] != name) {
        return 913;
    }
    output_store_calls += 1;
    last_store_was_complete = 0;
    if (fail_output_store_at == output_store_calls) return 912;
    matrix[(row - 1) * columns + column - 1] = value;
    if (strcmp(name, "RREC") == 0 && row == 1 && column == 16 && value == 1.0) {
        last_store_was_complete = 1;
    }
    return 0;
}

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
        strstr(display_buffer, "stfixest.action_protocol=1.1") == NULL ||
        strstr(display_buffer, "stfixest.model_wire=2.1") == NULL ||
        strstr(display_buffer, "stfixest.ols_result_wire=1.0") == NULL ||
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

static int check_estimator_actions_disabled(stata_call_function call)
{
    char *ols[] = {"estimate-ols"};
    char *ppml[] = {"estimate-ppml"};
    clear_output();
    if (call(1, ols) != 498 || strstr(error_buffer, "unsupported") == NULL) return 1;
    clear_output();
    if (call(1, ppml) != 498 || strstr(error_buffer, "unsupported") == NULL) return 2;
    return 0;
}

static int check_prepare_ols_test(stata_call_function call)
{
    char *arguments[] = {
        "prepare-ols-test",
        "ado=0.1.0-phase01",
        "protocol_major=1",
        "protocol_minor=0",
        "stata=18.0",
        "confirm=developer-preparation-test",
        "model_major=2",
        "model_minor=1",
        "source_matrix=S",
        "source_count=1",
        "term_matrix=T",
        "term_count=1",
        "component_matrix=C",
        "component_count=1",
        "source_names=1:x",
        "canonical_tokens=3:c.x",
        "raw_component_operators=1:c",
        "raw_time_series_operators=0:",
        "absorb_term_matrix=A",
        "absorb_term_count=0",
        "absorb_component_matrix=AC",
        "absorb_component_count=0",
        "absorb_labels=.",
        "absorb_saved_names=.",
        "binding_matrix=B",
        "binding_count=3",
        "include_intercept=1",
        "keep_singletons=0",
        "run_borrowed_map=1",
        "output_original_observation=4",
        "output_retained=5",
        "output_drop_reason=6"
    };
    ST_int row;
    plugin_nvars = 6;
    for (row = 0; row < 5; ++row) {
        plugin_data[3][row] = NAN;
        plugin_data[4][row] = NAN;
        plugin_data[5][row] = NAN;
    }
    plugin_data[0][0] = 0.0;
    plugin_data[0][1] = 1.0;
    plugin_data[0][2] = 1.0;
    plugin_data[0][3] = 0.0;
    plugin_data[0][4] = 1.0;
    plugin_data[1][1] = -0.0;
    plugin_data[1][2] = 3.0;
    plugin_data[1][4] = 5.0;
    plugin_data[2][1] = 2.0;
    plugin_data[2][2] = NAN;
    plugin_data[2][4] = 4.0;
    clear_output();
    if (call((int) (sizeof(arguments) / sizeof(arguments[0])), arguments) != 0) return 1;
    if (strstr(display_buffer, "stfixest.prepare_ols_test=pass") == NULL ||
        strstr(display_buffer, "stfixest.prepare_ols.n_retained=2") == NULL ||
        strstr(display_buffer, "stfixest.prepare_ols.map_checked=1") == NULL ||
        strstr(display_buffer, "stfixest.prepare_ols.map_iterations=1") == NULL ||
        strstr(display_buffer, "stfixest.prepare_ols.map_stop=certified") == NULL ||
        error_buffer[0] != '\0') {
        return 2;
    }
    for (row = 0; row < 5; ++row) {
        if (plugin_data[3][row] != (ST_double) (row + 1)) return 3;
    }
    if (plugin_data[4][0] != 0.0 || plugin_data[4][1] != 1.0 ||
        plugin_data[4][2] != 0.0 || plugin_data[4][3] != 0.0 ||
        plugin_data[4][4] != 1.0) {
        return 4;
    }
    if (plugin_data[5][0] != 1.0 || plugin_data[5][1] != 0.0 ||
        plugin_data[5][2] != 3.0 || plugin_data[5][3] != 1.0 ||
        plugin_data[5][4] != 0.0) {
        return 5;
    }
    return 0;
}

static int check_prepare_ols_factor(stata_call_function call)
{
    char *arguments[] = {
        "prepare-ols-test",
        "ado=0.1.0-phase01",
        "protocol_major=1",
        "protocol_minor=0",
        "stata=18.0",
        "confirm=developer-preparation-test",
        "model_major=2",
        "model_minor=1",
        "source_matrix=S",
        "source_count=1",
        "term_matrix=TF",
        "term_count=3",
        "component_matrix=CF",
        "component_count=3",
        "source_names=1:g",
        "canonical_tokens=4:1b.g|3:2.g|3:3.g",
        "raw_component_operators=2:1b|1:2|1:3",
        "raw_time_series_operators=0:|0:|0:",
        "absorb_term_matrix=A",
        "absorb_term_count=0",
        "absorb_component_matrix=AC",
        "absorb_component_count=0",
        "absorb_labels=.",
        "absorb_saved_names=.",
        "binding_matrix=B",
        "binding_count=3",
        "include_intercept=1",
        "keep_singletons=0",
        "output_original_observation=4",
        "output_retained=5",
        "output_drop_reason=6"
    };
    ST_int row;
    plugin_nvars = 6;
    for (row = 0; row < 5; ++row) {
        plugin_data[3][row] = NAN;
        plugin_data[4][row] = NAN;
        plugin_data[5][row] = NAN;
    }
    plugin_data[0][0] = 0.0;
    plugin_data[0][1] = 1.0;
    plugin_data[0][2] = 1.0;
    plugin_data[0][3] = 0.0;
    plugin_data[0][4] = 1.0;
    plugin_data[1][1] = 2.0;
    plugin_data[1][2] = 3.0;
    plugin_data[1][4] = 5.0;
    plugin_data[2][0] = -1.5;
    plugin_data[2][1] = 1.0;
    plugin_data[2][2] = NAN;
    plugin_data[2][4] = 3.0;
    clear_output();
    if (call((int) (sizeof(arguments) / sizeof(arguments[0])), arguments) != 0) return 1;
    if (strstr(display_buffer, "stfixest.prepare_ols.n_retained=2") == NULL ||
        strstr(display_buffer, "stfixest.prepare_ols.n_columns=3") == NULL ||
        strstr(display_buffer, "stfixest.prepare_ols.n_coefficient_slots=4") == NULL ||
        strstr(display_buffer, "stfixest.prepare_ols.n_categorical_sources=1") == NULL ||
        error_buffer[0] != '\0') {
        return 2;
    }
    if (plugin_data[5][0] != 1.0 || plugin_data[5][1] != 0.0 ||
        plugin_data[5][2] != 3.0 || plugin_data[5][3] != 1.0 ||
        plugin_data[5][4] != 0.0) {
        return 3;
    }
    return 0;
}

static int check_prepare_ols_categorical_interaction(stata_call_function call)
{
    char *arguments[] = {
        "prepare-ols-test",
        "ado=0.1.0-phase01",
        "protocol_major=1",
        "protocol_minor=0",
        "stata=18.0",
        "confirm=developer-preparation-test",
        "model_major=2",
        "model_minor=1",
        "source_matrix=SI",
        "source_count=2",
        "term_matrix=TI",
        "term_count=1",
        "component_matrix=CI",
        "component_count=2",
        "source_names=1:g|1:h",
        "canonical_tokens=7:2.g#1.h",
        "raw_component_operators=1:2|1:1",
        "raw_time_series_operators=0:|0:",
        "absorb_term_matrix=A",
        "absorb_term_count=0",
        "absorb_component_matrix=AC",
        "absorb_component_count=0",
        "absorb_labels=.",
        "absorb_saved_names=.",
        "binding_matrix=BI",
        "binding_count=4",
        "include_intercept=1",
        "keep_singletons=0",
        "output_original_observation=5",
        "output_retained=6",
        "output_drop_reason=7"
    };
    ST_int row;
    ST_int variable;
    plugin_nvars = 7;
    memset(plugin_read_counts, 0, sizeof(plugin_read_counts));
    for (row = 0; row < 5; ++row) {
        plugin_data[4][row] = NAN;
        plugin_data[5][row] = NAN;
        plugin_data[6][row] = NAN;
    }
    plugin_data[0][0] = 0.0;
    plugin_data[0][1] = 1.0;
    plugin_data[0][2] = 1.0;
    plugin_data[0][3] = 0.0;
    plugin_data[0][4] = 1.0;
    plugin_data[1][1] = 2.0;
    plugin_data[1][2] = 3.0;
    plugin_data[1][4] = 5.0;
    plugin_data[2][0] = -1.0;
    plugin_data[2][1] = 2.0;
    plugin_data[2][2] = 1.0;
    plugin_data[2][4] = 2.0;
    plugin_data[3][1] = 1.0;
    plugin_data[3][2] = NAN;
    plugin_data[3][4] = 2.0;
    clear_output();
    if (call((int) (sizeof(arguments) / sizeof(arguments[0])), arguments) != 0) return 1;
    if (strstr(display_buffer, "stfixest.prepare_ols.n_retained=2") == NULL ||
        strstr(display_buffer, "stfixest.prepare_ols.n_columns=2") == NULL ||
        strstr(display_buffer, "stfixest.prepare_ols.n_categorical_sources=2") == NULL ||
        error_buffer[0] != '\0') {
        return 2;
    }
    for (variable = 2; variable <= 4; ++variable) {
        if (plugin_read_counts[variable - 1][0] != 0 ||
            plugin_read_counts[variable - 1][1] != 1 ||
            plugin_read_counts[variable - 1][2] != 1 ||
            plugin_read_counts[variable - 1][3] != 0 ||
            plugin_read_counts[variable - 1][4] != 1) {
            return 3;
        }
    }
    if (plugin_data[6][0] != 1.0 || plugin_data[6][1] != 0.0 ||
        plugin_data[6][2] != 3.0 || plugin_data[6][3] != 1.0 ||
        plugin_data[6][4] != 0.0) {
        return 4;
    }

    for (row = 0; row < 5; ++row) {
        plugin_data[4][row] = NAN;
        plugin_data[5][row] = NAN;
        plugin_data[6][row] = NAN;
    }
    plugin_data[2][1] = -1.0;
    clear_output();
    if (call((int) (sizeof(arguments) / sizeof(arguments[0])), arguments) != 459) return 5;
    for (row = 0; row < 5; ++row) {
        if (!isnan(plugin_data[4][row]) || !isnan(plugin_data[5][row]) ||
            !isnan(plugin_data[6][row])) {
            return 6;
        }
    }
    return 0;
}

static int check_prepare_ols_aweight(stata_call_function call)
{
    char *arguments[] = {
        "prepare-ols-test",
        "ado=0.1.0-phase01",
        "protocol_major=1",
        "protocol_minor=0",
        "stata=18.0",
        "confirm=developer-preparation-test",
        "model_major=2",
        "model_minor=1",
        "source_matrix=S",
        "source_count=1",
        "term_matrix=T",
        "term_count=1",
        "component_matrix=C",
        "component_count=1",
        "source_names=1:x",
        "canonical_tokens=3:c.x",
        "raw_component_operators=1:c",
        "raw_time_series_operators=0:",
        "absorb_term_matrix=A",
        "absorb_term_count=0",
        "absorb_component_matrix=AC",
        "absorb_component_count=0",
        "absorb_labels=.",
        "absorb_saved_names=.",
        "binding_matrix=BW",
        "binding_count=4",
        "include_intercept=1",
        "keep_singletons=0",
        "output_original_observation=5",
        "output_retained=6",
        "output_drop_reason=7"
    };
    ST_int row;
    plugin_nvars = 7;
    for (row = 0; row < 5; ++row) {
        plugin_data[4][row] = NAN;
        plugin_data[5][row] = NAN;
        plugin_data[6][row] = NAN;
    }
    plugin_data[0][0] = 0.0;
    plugin_data[0][1] = 1.0;
    plugin_data[0][2] = 1.0;
    plugin_data[0][3] = 0.0;
    plugin_data[0][4] = 1.0;
    plugin_data[1][1] = 2.0;
    plugin_data[1][2] = 3.0;
    plugin_data[1][4] = 5.0;
    plugin_data[2][1] = 2.0;
    plugin_data[2][2] = 3.0;
    plugin_data[2][4] = 4.0;
    plugin_data[3][0] = -1.0;
    plugin_data[3][1] = 2.0;
    plugin_data[3][2] = 0.0;
    plugin_data[3][4] = NAN;
    clear_output();
    if (call((int) (sizeof(arguments) / sizeof(arguments[0])), arguments) != 0) return 1;
    if (strstr(display_buffer, "stfixest.prepare_ols.n_retained=1") == NULL ||
        error_buffer[0] != '\0') {
        return 2;
    }
    if (plugin_data[5][0] != 0.0 || plugin_data[5][1] != 1.0 ||
        plugin_data[5][2] != 0.0 || plugin_data[5][3] != 0.0 ||
        plugin_data[5][4] != 0.0) {
        return 3;
    }
    if (plugin_data[6][0] != 1.0 || plugin_data[6][1] != 0.0 ||
        plugin_data[6][2] != 4.0 || plugin_data[6][3] != 1.0 ||
        plugin_data[6][4] != 4.0) {
        return 4;
    }

    for (row = 0; row < 5; ++row) {
        plugin_data[4][row] = NAN;
        plugin_data[5][row] = NAN;
        plugin_data[6][row] = NAN;
    }
    plugin_data[0][0] = 1.0;
    clear_output();
    if (call((int) (sizeof(arguments) / sizeof(arguments[0])), arguments) != 402) return 5;
    if (strstr(error_buffer, "analytic weight") == NULL) return 6;
    for (row = 0; row < 5; ++row) {
        if (!isnan(plugin_data[4][row]) || !isnan(plugin_data[5][row]) ||
            !isnan(plugin_data[6][row])) {
            return 7;
        }
    }
    return 0;
}

static int check_prepare_ols_fweight(stata_call_function call)
{
    char *arguments[] = {
        "prepare-ols-test",
        "ado=0.1.0-phase01",
        "protocol_major=1",
        "protocol_minor=0",
        "stata=18.0",
        "confirm=developer-preparation-test",
        "model_major=2",
        "model_minor=1",
        "source_matrix=S",
        "source_count=1",
        "term_matrix=T",
        "term_count=1",
        "component_matrix=C",
        "component_count=1",
        "source_names=1:x",
        "canonical_tokens=3:c.x",
        "raw_component_operators=1:c",
        "raw_time_series_operators=0:",
        "absorb_term_matrix=A",
        "absorb_term_count=0",
        "absorb_component_matrix=AC",
        "absorb_component_count=0",
        "absorb_labels=.",
        "absorb_saved_names=.",
        "binding_matrix=BF",
        "binding_count=4",
        "include_intercept=1",
        "keep_singletons=0",
        "output_original_observation=5",
        "output_retained=6",
        "output_drop_reason=7"
    };
    ST_int row;
    plugin_nvars = 7;
    for (row = 0; row < 5; ++row) {
        plugin_data[4][row] = NAN;
        plugin_data[5][row] = NAN;
        plugin_data[6][row] = NAN;
    }
    plugin_data[0][0] = 0.0;
    plugin_data[0][1] = 1.0;
    plugin_data[0][2] = 1.0;
    plugin_data[0][3] = 0.0;
    plugin_data[0][4] = 1.0;
    plugin_data[1][1] = 2.0;
    plugin_data[1][2] = 3.0;
    plugin_data[1][4] = 5.0;
    plugin_data[2][1] = 2.0;
    plugin_data[2][2] = 3.0;
    plugin_data[2][4] = 4.0;
    plugin_data[3][0] = 1.5;
    plugin_data[3][1] = 2.0;
    plugin_data[3][2] = 3.0;
    plugin_data[3][4] = 4.0;
    clear_output();
    if (call((int) (sizeof(arguments) / sizeof(arguments[0])), arguments) != 0) return 1;
    if (strstr(display_buffer, "stfixest.prepare_ols.n_retained=3") == NULL ||
        error_buffer[0] != '\0') {
        return 2;
    }
    if (plugin_data[5][0] != 0.0 || plugin_data[5][1] != 1.0 ||
        plugin_data[5][2] != 1.0 || plugin_data[5][3] != 0.0 ||
        plugin_data[5][4] != 1.0) {
        return 3;
    }

    for (row = 0; row < 5; ++row) {
        plugin_data[4][row] = NAN;
        plugin_data[5][row] = NAN;
        plugin_data[6][row] = NAN;
    }
    plugin_data[3][1] = INFINITY;
    clear_output();
    if (call((int) (sizeof(arguments) / sizeof(arguments[0])), arguments) != 401) return 4;
    if (strstr(error_buffer, "frequency weight") == NULL) return 5;
    for (row = 0; row < 5; ++row) {
        if (!isnan(plugin_data[4][row]) || !isnan(plugin_data[5][row]) ||
            !isnan(plugin_data[6][row])) {
            return 6;
        }
    }
    return 0;
}

static int check_prepare_ols_pweight(stata_call_function call)
{
    char *arguments[] = {
        "prepare-ols-test",
        "ado=0.1.0-phase01",
        "protocol_major=1",
        "protocol_minor=0",
        "stata=18.0",
        "confirm=developer-preparation-test",
        "model_major=2",
        "model_minor=1",
        "source_matrix=S",
        "source_count=1",
        "term_matrix=T",
        "term_count=1",
        "component_matrix=C",
        "component_count=1",
        "source_names=1:x",
        "canonical_tokens=3:c.x",
        "raw_component_operators=1:c",
        "raw_time_series_operators=0:",
        "absorb_term_matrix=A",
        "absorb_term_count=0",
        "absorb_component_matrix=AC",
        "absorb_component_count=0",
        "absorb_labels=.",
        "absorb_saved_names=.",
        "binding_matrix=BP",
        "binding_count=4",
        "include_intercept=1",
        "keep_singletons=0",
        "output_original_observation=5",
        "output_retained=6",
        "output_drop_reason=7"
    };
    ST_int row;
    plugin_nvars = 7;
    for (row = 0; row < 5; ++row) {
        plugin_data[4][row] = NAN;
        plugin_data[5][row] = NAN;
        plugin_data[6][row] = NAN;
    }
    plugin_data[0][0] = 0.0;
    plugin_data[0][1] = 1.0;
    plugin_data[0][2] = 1.0;
    plugin_data[0][3] = 0.0;
    plugin_data[0][4] = 1.0;
    plugin_data[1][1] = 2.0;
    plugin_data[1][2] = 3.0;
    plugin_data[1][4] = 5.0;
    plugin_data[2][1] = 2.0;
    plugin_data[2][2] = 3.0;
    plugin_data[2][4] = 4.0;
    plugin_data[3][0] = -1.0;
    plugin_data[3][1] = 0.8;
    plugin_data[3][2] = 1.1;
    plugin_data[3][4] = 1.4;
    clear_output();
    if (call((int) (sizeof(arguments) / sizeof(arguments[0])), arguments) != 0) return 1;
    if (strstr(display_buffer, "stfixest.prepare_ols.n_retained=3") == NULL ||
        error_buffer[0] != '\0') {
        return 2;
    }
    if (plugin_data[5][0] != 0.0 || plugin_data[5][1] != 1.0 ||
        plugin_data[5][2] != 1.0 || plugin_data[5][3] != 0.0 ||
        plugin_data[5][4] != 1.0) {
        return 3;
    }

    for (row = 0; row < 5; ++row) {
        plugin_data[4][row] = NAN;
        plugin_data[5][row] = NAN;
        plugin_data[6][row] = NAN;
    }
    plugin_data[3][1] = 0.0;
    clear_output();
    if (call((int) (sizeof(arguments) / sizeof(arguments[0])), arguments) != 459) return 4;
    if (strstr(error_buffer, "probability weight") == NULL) return 5;
    for (row = 0; row < 5; ++row) {
        if (!isnan(plugin_data[4][row]) || !isnan(plugin_data[5][row]) ||
            !isnan(plugin_data[6][row])) {
            return 6;
        }
    }
    return 0;
}

static void reset_estimate_outputs(void)
{
    ST_int row;
    memset(result_b, 0, sizeof(result_b));
    memset(result_v, 0, sizeof(result_v));
    memset(result_status, 0, sizeof(result_status));
    memset(result_fit, 0, sizeof(result_fit));
    memset(result_dof, 0, sizeof(result_dof));
    memset(result_solver, 0, sizeof(result_solver));
    memset(result_fe, 0, sizeof(result_fe));
    memset(result_cluster, 0, sizeof(result_cluster));
    memset(result_subset, 0, sizeof(result_subset));
    memset(result_certificate, 0, sizeof(result_certificate));
    memset(result_receipt, 0, sizeof(result_receipt));
    output_store_calls = 0;
    fail_output_store_at = 0;
    last_store_was_complete = 0;
    memset(prepared_store_name_pointers, 0, sizeof(prepared_store_name_pointers));
    plugin_nvars = 6;
    for (row = 0; row < 5; ++row) {
        plugin_data[0][row] = 1.0;
        plugin_data[2][row] = (ST_double) row;
        plugin_data[3][row] = NAN;
        plugin_data[4][row] = NAN;
        plugin_data[5][row] = NAN;
    }
    plugin_data[1][0] = 1.0;
    plugin_data[1][1] = 3.0;
    plugin_data[1][2] = 2.0;
    plugin_data[1][3] = 7.0;
    plugin_data[1][4] = 4.0;
}

static ST_int call_estimate_ols_test(stata_call_function call)
{
    char *arguments[] = {
        "estimate-ols-test",
        "ado=0.1.0-phase01",
        "protocol_major=1",
        "protocol_minor=1",
        "stata=18.0",
        "confirm=developer-estimation-test",
        "plugin=0.1.0",
        "result_major=1",
        "result_minor=0",
        "model_major=2",
        "model_minor=1",
        "source_matrix=S",
        "source_count=1",
        "term_matrix=T",
        "term_count=1",
        "component_matrix=C",
        "component_count=1",
        "source_names=1:x",
        "canonical_tokens=3:c.x",
        "raw_component_operators=1:c",
        "raw_time_series_operators=0:",
        "absorb_term_matrix=A",
        "absorb_term_count=0",
        "absorb_component_matrix=AC",
        "absorb_component_count=0",
        "absorb_labels=.",
        "absorb_saved_names=.",
        "cluster_term_matrix=CT",
        "cluster_term_count=0",
        "cluster_component_matrix=CC",
        "cluster_component_count=0",
        "cluster_labels=.",
        "binding_matrix=B",
        "binding_count=3",
        "include_intercept=1",
        "keep_singletons=0",
        "vce=iid",
        "dof=pairwise",
        "account_cluster_nesting=0",
        "output_sample=4",
        "output_residual=5",
        "output_drop_reason=6",
        "output_b_matrix=RB",
        "output_v_matrix=RV",
        "output_status_matrix=RSTATUS",
        "output_fit_matrix=RFIT",
        "output_dof_matrix=RDOF",
        "output_solver_matrix=RSOLVER",
        "output_fe_matrix=RFE",
        "output_cluster_matrix=RCLUSTER",
        "output_subset_matrix=RSUBSET",
        "output_certificate_matrix=RCERT",
        "output_receipt_matrix=RREC"
    };
    return call((int) (sizeof(arguments) / sizeof(arguments[0])), arguments);
}

static int check_estimate_ols_atomic_store(stata_call_function call)
{
    ST_int ordinal;
    ST_int name_index;
    reset_estimate_outputs();
    clear_output();
    if (call_estimate_ols_test(call) != 0) return 1;
    if (error_buffer[0] != '\0' || result_receipt[0][0] != 5354601.0 ||
        result_receipt[0][15] != 1.0 || !last_store_was_complete) {
        return 2;
    }
    if (output_store_calls != 213) return 3;
    for (name_index = 0; name_index < 11; ++name_index) {
        if (prepared_store_name_pointers[name_index] == NULL) return 8;
    }
    if (!isfinite(result_b[0][0]) || !isfinite(result_b[0][1]) ||
        !isfinite(result_v[0][0]) || !isfinite(result_v[1][1])) {
        return 4;
    }

    for (ordinal = 1; ordinal <= 213; ++ordinal) {
        reset_estimate_outputs();
        fail_output_store_at = ordinal;
        clear_output();
        if (call_estimate_ols_test(call) != 3499) return 5;
        if (strstr(error_buffer, "SPI") == NULL || output_store_calls != ordinal) return 6;
        if (result_receipt[0][15] != 0.0 || last_store_was_complete) return 7;
    }
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
    plugin.pollstd = mock_poll;
    plugin.nobs = mock_nobs;
    plugin.nvars = mock_nvars;
    plugin.nvar = mock_nvars;
    plugin.isstr = mock_isstr;
    plugin.ismissing = mock_missing;
    plugin.safevdata = mock_vdata;
    plugin.safestore = mock_vstore;
    plugin.rowsof = mock_rowsof;
    plugin.colsof = mock_colsof;
    plugin.safematel = mock_mat_el;
    plugin.safematstore = mock_mat_store;
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
    result = check_estimator_actions_disabled(call);
    if (result != 0) {
        fprintf(stderr, "disabled estimator path failed (%d): %s / %s\n", result, display_buffer, error_buffer);
        dlclose(library);
        return 50 + result;
    }
    result = check_prepare_ols_test(call);
    if (result != 0) {
        fprintf(stderr, "prepare OLS test path failed (%d): %s / %s\n", result, display_buffer, error_buffer);
        dlclose(library);
        return 60 + result;
    }
    result = check_prepare_ols_aweight(call);
    if (result != 0) {
        fprintf(stderr, "prepare OLS aweight path failed (%d): %s / %s\n", result, display_buffer, error_buffer);
        dlclose(library);
        return 70 + result;
    }
    result = check_prepare_ols_factor(call);
    if (result != 0) {
        fprintf(stderr, "prepare OLS factor path failed (%d): %s / %s\n", result, display_buffer, error_buffer);
        dlclose(library);
        return 75 + result;
    }
    result = check_prepare_ols_categorical_interaction(call);
    if (result != 0) {
        fprintf(stderr, "prepare OLS categorical interaction path failed (%d): %s / %s\n", result, display_buffer, error_buffer);
        dlclose(library);
        return 77 + result;
    }
    result = check_prepare_ols_fweight(call);
    if (result != 0) {
        fprintf(stderr, "prepare OLS fweight path failed (%d): %s / %s\n", result, display_buffer, error_buffer);
        dlclose(library);
        return 80 + result;
    }
    result = check_prepare_ols_pweight(call);
    if (result != 0) {
        fprintf(stderr, "prepare OLS pweight path failed (%d): %s / %s\n", result, display_buffer, error_buffer);
        dlclose(library);
        return 90 + result;
    }
    result = check_estimate_ols_atomic_store(call);
    if (result != 0) {
        fprintf(stderr, "guarded estimate OLS atomic-store path failed (%d): %s / %s\n", result, display_buffer, error_buffer);
        dlclose(library);
        return 100 + result;
    }

    dlclose(library);
    puts("actual-plugin ABI/self-test harness: PASS");
    return 0;
}
