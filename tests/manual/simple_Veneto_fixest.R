#!/usr/bin/env Rscript

# Benchmark fixest on the exact samples exported by simple_Veneto_fixest.do.
# R startup and CSV loading are outside the timed region.  The native arm
# leaves fixest defaults unchanged.  The matched-controls arm changes only
# fixed-effect tolerance and maximum iterations to fereg's defaults; it does
# not change the fixest demeaning algorithm.

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 2L) {
    stop("usage: simple_Veneto_fixest.R SAMPLE_DIR OUTPUT_DIR", call. = FALSE)
}

sample_dir <- normalizePath(args[[1L]], mustWork = TRUE)
output_dir <- normalizePath(args[[2L]], mustWork = TRUE)
repetitions <- 10L
threads <- 8L

suppressPackageStartupMessages(library(fixest))
setFixest_nthreads(threads)
stopifnot(getFixest_nthreads() == threads)

read_sample <- function(filename, expected_n) {
    data <- read.csv(file.path(sample_dir, filename),
                     colClasses = c("integer", "integer", "integer", "numeric"))
    stopifnot(nrow(data) == expected_n)
    data
}

samples <- list(
    post_singleton = read_sample("veneto_post_singleton.csv", 69970L),
    largest_connected = read_sample("veneto_largest_connected.csv", 62178L)
)

fit_native <- function(data) {
    feols(outcome ~ 1 | worker + firm, data = data, nthreads = threads,
          notes = FALSE)
}

fit_matched_controls <- function(data) {
    feols(outcome ~ 1 | worker + firm, data = data, nthreads = threads,
          fixef.tol = 1e-8, fixef.iter = 16000L, notes = FALSE)
}

fitters <- list(
    fixest_native = fit_native,
    fixest_matched_controls = fit_matched_controls
)

rows <- vector("list", length(samples) * length(fitters) * repetitions)
row_index <- 0L

for (sample_name in names(samples)) {
    data <- samples[[sample_name]]
    for (fitter in fitters) invisible(fitter(data))

    for (repetition in seq_len(repetitions)) {
        arm_order <- if (repetition %% 2L == 1L) names(fitters) else rev(names(fitters))
        for (arm in arm_order) {
            start <- proc.time()[["elapsed"]]
            fit <- fitters[[arm]](data)
            elapsed_seconds <- proc.time()[["elapsed"]] - start
            row_index <- row_index + 1L
            rows[[row_index]] <- data.frame(
                sample = sample_name,
                estimator = arm,
                repetition = repetition,
                elapsed_seconds = elapsed_seconds,
                observations = nobs(fit),
                rss = deviance(fit),
                demeaning_iterations = max(fit$iterations),
                stringsAsFactors = FALSE
            )
        }
    }
}

runs <- do.call(rbind, rows)
runs <- runs[order(runs$sample, runs$estimator, runs$repetition), ]
write.csv(runs, file.path(output_dir, "fixest_runs.csv"), row.names = FALSE)

group_keys <- unique(runs[c("sample", "estimator")])
summary_rows <- lapply(seq_len(nrow(group_keys)), function(index) {
    key <- group_keys[index, ]
    selected <- runs$sample == key$sample & runs$estimator == key$estimator
    elapsed <- runs$elapsed_seconds[selected]
    data.frame(
        sample = key$sample,
        estimator = key$estimator,
        mean_seconds = mean(elapsed),
        median_seconds = median(elapsed),
        min_seconds = min(elapsed),
        max_seconds = max(elapsed),
        observations = unique(runs$observations[selected]),
        rss = unique(runs$rss[selected]),
        demeaning_iterations = unique(runs$demeaning_iterations[selected]),
        stringsAsFactors = FALSE
    )
})
summary <- do.call(rbind, summary_rows)
summary <- summary[order(summary$sample, summary$estimator), ]
write.csv(summary, file.path(output_dir, "fixest_summary.csv"), row.names = FALSE)

cat(sprintf("R_VERSION=%s\n", as.character(getRversion())))
cat(sprintf("FIXEST_VERSION=%s\n", as.character(packageVersion("fixest"))))
cat(sprintf("FIXEST_THREADS=%d\n", getFixest_nthreads()))
print(summary, row.names = FALSE)
cat(sprintf(
    "FIXEST_VENETO|PASS|full_N=%d|largest_N=%d|threads=%d|repetitions=%d\n",
    nrow(samples$post_singleton), nrow(samples$largest_connected),
    threads, repetitions
))
