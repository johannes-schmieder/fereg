args <- commandArgs(trailingOnly=TRUE)
source(file.path(args[[1]], "campaign_fixest.R"))
suppressPackageStartupMessages(library(fixest))
set.seed(7301)
data <- expand.grid(g1=seq_len(8), g2=seq_len(7), period=seq_len(10))
data <- data[sample(nrow(data)), ]
data$x1 <- rnorm(nrow(data)); data$x2 <- rnorm(nrow(data))
eta <- 0.2 + 0.12*data$g1 - 0.08*data$g2 + 0.3*data$x1 - 0.1*data$x2
data$ols <- eta + rnorm(nrow(data), sd=0.2)
data$ppml <- exp(eta)*rgamma(nrow(data), shape=8, scale=1/8)
labels1 <- c(100000, 99999, 2147483648, 1e12, -100000, 200000, 17, 123456789)
labels2 <- c(1e11, 42, -200000, 999999, 888888, 333333, 777777)
large <- data
large$g1 <- labels1[data$g1]; large$g2 <- labels2[data$g2]
for (model in c("ols", "ppml")) {
    fml <- as.formula(paste(model, "~ x1 + x2 | g1 + g2"))
    options <- list(fml=fml, nthreads=1, fixef.tol=1e-11, notes=FALSE)
    if (model=="ppml") options$glm.tol <- 1e-10
    estimator <- if (model=="ols") feols else fepois
    fit <- do.call(estimator, c(options, list(data=large)))
    original <- do.call(estimator, c(options, list(data=data)))
    effects <- fixef(fit, nthreads=1, fixef.tol=1e-11)
    saved <- lapply(c("g1", "g2"), function(name) campaign_fe_values(effects[[name]], large[[name]][obs(fit)]))
    eta_saved <- as.vector(as.matrix(large[c("x1", "x2")]) %*% coef(fit)) + Reduce(`+`, saved)
    prediction <- if (model=="ols") eta_saved else exp(eta_saved)
    # Actual estimation with relabelled, reordered large IDs must preserve both
    # the built-in predictor and an independent explicit-dummy fit.
    explicit <- as.formula(paste(model, "~ x1 + x2 + factor(g1) + factor(g2)"))
    oracle <- if (model=="ols") lm(explicit, data=large) else
        glm(explicit, data=large, family=quasipoisson(link="log"), control=glm.control(epsilon=1e-12))
    stopifnot(max(abs(prediction-fitted(fit))) < 1e-8,
              max(abs(prediction-fitted(original))) < 1e-8,
              max(abs(prediction-fitted(oracle))) < 1e-8,
              max(abs(coef(fit)-coef(oracle)[c("x1", "x2")])) < 1e-9)
}
stopifnot(inherits(try(campaign_fe_values(c("100000"=1, "1e+05"=2), 100000), silent=TRUE), "try-error"),
          inherits(try(campaign_fe_values(c("100000"=1), 200000), silent=TRUE), "try-error"))
cat("SAVED_FE_FIXEST_TEST_PASS\n")
