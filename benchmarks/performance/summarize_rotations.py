"""Summarize a frozen rotation without ranking failed or incomplete cells."""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import hashlib
import json
import math
from pathlib import Path
from statistics import median


def read(path):
    return json.loads(path.read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def distribution(values):
    values = sorted(values)
    return {"n": len(values), "values": values, "min": min(values),
            "median": median(values), "max": max(values)} if values else None


def positive(value):
    return isinstance(value, (int, float)) and math.isfinite(value) and value > 0


def summarize(root, *, partial=False):
    plan, identity = read(root / "plan.json"), read(root / "identity.json")
    fingerprint = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()
    expected = {(case["id"], rep + 1) for case in plan["cases"] for rep in range(len(case["seeds"]))}
    if len(expected) != sum(len(case["seeds"]) for case in plan["cases"]):
        raise ValueError("duplicate registered case")
    observed = {(p.parent.parent.name, int(p.parent.name.removeprefix("pair-")))
                for p in root.glob("*/pair-*/comparison.json")}
    if observed - expected:
        raise ValueError("unregistered comparison")
    complete = read(root / "complete.json") if (root / "complete.json").exists() else None
    if not partial and (observed != expected or complete != {"complete": True, "pairs": len(expected)}):
        raise ValueError("incomplete rotation; use --partial for diagnostic progress only")
    bindings = {name: digest(root / name) for name in ("identity.json", "plan.json")}
    if complete is not None:
        bindings["complete.json"] = digest(root / "complete.json")
    cases, raw, statuses, oracle_failures = [], {}, Counter(), []
    for case in plan["cases"]:
        pairs, measurements = [], defaultdict(list)
        for rep, seed in enumerate(case["seeds"], 1):
            directory = root / case["id"] / f"pair-{rep}"
            if (case["id"], rep) not in observed:
                pairs.append({"seed": seed, "status": "missing"})
                continue
            record = read(directory / "comparison.json")
            if (record["case"], record["repetition"], record["seed"]) != (case["id"], rep, seed):
                raise ValueError("comparison disagrees with registration")
            if record["order"] != plan["orders"][(rep - 1) % len(plan["orders"])]:
                raise ValueError("comparison order disagrees with registration")
            arms = {}
            for variant in ("baseline", "candidate", "stock"):
                receipt = read(directory / variant / "result.json")
                arm = case["stock_arm"] if variant == "stock" else ("fereg" if case["cell"]["model"] == "ols" else "feppml")
                if receipt["cell"] != case["cell"] or receipt["arm"] != arm:
                    raise ValueError("arm or cell disagrees with registration")
                if receipt["measurement_fingerprint"] != fingerprint:
                    raise ValueError("measurement identity mismatch")
                arms[variant] = receipt
                statuses[f"{variant}:{receipt['status']}"] += 1
                if receipt["status"] == "pass":
                    for field in ("seconds", "elapsed_seconds", "peak_rss_gib"):
                        if positive(receipt.get(field)):
                            measurements[f"{variant}_{field}"].append(receipt[field])
                raw[case["id"], seed, variant] = receipt
            for path in directory.rglob("*"):
                if path.is_file() and path.suffix in (".json", ".csv"):
                    bindings[str(path.relative_to(root))] = digest(path)
            if case.get("oracle"):
                for variant in ("baseline", "candidate"):
                    checks = record.get("oracle_checks", {}).get(variant, {})
                    if not checks or not all(checks.values()):
                        oracle_failures.append({"case": case["id"], "seed": seed,
                                                "variant": variant, "checks": checks})
            pair = {"seed": seed, "status": "complete", "comparisons": {}}
            for reference in ("baseline", "stock"):
                comparison = record["comparisons"][reference]
                reasons = [name for name, passed in comparison["checks"].items() if not passed]
                if comparison["status"] != "pass":
                    reasons.append(comparison["status"])
                if not comparison["checks"]:
                    reasons.append("missing_scientific_checks")
                for variant in ("candidate", reference):
                    arm = arms[variant]
                    if arm["status"] != "pass":
                        reasons.append(f"{variant}:{arm['status']}")
                    if not positive(arm.get("seconds")):
                        reasons.append(f"{variant}:invalid_command_time")
                    if not arm.get("sample_sha256"):
                        reasons.append(f"{variant}:missing_exact_sample")
                    if case.get("oracle") and variant != "stock":
                        checks = record.get("oracle_checks", {}).get(variant, {})
                        if not checks or not all(checks.values()):
                            reasons.append(f"{variant}:independent_oracle")
                if arms["candidate"].get("sample_sha256") != arms[reference].get("sample_sha256"):
                    reasons.append("sample_mismatch")
                result = {"eligible": not reasons, "reasons": sorted(set(reasons))}
                if not reasons:
                    result["seconds_ratio"] = arms["candidate"]["seconds"] / arms[reference]["seconds"]
                    if all(positive(arms[v].get("peak_rss_gib")) for v in ("candidate", reference)):
                        result["rss_ratio"] = arms["candidate"]["peak_rss_gib"] / arms[reference]["peak_rss_gib"]
                pair["comparisons"][reference] = result
            pairs.append(pair)
        summary = {"case": case["id"], "cell": case["cell"], "stock_arm": case["stock_arm"],
                   "oracle": bool(case.get("oracle")), "pairs": pairs,
                   "measurements": {key: distribution(values) for key, values in measurements.items()},
                   "comparisons": {}}
        for reference in ("baseline", "stock"):
            eligible = all(p.get("comparisons", {}).get(reference, {}).get("eligible", False) for p in pairs)
            summary["comparisons"][reference] = {"eligible": eligible,
                "seconds_ratio": distribution([p["comparisons"][reference]["seconds_ratio"] for p in pairs]) if eligible else None,
                "rss_ratio": distribution([p["comparisons"][reference]["rss_ratio"] for p in pairs
                                           if "rss_ratio" in p["comparisons"][reference]]) if eligible else None}
        cases.append(summary)
    by_case = {case["case"]: case for case in cases}
    groups = defaultdict(list)
    for case in plan["cases"]:
        key = json.dumps({k: v for k, v in case["cell"].items() if k != "threads"}, sort_keys=True)
        groups[key].append(case)
    scaling = []
    for group in groups.values():
        group.sort(key=lambda c: c["cell"]["threads"])
        for low, high in zip(group, group[1:]):
            if low["cell"]["threads"] == high["cell"]["threads"]:
                raise ValueError("duplicate thread setting in scaling family")
            ratios, reasons = [], []
            for case in (low, high):
                if not by_case[case["id"]]["comparisons"]["baseline"]["eligible"]:
                    reasons.append(f"{case['id']}:native_science_unavailable")
            if low["seeds"] != high["seeds"]:
                reasons.append("different_registered_seeds")
            for seed in low["seeds"]:
                a, b = raw.get((low["id"], seed, "candidate")), raw.get((high["id"], seed, "candidate"))
                if not a or not b or a["status"] != "pass" or b["status"] != "pass":
                    reasons.append(f"seed-{seed}:unavailable")
                    continue
                if not a.get("input_sha256") or a["input_sha256"] != b.get("input_sha256") or not a.get("sample_sha256") or a["sample_sha256"] != b.get("sample_sha256"):
                    reasons.append(f"seed-{seed}:different_input_or_sample")
                if not all(positive(r.get("seconds")) for r in (a, b)):
                    reasons.append(f"seed-{seed}:invalid_command_time")
                else:
                    ratios.append(b["seconds"] / a["seconds"])
            scaling.append({"lower_case": low["id"], "higher_case": high["id"],
                "eligible": not reasons, "reasons": reasons,
                "higher_over_lower": distribution(ratios) if not reasons else None,
                "slower_on_every_seed": min(ratios) > 1 if not reasons and ratios else None})
    targets = defaultdict(list)
    for case in cases:
        # Small oracle probes and strict references never dilute headline losses.
        key = f"{case['cell']['model']}:{case['stock_arm']}:{'oracle' if case['oracle'] else 'timing'}"
        targets[key].append(case)
    target_counts = {}
    for key, group in targets.items():
        eligible = [c for c in group if c["comparisons"]["stock"]["eligible"]]
        ratios = [c["comparisons"]["stock"]["seconds_ratio"]["median"] for c in eligible]
        target_counts[key] = {"registered": len(group), "ranked": len(eligible),
            "unranked": len(group) - len(eligible), "at_most_0_90": sum(r <= 0.90 for r in ratios),
            "above_1_20": sum(r > 1.20 for r in ratios), "case_median_ratios": distribution(ratios)}
    return {"schema": "fereg-rotation-summary-v1", "complete": observed == expected and complete == {"complete": True, "pairs": len(expected)},
            "registered_pairs": len(expected), "completed_pairs": len(observed),
            "statuses": dict(sorted(statuses.items())), "oracle_failures": oracle_failures,
            "cases": cases, "scaling": scaling, "targets": target_counts, "bindings": bindings,
            "interpretation": "Ratios use command clocks. Case medians require all registered seeds to pass. Scaling is diagnostic on native-certified same-input/sample fits. Two seeds do not establish a statistical guarantee."}


def markdown(summary):
    lines = ["# Bounded performance rotation", "", summary["interpretation"], "",
             f"Completed {summary['completed_pairs']}/{summary['registered_pairs']} registered pairs.", "",
             "| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |",
             "|---|---:|---:|"]
    def ratios(value):
        return "NR" if value is None else " / ".join(f"{value[k]:.3f}" for k in ("min", "median", "max"))
    for case in summary["cases"]:
        lines.append(f"| {case['case']} | {ratios(case['comparisons']['baseline']['seconds_ratio'])} | {ratios(case['comparisons']['stock']['seconds_ratio'])} |")
    lines += ["", "NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.",
              "", "## Native thread scaling", "", "| Lower → higher setting | Higher/lower min / median / max | Slower on every seed |", "|---|---:|---|"]
    for row in summary["scaling"]:
        lines.append(f"| {row['lower_case']} → {row['higher_case']} | {ratios(row['higher_over_lower'])} | {row['slower_on_every_seed']} |")
    lines += ["", f"Independent-oracle failures: {len(summary['oracle_failures'])}.", "",
              "Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.", ""]
    return "\n".join(lines)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("results", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--partial", action="store_true")
    args = parser.parse_args()
    result = summarize(args.results, partial=args.partial)
    args.output.mkdir(parents=True, exist_ok=False)
    (args.output / "summary.json").write_text(json.dumps(result, indent=2, allow_nan=False) + "\n")
    (args.output / "README.md").write_text(markdown(result))
