#!/usr/bin/env python3
"""Bounded OLS diagnostics using the existing strict capture, never an F0 verdict.

The F0 capture module is a renderer/parser, below its closed-profile evaluator.
Here each child supplies one cold and three warm fits. Only the final warm fit
uses that module's timing field; additional timings are separately recorded.
All three warm fits must exactly reproduce the cold scientific matrices.
"""
from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import re
import statistics
import subprocess
import sys
import tomllib
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from scripts import fereg_f0_stata_capture as capture
from scripts import run_fereg_f0_local as local

MANIFEST = ROOT / "benchmarks/manifests/ols_arithmetic_development_v1.json"
STATA = "/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp"
GENERATOR = ROOT / "tests/stata/development/feppml_solver_development.do"
SHA = capture.sha256_file
SCIENCE = ("b", "V", "fereg_status", "fereg_fit", "fereg_dof", "fereg_certificate")
TOPOLOGY = {
    "no_fe": ("balanced_1fe", "easy"),
    "one_fe": ("balanced_1fe", "long"),
    "balanced_two": ("balanced_2fe", "easy"),
    "weak_two": ("chain_2fe", "long"),
    "balanced_three": ("balanced_3fe", "easy"),
    "sparse_three": ("sparse_3fe", "long"),
    "mixed_three": ("balanced_3fe", "long"),
}


class ScientificMismatch(ValueError):
    """A complete reference disagrees, distinct from unavailable evidence."""


def write_json(path, value):
    path.write_text(json.dumps(value, sort_keys=True, indent=2, allow_nan=False) + "\n")


def scopes():
    m = json.loads(MANIFEST.read_text())
    result = []
    for design, widths in m["core"]:
        for k in widths:
            for threads in m["threads"]:
                result.append(dict(design=design, k=k, threads=threads, memory="fast", guard=""))
    for design, k in m["low"]:
        for threads in m["threads"]:
            result.append(dict(design=design, k=k, threads=threads, memory="low", guard=""))
    for guard in m["guards"]:
        result.append(dict(design="one_fe" if guard != "saved" else "weak_two",
                           k=50 if guard in ("aweight", "fweight", "pweight") else 10,
                           threads=7, memory="fast", guard=guard))
    for s in result:
        s["id"] = f'{s["design"]}_k{s["k"]}_t{s["threads"]}_{s["memory"]}' + ("_" + s["guard"] if s["guard"] else "")
    return result


def run_stata(work, driver, *, environment=None, timeout=120):
    work.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    for name in list(env):
        if name.startswith("FEREG_"):
            del env[name]
    env.update(environment or {})
    cmd = [sys.executable, str(ROOT / "ci/supervise_stata.py"),
           "--timeout-seconds", str(timeout), "--process-json", str(work / "process.json"),
           "--stdout-log", str(work / "stdout.log"), "--stderr-log", str(work / "stderr.log"),
           "--cwd", str(work), "--", STATA, "-b", "do", str(driver)]
    subprocess.run(cmd, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, check=False)
    record = json.loads((work / "process.json").read_text())
    logs = [work / f"{driver.stem}.log", work / "stdout.log", work / "stderr.log"]
    text = "\n".join(p.read_text(errors="replace") for p in logs if p.exists())
    if record["timed_out"] or record["process_rc"] != 0 or record["launch_error"]:
        raise RuntimeError(f'Stata process failed: {work.name}; timeout={record["timed_out"]}')
    return text


def reference_identity(reference):
    records = tomllib.loads((ROOT / "references.lock.toml").read_text())["reference"]
    identity = {}
    for r in records:
        if r["name"] not in ("require", "ftools", "reghdfe"):
            continue
        for name, expected in r["required_capture_sha256"].items():
            if SHA(reference / r["name"] / name) != expected:
                raise ValueError(f'pinned reference differs: {r["name"]}/{name}')
        identity[r["name"]] = {"commit": r["commit"], "files": r["required_capture_sha256"]}
    return identity


