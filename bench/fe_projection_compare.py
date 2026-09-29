"""Complete-pair FE timing summaries; failed/missing arms remain explicit."""
from __future__ import annotations
import csv
import hashlib
import json
import math
from pathlib import Path
import statistics
import numpy as np


def measurement(directory, cell, arm):
    process = json.loads((directory / "process.json").read_text())
    if process["process_status"] != "ok":
        raise ValueError(process["process_status"])
    fields = dict(line.split("\t", 1) for line in (directory / "result.tsv").read_text().splitlines())
    if (int(fields["rows"]) != cell["rows"] or int(fields["rank"]) != cell["regressors"]
            or int(fields["rhs_count"]) != cell["regressors"] + 2):
        raise ValueError("sample, RHS or rank mismatch")
    if arm == "legacy-auto":
        traces = [line for line in (directory / "process.log").read_text().splitlines()
                  if line.startswith("FEREG_PHASE|fit|")]
        if len(traces) != 1:
            raise ValueError("missing or ambiguous legacy projection phase trace")
        phase = dict(part.split("=", 1) for part in traces[0].split("|")[2:])
        times = [float(phase[key]) for key in ("rhs", "raw_moments", "initial_absorb")]
        if any(not math.isfinite(value) or value < 0 for value in times):
            raise ValueError("invalid legacy phase time")
        seconds = sum(times)
    else:
        seconds = float(fields["projection_seconds"])
    if not math.isfinite(seconds) or seconds <= 0:
        raise ValueError("nonpositive/nonfinite projection time")
    coefficients = np.fromfile(directory / "coefficients.f64", dtype="<f8")
    residuals = np.fromfile(directory / "residuals.f64", dtype="<f8")
    if (coefficients.shape != (cell["regressors"],) or residuals.shape != (cell["rows"],)
            or not np.isfinite(coefficients).all() or not np.isfinite(residuals).all()):
        raise ValueError("malformed coefficient or residual results")
    return seconds, coefficients, residuals, fields


def accuracy(actual, reference):
    _, coefficients, residuals, _ = actual
    _, expected, reference_residuals, _ = reference
    error = np.abs(coefficients - expected)
    coefficients_ok = bool(np.all(error <= 1e-10 + 1e-7 * np.abs(expected)))
    sse = float(np.dot(residuals, residuals))
    reference_sse = float(np.dot(reference_residuals, reference_residuals))
    objective_ok = abs(sse - reference_sse) <= 1e-10 + 1e-7 * abs(reference_sse)
    # Large weak-graph residual distances are diagnostic (SPEC §20); coefficients
    # and the objective remain binding, alongside independent backend oracles.
    return {"accurate": coefficients_ok and objective_ok,
            "coefficient_max_error": float(error.max(initial=0.)),
            "residual_max_error": float(np.max(np.abs(residuals - reference_residuals), initial=0.)),
            "objective_relative_error": abs(sse-reference_sse) / max(1., abs(reference_sse))}


def group(root, identity, cell, arm, threads):
    base = root / "commands" / cell["id"]
    record = {"commit": identity["commit"], "dirty": identity["dirty"],
              "partial": identity["partial"], "binary_sha256": identity["binary_sha256"],
              "input_sha256": identity.get("inputs", {}).get(cell["id"], ""),
              "seed": identity["seed"], "numpy": identity["numpy"], "pandas": identity["pandas"],
              "tolerance": identity["tolerance"], "reference_tolerance": identity["reference_tolerance"],
              "cell": cell["id"], "rows": cell["rows"], "regressors": cell["regressors"],
              "threads": threads, "arm": arm, "expected": identity["repetitions"],
              "completed": 0, "accurate": False, "status": "missing",
              "median_seconds": "", "min_seconds": "", "max_seconds": "",
              "mad_seconds": "", "ratio_to_legacy": "", "used": "", "iterations": "",
              "setup_iterations": "", "fallbacks": "", "coefficient_max_error": "",
              "residual_max_error": "", "objective_relative_error": ""}
    record["science_sha256"] = ""
    measured = []
    try:
        for rep in ["warmup"] + list(range(1, identity["repetitions"] + 1)):
            measured.append(measurement(base / f"{arm}-t{threads}-{rep}", cell, arm))
            if rep != "warmup":
                record["completed"] += 1
        references = []
        for name in ("legacy-auto", "next-auto"):
            try:
                references.append(measurement(base / f"{name}-t1-reference", cell, name))
            except (ValueError, OSError, KeyError) as error:
                raise ValueError(f"reference:{name}:{error}") from error
        reference, witness = references
        if not accuracy(witness, reference)["accurate"]:
            raise ValueError("tight-reference disagreement")
        comparisons = [accuracy(value, reference) for value in measured]
        times = [value[0] for value in measured[1:]]
        record.update(completed=len(times), accurate=all(c["accurate"] for c in comparisons),
                      status="ok", median_seconds=statistics.median(times),
                      min_seconds=min(times), max_seconds=max(times))
        record["mad_seconds"] = statistics.median(abs(t-record["median_seconds"]) for t in times)
        for key in ("coefficient_max_error", "residual_max_error", "objective_relative_error"):
            record[key] = max(c[key] for c in comparisons)
        for key in ("used", "iterations", "setup_iterations", "fallbacks"):
            record[key] = ";".join(sorted({value[3].get(key, "") for value in measured}))
        digests = {hashlib.sha256(value[1].tobytes() + value[2].tobytes()).hexdigest()
                   for value in measured}
        record["science_sha256"] = ";".join(sorted(digests))
        if not record["accurate"]:
            record["status"] = "accuracy-failure"
        elif arm.startswith("next-") and len(digests) != 1:
            record.update(status="repeat-mismatch", accurate=False)
    except (ValueError, OSError, KeyError) as error:
        record["status"] = "missing" if isinstance(error, FileNotFoundError) else str(error)
    return record


def summarize(root, target):
    root, target = Path(root), Path(target)
    identity = json.loads((root / "run.json").read_text())
    records = []
    for cell in identity["cells"]:
        for threads in identity["threads"]:
            for arm in ("legacy-auto", "next-auto", "next-lsmr", "next-cmg"):
                if arm == "next-cmg" and cell.get("slopes"):
                    continue
                records.append(group(root, identity, cell, arm, threads))
    baseline = {(r["cell"], r["threads"]): r for r in records if r["arm"] == "legacy-auto"}
    for cell in identity["cells"]:
        for arm in ("next-auto", "next-lsmr", "next-cmg"):
            comparable = [r for r in records if r["cell"] == cell["id"] and r["arm"] == arm
                          and r["accurate"]]
            if len({r["science_sha256"] for r in comparable}) > 1:
                for record in comparable:
                    record.update(status="thread-mismatch", accurate=False)
    for record in records:
        reference = baseline[(record["cell"], record["threads"])]
        if record["accurate"] and reference["accurate"]:
            record["ratio_to_legacy"] = record["median_seconds"] / reference["median_seconds"]
    with target.open("w", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=list(records[0]))
        writer.writeheader()
        writer.writerows(records)
    return records
