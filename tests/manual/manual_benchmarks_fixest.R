#!/usr/bin/env Rscript

# Compare R fixest with the fereg(auto) cells produced by
# manual_benchmarks.do. Dataset loading and package startup are excluded from
# the timed region. The matched-controls arm changes only FE tolerance and
# maximum iterations; it does not change fixest's demeaning algorithm.

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 3L) {
    stop(paste(
        "usage: manual_benchmarks_fixest.R",
        "MANUAL_BENCHMARK_RUNS_CSV DATASET_DIR OUTPUT_DIR"
    ), call. = FALSE)
}

benchmark_path <- normalizePath(args[[1L]], mustWork = TRUE)
dataset_dir <- normalizePath(args[[2L]], mustWork = TRUE)
output_dir <- normalizePath(args[[3L]], mustWork = TRUE)

suppressPackageStartupMessages(library(fixest))

benchmark_runs <- read.csv(benchmark_path, stringsAsFactors = FALSE)
fereg_runs <- subset(benchmark_runs, command == "fereg" & status == "pass")
if (nrow(fereg_runs) == 0L) stop("no successful fereg rows", call. = FALSE)

cell_columns <- c(
    "problem_type", "topology", "fe_dimensions", "base_n",
    "n_observations", "controls", "requested_threads"
)
cells <- unique(fereg_runs[cell_columns])
cells <- cells[order(cells$problem_type, cells$n_observations,
                     cells$requested_threads), ]

expected_problems <- c("two_fe_dense", "two_fe_sparse", "three_fe_sparse")
stopifnot(setequal(unique(cells$problem_type), expected_problems))
stopifnot(nrow(cells) == 27L)
stopifnot(all(cells$controls == 4L))

make_formula <- function(problem_type) {
    rhs <- "x1 + x2 + x3 + x4"
    fixed_effects <- switch(
        problem_type,
        two_fe_dense = "worker + firm",
        two_fe_sparse = "worker + firm",
        three_fe_sparse = "worker + firm + county",
        stop(sprintf("unsupported problem type: %s", problem_type), call. = FALSE)
    )
    as.formula(sprintf("y ~ %s | %s", rhs, fixed_effects))
}

dataset_cache <- new.env(parent = emptyenv())
load_dataset <- function(problem_type, base_n, n_observations) {
    filename <- sprintf(
        "%s_base%d_N%d.csv", problem_type, base_n, n_observations
    )
    if (!exists(filename, envir = dataset_cache, inherits = FALSE)) {
        path <- file.path(dataset_dir, filename)
        data <- read.csv(path, check.names = FALSE)
        stopifnot(nrow(data) == n_observations)
        assign(filename, data, envir = dataset_cache)
    }
    get(filename, envir = dataset_cache, inherits = FALSE)
}

run_rows <- vector("list", nrow(fereg_runs) * 2L)
row_index <- 0L

for (cell_index in seq_len(nrow(cells))) {
    cell <- cells[cell_index, ]
    data <- load_dataset(
        cell$problem_type, cell$base_n, cell$n_observations
    )
    formula <- make_formula(cell$problem_type)
    threads <- cell$requested_threads
    repetitions <- sum(
        fereg_runs$problem_type == cell$problem_type &
        fereg_runs$base_n == cell$base_n &
        fereg_runs$n_observations == cell$n_observations &
        fereg_runs$requested_threads == cell$requested_threads
    )
    stopifnot(repetitions == 5L)

    fitters <- list(
        fixest_native = function() {
            feols(formula, data = data, nthreads = threads, notes = FALSE)
        },
        fixest_matched_controls = function() {
            feols(
                formula, data = data, nthreads = threads,
                fixef.tol = 1e-8, fixef.iter = 16000L, notes = FALSE
            )
        }
    )

    for (fitter in fitters) invisible(fitter())

    for (repetition in seq_len(repetitions)) {
        arm_order <- if (repetition %% 2L == 1L) {
            names(fitters)
        } else {
            rev(names(fitters))
        }
        for (arm in arm_order) {
            start <- proc.time()[["elapsed"]]
            fit <- fitters[[arm]]()
            elapsed_seconds <- proc.time()[["elapsed"]] - start
            row_index <- row_index + 1L
            run_rows[[row_index]] <- data.frame(
                problem_type = cell$problem_type,
                topology = cell$topology,
                fe_dimensions = cell$fe_dimensions,
                base_n = cell$base_n,
                n_observations = cell$n_observations,
                controls = cell$controls,
                requested_threads = threads,
                estimator = arm,
                repetition = repetition,
                elapsed_seconds = elapsed_seconds,
                n_estimation = nobs(fit),
                rss = deviance(fit),
                demeaning_iterations = max(fit$iterations),
                stringsAsFactors = FALSE
            )
        }
    }
}

