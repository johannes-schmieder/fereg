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
