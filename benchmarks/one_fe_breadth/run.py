"""Bounded one-FE inference/output comparisons; local development evidence only."""
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

REPO = Path(__file__).resolve().parents[2]
DATA = REPO / "target/one-fe-profile-20260919"
ARTIFACTS = REPO / "target/one-fe-optimization-20260919"
CASES = {
    "iid": ("", "iid", "none"),
    "robust": ("robust", "hetero", "none"),
    "cluster_fe": ("cluster g1", "~g1", "none"),
    "cluster_cross": ("cluster c1", "~c1", "none"),
    "cluster2_cross": ("cluster c1 c2", "~c1+c2", "none"),
    "cluster2_nested": ("cluster g1 c2", "~g1+c2", "none"),
    "iid_saved": ("", "iid", "saved"),
    "cluster_fe_saved": ("cluster g1", "~g1", "saved"),
    "cluster2_saved": ("cluster c1 c2", "~c1+c2", "saved"),
    "iid_outputs": ("", "iid", "all"),
}
PRIMARY_CASES = tuple(CASES)
# Focused diagnostic outside the frozen twenty-cell primary matrix.
CASES["robust_saved"] = ("robust", "hetero", "saved")


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def sha(path):
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(2**20), b""):
            h.update(block)
    return h.hexdigest()


def replace_once(text, old, new):
    assert text.count(old) == 1, old
    return text.replace(old, new)


def stage(root, profile):
    folder = profile.stage(root)
    path = folder / "stata_worker.do"
    text = path.read_text()
    text = replace_once(text, "local trace : environment FEREG_LOCAL_TRACE", """local trace : environment FEREG_LOCAL_TRACE
local breadth_outputs : environment ONE_FE_BREADTH_OUTPUTS""")
    text = replace_once(text, "timer clear 91\ntimer on 91", "timer clear 92\ntimer on 92\ntimer clear 91\ntimer on 91")
    text = replace_once(text, "local seconds = r(t91)", """local fit_seconds = r(t91)
if !`rc' & "`breadth_outputs'" == "all" {
    capture quietly predict double out_xb, xb
    local rc = _rc
    if !`rc' {
        capture quietly predict double out_xbd, xbd
        local rc = _rc
    }
}
timer off 92
quietly timer list 92
local seconds = r(t92)""")
    text = replace_once(text, 'file write `meta\' "stata_processors"', '''file write `meta' "fit_seconds" _tab %24.17g (`fit_seconds') _n
file write `meta' "stata_processors"''')
    old = '''    capture unab savedvars : saved_*
    if !_rc {
        format `savedvars' %24.17g
        quietly export delimited obs_id `savedvars' using `"`output'/saved_outputs.csv"', replace datafmt
    }'''
    new = '''    if "`breadth_outputs'" != "none" {
        local outputvars out_fe
        if "`breadth_outputs'" == "all" local outputvars out_fe out_resid out_xb out_xbd
        mata: __sb_export_input(st_local("output") + "/outputs.bin", tokens(st_local("outputvars")))
    }'''
    text = replace_once(text, old, new)
    path.write_text(text)
    path = folder / "r_worker.R"
    text = path.read_text()
    text = replace_once(text, 'vcov="iid", notes=FALSE', '''vcov=if (startsWith(cell$r_vce, "~")) as.formula(cell$r_vce) else cell$r_vce,
              ssc=ssc(K.adj=TRUE, K.fixef="nonnested", G.adj=TRUE, G.df="min", t.df="min"), notes=FALSE''')
    helper = '''recover_outputs <- function(fit) {
    if (is.null(fit) || cell$outputs == "none") return(NULL)
    # Use the fitted object's existing level codes, as fixest's own recovery
    # does, rather than recoding or character-matching a million observations.
    effects <- fixef(fit, sorted=FALSE, notes=FALSE)$g1
    contribution <- unname(effects[fit$fixef_id$g1])
    intercept <- mean(contribution)
    saved_outputs <- list(contribution - intercept)
    if (cell$outputs == "all") {
        residual <- resid(fit)
        prediction <- fitted(fit)
        finite <- prediction - contribution + intercept
        saved_outputs <- list(contribution - intercept, residual, finite, prediction)
    }
    saved_outputs
}
'''
    text = replace_once(text, 'if (c$phase == "warm") {', helper + 'if (c$phase == "warm") {')
    text = replace_once(text, 'invisible(fit_model(warm))', 'invisible(recover_outputs(fit_model(warm)))')
    marker = 'seconds <- proc.time()[["elapsed"]] - start'
    text = replace_once(text, marker, '''fit_seconds <- proc.time()[["elapsed"]] - start
saved_outputs <- recover_outputs(fit)
''' + marker)
    text = replace_once(text, 'R_version=as.character(getRversion())', 'fit_seconds=fit_seconds, R_version=as.character(getRversion())')
    text = replace_once(text,
                        'meta$covariance_normalization <- if (cell$model == "ppml") "HC0 multiplied by N/(N-1)" else "native IID"',
                        'meta$covariance_normalization <- paste("native", cell$r_vce, "with explicit nonnested-K/min-G SSC")')
    text = replace_once(text, 'meta$N <- nobs(fit)', '''if (!is.null(saved_outputs)) {
        con <- file(file.path(output, "outputs.bin"), "wb")
        for (values in saved_outputs) writeBin(as.double(values), con, size=8, endian="little")
        close(con)
    }
    meta$df_K_vcov <- attr(vcov(fit, attr=TRUE), "df.K")
    meta$G <- attr(vcov(fit, attr=TRUE), "G")
    meta$N <- nobs(fit)''')
    path.write_text(text)
    return folder


