"""Sequential, fresh-process, same-node comparison groups with RSS sampling."""
from __future__ import annotations

import argparse
import csv
import math
import os
import signal
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

import numpy as np
import psutil

from common import HERE, arm_order, cell_id, dataset_id, read_json, sha256, thread_environment, write_json, manifest_for
from generate import generate
from worker_diagnostics import failure_evidence


def numeric(value):
    try:
        number = float(value)
        return number if math.isfinite(number) else None
    except (ValueError, TypeError):
        return value


def metadata(output):
    if (output / "metadata.json").exists():
        return read_json(output / "metadata.json")
    if (output / "metadata.tsv").exists():
        with (output / "metadata.tsv").open() as f:
            return {k: numeric(v.strip()) for k, v in csv.reader(f, delimiter="\t")}
    return {}


def sample_hash(path):
    digest = __import__("hashlib").sha256()
    with path.open() as f:
        reader = csv.DictReader(f)
        rows = 0
        for row in reader:
            digest.update(int(float(row["obs_id"])).to_bytes(8, "little"))
            rows += 1
    return digest.hexdigest(), rows


def measure(root, cell, arm, phase, data, warm, output, manifest, identity=None, limits=None):
    arm_start = time.monotonic()
    output.mkdir(parents=True, exist_ok=False)
    threads = cell["threads"]
    config = {"output": str(output), "cell": cell, "data": str(data), "warm": str(warm), "phase": phase,
              "arm": arm, "fixest_strict": manifest.get('fixest_strict')}
    write_json(output / "request.json", config)
    environment = os.environ.copy()
    # Prevent inherited user/developer overrides from changing the estimator.
    for key in list(environment):
        if key.startswith("FEREG_") or key in ("PYTHONPATH", "LD_PRELOAD"):
            del environment[key]
    environment.update(thread_environment(threads))
    temporary = data.parent / f"worker-{arm}"
    temporary.mkdir(exist_ok=False)
    environment.update({"R_LIBS_USER": str(root / "env/R"), "MPLCONFIGDIR": str(temporary / "mpl"),
                        "TMPDIR": str(temporary), "NUMBA_CACHE_DIR": str(temporary / 'numba'),
                        "PYTHONDONTWRITEBYTECODE": "1"})
    # Use a deterministic nested physical-core subset, preserving the full
    # whole-node allocation even when the application requests fewer cores.
    cpu_ids = sorted(os.sched_getaffinity(0))[:threads]
    prefix = ["taskset", "-c", ",".join(map(str, cpu_ids))]
    import_path = temporary / "import.bin"
    if arm in ("fixest", "fixest_strict"):
        command = ["Rscript", str(HERE / "r_worker.R"), str(output / "request.json")]
    elif arm == "pyfixest":
        command = [str(root / "env/python/bin/python"), str(HERE / "python_worker.py"), str(output / "request.json")]
    else:
        # Each Stata process gets its own writable pinned package closure.
        # ftools may compile Mata during initialization; never race in a shared tree.
        reference_path = root / 'references'
        if manifest.get('schema') == 'fereg-systematic-light-v2':
            reference_path = temporary / 'references'
            shutil.copytree(root / 'references', reference_path)
        absorption = ' '.join(e + ('##c.('+' '.join(cell['slopes'][e])+')' if e in cell.get('slopes', {}) else '') for e in cell['effects'])
        slope_names = ' '.join(z for values in cell.get('slopes', {}).values() for z in values)
        (output / 'surface.do').write_text(f'local absorption "{absorption}"\nlocal slope_names "{slope_names}"\nlocal memory "{cell.get("memory", "")}"\n')
        command = ["stata-mp", "-b", "do", str(HERE / "stata_worker.do"),
                   str(root / "candidate"), str(reference_path), str(data), str(warm), str(output),
                   arm, cell["model"], str(cell["k"]), str(threads), str(len(cell["effects"])), phase, str(import_path)]
    maximum_rss = 0
    samples = 0
    peaks_threads = 0
    started = None
    reason = None
    wall_start = time.monotonic()
    import_verified = False
    finished = None
    limits = limits or dict(startup=manifest['measurement']['startup_timeout_seconds'],
                            command=manifest['measurement']['command_timeout_seconds'], extraction=600)
    with (output / "stdout.txt").open("w") as stdout, (output / "stderr.txt").open("w") as stderr:
        process = subprocess.Popen(prefix + command, cwd=output, env=environment,
                                   stdout=stdout, stderr=stderr, start_new_session=True)
        try:
            while process.poll() is None:
                now = time.monotonic()
                if (output / "INPUT.READY").exists() and not import_verified:
                    info = read_json(data.with_suffix(".json"))
                    hashes = {}
                    with import_path.open("rb") as stream:
                        for column in info["columns"]:
                            digest = __import__("hashlib").sha256()
                            remaining = info["n"] * 8
                            while remaining:
                                block = stream.read(min(remaining, 2**20))
                                if not block:
                                    raise ValueError("truncated Stata import audit")
                                digest.update(block)
                                remaining -= len(block)
                            hashes[column] = digest.hexdigest()
                        if stream.read(1):
                            raise ValueError("extra Stata import audit bytes")
                    if hashes != info["column_sha256"]:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait()
                        raise ValueError("Stata imported values differ from canonical hashes")
                    import_path.unlink()
                    write_json(output / "import-hashes.json", hashes)
                    (output / "IMPORT.CHECKED").write_text("PASS\n")
                    import_verified = True
                active = (output / "START").exists() and not (output / "DONE").exists()
                if (output / 'DONE').exists() and finished is None:
                    finished = now
                if active and started is None:
                    started = now
                if active:
                    try:
                        parent = psutil.Process(process.pid)
                        tree = [parent, *parent.children(recursive=True)]
                        rss, nthreads = 0, 0
                        for child in tree:
                            try:
                                rss += child.memory_info().rss
                                nthreads += child.num_threads()
                            except (psutil.NoSuchProcess, psutil.AccessDenied):
                                pass
                        maximum_rss = max(maximum_rss, rss)
                        peaks_threads = max(peaks_threads, nthreads)
                        samples += 1
                        # A pre-invocation boundary sample also covers commands
                        # shorter than the periodic sampler interval. Handshake
                        # waiting is excluded from the command's own timer.
                        if not (output / "START.CHECKED").exists():
                            (output / "START.CHECKED").write_text("SAMPLED\n")
                    except psutil.NoSuchProcess:
                        pass
                if started is None and now - wall_start > limits['startup']:
                    reason = "startup_timeout"
                elif started is not None and active and now - started > limits['command']:
                    reason = "command_timeout"
                elif finished is not None and now - finished > limits['extraction']:
                    reason = "extraction_timeout"
                if reason:
                    os.killpg(process.pid, signal.SIGTERM)
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
                    break
                time.sleep(manifest["measurement"]["rss_interval_seconds"])
        finally:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()
    meta = metadata(output)
    sentinel = any("SYSTEMATIC_WORKER_COMPLETE" in p.read_text(errors="replace")
                   for p in [output / "stdout.txt", *output.glob("*.log")] if p.exists())
    status = reason or ("pass" if process.returncode == 0 and meta.get("rc") == 0 and sentinel else "worker_failure")
    if status == "pass" and (samples < 1 or maximum_rss <= 0 or not (output / "DONE").exists()
                             or not isinstance(meta.get("seconds"), (int, float)) or meta["seconds"] <= 0):
        status = "measurement_failure"
    receipt = {"schema": "systematic-arm-v1", "cell": cell, "arm": arm, "phase": phase,
               "status": status, "process_returncode": process.returncode, "sentinel": sentinel,
               "measurement_fingerprint": identity,
               "metadata": meta, "seconds": meta.get("seconds"), "peak_rss_gib": maximum_rss / 2**30 if samples else None,
               "rss_samples": samples, "rss_interval_seconds": manifest["measurement"]["rss_interval_seconds"],
               "input_value_audit": "columnwise IEEE-754 SHA-256",
               "peak_process_tree_threads": peaks_threads, "cpu_affinity": cpu_ids,
               "job_id": os.environ.get("JOB_ID"), "task_id": os.environ.get("SGE_TASK_ID"),
               "host": socket.gethostname(), "input_sha256": sha256(data),
               "elapsed_seconds": time.monotonic()-arm_start, "limits": limits,
               "command_time_lower_bound": limits['command'] if reason == "command_timeout" else None}
    receipt["failure"] = failure_evidence(output, meta, status)
    if (output / "sample.csv").exists():
        receipt["sample_sha256"], receipt["sample_rows"] = sample_hash(output / "sample.csv")
        (output / "sample.csv").unlink()
    write_json(output / "result.json", receipt)
    shutil.rmtree(temporary)
    return receipt


