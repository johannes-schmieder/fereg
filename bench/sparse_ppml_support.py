"""Benchmark capped-support HiGHS LPs against public feppml on synthetic designs.

Run from the repo with NumPy and SciPy available, passing installed Stata and
the packaged release to check. Output goes to a new local directory.

    python bench/sparse_ppml_support.py target/tmp/sparse-ppml-support \
        --package dist --stata /path/to/stata-mp --exact-candidate

Use `--arms default ir` for a Stata-only comparison. Use `--arms highs-ds
highs-ipm` for independent LP-only measurements without a package. Selecting
`--cases near_blocker:12 --power 40` exercises the numerical false-drop case,
and `--cases multidir:32 multidir:256` scales independent mixed-slope directions.

The open/closed two-FE pair differs only by a closing zero connector. Every
positive block is a distinct K2,2. Open connector margins are all -1; on the
closed loop the connector row sum equals a sum of positive rows. The coupled
families use three FE dimensions and two continuous channels. At each fixed
cell, varying x among positives forces the corresponding z slope to cancel
the x coefficient. The factorial levels then force positive contrast in all
three FE terms when the x coefficient is nonzero. With group-specific x slopes
each block supplies a separate direction. In the near families, perturbations
of opposite signs close the recession cone on affected cells.

The LP maximizes the sum of capped slacks on zero outcomes, with equality on
positives and unrestricted FE/slopes. Its numerical checks are against the
full design. Passing `--exact-candidate` also rationalizes and independently
verifies both candidate witnesses on the original design. Failure of bounded
rational reconstruction does not establish that an exact witness is absent.
Each synthetic fixture has independent analytic primal and survivor witnesses.

`solver_s` includes the SciPy/HiGHS interface and HiGHS preprocessing.
`sparse_pipeline_s` adds sparse design build and numerical candidate checks.
Exact reconstruction is optional and separately timed. Stata `wall` includes
the complete feppml command. Reported separation phase times are nested within
that command. Process wall time and peak RSS include each process's startup,
fixture generation or input, and output; report them at that scope. Failed or
timed-out arms remain in results.jsonl and their raw logs are saved in the output directory.
"""
import argparse
import csv
from fractions import Fraction
from hashlib import sha256
from itertools import product
import json
import os
from pathlib import Path
import platform
import re
import resource
import signal
import shutil
import subprocess
import sys
from time import monotonic, sleep
import warnings

import numpy as np
from scipy import __version__ as scipy_version
from scipy.optimize import OptimizeWarning, linprog
from scipy.sparse import coo_matrix, eye, hstack


COLS = ("y", "g", "h", "k", "x", "z", "retain")
DEFAULT_CASES = [
    (family, size) for size in (256, 2048, 16384) for family in ("open", "closed")
] + [
    (family, size) for size in (64, 512) for family in ("coupled", "near")
]
MULTIDIR = ("multidir", "multidir_near")
COUPLED = ("coupled", "near", "near_blocker", *MULTIDIR)


