"""Bounded one-FE diagnostic using the existing fresh-process local workers."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import sys

import numpy as np
import pandas as pd

REPO = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def stage(root):
    folder = root / "controller"
    folder.mkdir(exist_ok=False)
    for name in ("common.py", "measure.py", "r_worker.R", "stata_worker.do"):
        shutil.copyfile(REPO / "benchmarks/local_fixest_gap" / name, folder / name)
    # Phase instrumentation and sampling are separate experiments.
    path = folder / "r_worker.R"
    text = path.read_text()
    text = text.replace('if (isTRUE(c$trace)) Rprof(file.path(output,"Rprof.out"), interval=.001)', '')
    text = text.replace('''if (isTRUE(c$trace)) {
 Rprof(NULL)
 write.csv(summaryRprof(file.path(output,"Rprof.out"))$by.self, file.path(output,"Rprof-summary.csv"))
}''', '')
    text = text.replace('fit <- tryCatch(fit_model(data), error=function(e) { error <<- conditionMessage(e); NULL })',
                        '''for (profile_repeat in seq_len(as.integer(Sys.getenv("ONE_FE_PROFILE_REPEAT", "1")))) {
 fit <- tryCatch(fit_model(data), error=function(e) { error <<- conditionMessage(e); NULL })
}''')
    marker = 'write_json(meta, file.path(output,"metadata.json")'
    assert text.count(marker) == 1
    text = text.replace(marker, '''if (!is.null(fit$profile_phases)) {
    write_json(list(phases=fit$profile_phases, ols=fit$profile_ols, demean=as.list(fit$profile_demean)),
               file.path(output,"profile.json"), auto_unbox=TRUE, digits=NA)
}
''' + marker)
    path.write_text(text)
    path = folder / "stata_worker.do"
    text = path.read_text()
    text = text.replace('if "`trace\'" == "1" profiler on', '')
    text = text.replace('if "`trace\'" == "1" profiler off', '')
    text = text.replace('if "`trace\'" == "1" profiler report', '')
    text = text.replace('timer clear 91', '''local outer_trace : environment ONE_FE_OUTER_TRACE
local profile_repeat : environment ONE_FE_PROFILE_REPEAT
if "`profile_repeat'" == "" local profile_repeat 1
if "`outer_trace'" == "1" profiler on
timer clear 91''')
    text = text.replace("timer on 91\ncapture quietly `command'\nlocal rc = _rc",
                        "timer on 91\nlocal rc 0\nforvalues profile_j = 1/`profile_repeat' {\n capture quietly `command'\n local rc = _rc\n if `rc' continue, break\n}")
    text = text.replace('timer off 91', '''timer off 91
if "`outer_trace'" == "1" profiler off''')
    text = text.replace('tempname meta', '''if "`outer_trace'" == "1" profiler report
tempname meta''')
    text = text.replace('file close `meta\'', '''local publication_trace : environment ONE_FE_PUBLICATION
if "`publication_trace'" == "1" {
    forvalues profile_timer = 81/85 {
        quietly timer list `profile_timer'
        file write `meta' "profile_`profile_timer'" _tab %24.17g (r(t`profile_timer')) _n
    }
}
file close `meta\'''' )
    path.write_text(text)
    path = folder / "measure.py"
    text = path.read_text()
    marker = '                        (output / "START.CHECKED").write_text("SAMPLED\\n")'
    assert text.count(marker) == 1
    text = text.replace(marker, marker + '''
                        if os.environ.get("ONE_FE_SAMPLE") == "1":
                            subprocess.Popen(["/usr/bin/sample", str(process.pid), "3", "1", "-mayDie",
                                              "-file", str(output / "sample.txt")],
                                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
''')
    path.write_text(text)
    return folder


def compare(a, b, exact_native=True):
    results = [json.loads((p / "result.json").read_text()) for p in (a, b)]
    assert all(r["status"] == "pass" for r in results), results
    assert results[0]["sample_sha256"] == results[1]["sample_sha256"]
    assert results[0]["metadata"]["N"] == results[1]["metadata"]["N"]
    errors = {}
    for file, keys, values in [("coefficients.csv", ["name"], ["value", "se"]),
                               ("covariance.csv", ["row", "column"], ["value"])]:
        x, y = [pd.read_csv(p / file).set_index(keys).sort_index() for p in (a, b)]
        assert x.index.equals(y.index)
        for col in values:
            rtol = 1e-7 if file == "coefficients.csv" and col == "value" else 1e-6
            np.testing.assert_allclose(x[col], y[col], rtol=rtol, atol=1e-10)
            errors[file + ":" + col] = float(np.max(np.abs(x[col] - y[col])))
    np.testing.assert_allclose(results[0]["metadata"]["rss"], results[1]["metadata"]["rss"], rtol=1e-8)
    if exact_native and results[0]["arm"] == results[1]["arm"] == "fereg":
        for name in ("b", "V", "fereg_status", "fereg_dof", "fereg_certificate", "fereg_fit"):
            x, y = [pd.read_csv(p / (name + ".csv")) for p in (a, b)]
            np.testing.assert_array_equal(x.to_numpy(), y.to_numpy())
    return dict(sample_exact=True, errors=errors)


def oracle(data, output, k):
    frame = pd.read_stata(data)
    X = frame[[f"x{i}" for i in range(1, k + 1)]].to_numpy()
    D = pd.get_dummies(frame.g1, dtype=float).to_numpy()
    Z = np.column_stack([X, D])
    y = frame.y.to_numpy()
    b, _, rank, _ = np.linalg.lstsq(Z, y, rcond=None)
    residual = y - Z @ b
    rss = residual @ residual
    V = np.linalg.inv(Z.T @ Z)[:k, :k] * rss / (len(y) - rank)
    actual = pd.read_csv(output / "coefficients.csv").set_index("name")
    names = [f"x{i}" for i in range(1, k + 1)]
    np.testing.assert_allclose(actual.loc[names, "value"], b[:k], rtol=1e-7, atol=1e-10)
    actual_v = pd.read_csv(output / "covariance.csv").pivot(index="row", columns="column", values="value")
    np.testing.assert_allclose(actual_v.loc[names, names], V, rtol=1e-6, atol=1e-10)
    return dict(status="pass", rank=int(rank), residual_df=len(y)-int(rank), rss=float(rss))


def main(args):
    root = args.root.resolve()
    folder = stage(root) if not (root / "controller").exists() else root / "controller"
    sys.path.insert(0, str(folder))
    from common import read_json, write_json, sha256
    measure = load("profile_measure", folder / "measure.py").measure
    generate = load("profile_generate", REPO / "benchmarks/systematic/generate.py").generate
    manifest = read_json(REPO / "benchmarks/systematic/manifest.json")
    manifest["measurement"]["command_timeout_seconds"] = 120
    manifest["measurement"]["rss_interval_seconds"] = .05
    (root / "work/refs").mkdir(parents=True, exist_ok=True)
    source = root / ("detailed" if args.part in ("deep", "sample", "publication") else "baseline")
    if not os.environ.get("STATA_MP_PATH"):
        raise RuntimeError("STATA_MP_PATH must identify the licensed local Stata executable")
    plugin = sha256(source / "dist/fereg_macos.plugin")
    identity = dict(plugin_sha256=plugin, fixest_archive_sha256=sha256(root / "fixest.tar.gz"),
                    controller={p.name: sha256(p) for p in folder.iterdir() if p.is_file()},
                    source_commit=(root / "source-commit.txt").read_text().strip())
    write_json(root / f"identity-{args.part}.json", identity)

    def dataset(n, k, groups=None, shuffled=False):
        tag = f"n{n}-k{k}-g{groups or 'default'}-shuffle{int(shuffled)}"
        path = root / "data" / (tag + ".dta")
        cell = dict(model="ols", design="one_fe", n=n, k=k, threads=1,
                    effects=["g1"], section="width")
        if not path.exists():
            generate(cell, 104729, path, manifest)
            frame = pd.read_stata(path, convert_categoricals=False)
            if groups:
                frame.g1 = np.arange(n) % groups + 1
            if shuffled:
                frame = frame.iloc[np.random.default_rng(7817).permutation(n)].reset_index(drop=True)
            if groups or shuffled:
                import datetime
                frame.to_stata(path, write_index=False, version=118, time_stamp=datetime.datetime(2000, 1, 1))
                info = read_json(path.with_suffix(".json"))
                info["column_sha256"] = {c: hashlib.sha256(np.asarray(frame[c], dtype="<f8").tobytes()).hexdigest() for c in frame}
                info["sha256"] = sha256(path)
                info["profile_variant"] = dict(groups=groups, shuffled=shuffled)
                write_json(path.with_suffix(".json"), info)
            with path.with_suffix(".bin").open("wb") as stream:
                for c in frame:
                    stream.write(np.asarray(frame[c], dtype="<f8").tobytes())
        return path, cell, tag

    def fit(data, cell, tag, arm, rep, trace=False):
        warm, _, _ = dataset(7680, cell["k"])
        output = root / "runs" / f"{tag}-t{cell['threads']}-r{rep}-{'trace' if trace else 'stock'}-{arm}"
        if output.exists():
            raise RuntimeError(f"refusing to overwrite {output}")
        library = "R-traced" if trace else "R-stock"
        os.environ["R_LIBS_USER"] = str(root / library) + ":" + args.r_dependencies
        result = measure(root, cell, arm, "warm", data, warm, output, manifest,
                         identity=identity, source=source, trace=trace)
        print(output.name, result["status"], result["seconds"], flush=True)
        assert result["status"] == "pass", output
        return output

    if args.part == "smoke":
        data, cell, tag = dataset(360, 10)
        cell["threads"] = 4
        outputs = [fit(data, cell, tag, arm, 1, trace) for trace in (False, True) for arm in ("fereg", "fixest")]
        checks = [oracle(data, output, 10) for output in outputs]
        checks += [compare(outputs[0], output) for output in outputs[1:]]
        assert (outputs[-1] / "profile.json").exists()
        write_json(root / "smoke.json", dict(status="pass", checks=checks))
    elif args.part in ("outer", "sample"):
        assert read_json(root / "smoke.json")["status"] == "pass"
        os.environ["ONE_FE_OUTER_TRACE"] = "1" if args.part == "outer" else "0"
        os.environ["ONE_FE_SAMPLE"] = "1" if args.part == "sample" else "0"
        os.environ["ONE_FE_PROFILE_REPEAT"] = "8" if args.part == "sample" else "1"
        for k in (10, 50):
            data, cell, tag = dataset(983040, k)
            cell["threads"] = 16
            for arm in (("fereg",) if args.part == "outer" else ("fereg", "fixest")):
                output = fit(data, cell, tag + "-" + args.part, arm, 1, True)
                baseline = root / "runs" / f"{tag}-t16-r3-stock-{arm}"
                write_json(output / "instrumentation-check.json", compare(baseline, output))
        write_json(root / (args.part + ".json"), dict(status="pass", identity=identity))
    elif args.part in ("deep", "publication"):
        assert read_json(root / "smoke.json")["status"] == "pass"
        os.environ["ONE_FE_PUBLICATION"] = "1" if args.part == "publication" else "0"
        for k in (10, 50):
            data, cell, tag = dataset(983040, k)
            cell["threads"] = 16
            baseline = root / "runs" / f"{tag}-t16-r3-stock-fereg"
            for rep in (1, 2, 3):
                output = fit(data, cell, tag + "-" + args.part, "fereg", rep, True)
                write_json(output / "instrumentation-check.json", compare(baseline, output))
                text = (output / "stderr.txt").read_text()
                assert all("FEREG_PHASE|" + name + "|" in text for name in
                           ("input_detail", "prepare_detail", "finite_detail")), "diagnostic binary missing timers"
                if args.part == "publication":
                    assert read_json(output / "result.json")["metadata"]["profile_81"] > 0
        write_json(root / (args.part + ".json"), dict(status="pass", identity=identity))
    else:
        assert read_json(root / "smoke.json")["status"] == "pass"
        cases = [(245760, 10, None, False), (983040, 10, None, False), (983040, 50, None, False)]
        if args.part == "shape":
            cases = [(983040, 50, 1000, False), (983040, 50, None, True)]
        for n, k, groups, shuffled in cases:
            data, cell, tag = dataset(n, k, groups, shuffled)
            for threads in ((4, 16) if args.part == "shape" else (1, 4, 16)):
                cell["threads"] = threads
                baseline = {}
                for rep in (1, 2, 3):
                    order = ("fereg", "fixest") if rep % 2 else ("fixest", "fereg")
                    outputs = {arm: fit(data, cell, tag, arm, rep) for arm in order}
                    write_json(outputs["fereg"] / "comparison.json", compare(outputs["fereg"], outputs["fixest"]))
                    baseline = outputs
                for rep in (1, 2):
                    order = ("fixest", "fereg") if rep % 2 else ("fereg", "fixest")
                    for arm in order:
                        output = fit(data, cell, tag, arm, rep, True)
                        write_json(output / "instrumentation-check.json", compare(baseline[arm], output))
        write_json(root / (args.part + ".json"), dict(status="pass", identity=identity))
    print("ONE_FE_PROFILE_COMPLETE", args.part, flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("--part", choices=("smoke", "primary", "shape", "deep", "outer", "sample", "publication"), required=True)
    p.add_argument("--r-dependencies", required=True)
    main(p.parse_args())
