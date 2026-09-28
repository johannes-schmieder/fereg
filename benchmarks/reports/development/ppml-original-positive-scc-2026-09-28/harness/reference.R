args <- commandArgs(trailingOnly=TRUE)
.libPaths(c(args[[1]], .libPaths()))
suppressPackageStartupMessages(library(fixest))
suppressPackageStartupMessages(library(jsonlite))
stopifnot(as.character(packageVersion("fixest")) == "0.14.2")
input <- args[[2]]
output <- args[[3]]
settings <- fromJSON(args[[4]])
stopifnot(settings$fixef_tol > 10000*.Machine$double.eps,
          settings$fixef_tol <= 1e-11, settings$glm_tol > 0, settings$glm_tol <= 1e-10)
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
fit <- fepois(formula, data=data, nthreads=1, vcov="hetero", ssc=ssc(K.adj=FALSE),
              notes=FALSE, fixef.tol=settings$fixef_tol, glm.tol=settings$glm_tol)
V <- vcov(fit) * nobs(fit)/(nobs(fit)-1)
write.csv(data.frame(name=names(coef(fit)), value=unname(coef(fit)), se=sqrt(diag(V))),
          file.path(output,"coefficients.csv"), row.names=FALSE)
rows <- expand.grid(row=rownames(V), column=colnames(V), stringsAsFactors=FALSE)
rows$value <- as.vector(V)
write.csv(rows, file.path(output,"covariance.csv"), row.names=FALSE)
write_json(list(N=nobs(fit), deviance=deviance(fit), loglik=as.numeric(logLik(fit)),
                df_k=fit$nparams, converged=is.null(fit$convStatus) || fit$convStatus,
                iterations=fit$iterations, fixef_tol=settings$fixef_tol, glm_tol=settings$glm_tol),
           file.path(output,"metadata.json"), auto_unbox=TRUE, digits=NA, pretty=TRUE)
write.csv(data.frame(obs_id=data$obs_id[obs(fit)]), file.path(output,"sample.csv"), row.names=FALSE)
writeBin(as.double(fitted(fit)), file.path(output,"means.bin"), size=8, endian="little")
cat("PPML_ORIGINAL_REFERENCE_COMPLETE\n")
