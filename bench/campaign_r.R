args <- commandArgs(trailingOnly=TRUE)
script <- sub("^--file=", "", commandArgs()[grepl("^--file=", commandArgs())])
source(file.path(dirname(script), "campaign_fixest.R"))
s <- read.delim(args[[1]], header=FALSE, sep="\t", quote="", col.names=c("key","value"), colClasses="character")
c <- setNames(as.list(s$value), s$key)
.libPaths(c(c$r_library, .libPaths()))
suppressPackageStartupMessages(library(fixest))
stopifnot(as.character(packageVersion("fixest")) == "0.14.2")
k <- as.integer(c$k); q <- as.integer(c$effects); threads <- as.integer(c$threads)
setFixest_nthreads(threads)
terms <- paste0("x", seq_len(k))
fml <- as.formula(paste("y ~",paste(terms,collapse=" + "),"|",paste(paste0("g",seq_len(q)),collapse=" + ")))
options <- list(fml=fml,nthreads=threads,notes=FALSE)
options$vcov <- switch(c$vce,iid="iid",robust="hetero",cluster=~cluster)
# Explicit matching of the PPML finite-sample convention. Retain stock V too.
if (c$model == "ppml") options$ssc <- ssc(K.adj=FALSE,G.adj=TRUE)
if (c$arm == "fixest-strict") {
    options$fixef.tol <- 1e-11
    if(c$model == "ppml") options$glm.tol <- 1e-10
}
fit_model <- function(data, warm=FALSE) {
    o <- options; o$data <- data
    if (warm) o$fml <- as.formula(paste("y ~",paste(terms,collapse=" + "),"| g1"))
    do.call(if(c$model=="ols") feols else fepois,o)
}
invisible(fit_model(foreign::read.dta(c$warm,convert.factors=FALSE),TRUE))
data <- foreign::read.dta(c$dataset,convert.factors=FALSE)
identity <- jsonlite::fromJSON(sub("\\.dta$",".json",c$dataset))
for(name in names(identity$column_sha256)) {
    bytes <- writeBin(as.double(data[[name]]),raw(),size=8,endian="little")
    stopifnot(digest::digest(bytes,algo="sha256",serialize=FALSE)==identity$column_sha256[[name]])
}
rm(bytes)
invisible(gc())
writeLines("ready",file.path(c$output,"READY"))
error <- NULL; saved <- NULL
start <- proc.time()[["elapsed"]]
fit <- tryCatch({
    fit <- fit_model(data)
    if (identical(fit$convStatus,FALSE)) stop("fixest did not converge")
    if (c$save_fe=="1") {
        effects <- fixef(fit,nthreads=threads,fixef.tol=1e-11)
        selected <- obs(fit)
        saved <- lapply(seq_len(q),function(j) campaign_fe_values(effects[[paste0("g",j)]],data[[paste0("g",j)]][selected]))
        stopifnot(all(vapply(saved,function(x) all(is.finite(x)),logical(1))))
    }
    Vstock <- vcov(fit,attr=TRUE,nthreads=threads)
    multiplier <- if(c$model=="ppml" && c$vce=="robust") nobs(fit)/(nobs(fit)-1) else 1
    V <- Vstock*multiplier
    fit
},error=function(e){error <<- conditionMessage(e);NULL})
seconds <- proc.time()[["elapsed"]]-start
writeLines("done",file.path(c$output,"DONE"))
meta <- c(rc=if(is.null(fit)) 1 else 0,seconds=seconds,complete=1,
          fixest_version=as.character(packageVersion("fixest")),requested_threads=threads,
          configured_threads=threads)
if(!is.null(fit)) {
    b <- coef(fit)
    write.csv(data.frame(name=names(b),value=unname(b)),file.path(c$output,"coefficients.csv"),row.names=FALSE)
    writeV <- function(V,path) {
        rows <- expand.grid(row=rownames(V),column=colnames(V),stringsAsFactors=FALSE)
        rows$value <- as.vector(V); write.csv(rows,path,row.names=FALSE)
    }
    writeV(V,file.path(c$output,"covariance.csv"))
    writeV(Vstock,file.path(c$output,"covariance-stock.csv"))
    selected <- obs(fit)
    rows <- data.frame(obs_id=data$obs_id,retained=as.integer(seq_len(nrow(data)) %in% selected))
    if(c$save_fe=="1") {
        rows$fitted <- NA_real_
        predictor <- as.vector(as.matrix(data[selected,terms,drop=FALSE]) %*% b[terms]) + Reduce(`+`,saved)
        rows$fitted[selected] <- if(c$model=="ppml") exp(predictor) else predictor
        for(j in seq_len(q)) {rows[[paste0("saved",j)]] <- NA_real_; rows[[paste0("saved",j)]][selected] <- saved[[j]]}
        stopifnot(max(abs(rows$fitted[selected]-fitted(fit))) <= 1e-8*max(1,max(abs(fitted(fit)))))
    }
    write.csv(rows,file.path(c$output,"rows.csv"),row.names=FALSE,na=".")
    absorbed <- fit$nparams-length(b)
    if(c$model=="ols" && c$vce=="cluster") {
        stopifnot(is.finite(attr(Vstock,"df.K")))
        absorbed <- attr(Vstock,"df.K")-length(b)-1
    }
    meta <- c(meta,N=nobs(fit),df_a=absorbed,df_a_unadjusted=fit$nparams-length(b),df_m=length(b),
              converged=1,covariance_multiplier=multiplier,
              finite_slope_rank=length(b),slope_omissions=length(fit$collin.var),
              covariance_policy=paste(c$model,c$vce,if(c$model=="ppml") "K.adj=FALSE" else "stock",sep="-"))
    if(c$model=="ols") meta <- c(meta,df_r=attr(Vstock,"df.t"),correction_df=nobs(fit)-attr(Vstock,"df.K"))
    if(c$vce=="cluster") meta <- c(meta,N_clust=length(unique(data$cluster[selected])))
    if(c$model=="ols") meta <- c(meta,rss=sum(resid(fit)^2))
    else meta <- c(meta,ll=as.numeric(logLik(fit)),deviance=as.numeric(deviance(fit)))
}
if(!is.null(error)) meta <- c(meta,error=gsub("[\t\r\n]"," ",error))
write.table(data.frame(names(meta),unname(meta)),file.path(c$output,"metadata.tsv"),sep="\t",quote=FALSE,row.names=FALSE,col.names=FALSE)
cat(sprintf("CAMPAIGN_COMPLETE rc=%d\n",if(is.null(fit)) 1 else 0))
quit(status=if(is.null(fit)) 1 else 0)
