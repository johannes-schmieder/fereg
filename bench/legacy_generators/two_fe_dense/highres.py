"""Prepare a separate Mac single-command timing diagnostic with two clocks.

This adds a calibrated monotonic bracket to the existing public-command timer.
The estimator binary and timed worker body are unchanged. The old generated
worker and its manifest are retained, and the intentional instrumentation has
its own source/binary identity. It does not redefine ordinary benchmark scores.
"""
import argparse
import difflib
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

import run


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def prepare(root, artifacts, variants=('baseline', 'usefulpilot'), data_root=None):
    assert not root.exists(), "use a fresh diagnostic directory"
    assert variants and len(variants) == len(set(variants))
    for variant in variants:
        assert (artifacts / f'identity-{variant}.json').is_file()
    data = (artifacts if data_root is None else data_root) / 'data'
    assert data.is_dir()
    root.mkdir(parents=True)
    (root / "data").symlink_to(data, target_is_directory=True)
    folder = run.stage(root)
    tooling = root / "clock"
    tooling.mkdir()
    repo = Path(__file__).resolve().parents[2]
    source = Path(__file__).with_name("highres_clock.c")
    plugin = tooling / "clock.plugin"
    command = ["/Library/Developer/CommandLineTools/usr/bin/clang", "-std=c11", "-O3",
               "-fPIC", "-Wall", "-Wextra", "-Wpedantic", "-Wconversion", "-Wshadow",
               "-Wstrict-prototypes", "-Wmissing-prototypes", "-Werror", "-DSYSTEM=APPLEMAC",
               "-isysroot", "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk",
               "-target", "arm64-apple-macos11", "-mmacosx-version-min=11.0", "-bundle",
               "-I" + str(repo / "plugin"), str(repo / "plugin/stplugin.c"), str(source),
               "-Wl,-exported_symbols_list," + str(repo / "plugin/exports_macos.txt"),
               "-o", str(plugin)]
    with (tooling / "build.txt").open("w") as log:
        subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True)
        subprocess.run(["/usr/bin/codesign", "--force", "--sign", "-", str(plugin)],
                       stdout=log, stderr=subprocess.STDOUT, check=True)
    worker = folder / "stata_worker.do"
    before = worker.read_text()
    shutil.copy(worker, tooling / "uninstrumented-stata-worker.do")
    shutil.copy(folder / "source-manifest.json", tooling / "uninstrumented-manifest.json")
    setup = '''local highres_plugin : environment TWO_FE_HIGHRES_CLOCK
if "`highres_plugin'" == "" exit 198
program __twofe_clock, plugin using(`"`highres_plugin'"')
tempname highres_elapsed
scalar `highres_elapsed' = .
local clock_empty_min = .
local clock_empty_max = 0
local clock_empty_sum = 0
forvalues clock_j = 1/128 {
    plugin call __twofe_clock, begin
    plugin call __twofe_clock, end `highres_elapsed'
    local clock_empty_min = min(`clock_empty_min', scalar(`highres_elapsed'))
    local clock_empty_max = max(`clock_empty_max', scalar(`highres_elapsed'))
    local clock_empty_sum = `clock_empty_sum' + scalar(`highres_elapsed')
}
local clock_empty_mean = `clock_empty_sum' / 128
assert `clock_empty_min' >= 0 & `clock_empty_max' < .01
'''
    text = run.breadth.replace_once(before, 'file open `mark\' using `"`output\'/READY"\', write text replace',
                                   setup + 'file open `mark\' using `"`output\'/READY"\', write text replace')
    text = run.breadth.replace_once(text, 'timer clear 92\ntimer on 92',
                                   'timer clear 92\nplugin call __twofe_clock, begin\ntimer on 92')
    text = run.breadth.replace_once(text, 'timer off 92\nquietly timer list 92',
                                   "timer off 92\nplugin call __twofe_clock, end `highres_elapsed'\nquietly timer list 92")
    metadata = '''file write `meta' "highres_seconds" _tab %24.17g (scalar(`highres_elapsed')) _n
file write `meta' "clock_empty_min" _tab %24.17g (`clock_empty_min') _n
file write `meta' "clock_empty_max" _tab %24.17g (`clock_empty_max') _n
file write `meta' "clock_empty_mean" _tab %24.17g (`clock_empty_mean') _n
'''
    text = run.breadth.replace_once(text, 'file write `meta\' "fit_seconds"',
                                   metadata + 'file write `meta\' "fit_seconds"')
    worker.write_text(text)
    (tooling / "worker.patch").write_text(''.join(difflib.unified_diff(
        before.splitlines(True), text.splitlines(True), fromfile="original", tofile="two-clock")))
    # Manifest hashes bind the intentionally instrumented worker, not the old
    # generation recipe alone. Preserve both versions and the explicit patch.
    run.write(folder / "source-manifest.json", run.controller_manifest(folder))
    run.write(folder / "clock-identity.json", dict(
        schema="two-fe-single-command-clock-v1", clock="CLOCK_MONOTONIC_RAW",
        sources={str(p.relative_to(repo)): sha(p) for p in [Path(__file__).resolve(), source,
                 repo / "plugin/stplugin.c", repo / "plugin/stplugin.h", repo / "plugin/exports_macos.txt"]},
        plugin_sha256=sha(plugin), worker_patch_sha256=sha(tooling / "worker.patch"),
        original_worker_sha256=sha(tooling / "uninstrumented-stata-worker.do"),
        original_manifest_sha256=sha(tooling / "uninstrumented-manifest.json"),
        empty_brackets_per_worker=128, elapsed_field="highres_seconds",
        subtract_overhead=False, ordinary_timer_unchanged=True, command_repetitions=1))
    for name in [f'identity-{variant}.json' for variant in variants]:
        shutil.copy(artifacts / name, root / name)
    provenance = artifacts / 'baseline-provenance.json'
    if provenance.is_file():
        shutil.copy(provenance, root / provenance.name)
    run.validate_controller(folder)
    print("HIGHRES_PREPARED", root, flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument('--variants', nargs='+', default=['baseline', 'usefulpilot'])
    parser.add_argument('--data-root', type=Path,
                        help='root containing the captured data directory, when separate from artifacts')
    args = parser.parse_args()
    prepare(args.root.resolve(), args.artifacts.resolve(), args.variants,
            args.data_root.resolve() if args.data_root else None)
