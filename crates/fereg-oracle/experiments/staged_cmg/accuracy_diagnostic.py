#!/usr/bin/env python3
"""Exact, independent edge-flux audit of bounded equal-weight two-cell captures.

This audits fixed stored iterates and fixed update sequences. It neither solves
again nor changes the prototype certificate, repair count, or output contract.
"""
import argparse
from decimal import Decimal, localcontext
from fractions import Fraction
import hashlib
import json
import math
from pathlib import Path
import struct


def decode(value):
    if not isinstance(value, int) or not 0 <= value < 2**64:
        raise ValueError("invalid binary64 bits")
    result = struct.unpack(">d", struct.pack(">Q", value))[0]
    if not math.isfinite(result):
        raise ValueError("nonfinite captured value")
    return result


def bits(value):
    return struct.unpack(">Q", struct.pack(">d", value))[0]


def rationals(values):
    return [Fraction.from_float(value) for value in values]


def norm_text(square, precision):
    with localcontext() as ctx:
        ctx.prec = precision
        return str((Decimal(square.numerator) / Decimal(square.denominator)).sqrt())


def ordinary_apply(capture, x):
    """A diagnostic replay of the existing binary64 evaluator, never acceptance."""
    out = [0.0] * capture["n"]
    firms = capture["firms"]
    weights = [decode(v) for v in capture["weights_bits"]]
    totals = [decode(v) for v in capture["totals_bits"]]
    for worker, (lo, hi) in enumerate(zip(capture["offsets"], capture["offsets"][1:])):
        anchor = x[firms[lo]]
        total = 0.0
        for j in range(lo, hi):
            total += weights[j] * (x[firms[j]] - anchor)
        mean = total / totals[worker]
        for j in range(lo, hi):
            out[firms[j]] += weights[j] * ((x[firms[j]] - anchor) - mean)
    return out


def centered(capture, x):
    sums = [0.0] * len(capture["sizes"])
    for component, value in zip(capture["labels"], x):
        sums[component] += value
    means = [value / size for value, size in zip(sums, capture["sizes"])]
    return [value - means[component] for value, component in zip(x, capture["labels"])]


def evaluate(capture, edges, rhs, target, x, flattened):
    residual = rhs.copy()
    for i, j, conductance in edges:
        flux = conductance * (x[i] - x[j])
        residual[i] -= flux
        residual[j] += flux
    square = sum((v * v for v in residual), Fraction())
    component_sums = [Fraction() for _ in capture["sizes"]]
    for component, value in zip(capture["labels"], residual):
        component_sums[component] += value
    result = {
        "residual_norm_decimal_60": norm_text(square, 60),
        "residual_norm_decimal_100": norm_text(square, 100),
        "passes_exact_captured_target": square <= target * target,
        "squared_residual_over_target_squared": str(square / (target * target)),
        "maximum_absolute_residual": str(max(map(abs, residual))),
        "component_residual_sums": list(map(str, component_sums)),
        "flattened_binary64": flattened,
    }
    if flattened:
        values = list(map(float, x))
        product = ordinary_apply(capture, values)
        double_residual = [float(b) - a for b, a in zip(rhs, product)]
        square_sum = 0.0
        for value in double_residual:
            square_sum += value * value
        norm = math.sqrt(square_sum)
        result["ordinary_evaluator_residual_norm"] = norm
        result["ordinary_evaluator_passes_captured_target"] = norm <= float(target)
        result["coordinate_sha256"] = hashlib.sha256(b"".join(struct.pack(">d", v) for v in values)).hexdigest()
    return result, residual