def prepare(work, reference):
    work.mkdir(parents=True, exist_ok=True)
    if (work / "preparation.json").exists():
        raise ValueError("preparation already exists")
    refs = reference_identity(reference)
    m = json.loads(MANIFEST.read_text())
    selected = scopes()
    selected_designs = {scope["design"] for scope in selected}
    generated_controls = max(50, *(scope["k"] for scope in selected))
    datasets = {}
    for index, (design, (topology, difficulty)) in enumerate(TOPOLOGY.items()):
        if design not in selected_designs:
            continue
        dst = work / "datasets" / design
        dst.mkdir(parents=True)
        dataset = dst / "data.dta"
        env = {"FEREG_PPML_SOLVER_MODE": "generate", "FEREG_PPML_SOLVER_N": str(m["observations"]),
               "FEREG_PPML_SOLVER_K": str(generated_controls), "FEREG_PPML_SOLVER_SEED": str(m["seed_base"] + index),
               "FEREG_PPML_SOLVER_TOPOLOGY": topology, "FEREG_PPML_SOLVER_DIFFICULTY": difficulty,
               "FEREG_PPML_SOLVER_DATASET": str(dataset), "FEREG_PPML_SOLVER_DATA_CSV": str(dst / "generated.csv"),
               "FEREG_PPML_SOLVER_DATA_META": str(dst / "topology.tsv")}
        if design == "mixed_three":
            env["FEREG_PPML_SOLVER_THIRD_COLUMNS"] = "9"
        text = run_stata(dst, GENERATOR, environment=env)
        if "FEREG_PPML_SOLVER_DATA_PASS|" not in text:
            raise RuntimeError(f"generator did not complete: {design}")
        adapt = dst / "linear.do"
        adapt.write_text(f'''version 18
clear all
set more off
use "{dataset}", clear
set seed {m['seed_base'] + index + 100}
drop y
generate double y = 2 + .2*x1 - .1*x2 + .3*sin(g1/7) + rnormal()
capture confirm variable g2
if !_rc replace y = y + .2*cos(g2/11)
capture confirm variable g3
if !_rc replace y = y + .1*sin(g3/3)
generate double weight = .25 + mod(observation*37, 97)/31
generate int frequency = 1 + mod(observation, 3)
generate double slope = mod(observation, 19)/19
generate double duplicate = x1
save "{dataset}", replace
export delimited y x1-x{generated_controls} g* weight frequency slope duplicate observation using "{dst / 'linear.csv'}", replace
display "OLS_LINEAR_DATA_PASS"
exit 0
''')
        text = run_stata(dst, adapt)
        if "OLS_LINEAR_DATA_PASS" not in text:
            raise RuntimeError("linear generator failed")
        datasets[design] = {"path": str(dataset), "sha256": SHA(dataset), "n": m["observations"],
                            "linear_adaptation_sha256": SHA(adapt), "topology_sha256": SHA(dst / "topology.tsv"),
                            "seed": m["seed_base"] + index}
        print(f"prepared {design}", flush=True)
    write_json(work / "preparation.json", dict(manifest_sha256=SHA(MANIFEST), generator_sha256=SHA(GENERATOR),
               references=refs, datasets=datasets, qualification_eligible=False))


def command(scope, reference=False):
    controls = " ".join(f"x{i}" for i in range(1, scope["k"] + 1))
    if scope["guard"] == "rank":
        controls += " duplicate"
    weight = {"aweight": "[aw=weight]", "fweight": "[fw=frequency]", "pweight": "[pw=weight]"}.get(scope["guard"], "")
    vce = "vce(robust)" if scope["guard"] == "pweight" else ""
    design = scope["design"]
    fe = "" if design == "no_fe" else "g1" if design == "one_fe" else "g1 g2 g3" if design.endswith("three") else "g1 g2"
    if scope["guard"] == "slope":
        fe = "g1##c.slope"
    if scope["guard"] == "saved":
        fe = "saved1=g1 saved2=g2"
    absorb = f"absorb({fe})" if fe else ("noabsorb" if reference else "")
    options = "tolerance(1e-12)" if reference else f'threads({scope["threads"]}) memory({scope["memory"]}) solver(auto)'
    # Reference saved-FE normalizations need not be unique; compare predictions.
    return f'{"reghdfe" if reference else "fereg"} y {controls} {weight}, {absorb} {vce} {options}'.strip()


