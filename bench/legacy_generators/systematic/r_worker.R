args <- commandArgs(trailingOnly = TRUE)
suppressPackageStartupMessages(library(fixest))
suppressPackageStartupMessages(library(haven))
suppressPackageStartupMessages(library(jsonlite))
suppressPackageStartupMessages(library(digest))
c <- fromJSON(args[[1]], simplifyVector = TRUE)
cell <- c$cell
output <- c$output
setFixest_nthreads(cell$threads)
effect_terms <- vapply(cell$effects, function(e) {
    slopes <- cell$slopes[[e]]
    if (length(slopes)) paste0(e, "[", paste(slopes, collapse=", "), "]") else e
}, character(1))
formula <- as.formula(paste("y ~", paste(paste0("x", seq_len(cell$k)), collapse=" + "),
                            "|", paste(effect_terms, collapse=" + ")))
fit_model <- function(data) {
    if (cell$model == "ols") {
        feols(formula, data=data, nthreads=cell$threads, vcov="iid", notes=FALSE)
    } else {
        if (identical(c$arm, "fixest_strict")) {
            return(fepois(formula, data=data, nthreads=cell$threads, vcov="hetero",
                          ssc=ssc(K.adj=FALSE), notes=FALSE,
                          glm.tol=c$fixest_strict$glm_tol, fixef.tol=c$fixest_strict$fixef_tol))
        }
        fepois(formula, data=data, nthreads=cell$threads, vcov="hetero", ssc=ssc(K.adj=FALSE), notes=FALSE)
    }
}
if (c$phase == "warm") {
    cat("SYSTEMATIC_STAGE warmup\n")
    warm_error <- tryCatch({
        warm <- read_dta(c$warm)
        invisible(fit_model(warm))
        NULL
    }, error=function(e) conditionMessage(e))
    if (!is.null(warm_error)) {
        write_json(list(rc=1, error_stage="warmup", error=warm_error),
                   file.path(output,"metadata.json"), auto_unbox=TRUE, pretty=TRUE)
        cat("SYSTEMATIC_WORKER_COMPLETE rc=1\n")
        quit(status=1)
    }
    rm(warm)
    invisible(gc())
}
cat("SYSTEMATIC_STAGE input_validation\n")
data <- read_dta(c$data)
info <- fromJSON(sub("\\.dta$", ".json", c$data))
for (name in names(info$column_sha256)) {
    bytes <- writeBin(as.double(data[[name]]), raw(), size=8, endian="little")
    stopifnot(digest(bytes, algo="sha256", serialize=FALSE) == info$column_sha256[[name]])
}
rm(bytes)
invisible(gc())
writeLines("READY", file.path(output, "READY"))
Sys.sleep(.1)
writeLines("START", file.path(output, "START"))
while (!file.exists(file.path(output, "START.CHECKED"))) Sys.sleep(.001)
cat("SYSTEMATIC_STAGE estimation\n")
start <- proc.time()[["elapsed"]]
error <- NULL
fit <- tryCatch(fit_model(data), error=function(e) { error <<- conditionMessage(e); NULL })
seconds <- proc.time()[["elapsed"]] - start
writeLines("DONE", file.path(output, "DONE"))
meta <- list(rc=if (is.null(fit)) 1 else 0, seconds=seconds,
             R_version=as.character(getRversion()), fixest_version=as.character(packageVersion("fixest")),
             requested_threads=cell$threads, effective_threads=getFixest_nthreads(), error=error,
             error_stage=if (is.null(fit)) "estimation" else NULL)
if (!is.null(fit)) {
    cat("SYSTEMATIC_STAGE extraction\n")
    b <- coef(fit)
    standard_errors <- se(fit)
    correction <- if (cell$model == "ppml") nobs(fit)/(nobs(fit)-1) else 1
    standard_errors <- standard_errors * sqrt(correction)
    meta$covariance_normalization <- if (cell$model == "ppml") "HC0 multiplied by N/(N-1)" else "native IID"
    meta$covariance_multiplier <- correction
    write.csv(data.frame(name=names(b), value=unname(b), se=unname(standard_errors)),
              file.path(output,"coefficients.csv"), row.names=FALSE)
    V <- vcov(fit) * correction
    rows <- expand.grid(row=rownames(V), column=colnames(V), stringsAsFactors=FALSE)
    rows$value <- as.vector(V)
    write.csv(rows, file.path(output,"covariance.csv"), row.names=FALSE)
    meta$N <- nobs(fit)
    meta$loglik <- as.numeric(logLik(fit))
    meta$deviance <- as.numeric(deviance(fit))
    meta$converged <- if (is.null(fit$convStatus)) TRUE else fit$convStatus
    meta$df_k <- fit$nparams
    if (cell$model == "ols") meta$rss <- sum(resid(fit)^2)
    selected <- obs(fit)
    write.csv(data.frame(obs_id=data$obs_id[selected]), file.path(output,"sample.csv"), row.names=FALSE)
    meta$sample_kind <- "observation_ids"
}
write_json(meta, file.path(output,"metadata.json"), auto_unbox=TRUE, pretty=TRUE, na="null", digits=NA)
cat(sprintf("SYSTEMATIC_WORKER_COMPLETE rc=%d\n", meta$rc))
