#!/usr/bin/env python3
"""Licensed PyStata acceptance for sequential host-thread changes.

Use a complete flat package, including the three PPML .mo objects. Each mode
runs in a fresh process. Calls are strictly serialized by Future.result();
this does not claim that concurrent PyStata/Stata calls are supported. Native
overlap/reentry is exercised without live Stata by invocation_guard_test.c.
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import threading

MODES = ("main", "onethread", "newthread", "loadelsewhere", "recovery_main", "recovery")
SINGLE_THREAD_MODES = ("main", "onethread", "recovery_main")
PLUGINS = {"win32": "fereg_windows.plugin", "darwin": "fereg_macos.plugin",
           "linux": "fereg_linux.plugin"}
MATA_OBJECTS = ("_fereg_ppml_validation_error.mo", "_fereg_ppml_validate_result.mo",
                "_fereg_ppml_post_scalars.mo")
COMMANDS = {
    "selftest": "fereg, selftest threads(2)",
    "fereg": "fereg price weight length, absorb(foreign) vce(robust) threads(2)",
    "feppml": "feppml rep78 weight length, absorb(foreign) vce(robust) threads(2)",
    "regress": "regress price weight length i.foreign",
    "native_error": ("_fereg_load, action(loadonly)\n"
                     "local plugin_file `\"`r(plugin_file)'\"'\n"
                     "tempname native_handle\n"
                     "program `native_handle', plugin using(`\"`plugin_file'\"')\n"
                     "plugin call `native_handle', unknown_thread_test_action"),
    "ppml_panic": ("_fereg_estimate_ppml rep78 weight length, absorb(foreign) "
                   "vce(robust) threads(2) inject(panic)"),
}


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def child(args: argparse.Namespace) -> None:
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    phase = output.with_suffix(".phase")
    phase.write_text("initializing\n")
    sys.path.insert(0, args.utilities)
    from pystata import config
    config.init(args.edition, splash=False)
    from pystata import stata
    from sfi import Macro, Matrix, Scalar

    package = Path(args.package).resolve()
    stata.run(f'clear all\nset more off\nadopath ++ "{package.as_posix()}"\n'
              'sysuse auto, clear', quietly=True, echo=False)
    # Fail before testing if a different installation shadows any entry point
    # or a missing PPML object would silently resolve from the user's adopath.
    for name in ("fereg.ado", "feppml.ado", PLUGINS[sys.platform], *MATA_OBJECTS):
        stata.run(f'''quietly findfile {name}
global FEREG_PYSTATA_FILE "`r(fn)'"''', quietly=True, echo=False)
        actual = Path(Macro.getGlobal("FEREG_PYSTATA_FILE")).resolve()
        if actual != package / name:
            raise RuntimeError(f"staged package is shadowed: {name}")
    results: list[dict] = []
    witnesses: dict[str, str] = {}

    def witness() -> str:
        state = {"cmd": Macro.getGlobal("e(cmd)"), "N": Scalar.getValue("e(N)"),
                 "b": Matrix.get("e(b)"), "V": Matrix.get("e(V)")}
        return hashlib.sha256(json.dumps(state, sort_keys=True,
                                        allow_nan=False).encode()).hexdigest()

    def call(label: str, command: str, expected: int = 0) -> None:
        phase.write_text(f"{label}:{command}\n")
        before = witness() if expected else None
        stata.run("capture quietly {\n" + COMMANDS[command]
                  + "\n}\nglobal FEREG_PYSTATA_RC = _rc", quietly=True, echo=False)
        rc = int(Macro.getGlobal("FEREG_PYSTATA_RC"))
        record = {"label": label, "command": command, "rc": rc,
                  "expected": expected, "thread": threading.get_ident()}
        results.append(record)
        output.write_text(json.dumps({"mode": args.mode, "checks": results}, indent=2))
        if rc != expected:
            raise RuntimeError(f"{label}:{command} returned {rc}, expected {expected}")
        if expected:
            if witness() != before:
                raise RuntimeError(f"{command} changed prior estimates")
            record["prior_estimates_preserved"] = True
        elif command in ("fereg", "feppml", "regress"):
            actual = witness()
            if witnesses.setdefault(command, actual) != actual:
                raise RuntimeError(f"{command} changed e(b), e(V), e(N), or e(cmd)")
            record["science_sha256"] = actual

    def worker(label: str, command: str, expected: int = 0) -> None:
        with ThreadPoolExecutor(max_workers=1) as pool:
            pool.submit(call, label, command, expected).result()

    if args.mode == "main":
        call("first", "selftest")
        for index in range(3):
            call(str(index), "fereg")
        call("poisson", "feppml")
        call("control", "regress")
    elif args.mode == "onethread":
        with ThreadPoolExecutor(max_workers=1) as pool:
            pool.submit(call, "first", "selftest").result()
            for index in range(3):
                pool.submit(call, str(index), "fereg").result()
            pool.submit(call, "poisson", "feppml").result()
            pool.submit(call, "control", "regress").result()
    elif args.mode == "newthread":
        call("first", "selftest")
        call("baseline", "fereg")
        call("baseline_poisson", "feppml")
        for index in range(3):
            worker(str(index), "fereg")
        worker("poisson", "feppml")
        worker("selftest_other", "selftest")
        worker("control", "regress")
        call("return", "fereg")
        call("return_poisson", "feppml")
        call("return_selftest", "selftest")
    elif args.mode == "loadelsewhere":
        # Keep the first caller alive until every main-thread call completed,
        # then explicitly return to it; thread-ID reuse cannot mask a failure.
        with ThreadPoolExecutor(max_workers=1) as pool:
            pool.submit(call, "first", "selftest").result()
            pool.submit(call, "baseline", "fereg").result()
            pool.submit(call, "baseline_poisson", "feppml").result()
            for index in range(3):
                call(str(index), "fereg")
            call("poisson", "feppml")
            call("selftest_other", "selftest")
            call("control", "regress")
            pool.submit(call, "return", "fereg").result()
            pool.submit(call, "return_poisson", "feppml").result()
            pool.submit(call, "return_selftest", "selftest").result()
    elif args.mode in ("recovery_main", "recovery"):
        invoke = call if args.mode == "recovery_main" else worker
        call("first", "selftest")
        call("baseline", "fereg")
        invoke("invalid_native_action", "native_error", 198)
        call("recover_from_error", "fereg")
        invoke("caught_rust_panic", "ppml_panic", 3499)
        call("recover_from_panic", "feppml")
        call("invalid_native_action_main", "native_error", 198)
        invoke("recover_on_caller", "fereg")
        call("return_selftest", "selftest")
    threads = {record["thread"] for record in results}
    if len(threads) != (1 if args.mode in SINGLE_THREAD_MODES else 2):
        # Short-lived workers can receive different IDs, so permit more than
        # two for these modes, but always require an observed caller change.
        if args.mode in SINGLE_THREAD_MODES or len(threads) < 2:
            raise RuntimeError("the intended caller-thread pattern was not exercised")
    output.write_text(json.dumps({"mode": args.mode, "checks": results,
                                  "science": witnesses, "passed": True}, indent=2) + "\n")
    phase.write_text("PASS\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--utilities", required=True)
    parser.add_argument("--package", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--edition", choices=("mp", "se", "be"), default="mp")
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--mode", choices=MODES, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    if sys.platform not in PLUGINS:
        parser.error("unsupported platform")
    package = Path(args.package).resolve()
    for name in ("fereg.ado", "feppml.ado", PLUGINS[sys.platform], *MATA_OBJECTS):
        if not (package / name).is_file():
            parser.error(f"incomplete flat package: missing {name}")
    if args.mode:
        child(args)
        return 0
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    status = output / "pystata.status"
    (output / "summary.json").unlink(missing_ok=True)
    status.write_text("PYSTATA_THREADS=RUNNING\n")
    results = []
    for mode in MODES:
        target = output / f"{mode}.json"
        target.unlink(missing_ok=True)
        target.with_suffix(".phase").unlink(missing_ok=True)
        command = [sys.executable, str(Path(__file__).resolve()), "--utilities", args.utilities,
                   "--package", str(package), "--output", str(target), "--edition",
                   args.edition, "--mode", mode]
        # Startup and exception output can contain license details: keep it
        # local and report only the mode/status to the caller.
        with (output / f"{mode}.stdout.log").open("wb") as stdout, \
                (output / f"{mode}.stderr.log").open("wb") as stderr:
            try:
                process = subprocess.run(command, stdout=stdout, stderr=stderr,
                                         timeout=args.timeout, check=False)
                passed = (process.returncode == 0 and target.is_file()
                          and json.loads(target.read_text()).get("passed") is True)
            except subprocess.TimeoutExpired:
                passed = False
        if not passed:
            status.write_text(f"PYSTATA_THREADS=FAIL mode={mode}\n")
            print(f"PYSTATA_THREADS=FAIL mode={mode}; inspect local diagnostic files")
            return 1
        results.append(json.loads(target.read_text()))
        print(f"PYSTATA_THREADS_MODE={mode}:PASS", flush=True)
    summary = {"schema": "fereg-pystata-threads-v1", "plugin_sha256":
               digest(package / PLUGINS[sys.platform]), "modes": results}
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    status.write_text("PYSTATA_THREADS=PASS\n")
    print("PYSTATA_THREADS=PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