def instrument_driver(text, cmd):
    # Preserve the existing parser/transaction checks. Add a separate clock for
    # the cold fit and stabilizers; the final warm fit retains timer 97.
    needle = f"capture noisily {cmd}\nlocal science_rc = _rc"
    text = text.replace(needle, f"timer clear 98\ntimer on 98\n{needle}\ntimer off 98\nquietly timer list 98\ndisplay \"OLS_PERF_TIME|cold|\" %21.16g r(t98)")
    saved = "\n".join(f"matrix __op_{i} = e({name})" for i, name in enumerate(SCIENCE))
    text = text.replace("/* Exactly two untimed stabilizing calls", saved + "\n/* Exactly two untimed stabilizing calls")
    needle = f"capture quietly {cmd}\n    local stabilizer_rc = _rc"
    checks = "\n".join(
        f'    if mreldif(__op_{i}, e({name})) != 0 display "OLS_REPEAT_MISMATCH|{name}|" %21.16g mreldif(__op_{i}, e({name}))\n'
        f"    assert mreldif(__op_{i}, e({name})) == 0"
        for i, name in enumerate(SCIENCE)
    )
    text = text.replace(needle, f"timer clear 98\n    timer on 98\n    {needle}\n    timer off 98\n    quietly timer list 98\n    display \"OLS_PERF_TIME|warm|\" %21.16g r(t98)\n{checks}")
    # The cold capture has cleared e() by this point. Snapshot its matrices
    # immediately before that clear instead of after it.
    text = text.replace(saved + "\n/* Exactly two", "/* Exactly two")
    text = text.replace("ereturn clear\nfile write", saved + "\nereturn clear\nfile write", 1)
    return text


def capture_fit(work, scope, package, dataset, *, trace=False):
    plugin = package / "fereg_macos.plugin"
    spec = capture.ArmCaptureSpec(profile="local_screen", arm="candidate", case_id=scope["id"],
        manifest_sha256=SHA(MANIFEST), source_commit=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        package_root=package, plugin_filename=plugin.name, plugin_sha256=SHA(plugin),
        dataset_path=Path(dataset["path"]), dataset_sha256=dataset["sha256"],
        expected_observations=dataset["n"], command=command(scope), requested_threads=scope["threads"],
        output_dir=work, obs_id="observation", saved_outputs=("saved1", "saved2") if scope["guard"] == "saved" else ())
    driver = capture.write_arm_do(spec)
    driver.write_text(instrument_driver(driver.read_text(), spec.command))
    env = {"FEREG_PHASE_TIMING": "1", "FEREG_SOLVER_TRACE": "1"} if trace else {}
    text = run_stata(work, driver, environment=env)
    result = capture.load_completed_capture(spec, text)
    times = re.findall(r"^OLS_PERF_TIME\|(cold|warm)\|\s*([0-9.eE+-]+)\s*$", text, re.M)
    if [k for k, _ in times] != ["cold", "warm", "warm"]:
        raise ValueError("incomplete cold/warm timing inventory")
    warm = [float(t) for _, t in times[1:]] + [result.seconds]
    phase_groups = {}
    for line in text.splitlines():
        if line.startswith("FEREG_PHASE|"):
            fields = line.split("|")
            phase_groups.setdefault(fields[1], []).append(
                {k: float(v) for k, v in (part.split("=") for part in fields[2:])})
    return dict(scientific=result.scientific, raw=result.raw, environment=result.environment,
                cold_seconds=float(times[0][1]), warm_seconds=warm, median_seconds=statistics.median(warm),
                traced=trace, phases=phase_groups.get("fit", []), phase_groups=phase_groups,
                command=spec.command, plugin_sha256=spec.plugin_sha256,
                driver_sha256=SHA(driver), capture_sha256=SHA(work / capture.CAPTURE_FILENAME))