def read_coefficients(output):
    with (output / "coefficients.csv").open() as f:
        return {row["name"]: (float(row["value"]), float(row["se"])) for row in csv.DictReader(f)}


def compare(left, right, config):
    """Science precedes timing eligibility; missing evidence never implies parity."""
    a, b = read_json(left / "result.json"), read_json(right / "result.json")
    result = {"left": a["arm"], "right": b["arm"], "status": "unavailable", "checks": {}}
    if a["status"] != "pass" or b["status"] != "pass":
        return result
    checks = result["checks"]
    checks["same_input"] = a["input_sha256"] == b["input_sha256"]
    checks["same_runtime"] = a.get("measurement_fingerprint") == b.get("measurement_fingerprint")
    checks["same_node_task"] = all(a.get(k) is not None and a.get(k) == b.get(k)
                                    for k in ("host", "job_id", "task_id"))
    checks["sample_count"] = a["metadata"].get("N") == b["metadata"].get("N") and a["metadata"].get("N") is not None
    if a.get("sample_sha256") and b.get("sample_sha256"):
        checks["sample_exact"] = a["sample_sha256"] == b["sample_sha256"]
    else:
        result["sample_limitation"] = "Only retained row counts are exposed by this comparator"
    ac, bc = read_coefficients(left), read_coefficients(right)
    names = [f"x{i}" for i in range(1, a["cell"]["k"] + 1)]
    checks["coefficient_names"] = all(n in ac and n in bc for n in names)
    if checks["coefficient_names"]:
        ab, bb = np.array([ac[n][0] for n in names]), np.array([bc[n][0] for n in names])
        ase, bse = np.array([ac[n][1] for n in names]), np.array([bc[n][1] for n in names])
        checks["coefficients"] = bool(np.allclose(ab, bb, rtol=config["coefficient_rtol"], atol=config["coefficient_atol"]))
        checks["standard_errors"] = bool(np.allclose(ase, bse, rtol=config["vce_rtol"], atol=config["vce_atol"]))
        result["max_coefficient_error"] = float(np.max(np.abs(ab - bb)))
        result["max_se_error"] = float(np.max(np.abs(ase - bse)))
    checks["covariance_available"] = (left / "covariance.csv").exists() and (right / "covariance.csv").exists()
    if checks["covariance_available"]:
        def covariance(path):
            with path.open() as f:
                return {(r["row"], r["column"]): float(r["value"]) for r in csv.DictReader(f)}
        av, bv = covariance(left / "covariance.csv"), covariance(right / "covariance.csv")
        keys = [(n, m) for n in names for m in names]
        checks["covariance"] = all(x in av and x in bv for x in keys) and bool(np.allclose(
            [av.get(x, float("nan")) for x in keys], [bv.get(x, float("nan")) for x in keys],
            rtol=config["vce_rtol"], atol=config["vce_atol"]))
    if b["arm"] == "reghdfe":
        for field in ("df_a", "df_r"):
            checks[field] = isinstance(a["metadata"].get(field), (float, int)) and a["metadata"].get(field) == b["metadata"].get(field)
    checks["candidate_converged"] = a["metadata"].get("converged") == 1
    if b["arm"] in ("fixest", "fixest_strict", "ppmlhdfe") or (b["arm"] == "pyfixest" and a["cell"]["model"] == "ppml"):
        checks["comparator_converged"] = b["metadata"].get("converged") == 1
    if b["arm"] == "ppmlhdfe":
        # Accepted PPML capture contract assigns the normalized constant to
        # candidate finite rank and to comparator absorbed rank, respectively.
        # Their identified total rank and structural residual df must agree.
        am, bm = a["metadata"], b["metadata"]
        checks["total_rank"] = am.get("rank", -1) + am.get("df_a", -1) == bm.get("rank", -2) + bm.get("df_a", -2)
        checks["structural_df"] = am.get("df_resid") == bm.get("df") and am.get("df_resid") == am.get("N", 0) - am.get("rank", -1) - am.get("df_a", -1)
    for field in (["rss"] if a["cell"]["model"] == "ols" else ["deviance"]):
        avalue, bvalue = a["metadata"].get(field), b["metadata"].get(field)
        if isinstance(avalue, (int, float)) and isinstance(bvalue, (int, float)):
            checks[field] = math.isclose(avalue, bvalue, rel_tol=config["objective_rtol"], abs_tol=config["objective_atol"])
        elif b["arm"] not in ("greg", "gglm"):
            checks[field] = False
    if b["arm"] == "ppmlhdfe":
        ll_a, ll_b = a["metadata"].get("ll"), b["metadata"].get("ll")
        checks["reported_objective_no_worse"] = isinstance(ll_a, (int, float)) and isinstance(ll_b, (int, float)) and ll_a >= ll_b - config["objective_atol"] - config["objective_rtol"] * abs(ll_b)
    result["status"] = "pass" if all(checks.values()) else "accuracy_failure"
    return result


