"""Source-bound one-partition slope comparisons using the existing workers.

Ordinary timing and phase diagnostics are separate labels. Input generation and
the small explicit-dummy oracle are never inside the command timer.
"""
import argparse
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys

import numpy as np
import pandas as pd
from scipy.linalg import lstsq

REPO = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


profile = load("slope_profile", REPO / "benchmarks/one_fe_profile/run.py")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + "\n")


def stage(root):
    folder = profile.stage(root)
    path = folder / "r_worker.R"
    text = path.read_text()
    marker = 'paste(cell$effects, collapse=" + ")'
    assert text.count(marker) == 1
    path.write_text(text.replace(marker, '"g1[z]"'))
    return folder


def dataset(root, n, k, seed):
    path = root / "data" / f"n{n}-k{k}-s{seed}.dta"
    if path.exists():
        assert sha(path) == json.loads(path.with_suffix(".json").read_text())["sha256"]
        return path
    cell = dict(model="ols", design="one_fe", n=n, k=k, threads=1,
                effects=["g1"], section="width")
    manifest = json.loads((REPO / "benchmarks/systematic/manifest.json").read_text())
    generate = load("slope_generate", REPO / "benchmarks/systematic/generate.py")
    generate.generate(cell, seed, path, manifest)
    frame = pd.read_stata(path, convert_categoricals=False)
    frame["z"] = np.random.default_rng(719 + seed - 104729).normal(size=n)
    frame["y"] += 0.3 * np.sin(frame.g1) * frame.z
    frame.to_stata(path, write_index=False, version=118,
                   time_stamp=datetime.datetime(2000, 1, 1))
    np.testing.assert_array_equal(frame.to_numpy(), pd.read_stata(path).to_numpy())
    info = json.loads(path.with_suffix(".json").read_text())
    info.update(columns=list(frame), sha256=sha(path), slope_generator_sha256=sha(Path(__file__)),
                column_sha256={name: hashlib.sha256(np.asarray(frame[name], dtype="<f8").tobytes()).hexdigest()
                               for name in frame})
    write(path.with_suffix(".json"), info)
    with path.with_suffix(".bin").open("wb") as stream:
        for name in frame:
            stream.write(np.asarray(frame[name], dtype="<f8").tobytes())
    return path


def oracle(data, output, k):
    frame = pd.read_stata(data, convert_categoricals=False)
    assert len(frame) <= 1920
    groups = pd.get_dummies(frame.g1, dtype=float).to_numpy()
    X = frame[[f"x{j}" for j in range(1, k + 1)]].to_numpy()
    design = np.column_stack([X, groups, groups * frame.z.to_numpy()[:, None]])
    beta, _, rank, _ = lstsq(design, frame.y.to_numpy(), lapack_driver="gelsd")
    residual = frame.y.to_numpy() - design @ beta
    assert len(frame) > rank
    covariance = np.linalg.pinv(design.T @ design)[:k, :k] * (residual @ residual) / (len(frame) - rank)
    names = [f"x{j}" for j in range(1, k + 1)]
    actual_b = pd.read_csv(output / "coefficients.csv").set_index("name").loc[names, "value"].to_numpy()
    actual_v = pd.read_csv(output / "covariance.csv").pivot(index="row", columns="column", values="value").loc[names, names].to_numpy()
    result = json.loads((output / "result.json").read_text())
    np.testing.assert_allclose(actual_b, beta[:k], rtol=1e-7, atol=1e-10)
    np.testing.assert_allclose(actual_v, covariance, rtol=1e-6, atol=1e-10)
    np.testing.assert_allclose(result["metadata"]["rss"], residual @ residual, rtol=1e-8)
    return dict(status="pass", n=len(frame), rank=int(rank),
                coefficient_max_error=float(np.max(np.abs(actual_b - beta[:k]))),
                covariance_max_error=float(np.max(np.abs(actual_v - covariance))))