def parse_reference(text):
    dimensions, cells, scalars = {}, {}, {}
    for line in text.splitlines():
        if not line.startswith(capture.MACHINE_PREFIX):
            raise ValueError("unknown reference record")
        fields = line[len(capture.MACHINE_PREFIX):].split("|")
        kind = fields[0]
        if kind == "matrix_dimensions":
            _, _, name, rows, cols = fields
            if name in dimensions:
                raise ValueError("duplicate reference dimension")
            dimensions[name] = (int(rows), int(cols))
        elif kind == "matrix":
            _, _, name, row, col, rn, cn, value = fields
            key = (name, int(row), int(col))
            if key in cells:
                raise ValueError("duplicate reference cell")
            cells[key] = (unquote(rn), unquote(cn), capture.stata_hex_to_float(value))
        elif kind == "scalar":
            _, _, name, value = fields
            if name in scalars:
                raise ValueError("duplicate reference scalar")
            scalars[name] = capture.stata_hex_to_float(value)
        else:
            raise ValueError("unexpected reference record")
    if set(dimensions) != {"b", "V"} or set(scalars) != {"N", "df_a", "df_r", "rss", "r2"}:
        raise ValueError("incomplete reference inventory")
    matrices = {}
    expected = set()
    for name, (rows, cols) in dimensions.items():
        expected.update((name, r, c) for r in range(1, rows + 1) for c in range(1, cols + 1))
        entries = [cells.get((name, r, c)) for r in range(1, rows + 1) for c in range(1, cols + 1)]
        if any(v is None or not math.isfinite(v[2]) for v in entries):
            raise ValueError("incomplete/nonfinite reference matrix")
        for r in range(rows):
            if len({entries[r * cols + c][0] for c in range(cols)}) != 1:
                raise ValueError("inconsistent reference row stripes")
        for c in range(cols):
            if len({entries[r * cols + c][1] for r in range(rows)}) != 1:
                raise ValueError("inconsistent reference column stripes")
        matrices[name] = dict(rows=rows, columns=cols, values=[v[2] for v in entries],
                              column_names=[v[1] for v in entries[:cols]])
    if set(cells) != expected or not all(math.isfinite(v) for v in scalars.values()):
        raise ValueError("invalid reference payload")
    if dimensions["b"][0] != 1 or dimensions["V"] != (dimensions["b"][1],) * 2:
        raise ValueError("reference b/V dimensions disagree")
    covariance_rows = [cells[("V", r, 1)][0] for r in range(1, dimensions["V"][0] + 1)]
    if covariance_rows != matrices["V"]["column_names"] or matrices["b"]["column_names"] != covariance_rows:
        raise ValueError("reference b/V coordinate stripes disagree")
    return dict(matrices=matrices, scalars=scalars)


def reference_fit(work, scope, reference, dataset):
    work.mkdir(parents=True)
    paths = "\n".join(f'adopath ++ "{reference / name / "src"}"' for name in ("require", "ftools", "reghdfe"))
    checks = "\n".join(f'''findfile {name}.ado
assert `"`r(fn)'"' == "{reference / name / 'src' / (name + '.ado')}"''' for name in ("require", "ftools", "reghdfe"))
    driver = work / "reference.do"
    driver.write_text(f'''version 18
clear all
set more off
set linesize 255
set processors {scope['threads']}
{paths}
{checks}
do "{capture.HELPERS}"
use "{dataset['path']}", clear
capture noisily {command(scope, reference=True)}
local fit_rc = _rc
display "OLS_REFERENCE_RC=" `fit_rc'
if `fit_rc' exit `fit_rc'
quietly count if e(sample)
assert r(N) == _N
file open out using "{work / 'reference.machine'}", write text replace
foreach name in b V {{
    __fereg_f0_capture_matrix `name', filehandle(out) phase(science) arm(reference) caseid({scope['id']})
}}
foreach name in N df_a df_r rss r2 {{
    local val : display %21x e(`name')
    file write out "FEREG_F0_CAPTURE|1|scalar|science|`name'|`val'" _n
}}
file close out
display "OLS_REFERENCE_PASS"
exit 0
''')
    text = run_stata(work, driver)
    if not re.search(r"^OLS_REFERENCE_RC=0\s*$", text, re.M) or not re.search(r"^OLS_REFERENCE_PASS\s*$", text, re.M):
        raise ValueError("reference fit did not complete")
    return parse_reference((work / "reference.machine").read_text())


