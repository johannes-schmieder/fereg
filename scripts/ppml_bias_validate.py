#!/usr/bin/env python3
"""Independent native correction comparisons on reproducible synthetic controls."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess

import numpy as np
import pandas as pd

from ppml_bias_oracle import correction, fit_poisson, support_summary
from run_stata_tests import run_fixture, sha256_file


def source_identity(root):
    paths = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"], cwd=root, text=True
    ).splitlines()
    files = {p: sha256_file(root / p) for p in sorted(set(paths))
             if (root / p).is_file() and not p.startswith("benchmarks/reports/")}
    encoded = json.dumps(files, sort_keys=True).encode()
    return {"head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
            "manifest_sha256": hashlib.sha256(encoded).hexdigest(), "files": files}


def design(seed, duration):
    rng = np.random.default_rng(seed)
    n = 240
    g = np.repeat(np.arange(8), n // 8)
    h = np.tile(np.arange(6), n // 6)
    unit = np.arange(n) // 4
    x, z = rng.normal(size=(2, n))
    eta = .2 + .15 * g - .12 * h + .2 * x - .1 * z
    exposure = rng.exponential(np.exp(-eta)) if duration else rng.uniform(.5, 2., n)
    y = np.ones(n) if duration else rng.poisson(exposure * np.exp(eta))
    data = pd.DataFrame(dict(y=y, g=g+1, h=h+1, unit=unit+1, x=x, z=z,
                             exposure=exposure, offset=np.log(exposure),
                             pw=rng.uniform(.3, 3., n), fw=rng.integers(1, 4, n)))
    matrix = np.column_stack([np.eye(8)[g], np.eye(6)[h, 1:], x, z])
    return data, matrix


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stata", required=True)
    parser.add_argument("--source-manifest", type=Path, required=True)
    parser.add_argument("--plugin", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    work = args.work.resolve()
    work.mkdir(parents=True, exist_ok=False)
    lines = ["version 18.0", "clear all", "set more off", "set processors 2"]
    cases = []
    specifications = [(duration, weight, "") for duration in (False, True)
                  for weight in ("none", "fw", "pw")]
    specifications += [(True, "pw", "high_leverage"), (False, "none", "sparse"),
                   (False, "none", "collinear"), (True, "pw", "imbalanced")]
    for duration, weight, variant in specifications:
        name = f"{'duration' if duration else 'counts'}_{weight}"
        data, matrix = design(981731 + int(duration), duration)
        if variant:
            name += "_" + variant
        if variant == "high_leverage":
            data.loc[0, "pw"] = 10000.
        elif variant == "imbalanced":
            data.loc[data.g == 1, "pw"] *= 0.0001
            data.loc[data.g == 8, "pw"] *= 100.
        elif variant == "sparse":
            data.loc[np.random.default_rng(7631).uniform(size=len(data)) > .25, "y"] = 0.
        elif variant == "collinear":
            data["z"] = 2*data.x
            matrix = matrix[:, :-1]

        data.to_stata(work / f"{name}.dta", write_index=False, version=118)
        fitting_weight = np.ones(len(data)) if weight == "none" else data[weight].to_numpy()
        beta, mu, _ = fit_poisson(matrix, data.y, fitting_weight, data.exposure)
        d, c = correction(matrix, data.y, fitting_weight, mu, data.unit)
        cases.append((name, data, matrix, beta, mu, d, c))
        w = "" if weight == "none" else f"[{weight}={weight}]"
        adjustment = "offset(offset)" if duration else "exposure(exposure)"
        command = f"feppml y x z {w}, absorb(g h) {adjustment} keepsingletons threads(1)"
        lines += [f'use "{work / (name + ".dta")}", clear', command,
                  "matrix ordinary_b = e(b)", "matrix ordinary_v = e(V)",
                  "generate byte ordinary_sample = e(sample)",
                  command + " biascorrection(exact) biasunit(unit) biasfe(bf) mu(native_mu)",
                  "assert mreldif(ordinary_b,e(b))==0", "assert mreldif(ordinary_v,e(V))==0",
                  "assert ordinary_sample==e(sample)",
                  "assert e(bias_diagnostics)[2,1]==60", "assert e(bias_diagnostics)[7,1]==62"]
        for effect, values in enumerate((data.g, data.h), 1):
            expected_support = support_summary(values, data.unit, fitting_weight*mu)
            for col, expected in enumerate(expected_support, 1):
                lines.append(f"assert reldif(e(bias_support)[{effect},{col}],{expected:.17g})<1e-7")
        for label, mat in (("raw", "b_bias_raw"), ("d", "b_bias_d"), ("c", "b_bias_c"), ("bc", "b_bc")):
            for j, control in enumerate(("x", "z"), 1):
                lines.append(f"generate double {label}_{control} = e({mat})[1,{j}]")
        lines += ["format bf* native_mu raw_* d_* c_* bc_* %24.17g",
                  f'export delimited bf* native_mu raw_* d_* c_* bc_* using "{work / (name + ".csv")}", replace datafmt']
    lines.append('display "END_FIXTURE|bias_validation|rc=0"')
    fixture = work / "bias_validation.do"
    fixture.write_text("\n".join(lines) + "\n")
    native = run_fixture(args.stata, fixture, root, plugin=args.plugin,
                         plugin_sha256=sha256_file(args.plugin), log_dir=work / "logs", require_status=True)
    if native.error or native.stata_rc != 0:
        print(native.log_text[-12000:])
        raise RuntimeError(native.error or f"Stata returned {native.stata_rc}")
    results = []
    for name, data, matrix, beta, mu, d, c in cases:
        actual = pd.read_csv(work / f"{name}.csv")
        np.testing.assert_allclose(actual.native_mu, mu, rtol=1e-7, atol=1e-9)
        errors = {}
        for label, vector in (("raw", beta), ("d", d), ("c", c), ("bc", beta+d+c)):
            left = vector[:8][data.g.to_numpy()-1]
            right = np.r_[0., vector[8:13]][data.h.to_numpy()-1]
            finite = vector[-2:] if matrix.shape[1] == 15 else np.array([vector[-1], 0.])
            for expected, observed, tag in (
                (left, actual[f"bf_1_{label}"], "first"),
                (right, actual[f"bf_2_{label}"], "second"),
                (finite, actual[[f"{label}_x", f"{label}_z"]].iloc[0], "controls"),
            ):
                np.testing.assert_allclose(observed, expected, rtol=2e-7, atol=2e-9)
                errors[f"{label}_{tag}"] = float(np.max(np.abs(observed-expected)))
        results.append(dict(case=name, passed=True, maximum_errors=errors))
    receipt = dict(schema=1, source=json.loads(args.source_manifest.read_text()), plugin_sha256=sha256_file(args.plugin),
                   fixture_sha256=sha256_file(fixture), stata=native.metadata, results=results, passed=True)
    (work / "summary.json").write_text(json.dumps(receipt, indent=2, sort_keys=True)+"\n")
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
