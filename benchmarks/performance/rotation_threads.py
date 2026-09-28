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


def native_threads_match(receipt, expected):
    prefix = "feppml" if receipt["cell"]["model"] == "ppml" else "fereg"
    metadata = receipt.get("metadata", {})
    return (metadata.get(prefix + "_threads") == expected
            and metadata.get(prefix + "_requested_threads") == expected)
