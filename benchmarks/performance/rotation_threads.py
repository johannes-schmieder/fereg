"""Explicit per-arm thread settings for bounded same-artifact comparisons."""


def variant_cell(case, variant):
    overrides = case.get("arm_threads", {})
    if set(overrides) - {"baseline", "candidate", "stock"}:
        raise ValueError("thread override for an unregistered variant")
    cell = dict(case["cell"])
    threads = overrides.get(variant, cell["threads"])
    if type(threads) is not int or not 1 <= threads <= 28:
        raise ValueError("registered threads exceed bounded allocation")
    cell["threads"] = threads
    return cell


def selected_threads(case, variant):
    counts = case.get("native_effective_threads", {})
    if set(counts) - {"baseline", "candidate"}:
        raise ValueError("effective native count for an unregistered variant")
    requested = variant_cell(case, variant)["threads"]
    selected = counts.get(variant, requested)
    if type(selected) is not int or not 1 <= selected <= requested:
        raise ValueError("selected native count exceeds registered request")
    return selected


def native_threads_match(receipt, requested, effective=None):
    prefix = "feppml" if receipt["cell"]["model"] == "ppml" else "fereg"
    metadata = receipt.get("metadata", {})
    return (metadata.get(prefix + "_threads") == (requested if effective is None else effective)
            and metadata.get(prefix + "_requested_threads") == requested)