def main(args):
    root = args.root.resolve()
    root.mkdir(parents=True, exist_ok=False)
    (root / "work/refs").mkdir(parents=True)
    folder = stage(root)
    sys.path.insert(0, str(folder))
    measure = load("slope_measure", folder / "measure.py").measure
    artifacts = args.artifacts.resolve()
    identities = {}
    for variant in args.variants:
        if variant != "fixest":
            identity = json.loads((artifacts / f"identity-{variant}.json").read_text())
            assert sha(artifacts / variant / "dist/fereg_macos.plugin") == identity["plugin"]
            for relative, digest in identity["source"].items():
                assert sha(artifacts / variant / relative) == digest, relative
            identities[variant] = identity
    library = next(Path(p) / "fixest" for p in os.environ["R_LIBS_USER"].split(":")
                   if (Path(p) / "fixest/DESCRIPTION").exists())
    reference = dict(archive_sha256=sha(args.fixest_archive),
                     library={str(p.relative_to(library)): sha(p) for p in sorted(library.rglob("*")) if p.is_file()})
    manifest = json.loads((REPO / "benchmarks/systematic/manifest.json").read_text())
    manifest["measurement"].update(command_timeout_seconds=120, rss_interval_seconds=0.05)
    cells = [dict(model="ols", design="one_fe", n=args.n, k=k, threads=t,
                  effects=["g1"], absorb="g1##c.z", seed=args.seed, section="width")
             for k in args.widths for t in args.threads]
    plan = dict(schema="one-fe-slopes-v1", cells=cells, variants=args.variants,
                repeats=args.repeats, diagnostic=args.trace, solver=args.solver,
                identities=identities, reference=reference,
                controller={p.name: sha(p) for p in folder.iterdir() if p.is_file()},
                runner_sha256=sha(Path(__file__)))
    write(root / "plan.json", plan)
    inputs = {k: (dataset(root, args.n, k, args.seed),
                  dataset(root, min(args.n, 7680), k, args.seed)) for k in args.widths}
    checks = []
    for index, cell in enumerate(cells):
        data, warm = inputs[cell["k"]]
        for rep in range(args.repeats):
            shift = rep % len(args.variants)
            order = args.variants[shift:] + args.variants[:shift]
            outputs = {}
            for variant in order:
                output = root / "runs" / f"c{index:02d}-r{rep}-{variant}"
                native = variant != "fixest"
                result = measure(root, cell, "fereg" if native else "fixest", "warm", data, warm,
                                 output, manifest, source=artifacts / variant if native else None,
                                 identity=dict(native=identities.get(variant), reference=reference,
                                               plan_sha256=sha(root / "plan.json")),
                                 trace=args.trace, solver=args.solver)
                print(output.name, result["status"], result["seconds"], flush=True)
                assert result["status"] == "pass", output
                outputs[variant] = output
                if args.n <= 1920:
                    checks.append(dict(run=output.name, oracle=oracle(data, output, cell["k"])))
            for variant in args.variants[1:]:
                comparison = profile.compare(outputs[args.variants[0]], outputs[variant], exact_native=False)
                checks.append(dict(cell=index, repetition=rep, variant=variant, science=comparison))
            write(root / "progress.json", checks)
    write(root / "complete.json", dict(status="pass", checks=checks, plan_sha256=sha(root / "plan.json")))
    print("ONE_FE_SLOPES_COMPLETE", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument("--fixest-archive", type=Path, required=True)
    parser.add_argument("--variants", nargs="+", default=["baseline", "fixest"])
    parser.add_argument("--n", type=int, default=245760)
    parser.add_argument("--widths", type=int, nargs="+", default=[10, 50])
    parser.add_argument("--threads", type=int, nargs="+", default=[1, 16])
    parser.add_argument("--seed", type=int, default=104729)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--trace", action="store_true")
    parser.add_argument("--solver", choices=["auto", "map", "cg", "lsmr"], default="auto")
    main(parser.parse_args())
