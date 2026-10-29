#!/usr/bin/env python3
"""Compare pinned whitening ranks with exact rational input-space witnesses.

Four bounded phases, run once in order: prepare, build, run, report.
All generated files stay in --work-dir. No timing is collected.
"""
from __future__ import annotations

import argparse
from fractions import Fraction as F
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess

FORK = "3b5b9b9ab9b79e4a8cac2841f7c1d41bb2276104"
CANDIDATE = "41c53760e233ed6b0766af74f55951a4a0d51067"
RANK_TOL = F.from_float(1e-10)
DELTAS = [0.0, 0.5e-5, 0.99e-5, 1.01e-5, 2e-5]
TRANSFORMS = [(0.0, 1.0), (2.0**24, 1.0), (-2.0**24, 8.0), (1024.0, 2.0**-8)]
PROJECTION_BOUND = 1e-8
CERTIFICATE_BOUND = 1e-9
RECONSTRUCTION_GAMMA = 64 * math.ulp(1.0)


def write(path, value):
    path.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def input_digest(f):
    h = hashlib.sha256()
    columns = [(f["levels"], "Q")] + [(v, "d") for v in [*f["slopes"], f["weights"], f["y"]]]
    for field, code in columns:
        h.update(struct.pack("<Q", len(field)))
        for value in field:
            h.update(struct.pack("<" + code, value))
    return h.hexdigest()


def fixtures():
    result = []
    for d, delta in enumerate(DELTAS):
        for t, (shift, scale) in enumerate(TRANSFORMS):
            rows = []
            for level_index, level in enumerate([3, 11, 29, 97]):
                for weight in [2.0, 3.0, 5.0, 7.0]:
                    for signs in range(8):
                        u, v, e = [1.0 if signs & (1 << k) else -1.0 for k in range(3)]
                        a = scale * (level_index + 1)
                        rows.append((level, shift + a*u, -shift + a*(u + delta*v), weight,
                                     0.125*level_index + 0.3*u + 0.2*v + 0.4*e))
            # Fixed permutations, never selected from observed outcomes.
            orders = [list(range(128)), list(reversed(range(128))),
                      sorted(range(128), key=lambda i: ((1103515245*i + 12345) & 0x7fffffff))]
            for order_index, order in enumerate(orders):
                r = [rows[i] for i in order]
                result.append({"id": f"delta{d}-transform{t}-order{order_index}",
                               "levels": [x[0] for x in r], "slopes": [[x[j] for x in r] for j in [1, 2]],
                               "weights": [x[3] for x in r], "y": [x[4] for x in r]})
    return result


def oracle(f):
    """Exact weighted moments and Gram-Schmidt projection, no native kernels."""
    dropped, residual, certificates, ratios = [], [F(0)]*len(f["y"]), [], []
    for level in sorted(set(f["levels"])):
        idx = [i for i, label in enumerate(f["levels"]) if label == level]
        w = [F.from_float(f["weights"][i]) for i in idx]
        y = [F.from_float(f["y"][i]) for i in idx]
        total = sum(w)
        dot = lambda a, b: sum(wi*ai*bi for wi, ai, bi in zip(w, a, b))
        cols = [[F.from_float(f["slopes"][j][i]) for i in idx] for j in range(2)]
        centers = [sum(wi*x for wi, x in zip(w, col))/total for col in cols]
        z = [[x-center for x in col] for col, center in zip(cols, centers)]
        gram = [[dot(a, b) for b in z] for a in z]
        pivot = max(range(2), key=lambda j: (gram[j][j], j))
        other = 1-pivot
        ratio = (gram[other][other]-gram[other][pivot]**2/gram[pivot][pivot])/gram[other][other]
        kept = [pivot] + ([other] if ratio > RANK_TOL else [])
        dropped.extend([0, j+1, level] for j in range(2) if j not in kept)
        ratios.append({"level": level, "ratio": float(ratio), "kept_columns": sorted(j+1 for j in kept)})
        # Exact rational, observation-space orthogonalization of retained raw columns.
        basis = [[F(1)]*len(idx)]
        for j in sorted(kept):
            q = z[j].copy()
            for prior in basis:
                c = dot(q, prior)/dot(prior, prior)
                q = [x-c*p for x, p in zip(q, prior)]
            basis.append(q)
        fitted = [F(0)]*len(idx)
        for q in basis:
            c = dot(y, q)/dot(q, q)
            fitted = [value+c*qi for value, qi in zip(fitted, q)]
            norm = math.sqrt(float(dot(q, q)))
            certificates.append({"indices": idx, "unit_column": [float(x)/norm for x in q]})
        for i, yi, fit in zip(idx, y, fitted):
            residual[i] = yi-fit
    return {"unidentified": sorted(dropped), "demeaned": [float(x) for x in residual],
            "certificates": certificates, "pivots": ratios}


