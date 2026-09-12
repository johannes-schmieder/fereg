"""P0/P1 bounded OLS ownership comparisons using the existing local workers."""
import argparse
import copy
import datetime
import hashlib

import numpy as np
import pandas as pd

from common import read_json, sha256, write_json
from final_compare import baseline_compare, certify, matrix
from measure import measure
from route_holdouts import custom
from run import BASE, MANIFEST, data_for


PRIMARY = [
    dict(name="robust_missing", design="one_fe", n=983040, k=50, threads=16, vce="robust", missing=True),
    dict(name="aw_iid_missing", design="one_fe", n=983040, k=50, threads=16, weight="[aw=w]", missing=True),
    dict(name="fw_cluster", design="one_fe", n=245760, k=50, threads=4, weight="[fw=w]", vce="cluster c1", missing=True),
    dict(name="pw_robust", design="one_fe", n=245760, k=50, threads=16, weight="[pw=w]", vce="robust"),
    dict(name="interaction_cluster", design="one_fe", n=245760, k=50, threads=16, vce="cluster c1 c2", absorb="g1#c2", missing=True),
    dict(name="robust_complete", design="one_fe", n=983040, k=50, threads=16, vce="robust"),
]
CONTROLS = [
    dict(name="iid_complete", design="one_fe", n=983040, k=50, threads=16),
    dict(name="weak_robust", design="weak_d3", n=61440, k=50, threads=4, vce="robust", missing=True),
    dict(name="dense_cluster", design="two_fe_dense", n=61440, k=50, threads=4, vce="cluster c1"),
    *[dict(name=name, design=name, n=61440, k=50, threads=4) for name in
      ("bottleneck_ring", "cycle", "disconnected_blocks", "unbalanced")],
    dict(name="small_narrow", design="one_fe", n=7680, k=1, threads=1, weight="[aw=w]", vce="robust"),
    dict(name="no_fe", design="one_fe", n=7680, k=10, threads=4, effects=[], weight="[pw=w]", vce="robust"),
    dict(name="low_cluster", design="one_fe", n=30720, k=10, threads=4, weight="[aw=w]", vce="cluster c1", fit_options="memory(low)", missing=True),
    dict(name="saved", design="one_fe", n=30720, k=16, threads=4, weight="[aw=w]", absorb="saved_g=g1", fit_options="residuals(saved_e)", missing=True),
    dict(name="restricted_pairs", design="one_fe", n=245760, k=50, threads=16, weight="[aw=w]", vce="robust", restriction="if eligible"),
]


