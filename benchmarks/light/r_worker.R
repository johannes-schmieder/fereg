args <- commandArgs(trailingOnly=TRUE)
suppressPackageStartupMessages(library(fixest))
suppressPackageStartupMessages(library(haven))
suppressPackageStartupMessages(library(jsonlite))
suppressPackageStartupMessages(library(digest))
cfg <- fromJSON(args[[1]], simplifyVector=TRUE)
c <- cfg$cell
out <- cfg$output
setFixest_nthreads(cfg$threads)
fe <- if (c$slopes) "g1 + g2[z]" else paste(c$effects, collapse=" + ")
form <- as.formula(paste("y ~", paste(paste0("x", seq_len(c$k)), collapse=" + "), "|", fe))
vce <- if (length(c$clusters)) as.formula(paste("~", paste(c$clusters, collapse=" + "))) else if (c$model == "ols") "iid" else "hetero"
ssc_value <- if (c$model == "ppml") ssc(K.adj=FALSE, G.adj=TRUE, G.df="min") else ssc()
fit_model <- function(data) {
    fit <- if (c$model == "ols") feols(form, data=data, nthreads=cfg$threads, vcov=vce, ssc=ssc_value, notes=FALSE) else fepois(form, data=data, nthreads=cfg$threads, vcov=vce, ssc=ssc_value, notes=FALSE)
    saved <- if (c$save_fe) as.matrix(predict(fit, fixef=TRUE)) else NULL
    list(fit=fit, saved=saved)
}
warm <- read_dta(cfg$warm)
invisible(fit_model(warm))
rm(warm)
invisible(gc())
data <- read_dta(cfg$data)
info <- fromJSON(sub("\\.dta$", ".json", cfg$data))
for (name in names(info$column_sha256)) {
    bytes <- writeBin(as.double(data[[name]]), raw(), size=8, endian="little")
    stopifnot(digest(bytes, algo="sha256", serialize=FALSE) == info$column_sha256[[name]])
}
rm(bytes)
invisible(gc())
writeLines("START", file.path(out,"START"))
while (!file.exists(file.path(out,"START.CHECKED"))) Sys.sleep(.001)
started <- proc.time()[["elapsed"]]
error <- NULL
result <- tryCatch(fit_model(data), error=function(e) {error <<- conditionMessage(e); NULL})
seconds <- proc.time()[["elapsed"]]-started
writeLines("DONE", file.path(out,"DONE"))
meta <- list(rc=if(is.null(result)) 1 else 0, seconds=seconds, error=error,
             R_version=as.character(getRversion()), fixest_version=as.character(packageVersion("fixest")),
             effective_threads=getFixest_nthreads(), formula=deparse(form))
if (!is.null(result)) {
    fit <- result$fit
    b <- coef(fit)
    correction <- if(c$model=="ppml" && !length(c$clusters)) nobs(fit)/(nobs(fit)-1) else 1
    V <- vcov(fit)*correction
    write.csv(data.frame(name=names(b), value=unname(b), se=sqrt(diag(V))), file.path(out,"coefficients.csv"), row.names=FALSE)
    rows <- expand.grid(row=rownames(V), column=colnames(V), stringsAsFactors=FALSE)
    rows$value <- as.vector(V)
    write.csv(rows, file.path(out,"covariance.csv"), row.names=FALSE)
    selected <- obs(fit)
    xb <- as.vector(as.matrix(data[selected,names(b),drop=FALSE]) %*% b)
    fitted <- as.vector(fitted(fit))
    saved <- if(c$save_fe) rowSums(result$saved)[selected] else rep(NaN, length(selected))
    con <- file(file.path(out,"state.bin"), "wb")
    writeBin(as.double(c(data$obs_id[selected], fitted, xb, saved)), con, size=8, endian="little")
    close(con)
    meta$N <- nobs(fit)
    meta$loglik <- as.numeric(logLik(fit))
    meta$deviance <- as.numeric(deviance(fit))
    meta$converged <- if(is.null(fit$convStatus)) TRUE else fit$convStatus
    meta$df_k <- fit$nparams
    meta$covariance_multiplier <- correction
    if(c$model=="ols") meta$rss <- sum(resid(fit)^2)
}
write_json(meta, file.path(out,"metadata.json"), auto_unbox=TRUE, pretty=TRUE, na="null", digits=NA)
cat(sprintf("LIGHT_WORKER_COMPLETE rc=%d\n",meta$rc))
