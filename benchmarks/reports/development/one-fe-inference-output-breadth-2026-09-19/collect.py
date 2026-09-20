"""Archive compact source-bound receipts without licensed logs or large vectors."""
import argparse
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import statistics
import subprocess
import tarfile

import numpy as np
import pandas as pd

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]


def sha(path):
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(2**20), b""):
            h.update(block)
    return h.hexdigest()


def read(path):
    return json.loads(path.read_text())


def write(path, data):
    path.write_text(json.dumps(data, indent=2, sort_keys=True, allow_nan=False)+"\n")


def load_profile():
    spec = importlib.util.spec_from_file_location("profile", REPO / "benchmarks/one_fe_profile/run.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def collect(root, output):
    output.mkdir(parents=True, exist_ok=False)
    assert read(root / "main-complete.json")["status"] == "pass"
    profile = load_profile()
    receipts, identities, grouped, timings, phase_checks = [], {}, {}, [], []
    for path in sorted((root / "runs").iterdir()):
        result = read(path / "result.json")
        assert result["status"] == "pass" and result["process_returncode"] == 0 and result["sentinel"]
        assert result["metadata"]["rc"] == 0
        identity = result.pop("measurement_fingerprint")
        fingerprint = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()
        identities[fingerprint] = identity
        result["identity_sha256"] = fingerprint
        result["run"] = path.name
        result["exports"] = {p.name: sha(p) for p in path.iterdir() if p.suffix in (".bin", ".csv")}
        if result["arm"] == "fereg":
            runtime = pd.read_csv(path / "fereg_runtime.csv").pivot(index="row", columns="column", values="value")[2]
            result["native_phases_seconds"] = {name: float(runtime.loc[row]) for row, name in
                                              [(18, "preflight"), (19, "input"), (20, "native_to_precommit"), (21, "total_to_precommit")]}
            result["native_phases_seconds"]["wrapper_store_cleanup"] = result["metadata"]["fit_seconds"] - runtime.loc[21]
            # Warm-up emits the same tags. Only the last record per phase is
            # the measured full-size invocation.
            phases = {}
            for line in (path / "stderr.txt").read_text().splitlines():
                if line.startswith("FEREG_PHASE|"):
                    _, name, *fields = line.split("|")
                    phases[name] = {key: float(value) for key, value in (field.split("=", 1) for field in fields)}
            if phases:
                # Large ordinary IID overlaps preparation with raw moments;
                # its ols_prepare tag exists only in the small warm-up. The
                # measured preparation boundary is iid_plugin.prepare.
                if result["cell"]["case"] == "iid":
                    phases.pop("ols_prepare", None)
                result["diagnostic_phases"] = phases
                baseline = root / "runs" / path.name.replace("phases-", "main-")
                check = profile.compare(baseline, path)
                if (path / "outputs.bin").exists():
                    assert sha(path / "outputs.bin") == sha(baseline / "outputs.bin")
                phase_checks.append(dict(run=path.name, comparison=check))
        receipts.append(result)
        if path.name.startswith("main-"):
            cell = result["cell"]
            key = (cell["case"], cell["k"])
            grouped.setdefault(key, {}).setdefault(result["arm"], []).append(result)
    for (case, k), arms in grouped.items():
        assert len(arms["fereg"]) == len(arms["fixest"]) == 3
        ratios = []
        for f, r in zip(arms["fereg"], arms["fixest"]):
            assert f["run"].rsplit("-",1)[0] == r["run"].rsplit("-",1)[0]
            assert f["sample_sha256"] == r["sample_sha256"]
            fm, rm = f["metadata"], r["metadata"]
            assert fm["N"] == rm["N"] == 983040
            assert fm["num_singletons"] == 0 and fm["rank"] == k
            assert fm["fereg_df_correction"] == fm["N"] - rm["df_K_vcov"]
            if f["cell"]["vce"].startswith("cluster"):
                assert fm["N_clust"] == rm["G"]
                assert fm["df_r"] == rm["G"] - 1
            ratios.append(f["seconds"]/r["seconds"])
        row = dict(case=case, k=k, paired_ratios=ratios, median_paired_ratio=statistics.median(ratios),
                   fereg_wins=sum(r < 1 for r in ratios), pairs=len(ratios))
        for arm, values in arms.items():
            for key, series in {
                "total_ms": [r["seconds"]*1000 for r in values],
                "fit_ms": [r["metadata"]["fit_seconds"]*1000 for r in values],
                "postfit_output_ms": [(r["seconds"]-r["metadata"]["fit_seconds"])*1000 for r in values],
                "sampled_rss_gib": [r["peak_rss_gib"] for r in values],
            }.items():
                row[arm+"_"+key] = dict(median=statistics.median(series), minimum=min(series), maximum=max(series))
            if arm == "fereg":
                row["native_phases_ms"] = {name: statistics.median(r["native_phases_seconds"][name]*1000 for r in values)
                                           for name in values[0]["native_phases_seconds"]}
        timings.append(row)
    native = next(iter(identities.values()))["native"]
    commit = "82e23357af64d65dd9e7162e2317b2ace62e1495"
    archive = subprocess.check_output(["/opt/homebrew/bin/git", "archive", commit, "Cargo.toml", "Cargo.lock", "crates", "plugin", "stata", "tests", "scripts", "references.lock.toml"], cwd=REPO)
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        sources = {f.name: hashlib.sha256(tar.extractfile(f).read()).hexdigest() for f in tar.getmembers() if f.isfile()}
    assert sources == native["source"]
    write(output / "identity.json", dict(source_commit=commit, source_manifest_verified=len(sources), identities=identities,
                                         environment=read(REPO / "target/one-fe-profile-20260919/environment.json")))
    write(output / "receipts.json", receipts)
    write(output / "timings.json", timings)
    write(output / "science.json", dict(
        main=read(root / "main-complete.json"), smoke=read(root / "smoke-complete.json"),
        saved_smoke=read(root / "smoke2-complete.json"), phase_checks=phase_checks,
        exact_dof_and_cluster_checks="pass for all 60 primary pairs",
    ))
    write(output / "inputs.json", {p.name: read(p) for p in (root / "data").glob("*.json")})
    for folder in (root / "controller", root / "controller-smoke-v1"):
        destination = output / folder.name
        destination.mkdir()
        for file in folder.glob("*"):
            if file.is_file():
                shutil.copyfile(file, destination / file.name)
    for file in HERE.glob("*.py"):
        shutil.copyfile(file, output / file.name)
    for row in sorted(timings, key=lambda r: (r["k"], r["case"])):
        print(row["k"], row["case"], round(row["fereg_total_ms"]["median"]), round(row["fixest_total_ms"]["median"]), round(row["median_paired_ratio"], 3))


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("output", type=Path)
    a = p.parse_args()
    collect(a.root.resolve(), a.output.resolve())
