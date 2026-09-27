#!/usr/bin/env python3
"""Validate matched coefficient outputs before aggregating paired timings."""
import argparse
import hashlib
import json
from pathlib import Path

import numpy as np
import pandas as pd


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate_receipt(plan, settings, engine, source, receipt):
    if receipt["source"] != source:
        raise ValueError("Benchmark source manifest changed")
    expected = dict(settings, engine=engine, warm=plan["warm_calls"],
                    refit_reference=plan.get("refit_reference", False))
    expected["levels"] = expected.pop("levels_each")
    for field, value in expected.items():
        if receipt["settings"][field] != value:
            raise ValueError(f"Benchmark setting changed: {field}")
    if not receipt["passed"] or receipt["plugin_sha256"] != plan["plugin_sha256"]:
        raise ValueError("Benchmark execution failed or binary changed")
    for field in ("script_sha256", "mata_sha256"):
        if receipt[field] != plan[field]:
            raise ValueError(f"Benchmark plan changed: {field}")
    if plan.get("require_monitor") and (not receipt.get("timing_eligible") or
            receipt["monitor"]["sha256"] != plan["monitor_sha256"]):
        raise ValueError("Benchmark monitoring failed; timings remain unranked")


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--plan", type=Path, required=True)
    p.add_argument("--directory", type=Path, required=True)
    p.add_argument("--prefix", required=True)
    p.add_argument("--source-manifest", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    args = p.parse_args()
    plan = json.loads(args.plan.read_text())
    source = json.loads(args.source_manifest.read_text())
    cases = []
    for index, settings in enumerate(plan["cases"]):
        receipts, timings, errors = {}, {}, []
        for block in range(len(plan["orders"])):
            paths = {engine: args.directory / f"{args.prefix}-{index}-{block}-{engine}"
                     for engine in plan["orders"][block]}
            block_receipts = {engine: json.loads((path / "receipt.json").read_text())
                              for engine, path in paths.items()}
            for field in ("input_sha256", "signs_sha256", "plugin_sha256", "reference_sha256", "script_sha256", "mata_sha256"):
                if len({r[field] for r in block_receipts.values()}) != 1:
                    raise ValueError(f"Unmatched {field}")
            for engine, receipt in block_receipts.items():
                validate_receipt(plan, settings, engine, source, receipt)
                receipts[f"{block}-{engine}"] = dict(receipt_sha256=sha(paths[engine]/"receipt.json"),
                    coefficients_sha256=sha(paths[engine]/"coefficients.csv"),
                    timing_sha256=sha(paths[engine]/"timing.csv"),
                    process_seconds=receipt["process_seconds"],
                    peak_rss_bytes=receipt["process_peak_rss_bytes"],
                    probe_preparation_seconds=receipt["probe_preparation_seconds"])
                frame = pd.read_csv(paths[engine]/"timing.csv")
                if (list(frame["repeat"]) != list(range(plan["warm_calls"]+1)) or
                        not frame.engine.eq(engine).all() or
                        not np.isfinite(frame[["total", "correction"]]).all().all() or
                        frame[["total", "correction"]].lt(0).any().any()):
                    raise ValueError("Benchmark timing inventory is invalid")
                timings.setdefault(engine, []).append(frame)
            data = pd.read_stata(paths["native"]/"input.dta")
            native = pd.read_csv(paths["native"]/"coefficients.csv")
            first, second = native.groupby(data.g), native.groupby(data.h)
            dimension = int(data.g.max()+data.h.max()-1)
            increments = [np.r_[first[f"bf_1_{channel}"].first(),
                               second[f"bf_2_{channel}"].first().iloc[1:],
                               [native[f"{channel}_x"].iloc[0], native[f"{channel}_z"].iloc[0]]]
                          for channel in ("d", "c")]
            covariance = np.column_stack([np.r_[first[f"bf_1_{channel}"].first(),
                                     second[f"bf_2_{channel}"].first().iloc[1:]]
                                     for channel in ("sed", "sec", "covdc")])
            covariance[:, :2] **= 2
            for engine in ("mata_current", "mata_cached"):
                reference = pd.read_csv(paths[engine]/"coefficients.csv")
                error = max(float(np.max(np.abs(reference.d-increments[0]))),
                            float(np.max(np.abs(reference.c-increments[1]))),
                            float(np.max(np.abs(reference[["vd", "vc", "covdc"]].iloc[:dimension].to_numpy()-covariance))))
                if "vd_x" in native:
                    finite_covariance = np.array([[native[f"{channel}_{control}"].iloc[0]
                                                  for channel in ("vd", "vc", "covdc")]
                                                 for control in ("x", "z")])
                    error=max(error,float(np.max(np.abs(reference[["vd", "vc", "covdc"]].iloc[dimension:].to_numpy()-finite_covariance))))
                if error > plan["accuracy_absolute"]:
                    raise ValueError(f"Unmatched correction: {index}/{block}/{engine}: {error}")
                errors.append(error)
        summary = {}
        for engine, frames in timings.items():
            frame = pd.concat(frames)
            warm = frame[frame["repeat"] > 0]
            summary[engine] = {
                "cold": frame[frame["repeat"] == 0].drop(columns=["engine", "repeat"]).replace({np.nan: None}).to_dict("records"),
                "warm": {column: dict(median=float(warm[column].median()),
                                      minimum=float(warm[column].min()), maximum=float(warm[column].max()))
                         for column in frame.columns if column not in ("engine", "repeat") and warm[column].notna().all()},
                "warm_count": len(warm)}
        native = summary["native"]["warm"]["correction"]["median"]
        cases.append(dict(settings=settings, maximum_error=max(errors), measurements=summary, inputs=receipts,
                          correction_ratios_native_over_reference={engine: native/summary[engine]["warm"]["correction"]["median"]
                                                                   for engine in ("mata_current", "mata_cached")}))
    result = dict(schema=1, plan_sha256=sha(args.plan), analysis_script_sha256=sha(Path(__file__)),
                  source_manifest_sha256=sha(args.source_manifest),
                  performance_eligible=bool(plan.get("require_monitor")),
                  passed=True, cases=cases,
                  timing_scope="Core correction includes native FE materialization; Mata publication is separately timed. Probe-file preparation is outside Stata timings. Only refit-reference plans support end-to-end comparison. Process RSS includes all loaded Stata banks and saved outputs. Process elapsed includes startup, compilation, all repeated calls and exports; startup is excluded from repeated calls but is not individually timed.")
    with args.out.open("x") as handle:
        json.dump(result, handle, indent=2, sort_keys=True, allow_nan=False)
        handle.write("\n")
    print(f"All {len(cases)} matched benchmark cases passed; max discrepancy {max(c['maximum_error'] for c in cases):.3g}")


if __name__ == "__main__":
    main()