def fixture(family, size, power=22):
    """Return rows, mean on the retained sample and an analytic LP certificate."""
    rows = []
    proof_mass = []
    if family in ("open", "closed"):
        for j in range(size):
            for a, b in product((0, 1), repeat=2):
                rows.append((1 if family == "open" else 2, 2*j+a+1, 2*j+b+1, 0, 0, 0, 1))
                proof_mass.append(-1 if family == "closed" and a == b == 0 else 0)
            if j < size-1 or family == "closed":
                rows.append((0, 2*j+1, 2*((j+1) % size)+1, 0, 0, 0, family == "closed"))
                proof_mass.append(int(family == "closed"))
        # g_j + h_j = 0 on positives; g_j + h_{j+1} = -1 on open zero edges.
        g = np.repeat(np.arange(size), 2)
        direction = np.concatenate((g, -g, [0]))
        if family == "closed":
            direction[:] = 0
        mean = 1 if family == "open" else None
    else:
        epsilon = 2.0 ** -power
        for j, a, b, c in product(range(size), (0, 1), (0, 1), (0, 1)):
            g, h, k = 2*j+a+1, 2*j+b+1, 2*j+c+1
            beta = 1 + ((j + c) % 2)
            center = (j % 23) / 8
            fixed = 2*a + 3*b + 5*c
            near_group = family == "near" or (family == "multidir_near" and j % 2 == 1)
            if family in ("coupled", *MULTIDIR) and not near_group:
                shifts, masses, keep = [0, -1], [1, 0], [1, 0]
            elif near_group:
                shifts, masses, keep = [0, -epsilon, epsilon], [1, 1, 1], [1, 1, 1]
            else:
                # One opposite zero per cell. Exact dual uses a 1:1/epsilon leverage ratio.
                shifts, masses, keep = [-1, epsilon], [1, 1/epsilon], [1, 1]
            y = (5 if near_group else 3) if family in MULTIDIR else 2
            for x in (center-1, center+1):
                rows.append((y, g, h, k, x, -(fixed+x)/beta, 1))
                proof_mass.append(-sum(masses)/2)
            for shift, mass, retain in zip(shifts, masses, keep):
                rows.append((0, g, h, k, center, -(fixed+center)/beta + shift/beta, retain))
                proof_mass.append(mass)
        levels = np.arange(2*size)
        direction = np.concatenate((
            2*(levels % 2), 3*(levels % 2), 5*(levels % 2),
            1+((levels//2 + levels % 2) % 2), [1, 0],
        ))
        if family in MULTIDIR:
            allowed = (1-(levels//2) % 2) if family == "multidir_near" else np.ones(len(levels))
            direction = np.r_[direction[:-2] * np.tile(allowed, 4), allowed, 0]
        elif family != "coupled":
            direction[:] = 0
        mean = {"coupled": 4/3, "near": 4/5, "near_blocker": None,
                "multidir": 2, "multidir_near": 2}[family]
    rows = np.asarray(rows, dtype=np.float64)
    perm = np.random.default_rng(52431).permutation(len(rows))
    rows = rows[perm]
    mass = np.asarray(proof_mass)[perm]
    assert len(np.unique(rows[:, 1:6], axis=0)) == len(rows)
    return rows, mean, direction, mass


def sparse_design(rows, family, size):
    n = len(rows)
    levels = 2 * size
    fixed = rows[:, 1:4].astype(int)-1
    one = np.ones(n)
    if family in COUPLED:
        slope_x = fixed[:, 0] if family in MULTIDIR else np.zeros(n, dtype=int)
        p = (5*levels+1) if family in MULTIDIR else (4*levels+2)
        columns = [fixed[:, 0], levels+fixed[:, 1], 2*levels+fixed[:, 2],
                   3*levels+fixed[:, 2], 4*levels+slope_x, np.full(n, p-1)]
        values = [one, one, one, rows[:, 5], rows[:, 4], one]
        degree = [np.bincount(fixed[rows[:, 6] == 1, i], minlength=levels).min() for i in range(3)]
    else:
        columns = [fixed[:, 0], levels+fixed[:, 1], np.full(n, 2*levels)]
        values = [one, one, one]
        p = 2*levels+1
        degree = [np.bincount(fixed[rows[:, 6] == 1, i], minlength=levels).min() for i in range(2)]
    assert min(degree) >= 2
    x = coo_matrix((np.concatenate(values),
                    (np.tile(np.arange(n), len(columns)), np.concatenate(columns))),
                   shape=(n, p)).tocsr()
    x.eliminate_zeros()
    return x


def verify_analytic_oracle(rows, x, direction, mass):
    zero = rows[:, 0] == 0
    expected_keep = rows[:, 6].astype(bool)
    # Prove the geometric answer independently of the solver.
    # Inputs and witnesses are dyadic and have modest integer numerators. The
    # equalities are exact in binary64 at the scheduled scales, including mass sums.
    exact_margin = x @ direction
    assert np.max(np.abs(exact_margin[expected_keep])) == 0
    assert np.all(exact_margin[~expected_keep] == -1)
    assert np.all(mass[zero & expected_keep] >= 1)
    assert np.all(mass[~expected_keep] == 0)
    assert np.max(np.abs(x.T @ mass)) == 0


def exact_dot_checks(matrix, vector, accept, denominator=4096):
    """Check a rational reconstruction of a candidate on original binary64 input."""
    witness = [Fraction(float(value)).limit_denominator(denominator) for value in vector]
    for i in range(matrix.shape[0]):
        lo, hi = matrix.indptr[i:i+2]
        evaluate = sum(
            Fraction(float(value)) * witness[col]
            for col, value in zip(matrix.indices[lo:hi], matrix.data[lo:hi])
        )
        if not accept(i, evaluate):
            return False
    return True


def exact_solver_witness(x, beta, dual, zero, drop):
    """Validate that rational reconstructions certify both sides of the support."""
    def admissible(i, value):
        return value < 0 if drop[i] else value == 0

    primal = exact_dot_checks(x, beta, admissible)
    rational_dual = [Fraction(float(value)).limit_denominator(4096) for value in dual]
    dual_signs = all((rational_dual[i] == 0) if drop[i] else (rational_dual[i] > 0)
                     for i in np.flatnonzero(zero))
    dual_checked = dual_signs and exact_dot_checks(x.T.tocsr(), dual, lambda _, value: value == 0)
    return primal, dual_checked


def solve(case, family, size, method, power, limit, exact):
    rows, mean, direction, mass = fixture(family, size, power)
    zero = rows[:, 0] == 0
    expected_keep = rows[:, 6].astype(bool)
    start = monotonic()
    x = sparse_design(rows, family, size)
    p = x.shape[1]
    nz = zero.sum()
    ap = hstack((x[~zero], coo_matrix(((~zero).sum(), nz))), format="csc")
    az = hstack((x[zero], eye(nz, format="csc")), format="csc")
    with warnings.catch_warnings():
        # SciPy passes the HiGHS threads option through unchanged.
        warnings.filterwarnings("ignore", category=OptimizeWarning, message="Unrecognized options detected")
        solve_start = monotonic()
        lp = linprog(np.r_[np.zeros(p), -np.ones(nz)], A_ub=az, b_ub=np.zeros(nz),
                     A_eq=ap, b_eq=np.zeros((~zero).sum()), bounds=[(None, None)]*p + [(0, 1)]*nz,
                     method=method, options={"threads": 1, "time_limit": limit,
                                             "primal_feasibility_tolerance": 1e-9,
                                             "dual_feasibility_tolerance": 1e-9})
    finished = monotonic()
    out = {"family": family, "size": size, "method": method, "rows": len(rows),
           "columns": p, "design_nnz": x.nnz, "zero_rows": int(nz),
           "expected_drops": int((~expected_keep).sum()), "rows_saved": int(expected_keep.sum()),
           "scipy": scipy_version, "numpy": np.__version__, "power": power,
           "build_s": solve_start-start, "solver_s": finished-solve_start,
           "iterations": lp.nit, "crossover_iterations": lp.get("crossover_nit"),
           "status": lp.status, "message": lp.message,
           "python_peak_rss": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss}
    verify_analytic_oracle(rows, x, direction, mass)
    out["analytic_full_support_verified"] = True
    out["retained_mean"] = mean
    if lp.success:
        cert_start = monotonic()
        proxy = lp.x[p:] > .5
        predicted = np.zeros(len(rows), dtype=bool)
        predicted[zero] = proxy
        pred = x @ lp.x[:p]
        dual = np.empty(len(rows))
        dual[zero] = -lp.ineqlin.marginals
        dual[~zero] = -lp.eqlin.marginals
        survivor_dual = dual[zero][~proxy]
        wrong = predicted != ~expected_keep
        out.update({"row_mismatches": int(wrong.sum()), "dropped": int(proxy.sum()),
                    "lp_objective": -lp.fun, "lp_min_accepted_slack": float(lp.x[p:][proxy].min(initial=1)),
                    "lp_max_rejected_slack": float(lp.x[p:][~proxy].max(initial=0)),
                    "primal_positive_error": float(np.max(np.abs(pred[~zero]))),
                    "primal_zero_violation": float(np.max(pred[zero], initial=0)),
                    "primal_strict_margin": float(np.min(-pred[predicted], initial=1)),
                    "dual_balance_error": float(np.max(np.abs(x.T @ dual))),
                    "dual_min_zero_multiplier": float(np.min(dual[zero], initial=0)),
                    "dual_max_drop_multiplier": float(np.max(np.abs(dual[predicted]), initial=0)),
                    "dual_min_retained_multiplier": float(survivor_dual.min(initial=1)),
                    "candidate_check_s": monotonic()-cert_start})
        out["sparse_pipeline_s"] = out["build_s"] + out["solver_s"] + out["candidate_check_s"]
        if exact:
            cert_start = monotonic()
            primal, dual = exact_solver_witness(x, lp.x[:p], dual, zero, predicted)
            out["exact_reconstructed_primal"] = primal
            out["exact_reconstructed_dual"] = dual
            out["exact_candidate_check_s"] = monotonic()-cert_start
    (case / "lp-result.json").write_text(json.dumps(out, indent=2)+"\n")


META = ("time_separation_fe", "time_separation_simplex", "time_separation_ir",
        "total_seconds", "allocation_peak_delta")
METHODS = ("singleton", "fe", "simplex", "ir", "mu")
TRACE_FIELDS = ("executions", "status_mask", "total_iterations", "direct_drops", "singleton_drops")
PROGRESS = "BENCH_STARTED"
DONE = "BENCH_DONE"


def stata_driver(case, package, plugin_name, family, arm):
    options = "separation(ir)" if arm == "ir" else ""
    effects = ("g##c.x h k##c.z" if family in MULTIDIR
               else "g h k##c.z" if family in COUPLED else "g h")
    x = "x" if family in COUPLED and family not in MULTIDIR else ""
    paths = "\n".join(f'sysdir set {name.upper()} "{case/name}"' for name in ("personal", "plus", "site", "oldplace"))
    runtime = "\n".join(
        f'display "RUNTIME|{m}|" %22.15g B[rownumb(B,"{m}"),1]' for m in META)
    trace = "\n".join(
        f'display "TRACE|{m}_{f}|" %22.15g B[rownumb(B,"{m}"),colnumb(B,"{f}")]'
        for m, f in product(METHODS, TRACE_FIELDS))
    return f"""clear all
set more off
set linesize 255
set processors 1
{paths}
adopath ++ "{package}"
fereg, selftest verbose
assert "`r(plugin_file)'" == "{package}/{plugin_name}"
quietly import delimited using "{case.parent/'data.csv'}", varnames(1) asdouble clear
display "{PROGRESS}"
timer clear 95
timer on 95
capture noisily feppml y {x}, absorb({effects}) threads(1) {options}
local code = _rc
timer off 95
quietly timer list 95
display "WALL|" %22.15g r(t95)
display "CODE|" `code'
if !`code' {{
    quietly count if retain != e(sample)
    display "MISMATCHES|" r(N)
    display "N|" e(N)
    display "DROPS|" e(separated)
    display "SINGLETONS|" e(num_singletons)
    display "CONVERGED|" e(converged)
    matrix B = e(feppml_runtime)
    {runtime}
    matrix B = e(feppml_separation_sequence)
    {trace}
}}
display "{DONE}"
"""


def child_run(command, cwd, env, cap):
    launch = monotonic()
    with (cwd / "console.log").open("w") as console:
        p = subprocess.Popen(command, cwd=cwd, env=env, stdout=console,
                             stderr=subprocess.STDOUT, start_new_session=True)
        hit_cap = False
        while True:
            pid, status, use = os.wait4(p.pid, os.WNOHANG)
            if pid:
                p.returncode = os.waitstatus_to_exitcode(status)
                break
            if monotonic() - launch >= cap:
                hit_cap = True
                try:
                    os.killpg(p.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                _, status, use = os.wait4(p.pid, 0)
                p.returncode = os.waitstatus_to_exitcode(status)
                break
            # No separate memory sampler. wait4 provides OS high-water RSS.
            sleep(.02)
    return {"elapsed_process_s": monotonic()-launch, "timeout_s": cap,
            "timeout": hit_cap, "process_rc": p.returncode, "process_peak_rss": use.ru_maxrss}


def run(args):
    root = args.output.resolve()
    if any(arm in ("ir", "default") for arm in args.arms) and (args.stata is None or args.package is None):
        raise ValueError("Stata arms require --stata and --package")
    package = args.package.resolve() if args.package else None
    env = {k:v for k,v in os.environ.items() if not k.startswith("FEREG_")}
    for term in ("OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS",
                 "VECLIB_MAXIMUM_THREADS", "NUMEXPR_NUM_THREADS"):
        env[term] = "1"
    env["FEREG_PHASE_TIMING"] = "1"
    if args.trace:
        env["FEREG_SOLVER_TRACE"] = "1"
    docket = DEFAULT_CASES if not args.cases else [
        (name, int(size)) for name, size in (case.split(":") for case in args.cases)]
    system = {"darwin": "macos", "linux": "linux"}.get(sys.platform)
    if system is None:
        raise ValueError("The Unix benchmark runner supports macOS and Linux")
    plugin_name = f"fereg_{system}.plugin"
    plan = {"machine": platform.platform(), "python": sys.executable, "stata": str(args.stata),
            "plugin_sha256": sha256((package / plugin_name).read_bytes()).hexdigest() if package else None,
            "cases": docket, "arms": args.arms, "lp_timeout_s": args.lp_timeout,
            "stata_timeout_s": args.stata_timeout, "power": args.power,
            "rss_units": "bytes" if sys.platform == "darwin" else "KiB", "threads": 1,
            "exact_solver_witness_check": args.exact_candidate, "stata_trace": args.trace}
    root.mkdir()
    (root / "plan.json").write_text(json.dumps(plan, indent=2)+"\n")
    combined = root / "results.jsonl"
    for index, (family, size) in enumerate(docket):
        case = root / f"{family}-{size}"
        case.mkdir()
        data, _, direction, mass = fixture(family, size, args.power)
        verify_analytic_oracle(data, sparse_design(data, family, size), direction, mass)
        with (case / "data.csv").open("w") as file:
            csv_out = csv.writer(file)
            csv_out.writerow(COLS)
            csv_out.writerows(data)
        for arm in args.arms[::1 if index % 2 == 0 else -1]:
            arm_path = case / arm
            arm_path.mkdir()
            print(f"RUN {family} {size} {arm}", flush=True)
            if arm.startswith("highs"):
                detail = child_run([sys.executable, str(Path(__file__).resolve()), "solve", str(arm_path),
                                    family, str(size), arm, str(args.power), str(args.lp_timeout-2),
                                    str(int(args.exact_candidate))],
                                   arm_path, env, args.lp_timeout)
                result_file = arm_path / "lp-result.json"
                if result_file.is_file():
                    detail.update(json.loads(result_file.read_text()))
            else:
                for name in ("personal", "plus", "site", "oldplace"):
                    (arm_path / name).mkdir()
                (arm_path / "launch.do").write_text(stata_driver(arm_path, package, plugin_name, family, arm))
                detail = child_run([str(args.stata), "-q", "-b", "do", "launch.do"],
                                   arm_path, env, args.stata_timeout)
                log = arm_path / "launch.log"
                if log.is_file():
                    contents = log.read_text(errors="replace")
                    detail["stata_errors"] = re.findall(r"^fereg: .*", contents, re.M)
                    detail.update({"stata_started": "\n"+PROGRESS+"\n" in contents,
                                   "stata_completed": "\n"+DONE+"\n" in contents})
                    for line in re.findall(r"^(?:WALL|CODE|MISMATCHES|N|DROPS|SINGLETONS|CONVERGED|RUNTIME|TRACE)\|[^\n]+", contents, re.M):
                        parts = line.split("|")
                        key = "_".join(parts[:-1]).lower()
                        value = parts[-1].strip()
                        detail[key] = None if value == "." else float(value)
            detail.update({"arm": arm, "family": family, "size": size, "rows": len(data),
                           "expected_drops": int((data[:, 6] == 0).sum()),
                           "analytic_full_support_verified": True})
            with combined.open("a") as out:
                out.write(json.dumps(detail)+"\n")
            print(json.dumps({k:detail.get(k) for k in ("status", "code", "timeout", "row_mismatches",
                                                        "mismatches", "solver_s", "wall")}), flush=True)


def main():
    os.umask(0o077)
    if len(sys.argv) > 1 and sys.argv[1] == "solve":
        _, case, family, size, method, power, limit, exact = sys.argv[1:]
        solve(Path(case), family, int(size), method, int(power), float(limit), bool(int(exact)))
        return
    ap = argparse.ArgumentParser()
    ap.add_argument("output", type=Path)
    ap.add_argument("--package", type=Path)
    ap.add_argument("--stata", default=shutil.which("stata-mp"))
    ap.add_argument("--cases", nargs="+")
    ap.add_argument("--arms", nargs="+", choices=("default", "ir", "highs-ds", "highs-ipm"),
                    default=("default", "highs-ds", "highs-ipm", "ir"))
    ap.add_argument("--lp-timeout", type=float, default=25)
    ap.add_argument("--stata-timeout", type=float, default=20)
    ap.add_argument("--power", type=int, choices=range(1, 41), default=22, metavar="1–40")
    ap.add_argument("--trace", action="store_true")
    ap.add_argument("--exact-candidate", action="store_true")
    args = ap.parse_args()
    if args.lp_timeout <= 2 or args.stata_timeout <= 0:
        ap.error("--lp-timeout must exceed 2 seconds and --stata-timeout must be positive")
    run(args)


if __name__ == "__main__":
    main()