def run_group(root, group, scratch, manifest, identity=None, results_directory="results"):
    from light import enabled, active_arms, skips
    group_start = time.monotonic()
    cell, rep = group["cell"], group["repetition"]
    phase = "cold" if rep == 0 else "warm"
    seed = manifest["seeds"][max(0, rep - 1)]
    output = root / results_directory / cell_id(cell) / f"{phase}-{rep}"
    if (output / "GROUP.COMPLETE.json").exists():
        existing = read_json(output / "GROUP.COMPLETE.json")
        if existing.get("measurement_fingerprint") != identity:
            raise ValueError("cannot reuse a group from a different measurement controller")
        return existing
    if output.exists() and any(output.iterdir()):
        raise ValueError("partial comparison group requires isolated recovery; refusing cross-node pairing")
    output.mkdir(parents=True, exist_ok=True)
    data = scratch / (dataset_id(cell, seed) + ".dta")
    if not data.exists():
        generate(cell, seed, data, manifest)
    warm_cell = dict(cell, n=manifest["measurement"]["warmup_rows"], section="main")
    warm = scratch / (dataset_id(warm_cell, seed) + "-initialization.dta")
    if not warm.exists():
        generate(warm_cell, seed, warm, manifest)
    write_json(output / "input.json", read_json(data.with_suffix(".json")))
    independent = None
    if cell["n"] <= 7680:
        from oracle import oracle
        try:
            independent = oracle(data, cell)
        except Exception as exc:
            independent = dict(status='failed', error=f'{type(exc).__name__}: {exc}')
        write_json(output / "oracle.json", independent)
    arms = active_arms(group, manifest) if enabled(manifest) else group.get("arms", manifest["arms"][cell["model"]])
    receipts = []
    for arm in arm_order(arms, max(1, rep)):
        destination = output / arm
        if (destination / "result.json").exists():
            receipt = read_json(destination / "result.json")
            if receipt.get("measurement_fingerprint") != identity:
                raise ValueError("cannot reuse an arm from a different measurement controller")
        else:
            receipt = measure(root, cell, arm, phase, data, warm, destination, manifest, identity,
                              group.get('limits', {}).get(arm))
        receipts.append(receipt)
        print(f"ARM {cell_id(cell)} {phase}-{rep} {arm} {receipt['status']} {receipt['seconds']}", flush=True)
    candidate = "fereg" if cell["model"] == "ols" else "feppml"
    science = [compare(output / candidate, output / arm, manifest["science"]) for arm in arms if arm != candidate]
    oracle_checks = None
    if independent and independent['status'] != 'pass':
        oracle_checks = {'converged': False}
    elif independent and (output / candidate / "coefficients.csv").exists():
        values = read_coefficients(output / candidate)
        names = independent["coefficient_names"]
        tolerance = manifest["science"]
        oracle_checks = {
            'sample': independent['sample_sha256'] == read_json(output/candidate/'result.json').get('sample_sha256'),
            "coefficients": bool(np.allclose([values[n][0] for n in names], independent["coefficients"],
                                              rtol=tolerance["coefficient_rtol"], atol=tolerance["coefficient_atol"])),
            "standard_errors": bool(np.allclose([values[n][1] for n in names], np.sqrt(np.diag(independent["covariance"])),
                                                rtol=tolerance["vce_rtol"], atol=tolerance["vce_atol"]))}
    elif independent:
        oracle_checks = {'candidate_output': False}
    if independent and independent['status'] == 'pass' and oracle_checks:
        native = read_json(output/candidate/'result.json')
        oracle_checks['converged'] = native['status'] == 'pass' and native['metadata'].get('converged') == 1
        objective = 'rss' if cell['model'] == 'ols' else 'deviance'
        observed = native['metadata'].get(objective)
        oracle_checks['objective'] = isinstance(observed, (int, float)) and math.isclose(
            observed, independent[objective], rel_tol=manifest['science']['objective_rtol'], abs_tol=manifest['science']['objective_atol'])
        path = output/candidate/'covariance.csv'
        oracle_checks['covariance'] = False
        if path.exists():
            with path.open() as stream:
                covariance = {(r['row'],r['column']):float(r['value']) for r in csv.DictReader(stream)}
            names = independent['coefficient_names']
            observed = [[covariance.get((a,b),float('nan')) for b in names] for a in names]
            oracle_checks['covariance'] = bool(np.allclose(observed, independent['covariance'],
                rtol=manifest['science']['vce_rtol'], atol=manifest['science']['vce_atol']))
    result = {"cell": cell, "repetition": rep, "phase": phase, "seed": seed,
              "measurement_fingerprint": identity,
              "job_id": os.environ.get("JOB_ID"), "host": socket.gethostname(),
              "arm_status": {r["arm"]: r["status"] for r in receipts}, "comparisons": science,
              "oracle_checks": oracle_checks, "elapsed_seconds": time.monotonic()-group_start,
              "skips": skips(cell, rep, manifest) if enabled(manifest) else {}}
    write_json(output / "GROUP.COMPLETE.json", result)
    # Target files are regenerable and hash-bound; no accumulation in shared storage.
    data.unlink()
    data.with_suffix(".json").unlink()
    warm.unlink()
    warm.with_suffix('.json').unlink()
    return result


