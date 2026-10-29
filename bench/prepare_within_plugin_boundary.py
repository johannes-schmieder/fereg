#!/usr/bin/env python3
"""Freeze three public OLS slope-boundary cases using exact input-space math.

No native libraries, builds or estimation are invoked. --write updates the
maintained CSV and prospective declaration; the default checks their contents.
The original 60-case dependency witness and its frozen declaration stay intact.
"""
from __future__ import annotations

import argparse
import csv
from fractions import Fraction as F
import hashlib
import io
import json
import math
from pathlib import Path

from compare_within_slope_boundary import fixtures, oracle

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "tests/fixtures/ols/within_plugin_slope_boundary.csv"
DECLARATION = ROOT / "bench/results/within-plugin-slope-boundary-declaration.json"
STATA = ROOT / "tests/stata/development/within_plugin_slope_boundary.do"
CASES = ["delta0-transform1-order0", "delta2-transform1-order0", "delta3-transform1-order0"]
FINITE_TOL = F.from_float(1e-9)


def stata_hex(value):
    mantissa, exponent = value.hex().split("p")
    sign = "-" if mantissa.startswith("-") else "+"
    mantissa = mantissa.removeprefix("-").removeprefix("0x")
    power = int(exponent)
    return f"{sign}{mantissa}X{'-' if power < 0 else '+'}{abs(power):03x}"


def exact_fit(f):
    """Project onto the independently retained FE span, then fit finite q/v."""
    w = list(map(F.from_float, f["weights"]))
    dot = lambda a, b: sum(wi*x*y for wi, x, y in zip(w, a, b))
    pivots = oracle(f)["pivots"]
    basis, basis_rows = [], [[0.0]*128 for _ in range(3)]
    for decision in pivots:
        indices = [i for i, level in enumerate(f["levels"]) if level == decision["level"]]
        columns = [[F(1)]*128] + [list(map(F.from_float, f["slopes"][j-1]))
                                  for j in decision["kept_columns"]]
        for channel, raw in enumerate(columns):
            z = [x if i in indices else F(0) for i, x in enumerate(raw)]
            for prior in basis:
                coefficient = dot(z, prior)/dot(prior, prior)
                z = [x-coefficient*p for x, p in zip(z, prior)]
            assert dot(z, z) > 0
            norm = math.sqrt(float(dot(z, z)))
            for i in indices:
                basis_rows[channel][i] = float(z[i])/norm
            basis.append(z)

    def project(raw):
        result = list(map(F.from_float, raw))
        for z in basis:
            coefficient = dot(result, z)/dot(z, z)
            result = [x-coefficient*p for x, p in zip(result, z)]
        return result

    py, pq, pv = (project(f[name]) for name in ["y", "q", "v"])
    raw_v = list(map(F.from_float, f["v"]))
    v_ratio = dot(pv, pv)/dot(raw_v, raw_v)
    keep_v = v_ratio > FINITE_TOL
    g00, g01, g11 = dot(pq, pq), dot(pq, pv), dot(pv, pv)
    h0, h1 = dot(pq, py), dot(pv, py)
    if keep_v:
        determinant = g00*g11-g01*g01
        assert determinant > 0
        beta_q, beta_v = (h0*g11-h1*g01)/determinant, (h1*g00-h0*g01)/determinant
    else:
        beta_q, beta_v = h0/g00, F(0)
    residual = [y-beta_q*q-beta_v*v for y, q, v in zip(py, pq, pv)]
    active = [pq, pv] if keep_v else [pq]
    # Independent, exact certificates. The intentionally omitted v has its own
    # relative-energy witness; it is not silently treated as an exact zero.
    assert all(dot(residual, column) == 0 for column in basis+active)
    constant = sum(wi*(F.from_float(y)-beta_q*F.from_float(q)-beta_v*F.from_float(v))
                   for wi, y, q, v in zip(w, f["y"], f["q"], f["v"]))/sum(w)
    rss = dot(residual, residual)*F(128)/sum(w)  # analytic-weight normalization
    finite_basis = [[float(x)/math.sqrt(float(dot(z, z))) for x in z]
                    for z in active]
    if not keep_v:
        finite_basis.append([0.0]*128)
    return {
        "rank": 1+int(keep_v), "v_status": 1 if keep_v else 3,
        "beta_q": float(beta_q), "beta_v": float(beta_v), "constant": float(constant),
        "rss": float(rss), "v_residual_energy_ratio": float(v_ratio), "pivots": pivots,
        "residual": list(map(float, residual)), "fe_basis": basis_rows,
        "finite_basis": finite_basis,
        "response_norm": math.sqrt(float(dot(list(map(F.from_float, f["y"])),
                                             list(map(F.from_float, f["y"]))))),
    }