def compare_reference(candidate, reference):
    raw = candidate["raw"]
    differences = {}
    for name, atol, rtol in (("b", 1e-10, 1e-7), ("V", 1e-10, 1e-6)):
        actual = capture._matrix(raw, "science", name)
        ref = reference["matrices"][name]
        values = capture._finite_matrix(actual)
        if actual["column_names"] != ref["column_names"] or (actual["rows"], actual["columns"]) != (ref["rows"], ref["columns"]):
            raise ScientificMismatch("reference coefficient order or matrix shape differs")
        differences[name] = max((abs(a - b) for a, b in zip(values, ref["values"])), default=0.0)
        if any(abs(a-b) > atol + rtol*max(abs(a), abs(b)) for a, b in zip(values, ref["values"])):
            raise ScientificMismatch(f"reference {name} numerical mismatch: {differences[name]}")
    for name, expected in reference["scalars"].items():
        actual = capture.stata_hex_to_float(raw["scalars"]["science"][name])
        exact = name in ("N", "df_a", "df_r")
        if abs(actual - expected) > (0 if exact else 1e-9 + 1e-7*max(abs(actual), abs(expected))):
            raise ScientificMismatch(f"reference {name} mismatch: {actual} != {expected}")
        differences[name] = abs(actual - expected)
    return differences


def source_identity(source):
    files = {}
    for path in source.rglob("*"):
        name = str(path.relative_to(source))
        if name.startswith("vendor/") and name.split("/", 3)[2:3] == ["target"]:
            continue
        if path.is_file() and (name.startswith(("crates/", "vendor/", "plugin/", "stata/"))
                or name in ("Cargo.lock", "Cargo.toml", "references.lock.toml")):
            files[name] = SHA(path)
    return files


def run(args):
    prep = json.loads((args.work / "preparation.json").read_text())
    if prep["manifest_sha256"] != SHA(MANIFEST) or prep["references"] != reference_identity(args.reference):
        raise ValueError("preparation binding changed")
    source = args.source.resolve()
    out = args.work / args.label
    out.mkdir()
    package = local.stage_package(source, source / "dist/fereg_macos.plugin", out / "package")
    # Capture compiled inputs independently of a commit label, including dirty overlays.
    write_json(out / "identity.json", dict(files=source_identity(source), plugin_sha256=SHA(package / "fereg_macos.plugin"),
               controller_sha256=SHA(Path(__file__)), manifest_sha256=SHA(MANIFEST)))
    selected = [s for s in scopes() if not args.scope or s["id"] in args.scope]
    if args.scope and set(args.scope) != {s["id"] for s in selected}:
        raise ValueError("unknown scope")
    records = []
    for s in selected:
        entry = dict(scope=s)
        stage = "candidate"
        try:
            entry["fit"] = capture_fit(out / s["id"], s, package, prep["datasets"][s["design"]], trace=args.trace)
            stage = "reference"
            refpath = args.work / "references" / (s["id"] + ".json")
            if not refpath.exists():
                if (args.work / "references" / s["id"]).exists():
                    raise RuntimeError("prior pinned reference attempt has no complete accepted record")
                ref = reference_fit(args.work / "references" / s["id"], s, args.reference, prep["datasets"][s["design"]])
                write_json(refpath, ref)
            entry["reference_differences"] = compare_reference(entry["fit"], json.loads(refpath.read_text()))
            entry["status"] = "pass"
            print(f'{s["id"]}: {entry["fit"]["median_seconds"]:.6f}s reference PASS', flush=True)
        except (RuntimeError, ValueError, capture.CaptureContractError, OSError) as error:
            entry["status"] = ("reference_mismatch" if isinstance(error, ScientificMismatch)
                               else "reference_unavailable" if stage == "reference" else "failure")
            entry["stage"] = stage
            entry["error"] = str(error)
            print(f'{s["id"]}: {entry["status"]}: {error}', flush=True)
        records.append(entry)
        write_json(out / "results.json", dict(qualification_eligible=False, records=records))


