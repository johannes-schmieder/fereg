"""Attribute preserved CMG forecast errors; no estimation or fitted multiplier."""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import zipfile


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def graph_counts(kind, n):
    # Exact integer graph construction from the preserved calibration generator.
    edges = Counter()
    for i in range(n):
        u, v = i % 128, (i // 128 + i % 17) % 64
        if kind == "tiny":
            u, v = (i // 64) * 2 + i % 2, (i // 64) * 2 + (i // 2) % 2
        elif kind == "mixed" and i >= n // 2:
            block = (i - n // 2) // 64
            u, v = 128 + block * 2 + i % 2, 64 + block * 2 + (i // 2) % 2
        edges[((0, u), (1, v))] += 1
    parent, degree, rows = {}, Counter(), Counter()

    def root(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    for (u, v), count in edges.items():
        for x in (u, v):
            parent.setdefault(x, x)
            degree[x] += 1
            rows[x] += count
        parent[root(u)] = root(v)
    components = Counter(root(x) for x in parent)
    return {
        "observations": n, "vertices": len(parent), "canonical_edges": len(edges),
        "component_size_histogram": dict(sorted(Counter(components.values()).items())),
        "degree_histogram": dict(sorted(Counter(degree.values()).items())),
        "minimum_observations_per_vertex": min(rows.values()),
    }


def principal_terms(n, v, threads, rhs, canonical_edges):
    # Pinned CMG memory.rs, default CmgOptions, 64-bit Linux, no workspace cap.
    # Excludes handle layouts and small fereg certificate bookkeeping.
    level_vertices = v * 128
    hierarchy_nnz = 6 * (v + 2 * canonical_edges)
    direct, concurrency = min(700, v), min(threads, rhs)
    return {
        "raw_input_edges": n * 16,
        "hierarchy_sparse": hierarchy_nnz * 8,
        "hierarchy_vertices": level_vertices * 48,
        "hierarchy_reports": 128 * 8 * 4,
        "terminal_factor": direct * direct * 16,
        "parallel_plan_sparse": hierarchy_nnz * 16,
        "parallel_plan_rows": level_vertices * 8,
        "plan_build_edge_scratch": hierarchy_nnz * 8,
        "plan_build_row_scratch": level_vertices * 32,
        "workspace_finest": v * 168 * concurrency,
        "workspace_recursive": level_vertices * 48 * concurrency,
        "workspace_terminal": direct * 24 * concurrency,
        "runtime_heap_allowance": (32 + 2 * threads) * 1024**2,
        "requested_worker_stacks": 8 * threads * 1024**2,
        "fereg_batch_vectors": v * rhs * 16,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cmg-source", type=Path, required=True)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[4]
    evidence = repo / "benchmarks/reports/development/pragmatic-memory-components-2026-09-18"
    manifest_path = repo / "benchmarks/manifests/pragmatic_memory_development_v2.json"
    manifest = json.loads(manifest_path.read_text())
    cases = {x["id"]: x for x in manifest["cases"]}
    calibration = {x["case"]: x for x in json.loads((evidence / "memory-calibration.json").read_text())}
    phases = {x["case"]: x for x in json.loads((evidence / "components-phase-summary.json").read_text())}
    bindings = json.loads((evidence / "memory-generated-bindings.json").read_text())
    hashes = {"manifest": sha256(manifest_path)}
    for name in ("diagnostics.zip", "memory-calibration.json", "components-phase-summary.json",
                 "memory-generated-bindings.json", "harness/calibration-r5/generate.py"):
        hashes[name] = sha256(evidence / name)
    for name in ("src/memory.rs", "src/options.rs", "src/execution.rs", "src/hierarchy.rs"):
        hashes["cmg/" + name] = sha256(args.cmg_source / name)
    for name in ("crates/fereg-core/src/fe/cmg.rs", "crates/fereg-core/src/fe/cmg_admission.rs",
                 "crates/fereg-plugin/src/estimate.rs"):
        hashes[name] = sha256(repo / name)
    results = []
    with zipfile.ZipFile(evidence / "diagnostics.zip") as archive:
        for case in ("ols_connected_cmg", "ols_tiny_components", "ols_mixed_components"):
            request, record = cases[case], calibration[case]
            cell, receipt = request["cell"], record["receipt"]
            graph = graph_counts(request["graph"], cell["n"])
            assert receipt["status"] == "pass", receipt["status"]
            assert receipt["metadata"]["N"] == cell["n"]
            assert receipt["metadata"]["num_singletons"] == 0
            assert graph["minimum_observations_per_vertex"] >= 2
            assert min(graph["degree_histogram"]) >= 2
            rhs = int(receipt["metadata"]["fereg_poolsize"])
            terms = principal_terms(cell["n"], graph["vertices"], cell["threads"], rhs, cell["n"])
            trace = archive.read(f"memory-diagnostics/{case}/calibration/stderr.txt").decode()
            workspace = int(re.search(r"stage=fit\|recorded=true\|parents=\d+\|workspace=(\d+)", trace)[1])
            remainder = workspace - sum(terms.values())
            # The large terms must explain the logged estimate independently;
            # do not silently fold an unexplained discrepancy into the model.
            assert 0 <= remainder <= 1024, (case, remainder)
            exact_edges = principal_terms(cell["n"], graph["vertices"], cell["threads"], rhs,
                                          graph["canonical_edges"])
            results.append({
                "case": case, "graph": graph, "input_binding": bindings[case],
                "measurement_fingerprint": receipt["measurement_fingerprint"],
                "native_forecast_bytes": record["memory_audit"]["native_predicted_bytes"],
                "observed_whole_abi_native_bytes": record["memory_audit"]["native_observed_bytes"],
                "logged_solver_workspace_bytes": workspace,
                "reconstructed_principal_terms_bytes": terms,
                "unattributed_small_layout_bytes": remainder,
                "counterfactual_exact_edges_reduction_bytes": sum(terms.values()) - sum(exact_edges.values()),
                "component_pair_native_peak_medians_bytes": {
                    arm: stats["native_peak_bytes"] for arm, stats in phases[case]["medians"].items()
                },
                "finest_is_direct_by_source": graph["vertices"] < 700,
            })
    print(json.dumps({"schema": 1, "scope": "attribution of preserved development receipts; no new solver run",
                      "counterfactuals_are_not_validated_forecasts": True,
                      "input_sha256": hashes, "cases": results}, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