def contents():
    originals = {f["id"]: f for f in fixtures()}
    rows, expected = [], []
    for case, name in enumerate(CASES, 1):
        f = originals[name]
        f["v"] = [1.0 if (i % 8) & 2 else -1.0 for i in range(128)]
        u = [1.0 if (i % 8) & 1 else -1.0 for i in range(128)]
        f["q"] = [a*b for a, b in zip(u, f["v"])]
        f["y"] = [y+0.7*q for y, q in zip(f["y"], f["q"])]
        fit = exact_fit(f)
        expected.append({"case": case, "dependency_fixture": name,
                         **{k: fit[k] for k in ["rank", "v_status", "beta_q", "beta_v", "constant",
                                                "rss", "v_residual_energy_ratio", "pivots"]}})
        for i in range(128):
            rows.append([case, i+1, f["levels"][i], f["slopes"][0][i], f["slopes"][1][i],
                         f["weights"][i], f["y"][i], f["q"][i], f["v"][i],
                         fit["residual"][i], f["y"][i]-fit["residual"][i],
                         *[fit[k] for k in ["rank", "v_status", "beta_q", "beta_v", "constant",
                                           "rss", "v_residual_energy_ratio", "response_norm"]],
                         *[column[i] for column in fit["fe_basis"]+fit["finite_basis"]],
                         *[stata_hex(values[i]) for values in [*f["slopes"], f["y"]]]])
    stream = io.StringIO(newline="")
    writer = csv.writer(stream, lineterminator="\n")
    writer.writerow("case row g z1 z2 w y q v ref_resid ref_fitted ref_rank ref_v_status ref_bq ref_bv ref_cons ref_rss ref_v_energy ref_ynorm fe0 fe1 fe2 finite_q finite_v hex_z1 hex_z2 hex_y".split())
    writer.writerows(rows)
    data = stream.getvalue()
    declaration = {
        "status": "prospective; native plugin not yet executed",
        "within_candidate": "d3407a16dec3715f6e9eee13fb9b6646d5b28035",
        "scope": "Actual public weighted sloped OLS explicit LSMR publication under the approved upstream whitening correction. No runtime or grammar change; no timing. Other scientific paths are unaffected by this fixture.",
        "cases": expected, "workers": [1, 4], "repeats_per_worker": 2,
        "native_estimation_calls": 12, "additional_frontend_only_rejections": 3,
        "rows_per_case": 128, "levels": [3, 11, 29, 97], "weights": [2, 3, 5, 7],
        "command": "fereg y q v i.g [aw=w], absorb(g##c.(z1 z2)) solver(lsmr) preconditioner(off) memory(fast) tolerance(1e-10) iterate(100) threads(WORKERS) deterministic residuals(actual_r) xbd(actual_f)",
        "oracle": "Reuse the maintained 60-fixture exact-rational centered-Gram rank decisions on actual binary64 slopes. Independently orthogonalize retained raw columns in weighted observation space, residualize rounded y+.7*(u*v), q=u*v and v, and solve their exact rational finite normal equations. Existing FE-collinearity rule is residual energy/raw energy <= exact binary64 1e-9. All retained FE and finite score moments are exactly zero. Nonzero omitted-v energy is reported explicitly.",
        "sample": "All 128 original rows, no singleton removals; rank 2/2/1 for cases1/2/3.",
        "original_axis": "q v 3b.g 11o.g 29o.g 97o.g _cons; replace v by o.v only in case3. Slots1..7 unchanged, statuses1,(1/1/3),2,3,3,3,5. Omitted coefficient and covariance rows/columns are exactly zero. Explicit ib3.g with sloped absorption remains unsupported rc498 and must preserve the accepted estimates and absent requested output.",
        "acceptance": {"actual_solver_code": 3, "actual_solver_name": "lsmr", "fallback": False,
                       "coefficients_absolute": 1e-8, "residual_and_fitted_absolute": 1e-8,
                       "rss_relative": 1e-8, "independent_normalized_moments": 1e-9,
                       "same_worker": "Exact b, V, status, sample, residual/fitted and solver fields except timing columns9/10.",
                       "cross_worker": "Exact sample, rank, stripes, statuses and both solver rows' discrete columns1/2/4/11/12/13/14 (requested/used solver, stop, fallback, warning count and requested/used preconditioner); b/V mreldif <=1e-8, residual/fitted absolute <=1e-8. Iteration counts, timing and certificate magnitudes need not agree across workers.",
                       "failure_policy": "Retain every failure and diagnose; no golden, fixture, tolerance or runtime changes to obtain a pass. Native publication or performance acceptance is not established by this witness."},
        "transport": "CSV binary64 round-trip decimal values imported with asdouble. Before estimation, Stata %21x values for z1/z2/y must match independently generated canonical hex strings; w/q/v and labels are exact small integers. The licensed runner records actual plugin and source identities; CSV and fixture hashes below freeze prospective inputs.",
        "sha256": {str(DATA.relative_to(ROOT)): hashlib.sha256(data.encode()).hexdigest(),
                   str(STATA.relative_to(ROOT)): hashlib.sha256(STATA.read_bytes()).hexdigest(),
                   "bench/prepare_within_plugin_boundary.py": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                   "bench/compare_within_slope_boundary.py": hashlib.sha256((ROOT/"bench/compare_within_slope_boundary.py").read_bytes()).hexdigest()},
    }
    return {DATA: data, DECLARATION: json.dumps(declaration, indent=2, allow_nan=False)+"\n"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    for path, text in contents().items():
        if args.write:
            path.write_text(text)
        else:
            assert path.read_text() == text, f"changed frozen fixture: {path.relative_to(ROOT)}"
        print(path.relative_to(ROOT))


if __name__ == "__main__":
    main()
