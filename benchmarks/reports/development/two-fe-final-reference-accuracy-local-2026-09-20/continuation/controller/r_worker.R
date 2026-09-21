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
formula <- as.formula(paste("y ~", if (cell$k == 0) "1" else paste(paste0("x", seq_len(cell$k)), collapse=" + "),
                            "|", paste(cell$effects, collapse=" + ")))
fit_model <- function(data) {
    if (cell$model == "ols") {
        if (cell$weight_kind == "none") {
feols(formula, data=data, nthreads=cell$threads, fixef.tol=cell$fixest_tolerance, vcov=if (startsWith(cell$r_vce, "~")) as.formula(cell$r_vce) else cell$r_vce,
              ssc=ssc(K.adj=TRUE, K.fixef=cell$reference_fixef_dof, K.exact=isTRUE(cell$exact_dof), G.adj=TRUE, G.df="min", t.df="min"), notes=FALSE)
} else {
feols(formula, data=data, weights=~w, nthreads=cell$threads, fixef.tol=cell$fixest_tolerance, vcov=if (startsWith(cell$r_vce, "~")) as.formula(cell$r_vce) else cell$r_vce,
              ssc=ssc(K.adj=TRUE, K.fixef=cell$reference_fixef_dof, K.exact=isTRUE(cell$exact_dof), G.adj=TRUE, G.df="min", t.df="min"), notes=FALSE)
}
    } else {
        fepois(formula, data=data, nthreads=cell$threads, vcov="hetero", ssc=ssc(K.adj=FALSE), notes=FALSE)
    }
}
recover_outputs <- function(fit, data) {
    if (is.null(fit) || cell$outputs == "none") return(NULL)
    effects <- fixef(fit, sorted=FALSE, notes=FALSE, fixef.tol=cell$fixest_recovery_tolerance)
    left <- unname(effects$g1[fit$fixef_id$g1])
    if (length(cell$effects) == 1) {
        intercept <- if (cell$weight_kind == "none") mean(left) else weighted.mean(left, data$w[obs(fit)])
        saved_outputs <- list(left - intercept)
        if (cell$outputs == "all") saved_outputs <- c(saved_outputs,
            list(resid(fit), fitted(fit) - left + intercept, fitted(fit)))
        return(saved_outputs)
    }
    right <- unname(effects$g2[fit$fixef_id$g2])
    contribution <- left + right
    component <- as.integer(data$component[obs(fit)])
    if (cell$weight_kind == "none") {
        intercept <- mean(contribution)
        centers <- as.vector(rowsum(right, component, reorder=FALSE)) /
                   as.vector(rowsum(rep(1, length(right)), component, reorder=FALSE))
    } else {
        weight <- data$w[obs(fit)]
        intercept <- weighted.mean(contribution, weight)
        centers <- as.vector(rowsum(weight * right, component, reorder=FALSE)) /
                   as.vector(rowsum(weight, component, reorder=FALSE))
    }
    centers <- centers[match(component, unique(component))]
    saved_outputs <- list(left + centers - intercept, right - centers)
    if (cell$outputs == "all") {
        residual <- resid(fit)
        prediction <- fitted(fit)
        finite <- prediction - contribution + intercept
        saved_outputs <- c(saved_outputs, list(residual, finite, prediction))
    }
    saved_outputs
}
if (c$phase == "warm") {
    warm <- read_input(c$warm)
    invisible(recover_outputs(fit_model(warm), warm))
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
saved_outputs <- recover_outputs(fit, data)
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
    meta$covariance_normalization <- paste("native", cell$r_vce, paste("with explicit", cell$reference_fixef_dof, "K/min-G SSC"))
    meta$covariance_multiplier <- correction
    write.csv(data.frame(name=as.character(names(b)), value=as.double(b), se=as.double(standard_errors)),
              file.path(output,"coefficients.csv"), row.names=FALSE)
    V <- vcov(fit) * correction
    rows <- expand.grid(row=as.character(rownames(V)), column=as.character(colnames(V)), stringsAsFactors=FALSE)
    rows$value <- as.double(V)
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
    if (cell$model == "ols") meta$rss <- if (cell$weight_kind == "none") sum(resid(fit)^2) else sum(data$w[obs(fit)] * resid(fit)^2) * nobs(fit) / sum(data$w[obs(fit)])
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