def prepare(root):
    if root.exists() and any(root.iterdir()):
        raise SystemExit("refusing to overwrite an existing declaration")
    root.mkdir(parents=True, exist_ok=True)
    source = Path(__file__).with_name("within_slope_boundary.rs")
    (root / "main.rs").write_bytes(source.read_bytes())
    data = fixtures()
    write(root / "fixtures.json", data)
    write(root / "oracle.json", {f["id"]: oracle(f) for f in data})
    (root / "Cargo.toml").write_text(f'''[package]
name = "within-slope-boundary-witness"
version = "0.1.0"
edition = "2021"
[workspace]
[[bin]]
name = "slope-boundary"
path = "main.rs"
[dependencies]
within_fork = {{ package = "within", git = "https://github.com/johannes-schmieder/within", rev = "{FORK}" }}
within_candidate = {{ package = "within", git = "https://github.com/johannes-schmieder/within", rev = "{CANDIDATE}" }}
rayon = "1.12"
serde = {{ version = "1", features = ["derive"] }}
serde_json = {{ version = "1", features = ["float_roundtrip"] }}
sha2 = "0.10"
[profile.dev]
opt-level = 2
debug = 0
incremental = false
''')
    write(root / "declaration.json", {
        "fork": FORK, "candidate": CANDIDATE, "deltas": DELTAS, "transforms": TRANSFORMS,
        "workers": [1, 4], "repeats": 2, "fixture_count": len(data), "calls": 4,
        "rows_per_fixture": 128, "levels": [3, 11, 29, 97], "weights": [2, 3, 5, 7],
        "solver": "Off; tolerance 1e-12; maxiter 100; local_size 8; fork screening disabled and reproducible mode",
        "scope": "isolated per-level within whitening boundary; no fereg sample/rank/omission policy changed; not full native qualification",
        "rank_oracle": "exact rational centered Gram on actual IEEE input; largest diagonal pivot, higher original column on ties; residual variance > exact binary64 1e-10 times own initial variance",
        "nominal_gram": "W*a^2 * [[1,1],[1,1+delta^2]]; actual input quantization handled exactly",
        "projection_bound": PROJECTION_BOUND, "normalized_retained_span_certificate_bound": CERTIFICATE_BOUND,
        "reconstruction": "Exact rational sum of returned original-coordinate x times input loadings, compared with y-demeaned; bound 1e-8 + 64*eps*sum(abs(products)). Reports unscaled maximum too; shifted near-alias coordinates are intrinsically ill-conditioned.",
        "reconstruction_gamma": RECONSTRUCTION_GAMMA,
        "labels": "Both libraries receive original labels 3/11/29/97. Fork max(label)+1 storage also returns absent levels: retain raw outputs, require every absent slot zero and unidentified, and compare exact rank identities on all observed labels. Candidate compaction must return exactly the observed layout.",
        "input_transport": "serde_json float_roundtrip; SHA-256 of lengths and little-endian level/IEEE-bit arrays compared for every native record",
        "acceptance": "Every arm agrees with independent exact rank identities and retained-span projection; fork/candidate identities exact; fixed-worker fields repeat exactly; cross-worker identities exact and projections within declared bound. Failures retained; no fixture replacement or tolerance changes.",
        "runner_sha256": digest(Path(__file__)),
        "hashes": {p: digest(root/p) for p in ["main.rs", "fixtures.json", "oracle.json", "Cargo.toml"]},
    })


