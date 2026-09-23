"""Bounded current-source opportunity screen; never resumes a frozen campaign.

Numerical exports and failed calls are retained. A completed screen is not a
scientific verdict: audit the exports separately before ranking its timings.
"""
import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import sys
import threading
import time

import numpy as np
import pandas as pd

REPO = Path(__file__).resolve().parents[2]
ANCHORS = [
    dict(model="ols", design="three_fe_sparse", n=61440, k=10, threads=4),
    dict(model="ols", design="three_fe_sparse", n=245760, k=50, threads=16),
    dict(model="ppml", design="one_fe", n=61440, k=10, threads=4),
    dict(model="ppml", design="strong_d6", n=61440, k=10, threads=4),
]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main(args):
    # Captured controllers are immutable source receipts, not import caches.
    sys.dont_write_bytecode = True
    root = args.root.resolve()
    root.mkdir(parents=True, exist_ok=False)
    controller = root / "controller"
    controller.mkdir()
    shutil.copyfile(Path(__file__), controller / "screen.py")
    shutil.copyfile(Path(__file__).with_name("designs.py"), controller / "designs.py")
    for name in ("common.py", "measure.py", "stata_worker.do", "r_worker.R"):
        shutil.copyfile(REPO / "benchmarks/local_fixest_gap" / name, controller / name)
    shutil.copyfile(REPO / "benchmarks/systematic/generate.py", controller / "generate.py")
    sys.path.insert(0, str(controller))
    from common import read_json, write_json, sha256
    surface = None
    if args.surface:
        shutil.copyfile(Path(__file__).with_name("surface.py"), controller / "surface.py")
        surface = load("generated_surface", controller / "surface.py")
        surface.stage(controller)
    from measure import measure
    from generate import generate
    from designs import augment
    worker = controller / "stata_worker.do"
    text = worker.read_text()
    before = '''if `dimensions' >= 1 local effects g1
if `dimensions' >= 2 local effects `effects' g2
if `dimensions' == 3 local effects `effects' g3'''
    assert text.count(before) == 1
    worker.write_text(text.replace(before, '''forvalues dimension = 1/`dimensions' {
    local effects `effects' g`dimension'
}'''))
    sys.path.insert(0, str(REPO / "benchmarks/two_fe_dense"))
    import route_development as isolation

    foreign = isolation.foreign_processes()
    if foreign:
        write_json(root / "preflight-blocked.json", foreign)
        raise RuntimeError("Competing fit/build process; no measurements started")
    manifest = read_json(REPO / "benchmarks/systematic/manifest.json")
    manifest["designs"]["three_fe_dense"] = dict(suite="panels", mobility=0.20,
        effects=["g1", "g2", "g3"])
    manifest["measurement"].update(command_timeout_seconds=args.timeout, startup_timeout_seconds=120)
    write_json(controller / "manifest.json", manifest)
    cells = read_json(args.cells) if args.cells else ANCHORS
    cells = [dict(c, effects=c.get("effects", manifest["designs"][c["design"]]["effects"]),
                  section=c.get("section", "diagnostic")) for c in cells]
    if surface:
        cells = [surface.configure(c) for c in cells]
    elif any("terms" in c or "outputs" in c for c in cells):
        raise ValueError("Generated terms/output requests require --surface")
    # The stock worker currently implements this deliberately narrow surface.
    # Native-only diagnostic controls may use the existing Stata options.
    if "fixest" in args.variants:
        for cell in cells:
            if any(cell.get(key) for key in
                    ("weight", "absorb", "restriction", "fit_options", "string_columns")) or (not surface and cell.get("vce")):
                raise ValueError("Stock comparison worker does not implement this cell's options")
    identities = {}
    for variant in args.variants:
        if variant == "fixest":
            reference = read_json(REPO / "benchmarks/systematic/references.json")["fixest"]
            assert sha256(args.fixest_archive) == reference["sha256"]
            identities[variant] = reference
        else:
            identity = read_json(args.artifacts / f"identity-{variant}.json")
            source = args.artifacts / variant
            assert sha256(source / "dist/fereg_macos.plugin") == identity["plugin"]
            assert all(sha256(source / p) == h for p, h in identity["source"].items())
            identities[variant] = identity
    write_json(root / "plan.json", dict(schema="next-subsystem-screen-v1", cells=cells,
        seed=args.seed, repeats=args.repeats, variants=args.variants, trace=args.trace,
        timeout=args.timeout, surface=args.surface, identities=identities,
        sources={str(p.relative_to(REPO)): sha256(p) for p in [Path(__file__).resolve(),
            REPO / "benchmarks/two_fe_dense/route_development.py",
            REPO / "benchmarks/systematic/manifest.json",
            REPO / "benchmarks/systematic/references.json"]},
        controller={p.name: sha256(p) for p in controller.iterdir()}))
    (root / "work/refs").mkdir(parents=True)
    datasets = []
    # Finish all data generation before launching any timed fit.
    for index, cell in enumerate(cells):
        paths = []
        for role, size in [("full", cell["n"]), ("warm", min(7680, cell["n"]))]:
            path = root / "data" / f"c{index}-{role}.dta"
            generate(dict(cell, n=size), args.seed, path, manifest)
            augment(cell, path)
            if surface:
                surface.augment(cell, path)
            frame = pd.read_stata(path, convert_categoricals=False)
            with path.with_suffix(".bin").open("wb") as stream:
                for column in frame:
                    stream.write(np.asarray(frame[column], dtype="<f8").tobytes())
            paths.append(path)
        datasets.append(paths)
    progress = []
    for index, (cell, (data, warm)) in enumerate(zip(cells, datasets)):
        for repetition in range(args.repeats):
            shift = (index + repetition) % len(args.variants)
            for variant in args.variants[shift:] + args.variants[:shift]:
                if isolation.foreign_processes():
                    raise RuntimeError("Competing workload appeared; retain completed attempts")
                stop, overlaps, monitor_errors = threading.Event(), [], []

                def monitor():
                    while not stop.wait(0.25):
                        try:
                            foreign = isolation.foreign_processes()
                            if foreign:
                                overlaps.append(dict(timestamp=time.time(), processes=foreign))
                        except Exception as error:
                            monitor_errors.append(repr(error))

                watcher = threading.Thread(target=monitor)
                watcher.start()
                arm = "fixest" if variant == "fixest" else ("fereg" if cell["model"] == "ols" else "feppml")
                output = root / "runs" / f"c{index}-r{repetition}-{variant}"
                try:
                    result = measure(root, cell, arm, "warm", data, warm, output, manifest,
                        identity=identities[variant],
                        source=None if variant == "fixest" else args.artifacts / variant,
                        trace=args.trace, solver=cell.get("solver", "auto"))
                finally:
                    stop.set()
                    watcher.join()
                record = dict(case=index, repetition=repetition, variant=variant,
                    result=str(output.relative_to(root)), status=result["status"],
                    seconds=result["seconds"], overlaps=overlaps, monitor_errors=monitor_errors)
                progress.append(record)
                write_json(root / "progress.json", progress)
                print(record, flush=True)
    write_json(root / "complete.json", dict(status="attempts_complete", attempts=len(progress),
        plan_sha256=sha256(root / "plan.json"), progress_sha256=sha256(root / "progress.json")))
    print("NEXT_SUBSYSTEM_SCREEN_COMPLETE", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--artifacts", required=True, type=lambda p: Path(p).resolve())
    parser.add_argument("--variants", nargs="+", default=["dtk1", "fixest"])
    parser.add_argument("--fixest-archive", required=True, type=Path)
    parser.add_argument("--cells", type=Path)
    parser.add_argument("--seed", type=int, default=202609221)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--timeout", type=int, default=90)
    parser.add_argument("--trace", action="store_true")
    parser.add_argument("--surface", action="store_true",
                        help="matched generated OLS terms and one-FE requested outputs")
    main(parser.parse_args())