def compare(args):
    """Five AB/BA pairs; each child's final warm call is the paired observation."""
    prep = json.loads((args.work / "preparation.json").read_text())
    if prep["manifest_sha256"] != SHA(MANIFEST) or prep["references"] != reference_identity(args.reference):
        raise ValueError("preparation binding changed")
    out = args.work / args.label
    out.mkdir()
    packages = {}
    for arm, source in (("baseline", args.baseline_source), ("candidate", args.source)):
        packages[arm] = local.stage_package(source, source / "dist/fereg_macos.plugin", out / ("package_" + arm))
        write_json(out / ("identity_" + arm + ".json"), dict(files=source_identity(source),
                   plugin_sha256=SHA(packages[arm] / "fereg_macos.plugin"),
                   controller_sha256=SHA(Path(__file__)), manifest_sha256=SHA(MANIFEST)))
    selected = [s for s in scopes() if s["id"] in (args.scope or [])]
    if not selected or {s["id"] for s in selected} != set(args.scope):
        raise ValueError("comparison requires explicit known scopes")
    records = []
    for scope in selected:
        reference = json.loads((args.work / "references" / (scope["id"] + ".json")).read_text())
        for pair in range(5):
            order = ("baseline", "candidate") if pair % 2 == 0 else ("candidate", "baseline")
            record = dict(scope=scope, pair=pair + 1, order=list(order), fits={})
            for arm in order:
                fit = capture_fit(out / f'{scope["id"]}_{pair}_{arm}', scope, packages[arm], prep["datasets"][scope["design"]])
                compare_reference(fit, reference)
                record["fits"][arm] = fit
            a, b = (record["fits"][arm] for arm in ("baseline", "candidate"))
            record["scientific_exact"] = a["scientific"] == b["scientific"]
            if not record["scientific_exact"]:
                write_json(out / "failed-pair.json", record)
                raise ValueError("baseline/candidate scientific results differ")
            record["ratio"] = b["warm_seconds"][-1] / a["warm_seconds"][-1]
            records.append(record)
            write_json(out / "results.json", dict(qualification_eligible=False, records=records))
            print(f'{scope["id"]} pair {pair + 1}: ratio={record["ratio"]:.6f} exact science', flush=True)