def input_for(spec, seed, warm=False):
    n = 7680 if warm else spec["n"]
    design, k = spec["design"], spec["k"]
    if design in MANIFEST["designs"]:
        original, _ = data_for(design, n, k, seed)
    else:
        original = custom(design, n, k, seed)
    path = BASE / "preparation-data" / f"{spec['name']}-n{n}-s{seed}.dta"
    if path.exists():
        return path
    path.parent.mkdir(parents=True, exist_ok=True)
    frame = pd.read_stata(original, convert_categoricals=False)
    i = np.arange(n)
    frame["w"] = 1 + i % 7
    frame["c1"] = (i // 120) % 97
    frame["c2"] = (i // 20) % 3
    if spec.get("restriction"):
        frame["eligible"] = (i % 20 < 2).astype(np.int8)
    if spec.get("missing"):
        frame.loc[i % 997 == 0, f"x{k}"] = np.nan
        if spec.get("weight"):
            frame.loc[i % 991 == 0, "w"] = np.nan
        if spec.get("vce", "").startswith("cluster"):
            frame.loc[i % 983 == 0, "c1"] = np.nan
    frame.to_stata(path, write_index=False, version=118,
                   time_stamp=datetime.datetime(2000, 1, 1))
    hashes = {}
    for name in frame:
        values = frame[name].to_numpy(dtype="<f8", copy=True)
        values[np.isnan(values)] = float.fromhex("0x1p+1023")
        hashes[name] = hashlib.sha256(values.tobytes()).hexdigest()
    write_json(path.with_suffix(".json"), {
        "schema": "ols-preparation-input-v1", "source_sha256": sha256(original),
        "sha256": sha256(path), "n": n, "k": k, "seed": seed,
        "columns": list(frame), "column_sha256": hashes, "recipe": spec,
        "binary_missing_encoding": "Stata system missing (2^1023)",
    })
    return path


def pair(spec, seed, rotation=0, trace=False, prefix=""):
    data = input_for(spec, seed)
    warm = input_for(spec, seed + 1, warm=True)
    cell = dict(model="ols", section="width", **spec)
    cell.setdefault("effects", ["g1"] if spec["design"] == "one_fe" else ["g1", "g2"])
    paths = {}
    arms = ["baseline", "candidate"]
    if rotation % 2:
        arms.reverse()
    for arm in arms:
        source = BASE / arm
        path = BASE / "runs" / f"prep-{prefix}{spec['name']}-s{seed}-{arm}"
        paths[arm] = path
        manifest = copy.deepcopy(MANIFEST)
        manifest["measurement"]["command_timeout_seconds"] = 90
        result = measure(BASE, cell, "fereg", "warm", data, warm, path, manifest,
                         identity={"base_commit": "e93fe437f1d8855e1c3aa47ad03026eb23a9942d",
                                   "variant": arm, "plugin_sha256": sha256(source / "dist/fereg_macos.plugin"),
                                   "seed": seed, "trace": trace}, source=source, trace=trace)
        print(path.name, result["status"], result["seconds"], flush=True)
        if result["status"] != "pass":
            raise RuntimeError(path)
        write_json(path / "native-science.json", certify(path))
    baseline_compare(paths["candidate"], paths["baseline"])
    for name in ("b", "V", "fereg_status", "fereg_dof", "fereg_fit", "fereg_certificate",
                 "fereg_fe", "fereg_cluster", "fereg_subset"):
        a, b = (paths[arm] / f"{name}.csv" for arm in ("candidate", "baseline"))
        assert a.exists() == b.exists()
        if a.exists():
            assert a.read_bytes() == b.read_bytes(), (spec["name"], name)
    cols = [0, 1, 2, 3, 4, 5, 6, 7, 10, 11, 12, 13, 14, 15]
    assert np.array_equal(matrix(paths["candidate"], "fereg_solver")[:, cols],
                          matrix(paths["baseline"], "fereg_solver")[:, cols])
    write_json(paths["candidate"] / "exact-science.json", {
        "b_V_sample_status_dof_fit_certificates_cluster_exact": True,
        "solver_except_times_exact": True,
    })
    saved = paths["candidate"] / "saved_outputs.csv"
    if saved.exists():
        assert saved.read_bytes() == (paths["baseline"] / saved.name).read_bytes()
    if trace:
        for arm, path in paths.items():
            text = (path / "stderr.txt").read_text()
            phases = [line for line in text.splitlines() if line.startswith("FEREG_PHASE|")]
            write_json(path / "phases.json", phases)
    return paths


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--part", choices=["primary", "controls", "profile", "single"], required=True)
    parser.add_argument("--name")
    parser.add_argument("--seed", type=int, default=104729)
    parser.add_argument("--prefix", default="")
    args = parser.parse_args()
    (BASE / "work/refs").mkdir(parents=True, exist_ok=True)
    if args.part == "single":
        pair(next(spec for spec in PRIMARY + CONTROLS if spec["name"] == args.name), args.seed, prefix=args.prefix)
    elif args.part == "profile":
        for spec in PRIMARY[:5]:
            pair(spec, 104729, trace=True, prefix="profile-")
    else:
        specs = PRIMARY if args.part == "primary" else CONTROLS
        for spec in specs:
            for rotation, seed in enumerate([104729, 8675309, 20260819] if args.part == "primary" else [59047]):
                pair(spec, seed, rotation, prefix=args.prefix)


if __name__ == "__main__":
    main()
