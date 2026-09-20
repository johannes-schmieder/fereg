args <- commandArgs(trailingOnly = TRUE)
suppressPackageStartupMessages(library(fixest))

suppressPackageStartupMessages(library(jsonlite))
suppressPackageStartupMessages(library(digest))
c <- fromJSON(args[[1]], simplifyVector = TRUE)
cell <- c$cell
output <- c$output
setFixest_nthreads(cell$threads)
read_input <- function(path) {
    meta <- fromJSON(sub("\\.dta$", ".json", path))
    con <- file(sub("\\.dta$", ".bin", path), "rb")
    on.exit(close(con))
    cols <- lapply(meta$columns, function(name) readBin(con, "double", n=meta$n, size=8, endian="little"))
    names(cols) <- meta$columns
    as.data.frame(cols)
}
formula <- as.formula(paste("y ~", paste(paste0("x", seq_len(cell$k)), collapse=" + "),
                            "|", paste(cell$effects, collapse=" + ")))
fit_model <- function(data) {
    if (cell$model == "ols") {
        feols(formula, data=data, nthreads=cell$threads, vcov=if (startsWith(cell$r_vce, "~")) as.formula(cell$r_vce) else cell$r_vce,
              ssc=ssc(K.adj=TRUE, K.fixef="nonnested", G.adj=TRUE, G.df="min", t.df="min"), notes=FALSE)
    } else {
        fepois(formula, data=data, nthreads=cell$threads, vcov="hetero", ssc=ssc(K.adj=FALSE), notes=FALSE)
    }
}
recover_outputs <- function(fit) {
    if (is.null(fit) || cell$outputs == "none") return(NULL)
    # Use the fitted object's existing level codes, as fixest's own recovery
    # does, rather than recoding or character-matching a million observations.
    effects <- fixef(fit, sorted=FALSE, notes=FALSE)$g1
    contribution <- unname(effects[fit$fixef_id$g1])
    intercept <- mean(contribution)
    saved_outputs <- list(contribution - intercept)
    if (cell$outputs == "all") {
        residual <- resid(fit)
        prediction <- fitted(fit)
        finite <- prediction - contribution + intercept
        saved_outputs <- list(contribution - intercept, residual, finite, prediction)
    }
    saved_outputs
}
if (c$phase == "warm") {
    warm <- read_input(c$warm)
    invisible(recover_outputs(fit_model(warm)))
    rm(warm)
    invisible(gc())
}
data <- read_input(c$data)
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

start <- proc.time()[["elapsed"]]
error <- NULL
for (profile_repeat in seq_len(as.integer(Sys.getenv("ONE_FE_PROFILE_REPEAT", "1")))) {
 fit <- tryCatch(fit_model(data), error=function(e) { error <<- conditionMessage(e); NULL })
}
fit_seconds <- proc.time()[["elapsed"]] - start
saved_outputs <- recover_outputs(fit)
seconds <- proc.time()[["elapsed"]] - start

writeLines("DONE", file.path(output, "DONE"))
meta <- list(rc=if (is.null(fit)) 1 else 0, seconds=seconds,
             fit_seconds=fit_seconds, R_version=as.character(getRversion()), fixest_version=as.character(packageVersion("fixest")),
             requested_threads=cell$threads, effective_threads=getFixest_nthreads(), error=error)
if (!is.null(fit)) {
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
    if (!is.null(saved_outputs)) {
        con <- file(file.path(output, "outputs.bin"), "wb")
        for (values in saved_outputs) writeBin(as.double(values), con, size=8, endian="little")
        close(con)
    }
    meta$df_K_vcov <- attr(vcov(fit, attr=TRUE), "df.K")
    meta$G <- attr(vcov(fit, attr=TRUE), "G")
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
if (!is.null(fit$profile_phases)) {
    write_json(list(phases=fit$profile_phases, ols=fit$profile_ols, demean=as.list(fit$profile_demean)),
               file.path(output,"profile.json"), auto_unbox=TRUE, digits=NA)
}
write_json(meta, file.path(output,"metadata.json"), auto_unbox=TRUE, pretty=TRUE, na="null", digits=NA)
cat(sprintf("SYSTEMATIC_WORKER_COMPLETE rc=%d\n", meta$rc))