def dataset(root, n, k):
    path = root / "data" / f"n{n}-k{k}.dta"
    if path.exists():
        assert sha(path) == json.loads(path.with_suffix(".json").read_text())["sha256"]
        return path
    original = DATA / "data" / f"n{n}-k{k}-gdefault-shuffle0.dta"
    frame = pd.read_stata(original, convert_categoricals=False)
    ids = frame.obs_id.to_numpy().astype(np.int64) - 1
    frame["c1"] = (ids % (31 if n == 360 else 1024) + 1).astype(float)
    frame["c2"] = ((ids // 7) % (17 if n == 360 else 257) + 1).astype(float)
    path.parent.mkdir(parents=True, exist_ok=True)
    frame.to_stata(path, write_index=False, version=118, time_stamp=datetime.datetime(2000, 1, 1))
    info = dict(n=n, k=k, columns=list(frame), parent_sha256=sha(original), sha256=sha(path),
                levels={c: int(frame[c].nunique()) for c in ("g1", "c1", "c2")},
                column_sha256={c: hashlib.sha256(np.asarray(frame[c], dtype="<f8").tobytes()).hexdigest() for c in frame})
    path.with_suffix(".json").write_text(json.dumps(info, indent=2) + "\n")
    with path.with_suffix(".bin").open("wb") as f:
        for c in frame:
            f.write(np.asarray(frame[c], dtype="<f8").tobytes())
    return path


def compare_outputs(a, b, n, mode):
    if mode == "none":
        return {}
    count = 4 if mode == "all" else 1
    x, y = [np.fromfile(p / "outputs.bin", dtype="<f8").reshape(count, n) for p in (a, b)]
    np.testing.assert_allclose(x, y, rtol=1e-7, atol=1e-9)
    return dict(max_abs_errors=np.max(np.abs(x-y), axis=1).tolist(),
                names=["fe", "residual", "xb", "xbd"][:count])


def oracle(data, output, cell):
    frame = pd.read_stata(data)
    k, n = cell["k"], len(frame)
    X = frame[[f"x{i}" for i in range(1, k+1)]].to_numpy()
    D = pd.get_dummies(frame.g1, dtype=float).to_numpy()
    Z = np.column_stack([X, D])
    beta, _, rank, _ = np.linalg.lstsq(Z, frame.y, rcond=None)
    residual = frame.y.to_numpy() - Z @ beta
    bread = np.linalg.inv(Z.T @ Z)
    influence = (Z @ bread[:, :k]) * residual[:, None]
    vce = cell["vce"]
    if not vce:
        V = bread[:k, :k] * (residual @ residual) / (n-rank)
    elif vce == "robust":
        V = influence.T @ influence * n / (n-rank)
    else:
        import itertools
        names = vce.split()[1:]
        V = np.zeros((k, k))
        for count in range(1, len(names)+1):
            for subset in itertools.combinations(names, count):
                _, codes = np.unique(frame[list(subset)].to_numpy(), axis=0, return_inverse=True)
                summed = np.zeros((codes.max()+1, k))
                np.add.at(summed, codes, influence)
                V += (-1)**(count+1) * (summed.T @ summed)
        nested = any(frame.groupby("g1")[name].nunique().max() == 1 for name in names)
        df_model = k+1 if nested else rank
        g = min(frame[name].nunique() for name in names)
        V *= (n-1)/(n-df_model) * g/(g-1)
        eig, vectors = np.linalg.eigh(V)
        if eig.min() < 0:
            V = (vectors * np.maximum(eig, 0)) @ vectors.T
    actual = pd.read_csv(output / "coefficients.csv").set_index("name")
    names = [f"x{i}" for i in range(1, k+1)]
    np.testing.assert_allclose(actual.loc[names, "value"], beta[:k], rtol=1e-7, atol=1e-10)
    actual_v = pd.read_csv(output / "covariance.csv").pivot(index="row", columns="column", values="value").loc[names, names]
    np.testing.assert_allclose(actual_v, V, rtol=1e-6, atol=1e-10)
    if cell["outputs"] != "none":
        fe = D @ beta[k:]
        intercept = fe.mean()
        expected = [fe-intercept]
        if cell["outputs"] == "all":
            expected += [residual, X @ beta[:k] + intercept, Z @ beta]
        actual = np.fromfile(output / "outputs.bin", dtype="<f8").reshape(len(expected), n)
        np.testing.assert_allclose(actual, expected, rtol=1e-7, atol=1e-9)
    return dict(status="pass", rank=int(rank), full_covariance_max_error=float(np.max(np.abs(actual_v.to_numpy()-V))))


def main(a):
    root = a.root.resolve()
    root.mkdir(parents=True, exist_ok=True)
    profile = load("breadth_profile", REPO / "benchmarks/one_fe_profile/run.py")
    folder = root / "controller"
    if not folder.exists():
        folder = stage(root, profile)
    sys.path.insert(0, str(folder))
    measure = load("breadth_measure", folder / "measure.py").measure
    manifest = json.loads((REPO / "benchmarks/systematic/manifest.json").read_text())
    manifest["measurement"].update(command_timeout_seconds=120, rss_interval_seconds=.05)
    (root / "work/refs").mkdir(parents=True, exist_ok=True)
    identities = {}
    for variant in a.variants:
        if variant == "fixest":
            continue
        artifact_root, name = (ARTIFACTS, "normonly") if variant == "fereg" else (a.artifacts.resolve(), variant)
        source = artifact_root / name
        native_identity = json.loads((artifact_root / f"identity-{name}.json").read_text())
        assert sha(source / "dist/fereg_macos.plugin") == native_identity["plugin"]
        identities[variant] = (source, native_identity)
    controller_hashes = {p.name: sha(p) for p in folder.iterdir() if p.is_file()}
    shared_identity = dict(fixest_archive_sha256=sha(DATA / "fixest.tar.gz"), controller=controller_hashes,
                           harness_sha256=sha(Path(__file__)), audit_sha256=sha(REPO / "benchmarks/one_fe_profile/run.py"))
    checks = []
    for n, k in ([(360, 10)] if a.part == "smoke" else [(983040, k) for k in a.widths]):
        data, warm = dataset(root, n, k), dataset(root, 7680, k)
        for case in a.cases:
            vce, r_vce, mode = CASES[case]
            cell = dict(model="ols", design="one_fe", n=n, k=k, threads=16, effects=["g1"],
                        vce=vce, r_vce=r_vce, outputs=mode, case=case,
                        absorb="out_fe=g1" if mode != "none" else "g1",
                        fit_options="residuals(out_resid)" if mode == "all" else "")
            os.environ["ONE_FE_BREADTH_OUTPUTS"] = mode
            for rep in range(1, a.repeats+1):
                rotation = (rep-1) % len(a.variants)
                order = a.variants[rotation:] + a.variants[:rotation]
                if a.trace:
                    order = [variant for variant in order if variant != "fixest"]
                outputs = {}
                for variant in order:
                    arm = "fixest" if variant == "fixest" else "fereg"
                    source, native_identity = identities.get(variant, (None, None))
                    identity = dict(shared_identity, native=native_identity)
                    output = root / "runs" / f"{a.label}-{case}-n{n}-k{k}-r{rep}-{variant}"
                    if a.trace:
                        os.environ["ONE_FE_BREADTH_TRACE_DIR"] = str(output)
                    result = measure(root, cell, arm, "warm", data, warm, output, manifest,
                                     identity=identity, source=source, trace=a.trace)
                    print(output.name, result["status"], result["seconds"], flush=True)
                    assert result["status"] == "pass", output
                    outputs[variant] = output
                    if a.part == "smoke":
                        checks.append(dict(run=output.name, oracle=oracle(data, output, cell)))
                if not a.trace:
                    for variant in a.variants[1:]:
                        reference, actual = outputs[a.variants[0]], outputs[variant]
                        comparison = profile.compare(reference, actual, exact_native=False)
                        if variant != "fixest" and a.variants[0] != "fixest":
                            for name in ("fereg_status", "fereg_dof"):
                                np.testing.assert_array_equal(pd.read_csv(reference / (name+".csv")),
                                                              pd.read_csv(actual / (name+".csv")))
                        checks.append(dict(case=case, n=n, k=k, rep=rep, variant=variant,
                                           science=comparison, outputs=compare_outputs(reference, actual, n, mode)))
                (root / (a.label + "-progress.json")).write_text(json.dumps(checks, indent=2)+"\n")
    (root / (a.label + "-complete.json")).write_text(json.dumps(dict(status="pass", checks=checks), indent=2)+"\n")
    print("ONE_FE_BREADTH_COMPLETE", a.label, flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("root", type=Path)
    p.add_argument("--part", choices=("smoke", "primary"), required=True)
    p.add_argument("--label", required=True)
    p.add_argument("--cases", choices=list(CASES), nargs="+", default=list(PRIMARY_CASES))
    p.add_argument("--widths", type=int, nargs="+", choices=(10, 50), default=[10, 50])
    p.add_argument("--repeats", type=int, default=3)
    p.add_argument("--trace", action="store_true")
    p.add_argument("--variants", nargs="+", default=["fereg", "fixest"])
    p.add_argument("--artifacts", type=Path, default=ARTIFACTS)
    main(p.parse_args())
