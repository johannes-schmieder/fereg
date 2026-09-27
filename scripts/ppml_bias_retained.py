#!/usr/bin/env python3
"""Replay immutable Separations retry inputs against a bound native artifact.

Inputs are synthetic. Local paths and licensed logs stay in the supplied work
directory; summary.json contains only relative case identities and hashes.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

import numpy as np
import pandas as pd

from ppml_bias_oracle import fit_poisson, two_fe_design
from run_stata_tests import run_fixture, sha256_file


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--inputs", type=Path, required=True)
    parser.add_argument("--plugin", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--stata", required=True)
    parser.add_argument("--source-manifest", type=Path, required=True)
    parser.add_argument("--limit", type=int)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    work = args.work.resolve()
    work.mkdir(parents=True, exist_ok=False)
    jobs = json.loads((args.inputs / "receipt.json").read_text())["jobs"]
    if args.limit:
        jobs = jobs[:args.limit]
    lines = ["version 18.0", "clear", "set more off", "set processors 1"]
    cases = []
    for job in jobs:
        identity = f"{job['scenario']}_{job['rep']}"
        source = args.inputs / identity / "prefit.dta"
        cases.append((identity, source))
        for branch in range(3):
            condition = "sb_fits==1" + (f" & sb_branch=={branch}" if branch else "")
            lines += [
                f'use "{source.resolve()}", clear',
                "generate long native_row = _n",
                f"feppml d x z [pw=sw] if {condition}, absorb(native_g=sb_sworker native_h=sb_sfirm) exposure(exposure) keepsingletons threads(1) mu(native_mu) eta(native_eta) dropreason(native_drop)",
                "generate byte native_sample = e(sample)",
                f"assert native_sample == ({condition})",
                "assert e(num_separated)==0",
                "generate double native_x = _b[x]",
                "generate double native_z = _b[z]",
                f'export delimited native_row native_sample native_mu native_eta native_g native_h native_drop native_x native_z using "{work / (identity + "_" + str(branch) + ".csv")}", replace datafmt',
            ]
    # Stata's default %9.0g export is not an accuracy receipt.
    lines = [line for line in lines if line != "assert e(num_separated)==0"]
    lines = [item for line in lines for item in
             (["format native_mu native_eta native_g native_h native_x native_z %24.17g", line]
              if line.startswith("export delimited") else [line])]
    fixture = work / "retained.do"
    lines.append('display "END_FIXTURE|retained|rc=0"')
    fixture.write_text("\n".join(lines) + "\n")
    digest = sha256_file(args.plugin)
    native = run_fixture(args.stata, fixture, root, plugin=args.plugin,
                         plugin_sha256=digest, log_dir=work / "logs", require_status=True)
    if native.error:
        print(native.log_text[-6000:])
        raise RuntimeError(native.error)
    results = []
    for identity, source in cases:
        data = pd.read_stata(source, convert_categoricals=False)
        for branch in range(3):
            mask = data.sb_fits.eq(1)
            if branch:
                mask &= data.sb_branch.eq(branch)
            selected = data.loc[mask]
            x, first, second = two_fe_design(selected)
            beta, mu, iterations = fit_poisson(x, selected.d.to_numpy(),
                selected.sw.to_numpy(), selected.exposure.to_numpy())
            native = pd.read_csv(work / f"{identity}_{branch}.csv")
            np.testing.assert_array_equal(native.native_row, np.arange(1, len(data)+1))
            np.testing.assert_array_equal(native.native_sample, mask.astype(int))
            native = native.loc[mask]
            anchor = native.loc[selected.sb_sfirm.eq(second[0]).to_numpy(), "native_h"].iloc[0]
            ng = native.native_g.to_numpy() + anchor
            nh = native.native_h.to_numpy() - anchor
            expected_g = beta[np.searchsorted(first, selected.sb_sworker)]
            expected_h = np.r_[0., beta[len(first):len(first)+len(second)-1]][np.searchsorted(second, selected.sb_sfirm)]
            np.testing.assert_allclose(native.native_mu, mu, rtol=1e-7, atol=1e-9)
            np.testing.assert_allclose(ng, expected_g, rtol=1e-7, atol=1e-9)
            np.testing.assert_allclose(nh, expected_h, rtol=1e-7, atol=1e-9)
            np.testing.assert_allclose(native[["native_x", "native_z"]].iloc[0], beta[-2:], rtol=1e-7, atol=1e-10)
            residual = selected.sw.to_numpy() * (selected.d.to_numpy() - native.native_mu.to_numpy())
            results.append(dict(case=identity, branch=branch, rows=int(mask.sum()),
                input_sha256=sha256_file(source), mean_max_abs=float(np.max(np.abs(native.native_mu-mu))),
                effects_max_abs=float(max(np.max(abs(ng-expected_g)),np.max(abs(nh-expected_h)))),
                normalized_score=float(np.max(abs(x.T@residual))/selected.sw.sum()),
                oracle_iterations=iterations, passed=True))
            print(identity, branch, results[-1]["mean_max_abs"], flush=True)
    summary = dict(schema=1, source=json.loads(args.source_manifest.read_text()),
        plugin_sha256=digest, fixture_sha256=sha256_file(fixture), fits=len(results),
        datasets=len(cases), results=results, passed=True)
    (work/"summary.json").write_text(json.dumps(summary, indent=2)+"\n")


if __name__ == "__main__":
    main()
