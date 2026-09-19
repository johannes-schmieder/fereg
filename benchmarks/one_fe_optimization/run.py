"""Paired full-command timings with exact input and scientific result audits."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

REPO = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main(a):
    root, previous = a.root.resolve(), a.data.resolve()
    profile = load("one_fe_profile", REPO / "benchmarks/one_fe_profile/run.py")
    folder = profile.stage(root) if not (root / "controller").exists() else root / "controller"
    sys.path.insert(0, str(folder))
    measure = load("optimization_measure", folder / "measure.py").measure
    (root / "work/refs").mkdir(parents=True, exist_ok=True)
    manifest = json.loads((REPO / "benchmarks/systematic/manifest.json").read_text())
    manifest["measurement"].update(command_timeout_seconds=120, rss_interval_seconds=.05)
    if a.part == "smoke":
        cells = [(360, 10, 4, None, False)]
    elif a.part == "primary":
        cells = [(983040, k, 16, None, False) for k in (10, 50)]
    else:
        cells = [(n, k, t, None, False) for n, k in [(245760, 10), (983040, 10), (983040, 50)]
                 for t in (1, 4, 16) if not (n == 983040 and t == 16)]
        cells += [(983040, 50, t, g, shuffled) for t in (4, 16) for g, shuffled in [(1000, False), (None, True)]]
    if a.cell:
        cells = [cell for cell in cells if f"n{cell[0]}-k{cell[1]}-g{cell[3] or 'default'}-s{int(cell[4])}-t{cell[2]}" == a.cell]
        assert cells, "requested cell is outside the frozen development cases"
    checks = []
    for n, k, threads, groups, shuffled in cells:
        tag = f"n{n}-k{k}-g{groups or 'default'}-shuffle{int(shuffled)}"
        data = previous / "data" / (tag + ".dta")
        warm = previous / "data" / f"n7680-k{k}-gdefault-shuffle0.dta"
        cell = dict(model="ols", design="one_fe", n=n, k=k, threads=threads, effects=["g1"], section="width")
        for rep in range(1, a.repeats + 1):
            rotate = (rep - 1) % len(a.variants)
            order = a.variants[rotate:] + a.variants[:rotate]
            outputs = {}
            for variant in order:
                output = root / "runs" / f"{a.label}-{a.part}-{tag}-t{threads}-r{rep}-{variant}"
                if variant == "fixest":
                    identity = dict(fixest_archive_sha256=hashlib.sha256((previous / "fixest.tar.gz").read_bytes()).hexdigest())
                    arm, source = "fixest", None
                else:
                    identity = json.loads((root / ("identity-" + variant + ".json")).read_text())
                    source = root / variant
                    assert hashlib.sha256((source / "dist/fereg_macos.plugin").read_bytes()).hexdigest() == identity["plugin"]
                    arm = "fereg"
                result = measure(root, cell, arm, "warm", data, warm, output, manifest,
                                 identity=identity, source=source, trace=a.trace)
                print(output.name, result["status"], result["seconds"], flush=True)
                assert result["status"] == "pass", output
                outputs[variant] = output
                if a.part == "smoke":
                    checks.append(dict(run=output.name, oracle=profile.oracle(data, output, k)))
            for variant in order:
                checks.append(dict(run=outputs[variant].name, science=profile.compare(outputs[a.variants[0]], outputs[variant])))
    (root / (a.label + "-" + a.part + ".json")).write_text(json.dumps(dict(status="pass", checks=checks), indent=2) + "\n")
    print("ONE_FE_OPTIMIZATION_COMPLETE", a.label, a.part, flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("--data", type=Path, default=REPO / "target/one-fe-profile-20260919")
    p.add_argument("--part", choices=("smoke", "primary", "controls"), required=True)
    p.add_argument("--variants", nargs="+", required=True)
    p.add_argument("--label", required=True)
    p.add_argument("--repeats", type=int, default=3)
    p.add_argument("--trace", action="store_true")
    p.add_argument("--cell", help="select one existing case for a focused regression recheck")
    main(p.parse_args())
