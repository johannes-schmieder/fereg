args <- commandArgs(trailingOnly=TRUE)
settings <- read.delim(args[[1]], header=FALSE, sep="\t", quote="", col.names=c("key","value"), colClasses="character")
config <- setNames(as.list(settings$value), settings$key)
.libPaths(c(config$r_library, .libPaths()))
suppressPackageStartupMessages(library(fixest))
stopifnot(as.character(packageVersion("fixest")) == "0.14.2")
data <- foreign::read.dta(config$dataset, convert.factors=FALSE)
k <- as.integer(config$k)
threads <- as.integer(config$threads)
effects <- paste0("g", seq_len(as.integer(config$effects)))
if (config$slopes == "1") effects[[1]] <- "g1[slope]"
formula <- as.formula(paste("y ~", paste(paste0("x", seq_len(k)), collapse=" + "), "|", paste(effects, collapse=" + ")))
options <- list(fml=formula, data=data, nthreads=threads, notes=FALSE)
if (config$weights == "analytic") options$weights <- ~weight
options$vcov <- switch(config$vce, iid="iid", robust="hetero", cluster=~cluster)
if (config$arm == "fixest-matched") {
    options$fixef.tol <- if (config$model == "ols") 1e-10 else 1e-11
    if (config$model == "ppml") options$glm.tol <- 1e-10
}
error <- NULL
start <- proc.time()[["elapsed"]]
fit <- tryCatch({
    result <- do.call(if (config$model == "ols") feols else fepois, options)
    if (identical(result$convStatus, FALSE)) stop("fixest did not converge")
    if (config$outputs == "1") {
        saved_fe <- fixef(result)
        saved_residual <- resid(result)
        stopifnot(length(saved_fe) > 0, length(saved_residual) == nobs(result))
    }
    result
}, error=function(e) {error <<- conditionMessage(e); NULL})
seconds <- proc.time()[["elapsed"]] - start
metadata <- c(rc=if (is.null(fit)) 1 else 0, seconds=seconds,
              fixest_version=as.character(packageVersion("fixest")), complete=1)
if (!is.null(fit)) {
    metadata <- c(metadata, N=nobs(fit))
    values <- coef(fit)[paste0("x", seq_len(k))]
    write.csv(data.frame(name=names(values), value=unname(values)),
              file.path(config$output,"coefficients.csv"), row.names=FALSE)
    write.csv(data.frame(obs_id=data$obs_id[obs(fit)]), file.path(config$output,"sample.csv"), row.names=FALSE)
}
if (!is.null(error)) metadata <- c(metadata, error=gsub("[\t\r\n]", " ", error))
write.table(data.frame(names(metadata), unname(metadata)), file.path(config$output,"metadata.tsv"),
            sep="\t", quote=FALSE, row.names=FALSE, col.names=FALSE)
cat(sprintf("FEREG_BENCH_COMPLETE rc=%d\n", if (is.null(fit)) 1 else 0))
quit(status=if (is.null(fit)) 1 else 0)
