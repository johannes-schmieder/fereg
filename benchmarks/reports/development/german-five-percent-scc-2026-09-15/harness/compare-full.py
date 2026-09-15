"""Bounded-memory comparison of completed German PPML public exports.

This compares samples, common finite coordinates and DoF. Mean differences are
descriptive until the separate model-space audit under ADR 0026 is complete.
Run only after both timed estimations finish. No estimation is invoked here.
"""
from pathlib import Path
import argparse
import csv
import hashlib
import itertools
import json
import re

import numpy as np
import pandas as pd


TERMS = [f"age{power}_ed{edu}" for edu in (10, 13, 16, 18, 99)
         for power in (2, 3)] + [f"{year}.year" for year in range(2002, 2010)]
RAW = ["obs_id", "persid", "betnr", "year", "logrwage2", "age", "edyrs"]


def table(path):
    return pd.read_csv(path, sep="\t", index_col=0, na_values=["."],
                       skipinitialspace=True).astype(float)


def scalars(path):
    return {k: float(v) if v.strip() != "." else None
            for k, v in csv.reader(path.open(), delimiter="\t")}


def completed(directory, sentinel):
    fit = dict(line.split("=", 1) for line in
               (directory / "fit-result.txt").read_text().splitlines())
    assert fit["FIT_RC"] == "0", (directory.name, fit)
    assert (directory / "result.txt").read_text().strip() == "STATA_RC=0"
    assert re.search(r"^" + re.escape(sentinel) + r"(?:\||$)",
                     (directory / "run.log").read_text(), re.M), directory.name
    completion = directory / "completion.json"
    if completion.exists():
        assert json.loads(completion.read_text())["accepted"] is True
    data = {"directory_basename": directory.name,
            "command_seconds": float(fit["COMMAND_SECONDS"]),
            "scalars": scalars(directory / "scalars.tsv")}
    stderr = (directory / "process.stderr").read_text()
    for field in ["maximum resident set size", "peak memory footprint", "swaps"]:
        matches = re.findall(r"^\s*(\d+)\s+" + re.escape(field) + r"\s*$",
                             stderr, re.M)
        data[field] = int(matches[-1]) if matches else None
    return data


def discrepancy(a, b, atol, rtol):
    assert a.shape == b.shape
    assert np.isfinite(a).all() and np.isfinite(b).all()
    delta = np.abs(a - b)
    ratio = delta / (atol + rtol * np.maximum(np.abs(a), np.abs(b)))
    return {"maximum_absolute_difference": float(delta.max()),
            "maximum_envelope_ratio": float(ratio.max()),
            "absolute_tolerance": atol, "relative_tolerance": rtol,
            "passed": bool(np.all(ratio <= 1))}