def run_bundle(root, plan, bundle_id):
    from light import enabled, choose_fitting
    manifest = manifest_for(root)
    if os.environ.get("NSLOTS") != "28":
        raise RuntimeError("all benchmark execution requires 28 reserved cores")
    if "E5-2680 v4" not in Path("/proc/cpuinfo").read_text():
        raise RuntimeError("unexpected CPU class")
    if len(os.sched_getaffinity(0)) != 28:
        raise RuntimeError("benchmark requires exactly 28 available CPU IDs")
    plan_data = read_json(plan)
    if enabled(manifest):
        if plan_data.get('manifest_sha256') != sha256(root/'input/manifest.json'):
            raise ValueError('Plan belongs to another manifest')
        all_groups = [g for b in plan_data['bundles'] for g in b['groups']]
        keys = [(cell_id(g['cell']), g['repetition']) for g in all_groups]
        if len(keys) != len(set(keys)):
            raise ValueError('Duplicate comparison in plan')
    results_directory = plan_data.get("results_directory", "results")
    if Path(results_directory).name != results_directory or results_directory in ("", ".", ".."):
        raise ValueError("result directory must be one named child of the run")
    identity = None
    if plan_data["mode"] in ("pilot", "production", "smoke") and (root/'env/frozen.json').exists():
        from provenance import verify
        identity = verify(root)
    setup = read_json(root / "env/setup.json")
    for name, digest in setup.items():
        if sha256(root / name) != digest:
            raise RuntimeError(f"setup identity changed: {name}")
    bundle = plan_data["bundles"][bundle_id - 1]
    scratch = Path(os.environ["TMPDIR"]) / f"systematic-{bundle_id}"
    scratch.mkdir(parents=True, exist_ok=False)
    start = time.monotonic()
    completed = []
    pending = []
    if enabled(manifest):
        pending = list(bundle['groups'])
        while pending:
            index = choose_fitting(pending, plan_data['wall_seconds']-(time.monotonic()-start))
            if index is None:break
            group = pending.pop(index)
            run_group(root, group, scratch, manifest, identity, results_directory)
            completed.append(group)
            write_json(root/'bundle-results'/f'{Path(plan).stem}-{bundle_id}.json',
                       dict(job_id=os.environ.get('JOB_ID'), task_id=bundle_id, measurement_fingerprint=identity,
                            plan_sha256=sha256(plan), completed=completed, pending=pending,
                            wall_seconds=time.monotonic()-start, status='running'))
        write_json(root/'bundle-results'/f'{Path(plan).stem}-{bundle_id}.json',
                   dict(job_id=os.environ.get('JOB_ID'),task_id=bundle_id,measurement_fingerprint=identity,
                        plan_sha256=sha256(plan),completed=completed,pending=pending,
                        wall_seconds=time.monotonic()-start,status='complete' if not pending else 'checkpointed'))
        print('SYSTEMATIC_BUNDLE_COMPLETE', flush=True)
        return
    for index, group in enumerate(bundle["groups"]):
        # Reserve enough time for every arm's command cap plus extraction before
        # starting an indivisible same-node comparison group.
        worst_case = len(group.get("arms", manifest["arms"][group["cell"]["model"]])) * 3600 + 600
        if time.monotonic() - start + worst_case > manifest["scc"]["wall_seconds"] - 600:
            pending = bundle["groups"][index:]
            break
        run_group(root, group, scratch, manifest, identity, results_directory)
        completed.append(group)
    write_json(root / "bundle-results" / f"{Path(plan).stem}-{bundle_id}.json",
               {"job_id": os.environ.get("JOB_ID"), "task_id": bundle_id,
                "measurement_fingerprint": identity, "plan_sha256": sha256(plan),
                "host": socket.gethostname(), "completed": completed, "pending": pending,
                "wall_seconds": time.monotonic() - start, "status": "complete" if not pending else "checkpointed"})
    print("SYSTEMATIC_BUNDLE_COMPLETE", flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("run_root", type=Path)
    p.add_argument("plan", type=Path)
    p.add_argument("bundle_id", type=int)
    args = p.parse_args()
    run_bundle(args.run_root, args.plan, args.bundle_id)
