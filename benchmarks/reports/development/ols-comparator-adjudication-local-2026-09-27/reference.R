args <- commandArgs(trailingOnly=TRUE)
.libPaths(c(args[[1]], .libPaths()))
suppressPackageStartupMessages(library(fixest))
columns <- readLines(file.path(dirname(args[[2]]), "columns.txt"))
values <- readBin(args[[2]], double(), n=file.info(args[[2]])$size/8, size=8, endian="little")
data <- as.data.frame(matrix(values, ncol=length(columns)))
names(data) <- columns
output <- args[[3]]
effects <- strsplit(args[[4]], ",", fixed=TRUE)[[1]]
controls <- grep("^x[0-9]+$", names(data), value=TRUE)
formula <- as.formula(paste("y ~", paste(controls, collapse=" + "), "|",
                           paste(effects, collapse=" + ")))
con <- file(file.path(output, "r-input.bin"), "wb")
for (name in names(data)) writeBin(as.double(data[[name]]), con, size=8, endian="little")
close(con)
for (arm in c("default", "strict", "strict_exact_dof")) {
    options <- list(fml=formula, data=data, nthreads=1, vcov="iid", notes=FALSE)
    if (arm != "default") options$fixef.tol <- 1e-11
    if (arm == "strict_exact_dof") options$ssc <- ssc(K.exact=TRUE)
    fit <- do.call(feols, options)
    V <- vcov(fit, attr=TRUE)
    write.csv(data.frame(name=names(coef(fit)), coefficient=unname(coef(fit)),
                         se=sqrt(diag(V))), file.path(output,paste0(arm,"-b.csv")), row.names=FALSE)
    write.table(unname(V), file.path(output,paste0(arm,"-V.csv")), sep=",", row.names=FALSE, col.names=FALSE)
    write.csv(data.frame(N=nobs(fit), rss=sum(resid(fit)^2), df_k=attr(V,"df.K"),
                         fit_nparams=fit$nparams, converged=is.null(fit$convStatus) || fit$convStatus),
              file.path(output,paste0(arm,"-meta.csv")), row.names=FALSE)
    write.csv(data.frame(obs_id=data$obs_id[obs(fit)]), file.path(output,paste0(arm,"-sample.csv")), row.names=FALSE)
}
cat("OLS_REFERENCE_ADJUDICATION_COMPLETE\n")