def check_frozen(root):
    declaration = json.loads((root / "declaration.json").read_text())
    assert digest(Path(__file__)) == declaration["runner_sha256"], "changed frozen runner"
    for name, expected in declaration["hashes"].items():
        assert digest(root/name) == expected, f"changed frozen {name}"
    return declaration


def build(root):
    check_frozen(root)
    for args in [["cargo", "generate-lockfile", "--offline"],
                 ["cargo", "build", "--offline", "--locked", "-j", "4", "--target-dir", str(root/"cargo")]]:
        subprocess.run(args, cwd=root, check=True)
    lock = (root/"Cargo.lock").read_text()
    assert FORK in lock and CANDIDATE in lock
    write(root/"build.json", {"binary_sha256": digest(root/"cargo/debug/slope-boundary"),
                              "lock_sha256": digest(root/"Cargo.lock"),
                              "rustc": subprocess.check_output(["rustc", "-Vv"], text=True),
                              "cargo": subprocess.check_output(["cargo", "-V"], text=True)})


def run(root):
    check_frozen(root)
    binary = root/"cargo/debug/slope-boundary"
    assert digest(binary) == json.loads((root/"build.json").read_text())["binary_sha256"]
    for arm in ["fork", "candidate"]:
        for workers in [1, 4]:
            output = root/f"{arm}-{workers}.jsonl"
            with output.open("x") as stream:
                subprocess.run([str(binary), arm, str(workers), str(root/"fixtures.json")],
                               check=True, stdout=stream, timeout=60)


