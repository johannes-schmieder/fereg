"""P2 bounded weighted-operator comparisons, using the existing local workers."""
import argparse
import copy
import datetime
import hashlib

import numpy as np
import pandas as pd

from common import read_json, sha256, write_json
from final_compare import baseline_compare, certify, matrix
from measure import measure
from preparation_compare import input_for as preparation_input
from run import BASE, MANIFEST


BASELINE = "39d1a2a876b92dcc7f8032be792ee171cb85cfdd"
PRIMARY = [
    dict(name="aw_strong", design="strong_d3", n=983040, k=50, threads=4, weight="[aw=w]"),
    dict(name="fw_strong", design="strong_d6", n=245760, k=50, threads=16, weight="[fw=w]"),
    dict(name="pw_cluster", design="strong_d3", n=245760, k=50, threads=16, weight="[pw=w]", vce="cluster c1 c2"),
]
CONTROLS = [
    *[dict(name=name, design=name, n=61440, k=50, threads=4) for name in
      ("weak_d3", "two_fe_dense", "bottleneck_ring", "cycle", "disconnected_blocks", "unbalanced")],
    dict(name="weighted_cut", design="strong_d3", n=61440, k=50, threads=4, weight="[aw=w]", weight_frame="cut"),
    dict(name="dispersed", design="strong_d3", n=61440, k=50, threads=4, weight="[aw=w]", weight_frame="dispersed"),
    dict(name="weighted_disconnected", design="disconnected_blocks", n=61440, k=50, threads=4, weight="[fw=w]"),
    dict(name="weighted_unbalanced", design="unbalanced", n=61440, k=50, threads=4, weight="[pw=w]", vce="robust"),
    dict(name="constant", design="strong_d3", n=61440, k=50, threads=4, weight="[aw=w]", weight_frame="constant"),
    dict(name="constant_unit", design="strong_d3", n=61440, k=50, threads=4, weight_frame="constant"),
    dict(name="one_fe", design="one_fe", n=245760, k=50, threads=16, weight="[aw=w]"),
    dict(name="small_narrow", design="strong_d3", n=7680, k=1, threads=1, weight="[aw=w]", vce="robust"),
    dict(name="rhs15", design="strong_d3", n=30720, k=13, threads=4, weight="[aw=w]"),
    dict(name="rhs16", design="strong_d3", n=30720, k=14, threads=4, weight="[aw=w]"),
    dict(name="rhs64", design="strong_d3", n=30720, k=62, threads=4, weight="[aw=w]"),
    dict(name="rhs65", design="strong_d3", n=30720, k=63, threads=4, weight="[aw=w]"),
    dict(name="low", design="strong_d3", n=30720, k=16, threads=4, weight="[aw=w]", fit_options="memory(low)"),
    dict(name="saved", design="strong_d3", n=30720, k=16, threads=4, weight="[aw=w]", absorb="saved_g=g1 g2", fit_options="residuals(saved_e)"),
]