def audit(capture):
    n = capture["n"]
    if capture.get("schema") != 1 or n <= 0:
        raise ValueError("invalid capture schema or dimension")
    vectors = {}
    for name in ["initial", "x", "rhs", "recursive", "truth", "direction", "product"]:
        vectors[name] = [decode(v) for v in capture[name + "_bits"]]
        if len(vectors[name]) != n:
            raise ValueError("captured vector dimension mismatch")
    firms, offsets = capture["firms"], capture["offsets"]
    weights = rationals([decode(v) for v in capture["weights_bits"]])
    totals = rationals([decode(v) for v in capture["totals_bits"]])
    if (offsets[0] != 0 or offsets[-1] != len(firms) or len(firms) != len(weights)
            or len(totals) + 1 != len(offsets) or any(not 0 <= f < n for f in firms)):
        raise ValueError("invalid captured incidence")
    labels, sizes = capture["labels"], capture["sizes"]
    if (len(labels) != n or not sizes or any(s <= 0 for s in sizes)
            or any(labels.count(i) != size for i, size in enumerate(sizes))
            or any(not 0 <= label < len(sizes) for label in labels)):
        raise ValueError("invalid components")
    edges = []
    for worker, (lo, hi) in enumerate(zip(offsets, offsets[1:])):
        if hi - lo != 2 or weights[lo] <= 0 or weights[lo] != weights[lo + 1]:
            raise ValueError("witness requires positive equal-weight two-cell workers")
        if totals[worker] != weights[lo] + weights[lo + 1]:
            raise ValueError("stored worker total differs from exact sum")
        i, j = firms[lo:hi]
        if i == j or labels[i] != labels[j]:
            raise ValueError("invalid two-cell component")
        edges.append((i, j, weights[lo] / 2))
    initial = vectors["initial"]
    naive, compensated, compensation = initial.copy(), initial.copy(), [0.0] * n
    exact = rationals(initial)
    increments = capture["increment_bits"]
    if len(increments) != capture["iterations"] or len(increments) * n > 1_048_576:
        raise ValueError("invalid bounded increment count")
    epochs = capture["epochs"]
    if (not epochs or epochs[0]["update_offset"] != 0
            or any(a["update_offset"] >= b["update_offset"] for a, b in zip(epochs, epochs[1:]))
            or any(not 0 <= e["update_offset"] < len(increments) for e in epochs)):
        raise ValueError("invalid repair epochs")
    anchors = {e["update_offset"]: e["x_bits"] for e in epochs}
    for update_index, update in enumerate(increments):
        if update_index in anchors and [bits(v) for v in naive] != anchors[update_index]:
            raise ValueError("repair epoch anchor differs from ordinary replay")
        if len(update) != n:
            raise ValueError("increment dimension mismatch")
        for i, term in enumerate(map(decode, update)):
            naive[i] += term
            corrected = term - compensation[i]
            total = compensated[i] + corrected
            compensation[i] = (total - compensated[i]) - corrected
            compensated[i] = total
            exact[i] += Fraction.from_float(term)
    if [bits(v) for v in naive] != capture["x_bits"]:
        raise ValueError("ordinary rounded-update replay differs from captured x")
    flattened = list(map(float, exact))
    rhs = rationals(vectors["rhs"])
    target = Fraction.from_float(decode(capture["target_bits"]))
    if target <= 0:
        raise ValueError("nonpositive target")
    final_epoch = epochs[-1]
    final_plain = [decode(v) for v in final_epoch["x_bits"]]
    final_compensated, final_compensation = final_plain.copy(), [0.0] * n
    final_exact = rationals(final_plain)
    for update in increments[final_epoch["update_offset"]:]:
        for i, term in enumerate(map(decode, update)):
            corrected = term - final_compensation[i]
            total = final_compensated[i] + corrected
            final_compensation[i] = (total - final_compensated[i]) - corrected
            final_compensated[i] = total
            final_exact[i] += Fraction.from_float(term)
    final_flattened = list(map(float, final_exact))
    candidates = {
        "final_epoch_kahan_flattened": (rationals(final_compensated), True),
        "final_epoch_exact_sum_unflattened": (final_exact, False),
        "final_epoch_exact_sum_flattened": (rationals(final_flattened), True),
        "final_epoch_exact_sum_flattened_centered": (rationals(centered(capture, final_flattened)), True),
        "stored": (rationals(vectors["x"]), True),
        "kahan_flattened": (rationals(compensated), True),
        "kahan_flattened_centered": (rationals(centered(capture, compensated)), True),
        "exact_sum_unflattened": (exact, False),
        "exact_sum_flattened": (rationals(flattened), True),
        "exact_sum_flattened_centered": (rationals(centered(capture, flattened)), True),
    }
    results = {}
    stored_residual = None
    for name, (values, is_flattened) in candidates.items():
        result, residual = evaluate(capture, edges, rhs, target, values, is_flattened)
        if name == "stored":
            stored_residual = residual
        result["maximum_coordinate_change_from_stored"] = float(max(abs(a - b) for a, b in zip(values, rationals(vectors["x"]))))
        results[name] = result
    error = [actual - observed for actual, observed in zip(stored_residual, rationals(vectors["truth"]))]
    observed_product = ordinary_apply(capture, vectors["x"])
    replayed_truth = [b - a for b, a in zip(vectors["rhs"], observed_product)]
    evaluator_matches = [bits(v) for v in replayed_truth] == capture["truth_bits"]
    rhs_sums = [sum((rhs[i] for i in range(n) if labels[i] == c), Fraction()) for c in range(len(sizes))]
    return {
        "n": n, "workers": len(edges), "iterations": capture["iterations"],
        "repairs_counter": capture["repairs"], "trace": capture["trace"],
        "recorded_update_values": len(increments) * n,
        "repair_epoch_offsets": [e["update_offset"] for e in epochs],
        "final_epoch_updates": len(increments) - final_epoch["update_offset"],
        "ordinary_update_replay_bit_exact": True,
        "ordinary_evaluator_matches_captured_truth_bits": evaluator_matches,
        "stored_totals_equal_exact_weight_sums": True,
        "target": float(target), "bnorm": decode(capture["bnorm_bits"]),
        "component_rhs_sums": list(map(str, rhs_sums)),
        "maximum_stored_coordinate_ulp": max(map(math.ulp, vectors["x"])),
        "stored_coordinate_range": max(vectors["x"]) - min(vectors["x"]),
        "captured_truth_vs_exact_residual_error_norm": norm_text(sum((v * v for v in error), Fraction()), 60),
        "candidates": results,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("captures", nargs="+", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    results = []
    for path in args.captures:
        payload = path.read_bytes()
        result = audit(json.loads(payload))
        result["capture_sha256"] = hashlib.sha256(payload).hexdigest()
        results.append(result)
    args.output.write_text(json.dumps({"schema": 1, "results": results}, indent=2) + "\n")


if __name__ == "__main__":
    main()