def report(root):
    declaration = check_frozen(root)
    data = {f["id"]: f for f in json.loads((root/"fixtures.json").read_text())}
    expected = json.loads((root/"oracle.json").read_text())
    failures, metrics, seen = [], [], {}
    for arm in ["fork", "candidate"]:
        for workers in [1, 4]:
            rows = [json.loads(line) for line in (root/f"{arm}-{workers}.jsonl").read_text().splitlines()]
            assert len(rows) == declaration["fixture_count"]*2
            for row in rows:
                name, r = row["fixture"], row["result"]
                key = (arm, workers, name, row["repeat"])
                assert key not in seen
                seen[key] = r
                f, o = data[name], expected[name]
                assert row["input_sha256"] == input_digest(f), f"input transport changed: {key}"
                if "error" in r:
                    failures.append({"key": key, "error": r["error"]})
                    continue
                if (len(r["demeaned"]) != len(f["y"]) or len(r["layout"]) != len(r["x"])
                        or not all(isinstance(x, (float, int)) and math.isfinite(x)
                                   for x in r["x"]+r["demeaned"]+[r["normal_ratio"]])):
                    failures.append({"key": key, "finite_values_and_lengths": False})
                    continue
                labels = set(f["levels"])
                observed = sorted(a for a in r["unidentified"] if a[2] in labels)
                layout = {tuple(a): x for a, x in zip(r["layout"], r["x"])}
                absent = {a for a in layout if a[2] not in labels}
                dropped = {tuple(a) for a in r["unidentified"]}
                expected_layout = {(0, j, level) for j in range(3) for level in labels}
                entire_layout = ({(0, j, level) for j in range(3) for level in range(max(labels)+1)}
                                 if arm == "fork" else expected_layout)
                layout_valid = (len(layout) == len(r["x"]) and set(layout) == entire_layout
                                and len(dropped) == len(r["unidentified"]) and dropped <= set(layout)
                                and set(layout)-absent == expected_layout
                                and all(a in dropped and layout[a] == 0.0 for a in absent)
                                and (arm == "fork" or not absent))
                reconstruction_error, reconstruction_fraction = 0.0, 0.0
                if layout_valid:
                    for i, level in enumerate(f["levels"]):
                        products = [F.from_float(layout[0, 0, level])]
                        products.extend(F.from_float(layout[0, j+1, level])*F.from_float(f["slopes"][j][i]) for j in range(2))
                        difference = abs(sum(products)-(F.from_float(f["y"][i])-F.from_float(r["demeaned"][i])))
                        bound = F.from_float(PROJECTION_BOUND) + F.from_float(RECONSTRUCTION_GAMMA)*sum(map(abs, products))
                        reconstruction_error = max(reconstruction_error, float(difference))
                        reconstruction_fraction = max(reconstruction_fraction, float(difference/bound))
                error = max(abs(a-b) for a, b in zip(r["demeaned"], o["demeaned"]))
                ynorm = math.sqrt(math.fsum(w*y*y for w,y in zip(f["weights"], f["y"])))
                certificate = max(abs(math.fsum(f["weights"][i]*q*r["demeaned"][i]
                                               for i,q in zip(c["indices"], c["unit_column"]))) / ynorm
                                  for c in o["certificates"])
                checks = {"converged": r["converged"], "rank": observed == o["unidentified"],
                          "projection": error <= PROJECTION_BOUND, "certificate": certificate <= CERTIFICATE_BOUND,
                          "layout_and_empty_slots": layout_valid, "coefficient_reconstruction": reconstruction_fraction <= 1,
                          "finite": all(math.isfinite(x) for x in r["x"]+r["demeaned"])}
                metrics.append({"key": key, "max_projection_error": error, "certificate": certificate, "checks": checks,
                                "max_reconstruction_error": reconstruction_error, "max_reconstruction_bound_fraction": reconstruction_fraction})
                if not all(checks.values()):
                    failures.append({"key": key, "checks": checks, "actual_dropped": observed,
                                     "expected_dropped": o["unidentified"], "projection_error": error, "certificate": certificate})
    for name in data:
        for arm in ["fork", "candidate"]:
            for workers in [1, 4]:
                if seen[arm, workers, name, 0] != seen[arm, workers, name, 1]:
                    failures.append({"key": [arm, workers, name], "repeatability": False})
            a, b = seen[arm, 1, name, 0], seen[arm, 4, name, 0]
            if "error" not in a and "error" not in b:
                if a["unidentified"] != b["unidentified"] or max(abs(x-y) for x,y in zip(a["demeaned"], b["demeaned"])) > PROJECTION_BOUND:
                    failures.append({"key": [arm, name], "cross_worker_agreement": False})
        for workers in [1, 4]:
            a, b = seen["fork", workers, name, 0], seen["candidate", workers, name, 0]
            a_observed = sorted(x for x in a.get("unidentified", []) if x[2] in set(data[name]["levels"]))
            b_observed = sorted(x for x in b.get("unidentified", []) if x[2] in set(data[name]["levels"]))
            if "error" not in a and "error" not in b and a_observed != b_observed:
                failures.append({"key": [workers, name], "migration_rank_parity": False,
                                 "fork": a_observed, "candidate": b_observed})
    summary = {"pass": not failures, "records": len(seen), "failures": failures,
               "max_projection_error": max((m["max_projection_error"] for m in metrics), default=None),
               "max_certificate": max((m["certificate"] for m in metrics), default=None), "metrics": metrics,
               "max_reconstruction_error": max((m["max_reconstruction_error"] for m in metrics), default=None),
               "max_reconstruction_bound_fraction": max((m["max_reconstruction_bound_fraction"] for m in metrics), default=None),
               "declaration_sha256": digest(root/"declaration.json")}
    write(root/"report.json", summary)
    print(json.dumps({k:v for k,v in summary.items() if k not in ["metrics", "failures"]}))
    print(f"Failures: {len(failures)}")
    return 0 if summary["pass"] else 1


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("phase", choices=["prepare", "build", "run", "report"])
    p.add_argument("--work-dir", type=Path, required=True)
    args = p.parse_args()
    root = args.work_dir.resolve()
    if args.phase == "prepare":
        prepare(root)
    elif args.phase == "build":
        build(root)
    elif args.phase == "run":
        run(root)
    else:
        return report(root)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