def input_for(spec, seed, warm=False):
    source = preparation_input(spec, seed, warm)
    if "weight_frame" not in spec:
        return source
    path = BASE / "weighted-data" / source.name
    if path.exists():
        return path
    path.parent.mkdir(parents=True, exist_ok=True)
    frame = pd.read_stata(source, convert_categoricals=False)
    i = np.arange(len(frame))
    if spec["weight_frame"] == "constant":
        frame["w"] = 128.0
    elif spec["weight_frame"] == "dispersed":
        frame["w"] = 10.0 ** (i % 13 - 6)
    elif spec["weight_frame"] == "cut":
        # Removing the low-weight observations exposes a long cycle. The
        # same unweighted graph has abundant long-range connections.
        groups = len(frame) // 480
        base = i // 4 % groups
        frame["g1"] = i // 4 + 1
        frame["g2"] = np.where(i % 4 == 0, base, np.where(i % 4 == 1,
            (base + 1) % groups, (i * 37 + i // (4 * groups) * 19) % groups)) + 1
        frame["w"] = np.where(i % 4 < 2, 1.0, 1e-10)
        frame["y"] += np.sin(frame["g2"] * 2 * np.pi / groups)
    else:
        raise ValueError(spec["weight_frame"])
    frame.to_stata(path, write_index=False, version=118, time_stamp=datetime.datetime(2000, 1, 1))
    info = read_json(source.with_suffix(".json"))
    info.update(schema="ols-weighted-schur-input-v1", source_sha256=sha256(source),
                sha256=sha256(path), recipe=spec)
    info["column_sha256"] = {c: hashlib.sha256(np.asarray(frame[c], dtype="<f8").tobytes()).hexdigest() for c in frame}
    write_json(path.with_suffix(".json"), info)
    return path


def pair(spec, seed, rotation=0, trace=False, prefix=""):
    data, warm = input_for(spec, seed), input_for(spec, seed + 1, warm=True)
    cell = dict(model="ols", section="width", **spec)
    cell["effects"] = ["g1"] if spec["design"] == "one_fe" else ["g1", "g2"]
    paths = {}
    for arm in (["candidate", "baseline"] if rotation % 2 else ["baseline", "candidate"]):
        source = BASE / arm
        path = BASE / "runs" / f"weighted-{prefix}{spec['name']}-s{seed}-{arm}"
        paths[arm] = path
        manifest = copy.deepcopy(MANIFEST)
        manifest["measurement"]["command_timeout_seconds"] = 90
        result = measure(BASE, cell, "fereg", "warm", data, warm, path, manifest,
                         identity={"base_commit": BASELINE, "variant": arm,
                                   "plugin_sha256": sha256(source / "dist/fereg_macos.plugin"),
                                   "seed": seed, "trace": trace}, source=source, trace=trace)
        print(path.name, result["status"], result["seconds"], flush=True)
        if result["status"] != "pass":
            raise RuntimeError(path)
        write_json(path / "native-science.json", certify(path))
        if trace:
            phases = [line for line in (path / "stderr.txt").read_text().splitlines() if line.startswith("FEREG_PHASE|")]
            write_json(path / "phases.json", phases)
    baseline_compare(paths["candidate"], paths["baseline"])
    for name in ("fereg_status", "fereg_dof", "fereg_fe", "fereg_cluster", "fereg_subset"):
        a, b = (paths[arm] / f"{name}.csv" for arm in ("candidate", "baseline"))
        assert a.exists() == b.exists()
        if a.exists():
            assert a.read_bytes() == b.read_bytes(), (spec["name"], name)
    if "weight" not in spec:
        for name in ("b", "V", "fereg_certificate", "fereg_fit"):
            assert (paths["candidate"] / f"{name}.csv").read_bytes() == (paths["baseline"] / f"{name}.csv").read_bytes()
        cols = [0, 1, 2, 3, 4, 5, 6, 7, 10, 11, 12, 13, 14, 15]
        assert np.array_equal(matrix(paths["candidate"], "fereg_solver")[:, cols], matrix(paths["baseline"], "fereg_solver")[:, cols])
    saved = paths["candidate"] / "saved_outputs.csv"
    if saved.exists():
        assert saved.read_bytes() == (paths["baseline"] / saved.name).read_bytes()
    write_json(paths["candidate"] / "science-parity.json", {
        "sample_status_dof_weight_cluster_exact": True,
        "independent_native_certificates_pass": True,
        "unit_weight_science_and_solver_decisions_exact": "weight" not in spec,
        "comparison": read_json(paths["candidate"] / "baseline-comparison.json"),
    })
    return paths


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--part", choices=["primary", "controls", "profile", "single"], required=True)
    parser.add_argument("--name")
    parser.add_argument("--seed", type=int, default=104729)
    parser.add_argument("--prefix", default="")
    args = parser.parse_args()
    (BASE / "work/refs").mkdir(parents=True, exist_ok=True)
    if args.part == "single":
        pair(next(s for s in PRIMARY + CONTROLS if s["name"] == args.name), args.seed, trace=True, prefix=args.prefix)
    else:
        specs = CONTROLS if args.part == "controls" else PRIMARY
        for i, spec in enumerate(specs):
            seeds = [104729, 8675309, 20260819] if args.part == "primary" else [59047]
            for rotation, seed in enumerate(seeds):
                prefix = args.prefix or {"profile": "profile-", "primary": "paired-", "controls": ""}[args.part]
                pair(spec, seed, i + rotation, trace=args.part == "profile", prefix=prefix)
