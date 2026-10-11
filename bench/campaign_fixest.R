campaign_fixest_options <- function(model, arm) {
    options <- list()
    if (model == "ppml") options$ssc <- fixest::ssc(K.adj=FALSE, G.adj=TRUE)
    if (arm == "fixest-strict") {
        options$fixef.tol <- 1e-11
        if (model == "ppml") options$glm.tol <- 1e-10
        # OLS finite-sample adjustments count disconnected FE components exactly.
        else options$ssc <- fixest::ssc(K.exact=TRUE)
    }
    options
}

# Campaign datasets use numeric categorical IDs. Join their numeric values,
# rather than formatting them with as.character(), which may use exponents.
campaign_fe_values <- function(effects, ids) {
    keys <- suppressWarnings(as.numeric(names(effects)))
    if (!is.numeric(ids) || any(!is.finite(ids)) ||
        any(!is.finite(keys)) || anyDuplicated(keys)) {
        stop("invalid or ambiguous numeric saved-FE IDs")
    }
    positions <- match(ids, keys)
    if (anyNA(positions)) stop("saved-FE ID is absent from fixest effects")
    values <- unname(effects[positions])
    if (any(!is.finite(values))) stop("nonfinite saved FE")
    values
}