def oracle(args):
    """Independent group elimination + NumPy SVD for unweighted 0/1-FE scopes.

    This is a development oracle, outside timed fits. It neither calls the Rust
    projection/finite solver nor uses reghdfe. Wider topology and weighted guards
    need their existing explicit-dummy testkit or a separately extended oracle.
    """
    import numpy as np

    prep = json.loads((args.work / "preparation.json").read_text())
    fits = json.loads((args.work / args.label / "results.json").read_text())["records"]
    records = []
    for entry in fits:
        scope = entry["scope"]
        if scope["id"] not in (args.scope or []):
            continue
        if scope["design"] not in ("no_fe", "one_fe") or scope["guard"] or entry["status"] != "pass":
            raise ValueError("oracle requires a passing unweighted no/one-FE scope")
        dataset = prep["datasets"][scope["design"]]
        path = Path(dataset["path"]).with_name("linear.csv")
        data = np.genfromtxt(path, delimiter=",", names=True)
        y = data["y"]
        x = np.column_stack([data[f"x{i}"] for i in range(1, scope["k"] + 1)]) if scope["k"] else np.empty((len(y), 0))
        groups = np.unique(data["g1"], return_inverse=True)[1] if scope["design"] == "one_fe" else np.zeros(len(y), dtype=int)
        counts = np.bincount(groups)
        ymeans = np.bincount(groups, weights=y) / counts
        xmeans = np.column_stack([np.bincount(groups, weights=column) / counts for column in x.T]) if scope["k"] else np.empty((len(counts), 0))
        yw, xw = y - ymeans[groups], x - xmeans[groups]
        beta, _, rank, _ = np.linalg.lstsq(xw, yw, rcond=None)
        residual = yw - xw @ beta
        gram = xw.T @ xw
        inverse = np.linalg.inv(gram)
        meanx = x.mean(axis=0)
        intercept = y.mean() - meanx @ beta
        transform = np.vstack([np.eye(scope["k"]), -meanx])
        bread = transform @ inverse @ transform.T
        bread[-1, -1] += 1 / len(y)
        df = len(y) - len(counts) - int(rank)
        rss = float(residual @ residual)
        covariance = bread * (rss / df)
        scientific = entry["fit"]["scientific"]
        b = np.array(scientific["coefficients"])
        v = np.array(scientific["vce"]).reshape(scientific["vce_shape"])
        oracle_b = np.append(beta, intercept)
        np.testing.assert_allclose(b, oracle_b, rtol=1e-9, atol=1e-10)
        np.testing.assert_allclose(v, covariance, rtol=1e-8, atol=1e-11)
        if int(rank) != scope["k"]:
            raise ValueError("unexpected oracle rank")
        finite_score = float(np.max(np.abs(xw.T @ residual), initial=0) / len(y))
        fe_score = float(np.max(np.abs(np.bincount(groups, weights=residual) / counts)))
        inverse_error = float(np.max(np.abs(gram @ inverse - np.eye(scope["k"])), initial=0))
        reconstructed = x @ beta + (ymeans - xmeans @ beta)[groups] + residual
        reconstruction = float(np.max(np.abs(y - reconstructed)))
        if max(finite_score, fe_score, inverse_error, reconstruction) > 1e-9:
            raise ValueError("independent oracle certificate failed")
        records.append(dict(scope=scope["id"], dataset_sha256=dataset["sha256"], csv_sha256=SHA(path),
                            numpy_version=np.__version__, rank=int(rank), residual_df=df, rss=rss,
                            coefficient_max_abs=float(np.max(np.abs(b - oracle_b))),
                            covariance_max_abs=float(np.max(np.abs(v - covariance))),
                            finite_score=finite_score, fe_score=fe_score, inverse_error=inverse_error,
                            reconstruction_max_abs=reconstruction))
    if {r["scope"] for r in records} != set(args.scope or []):
        raise ValueError("oracle scope inventory is incomplete")
    write_json(args.work / args.label / "independent-oracle.json", dict(qualification_eligible=False,
               method="explicit group elimination and independent NumPy SVD least squares; analytic IID covariance",
               controller_sha256=SHA(Path(__file__)), records=records))


def main():
    global MANIFEST
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("list", "prepare", "run", "compare", "oracle"))
    parser.add_argument("--manifest", type=Path, default=MANIFEST)
    parser.add_argument("--work", type=Path)
    parser.add_argument("--reference", type=Path)
    parser.add_argument("--source", type=Path, default=ROOT)
    parser.add_argument("--baseline-source", type=Path)
    parser.add_argument("--label")
    parser.add_argument("--scope", action="append")
    parser.add_argument("--trace", action="store_true")
    args = parser.parse_args()
    MANIFEST = args.manifest.resolve()
    if args.action == "list":
        print(json.dumps(scopes(), indent=2))
    elif args.action == "prepare":
        prepare(args.work, args.reference)
    elif args.action == "compare":
        compare(args)
    elif args.action == "oracle":
        oracle(args)
    else:
        run(args)


if __name__ == "__main__":
    main()
