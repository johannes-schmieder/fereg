args <- commandArgs(trailingOnly=TRUE)
.libPaths(c(args[[1]], .libPaths()))
suppressPackageStartupMessages(library(fixest))
stopifnot(as.character(packageVersion("fixest")) == "0.14.2")
input <- args[[2]]
output <- args[[3]]
arm <- args[[4]]
columns <- readLines(file.path(input, "columns.txt"))
path <- file.path(input, "input.bin")
data <- as.data.frame(matrix(readBin(path, double(), n=file.info(path)$size/8,
                                    size=8, endian="little"), ncol=length(columns)))
names(data) <- columns
con <- file(file.path(output, "r-input.bin"), "wb")
for (name in names(data)) writeBin(as.double(data[[name]]), con, size=8, endian="little")
close(con)
controls <- grep("^x[0-9]+$", names(data), value=TRUE)
formula <- as.formula(paste("y ~", paste(controls, collapse=" + "), "| g1 + g2"))
options <- list(fml=formula, data=data, nthreads=1, vcov="iid", notes=FALSE)
if (arm == "strict") options$fixef.tol <- 1e-11
if (arm == "very_strict") options$fixef.tol <- 1e-13
fit <- do.call(feols, options)
V <- vcov(fit, attr=TRUE)
write.csv(data.frame(name=names(coef(fit)), coefficient=unname(coef(fit)), se=sqrt(diag(V))),
          file.path(output,"b.csv"), row.names=FALSE)
write.table(unname(V), file.path(output,"V.csv"), sep=",", row.names=FALSE, col.names=FALSE)
write.csv(data.frame(N=nobs(fit), rss=sum(resid(fit)^2), df_k=attr(V,"df.K"),
                    fit_nparams=fit$nparams, converged=is.null(fit$convStatus) || fit$convStatus),
          file.path(output,"meta.csv"), row.names=FALSE)
write.csv(data.frame(obs_id=data$obs_id[obs(fit)]), file.path(output,"sample.csv"), row.names=FALSE)
writeBin(as.double(resid(fit)), file.path(output,"residual.bin"), size=8, endian="little")
writeLines(paste(fit$iterations, collapse=","), file.path(output,"iterations.txt"))
cat("OLS_DIRECT_REFERENCE_COMPLETE\n")
