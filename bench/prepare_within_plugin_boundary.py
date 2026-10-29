#!/usr/bin/env python3
"""Freeze public OLS slope-boundary cases using exact input-space math.

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
CASES = ["delta0-transform1-order0", "delta2-transform1-order0", "delta3-transform1-order0",
         "full-rank-orthogonal-control"]
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
        # This additional positive control is declared before any native run.
        # Reuse the original row/weight/y family, replacing only its two slope
        # inputs by independent Walsh u/v columns, with no shift or scaling.
        control = name == "full-rank-orthogonal-control"
        f = originals["delta3-transform0-order0"].copy() if control else originals[name]
        f["v"] = [1.0 if (i % 8) & 2 else -1.0 for i in range(128)]
        u = [1.0 if (i % 8) & 1 else -1.0 for i in range(128)]
        if control:
            f["slopes"] = [u, f["v"]]
        f["q"] = [a*b for a, b in zip(u, f["v"])]
        f["y"] = [y+0.7*q for y, q in zip(f["y"], f["q"])]
        fit = exact_fit(f)
        expectation = {"case": case, "dependency_fixture": None if control else name,
                       **{k: fit[k] for k in ["rank", "v_status", "beta_q", "beta_v", "constant",
                                              "rss", "v_residual_energy_ratio", "pivots"]}}
        if control:
            expectation.update(control_name=name, row_family_fixture="delta3-transform0-order0")
        expected.append(expectation)
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
        "status": "prospective four-case amendment preserving approved existing rejections; appended orthogonal positive control not yet executed",
        "within_candidate": "d3407a16dec3715f6e9eee13fb9b6646d5b28035",
        "scope": "Actual public weighted sloped OLS explicit LSMR publication under the approved upstream whitening correction. No runtime or grammar change; no timing. Other scientific paths are unaffected by this fixture.",
        "cases": expected, "workers": [1, 4], "repeats_per_worker": 2,
        "native_estimation_calls": 16, "additional_frontend_only_rejections": 4,
        "case_expectations": {"1": "four successful independent-oracle fits", "2": "four atomic rc459 rejections; exact-input oracle retained without publishing it", "3": "four atomic rc459 rejections; exact-input oracle retained without publishing it", "4": "four successful independent-oracle fits for the appended full-rank-orthogonal-control"},
        "approved_amendment": {
            "decision": "Johannes: Preserve rejection and continue migration (recommended). Keep existing rank-span mismatch rejection; no runtime/model, oracle, input or tolerance change.",
            "original_declaration_commit": "939a6f5e",
            "public_prediction_syntax_repair_commit": "b41e06fd",
            "first_rejection_amendment_commit": "b2691e67",
            "original_three_case_csv": {"bytes": 128156, "sha256": "891404052b5904e36a86b6901993dc270a996a684dfcd95fc1fe252a38e92147", "preservation": "The current CSV starts with exactly these bytes, including its unchanged header. Only case4 rows are appended."},
            "original_execution": "The first declaration failed before native estimation because xbd(actual_f) is not a public estimation option (rc198). The syntax-only repair used predict after residuals(). Case1 then passed all four fits and its atomic ib3.g rejection; case2 workers1 failed rc459 before finite fitting. Both original expectations and failures remain preserved, not reclassified as passes.",
            "bounded_case2_diagnosis": {
                "calls_per_arm": 1, "workers": 1, "stata": "19 MP, Mac Apple Silicon",
                "candidate_plugin_sha256": "22570a0ab4e01c215ad2ebfc049c74cb317bb342c57a59f6d4bb1ba77b745f48",
                "beta10_plugin_sha256": "aa453ba70c90a605877c1c85383f574ea6cbd5e4ba3a2875b61b451e9d7ceae0",
                "both_return_codes": 459,
                "stage": "Initial explicit owned LSMR seven-RHS projection. Fereg certificate relative infinity norms for y/q/v were approximately0.1094526513/3.891452469e-5/0.4999999996 in both arms. LSMR reported convergence; independent certification rejected, then original-RHS MAP fallback also remained uncertified. The final-residual stage was not reached.",
                "cause": "Within's centered per-level Gram1e-10 rule drops one slope in case2, while fereg's raw-channel normalized QR certificate retains the additional direction (last normalized norms2.95e-13 to1.18e-12 versus cutoff5.684e-14). MAP fallback uses a raw-Gram1e-12 cutoff. These existing rank policies span different spaces; no certificate is weakened here.",
            },
            "case3_followup": {
                "prior_run": "The b2691e67 fixture passed all four case1 fits and all four expected case2 rc459/data/e() rollback checks plus their base refusals. The first case3 fit then failed rc459. That failed success expectation is preserved.",
                "calls_per_arm": 1, "workers": 1, "both_return_codes": 459,
                "plugin_identities": "Same immutable candidate22570a0ab4e01c215ad2ebfc049c74cb317bb342c57a59f6d4bb1ba77b745f48 and beta10aa453ba70c90a605877c1c85383f574ea6cbd5e4ba3a2875b61b451e9d7ceae0 as the case2 diagnosis.",
                "stage": "Initial seven-RHS explicit LSMR projection; all recurrence convergence flags true; independent FE certification rejects before finite fitting or final-residual projection. Source path then restores original inputs for MAP fallback, whose result also fails certification.",
                "candidate_trace": {
                    "iterations": [4, 2, 4, 1, 1, 1, 1],
                    "within_normal": [3.06813615913363471e-13, 1.02499301919343182e-16, 8.19351616682404124e-18, 1.47245015925297468e-33, 2.50036746375777929e-33, 7.02316892549732338e-34, 1.98530342968452645e-33],
                    "fereg_relative_infinity": [6.74351940759489209e-6, 8.80149097141755664e-6, 4.05804386358973918e-10, 2.22044604925031283e-16, 2.22044604925031234e-16, 2.22044604925031136e-16, 2.77555756156289234e-16],
                },
                "beta10_trace": {
                    "iterations": [6, 3, 4, 3, 2, 1, 4],
                    "within_normal": [1.47604505734162892e-16, 5.03370614656509188e-12, 1.92726700056960885e-15, 2.23065962093342431e-16, 2.23065962093342333e-16, 2.23065962093342333e-16, 2.49821824636438218e-16],
                    "fereg_relative_infinity": [1.09451813256922617e-1, 1.17834002357897284e-5, 4.99999999417551078e-1, 2.22044604925031283e-16, 2.22044604925031234e-16, 2.22044604925031136e-16, 1.66533453693773481e-16],
                },
                "interpretation": "Both binaries reject safely, although their internal projections differ. Candidate within keeps both slopes as the exact oracle requires; its remaining fereg raw-channel certificate failure is not fixed or accepted as a scientific fit. Raw-channel conditioning is a source-based explanation to investigate separately. Johannes's instruction to preserve existing rejection is applied to this same public limitation; no runtime rank rule or tolerance changes.",
            },
            "appended_positive_control": "Case4, full-rank-orthogonal-control, is declared before its first native execution. It reuses the original delta3-transform0-order0 row/level/weight/response family and the same q=u*v and rounded y+.7q construction, but explicitly replaces its two slope inputs by independent Walsh u and v, both exactly +/-1, without offsets or per-level scaling. Their centered weighted Gram is diagonal with equal entries, condition1 and rank2 per level; finite v has exact zero residual energy. A prior unexecuted proposal for near-boundary zero-offset slopes was replaced by this well-conditioned control before freezing or executing it. The original three cases and all60 dependency fixtures remain unchanged. The additional control supplements retained rejection cases and failure records; no existing input, numerical tolerance or runtime policy changes.",
        },
        "rows_per_case": 128, "levels": [3, 11, 29, 97], "weights": [2, 3, 5, 7],
        "command": "fereg y q v i.g [aw=w], absorb(g##c.(z1 z2)) solver(lsmr) preconditioner(off) memory(fast) tolerance(1e-10) iterate(100) threads(WORKERS) deterministic residuals(actual_r); cases1/4 then predict double actual_f, xbd; cases2/3 expect rc459 and no output",
        "oracle": "Reuse the maintained 60-fixture exact-rational centered-Gram rank decisions on actual binary64 slopes. Independently orthogonalize retained raw columns in weighted observation space, residualize rounded y+.7*(u*v), q=u*v and v, and solve their exact rational finite normal equations. Existing FE-collinearity rule is residual energy/raw energy <= exact binary64 1e-9. All retained FE and finite score moments are exactly zero. Nonzero omitted-v energy is reported explicitly.",
        "sample": "Successful cases1/4 retain all128 original rows, no singleton removals, ranks2/1. Cases2/3 publish no sample/rank and preserve the populated prior regress result and unchanged current dataset. Their retained-span oracle ranks2/1 remain recorded in cases and CSV.",
        "original_axis": "Successful case1: q v 3b.g 11o.g 29o.g 97o.g _cons; case4 replaces v by o.v. Slots1..7 unchanged, statuses1,(1 or3),2,3,3,3,5. Omitted coefficient and covariance rows/columns are exactly zero. Explicit ib3.g with sloped absorption remains unsupported rc498 in all four cases and must preserve accepted estimates and absent requested output.",
        "acceptance": {"actual_solver_code": 3, "actual_solver_name": "lsmr", "fallback": False,
                       "coefficients_absolute": 1e-8, "residual_and_fitted_absolute": 1e-8,
                       "rss_relative": 1e-8, "independent_normalized_moments": 1e-9,
                       "same_worker": "Exact b, V, status, sample, residual/fitted and solver fields except timing columns9/10.",
                       "cross_worker": "Exact sample, rank, stripes, statuses and both solver rows' discrete columns1/2/4/11/12/13/14 (requested/used solver, stop, fallback, warning count and requested/used preconditioner); b/V mreldif <=1e-8, residual/fitted absolute <=1e-8. Iteration counts, timing and certificate magnitudes need not agree across workers.",
                       "cases2_and_3_rejection": "For each case at workers1/4, two attempts each require rc459 with no actual_r/actual_f output, unchanged dataset signature and complete populated regress e() rollback: named scalar/macro/matrix/function sets, scalar hex values, macros, all matrices and their stripes, and e(sample). No finite estimate or solver result is accepted from the failed call.",
                       "failure_policy": "Original case2/3 success expectations and failed runs remain recorded. Revised rejection outcomes preserve the existing public limitation under Johannes's instruction. Retain and diagnose any new failure; no silent expectation, input, oracle, tolerance or runtime changes. Native publication or performance acceptance is not established by this witness."},
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