fixest_runs <- do.call(rbind, run_rows)
fixest_runs <- fixest_runs[order(
    fixest_runs$problem_type, fixest_runs$n_observations,
    fixest_runs$requested_threads, fixest_runs$estimator,
    fixest_runs$repetition
), ]
write.csv(
    fixest_runs, file.path(output_dir, "manual_benchmarks_fixest_runs.csv"),
    row.names = FALSE
)

summarize_runs <- function(data, estimator_column) {
    keys <- unique(data[c(cell_columns, estimator_column)])
    names(keys)[names(keys) == estimator_column] <- "estimator"
    rows <- lapply(seq_len(nrow(keys)), function(index) {
        key <- keys[index, ]
        selected <- (
            data$problem_type == key$problem_type &
            data$base_n == key$base_n &
            data$n_observations == key$n_observations &
            data$requested_threads == key$requested_threads
        )
        if (estimator_column == "estimator") {
            selected <- selected & data$estimator == key$estimator
        }
        elapsed <- data$elapsed_seconds[selected]
        data.frame(
            key,
            median_seconds = median(elapsed),
            mean_seconds = mean(elapsed),
            min_seconds = min(elapsed),
            max_seconds = max(elapsed),
            successful_runs = length(elapsed),
            stringsAsFactors = FALSE
        )
    })
    do.call(rbind, rows)
}

fixest_summary <- summarize_runs(fixest_runs, "estimator")
write.csv(
    fixest_summary,
    file.path(output_dir, "manual_benchmarks_fixest_summary.csv"),
    row.names = FALSE
)

fereg_summary_input <- fereg_runs
fereg_summary_input$estimator <- "fereg_auto"
fereg_summary <- summarize_runs(fereg_summary_input, "estimator")

combined <- rbind(fereg_summary, fixest_summary)
combined$relative_to_fereg_auto <- NA_real_
for (cell_index in seq_len(nrow(cells))) {
    cell <- cells[cell_index, ]
    selected <- (
        combined$problem_type == cell$problem_type &
        combined$base_n == cell$base_n &
        combined$n_observations == cell$n_observations &
        combined$requested_threads == cell$requested_threads
    )
    fereg_seconds <- combined$median_seconds[
        selected & combined$estimator == "fereg_auto"
    ]
    stopifnot(length(fereg_seconds) == 1L, fereg_seconds > 0)
    combined$relative_to_fereg_auto[selected] <-
        combined$median_seconds[selected] / fereg_seconds
}
combined <- combined[order(
    combined$problem_type, combined$n_observations,
    combined$requested_threads, combined$estimator
), ]
write.csv(
    combined,
    file.path(output_dir, "manual_benchmarks_fereg_fixest_summary.csv"),
    row.names = FALSE
)

cat(sprintf("R_VERSION=%s\n", as.character(getRversion())))
cat(sprintf("FIXEST_VERSION=%s\n", as.character(packageVersion("fixest"))))
cat(sprintf("CELLS=%d RUNS=%d\n", nrow(cells), nrow(fixest_runs)))
cat("FIXEST_MANUAL_BENCHMARKS|PASS\n")