def compare(args):
    native = completed(args.native, args.native_sentinel)
    reference = completed(args.reference, args.reference_sentinel)
    ns, rs = native["scalars"], reference["scalars"]
    assert ns["converged"] == rs["converged"] == 1
    assert ns["N"] == rs["N"] == args.expected_retained
    assert ns["N_full"] == rs["N_full"] == args.expected_input
    assert ns["df_m"] == rs["df_m"] == len(TERMS)
    assert ns["num_singletons"] == rs["num_singletons"]
    assert ns["df_resid"] == rs["df"]
    assert ns["rank"] + ns["df_a"] == rs["rank"] + rs["df_a"]
    certificate = table(args.native / "feppml_certificate.tsv")
    assert certificate.loc["composite_passed", "value"] == 1
    nb, rb = [table(path / "b.tsv")[TERMS].to_numpy().ravel()
              for path in (args.native, args.reference)]
    nv, rv = [table(path / "V.tsv").loc[TERMS, TERMS].to_numpy()
              for path in (args.native, args.reference)]
    # Same envelopes as the accepted German subset comparisons.
    b_check = discrepancy(nb, rb, 1e-10, 1e-7)
    v_check = discrepancy(nv, rv, 1e-10, 1e-6)

    columns = RAW + ["sample", "fitted_mu", "absorbed"]
    readers = [pd.read_stata(path / "outputs.dta", columns=columns,
                            convert_categoricals=False, chunksize=args.chunk_rows)
               for path in (args.native, args.reference)]
    offset = retained = extreme_input = extreme_retained = 0
    mask_hash = hashlib.sha256()
    mean_max_absolute = mean_max_scaled = 0.0
    log_input_max = log_retained_max = -float("inf")
    try:
        for left, right in itertools.zip_longest(*readers):
            assert left is not None and right is not None, "different input lengths"
            assert len(left) == len(right)
            assert np.array_equal(left["obs_id"].to_numpy(),
                                  np.arange(offset + 1, offset + len(left) + 1))
            for name in RAW + ["sample"]:
                assert np.array_equal(left[name].to_numpy(), right[name].to_numpy(),
                                      equal_nan=True), (name, offset)
            sample = left["sample"].to_numpy()
            assert np.isin(sample, [0, 1]).all()
            active = sample == 1
            mask_hash.update(np.asarray(sample, dtype=np.uint8).tobytes())
            offset += len(left)
            retained += int(active.sum())
            log_y = left["logrwage2"].to_numpy(dtype=float)
            assert np.isfinite(log_y).all()
            extreme_input += int(np.sum(log_y > 20))
            extreme_retained += int(np.sum(log_y[active] > 20))
            log_input_max = max(log_input_max, float(log_y.max()))
            if active.any():
                log_retained_max = max(log_retained_max, float(log_y[active].max()))
            means = []
            for frame in (left, right):
                mu = frame["fitted_mu"].to_numpy(dtype=float)
                assert np.isnan(mu[~active]).all()
                assert np.isfinite(mu[active]).all() and np.all(mu[active] > 0)
                assert np.isfinite(frame["absorbed"].to_numpy(dtype=float)[active]).all()
                means.append(mu[active])
            if active.any():
                delta = np.abs(means[0] - means[1])
                denominator = np.maximum(1.0, np.maximum(means[0], means[1]))
                mean_max_absolute = max(mean_max_absolute, float(delta.max()))
                mean_max_scaled = max(mean_max_scaled, float((delta / denominator).max()))
            print(f"ROWS_COMPARED {offset} retained={retained}", flush=True)
    finally:
        for reader in readers:
            reader.close()
    assert offset == args.expected_input and retained == args.expected_retained
    result = {
        "scope": "public finite coefficients/covariance, all-row sample/order/raw inputs and DoF",
        "native": native, "reference": reference,
        "coefficient_comparison": b_check, "covariance_comparison": v_check,
        "input_rows": offset, "retained_rows": retained,
        "all_input_rows_and_sample_exact": True,
        "sample_uint8_sha256": mask_hash.hexdigest(),
        "same_total_rank_and_residual_dof": True,
        "native_composite_certificate_passed": True,
        "log_outcome_input_max": log_input_max,
        "log_outcome_retained_max": log_retained_max,
        "input_log_outcomes_above_20": extreme_input,
        "retained_log_outcomes_above_20": extreme_retained,
        "mean_max_absolute_difference": mean_max_absolute,
        "mean_max_difference_over_max_one_means": mean_max_scaled,
        "mean_comparison_is_descriptive": True,
        "independent_full_mean_score_and_model_space": "not evaluated by this script; separate audit required",
        "command_time_ratio_reference_over_native": reference["command_seconds"] / native["command_seconds"],
        "timing_scope": "single local commands, not a paired full-data speed qualification",
        "common_finite_b_V_sample_dof_pass": b_check["passed"] and v_check["passed"],
        "chunk_rows": args.chunk_rows,
    }
    args.output.write_text(json.dumps(result, indent=2, allow_nan=False) + "\n")
    assert result["common_finite_b_V_sample_dof_pass"], "finite comparison outside subset envelope"
    print("FULL_PUBLIC_COMPARISON_COMPLETE", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native", required=True, type=Path)
    parser.add_argument("--reference", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--expected-input", type=int, default=91045290)
    parser.add_argument("--expected-retained", type=int, default=88876264)
    parser.add_argument("--native-sentinel", default="FULL_GERMAN_PPML_COMPLETE")
    parser.add_argument("--reference-sentinel", default="FULL_GERMAN_PPML_REFERENCE_COMPLETE")
    parser.add_argument("--chunk-rows", type=int, default=262144)
    options = parser.parse_args()
    assert 1 <= options.chunk_rows <= 1048576
    assert not options.output.exists(), "preserve existing comparison receipts"
    compare(options)
