"""Prepare one immutable rotation attempt and launch its frozen controller."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys


def prepare(args, *, check_only=False):
    if not re.fullmatch(r"rotation[0-9]{2,}", args.attempt):
        raise ValueError("attempt must be rotation followed by at least two digits")
    root = args.run_root.resolve()
    prefix = root / args.attempt
    roots = prefix.with_name(prefix.name + "-roots")
    plan_path = prefix.with_name(prefix.name + "-plan.json")
    output = prefix.with_name(prefix.name + "-results")
    for path in (roots, plan_path, output):
        if path.exists() or path.is_symlink():
            raise FileExistsError(f"attempt is not fresh: {path}")
    registration = prefix.with_name(prefix.name + "-registration.json")
    plan = json.loads(registration.read_text())
    if not json.loads((root / "diagnostic02/complete.json").read_text())["all_fits_passed"]:
        raise ValueError("candidate diagnostic failures need adjudication")
    sources = {"baseline": args.baseline, "candidate": args.candidate}
    manifests = {"baseline": root / "source-manifest.json", "candidate": root / "candidate01-source.json"}
    for name, source in sources.items():
        manifest = json.loads(manifests[name].read_text())
        for filename, digest in manifest["files"].items():
            if hashlib.sha256((source / filename).read_bytes()).hexdigest() != digest:
                raise ValueError(f"{name} source changed: {filename}")
        plugin = source / "dist/fereg_linux.plugin"
        if hashlib.sha256(plugin.read_bytes()).hexdigest() != plan["plugins"][name]:
            raise ValueError(f"{name} plugin differs from registration")
    controller = prefix.with_name(prefix.name + "-harness") / "performance/rotations.py"
    required = [controller, args.reference / "env/frozen.json", args.reference / "input/manifest.json"]
    for path in required:
        if not path.is_file():
            raise FileNotFoundError(path)
    if not (args.reference / "references").is_dir():
        raise FileNotFoundError(args.reference / "references")
    command = [sys.executable, str(controller), "--roots", str(roots),
               "--reference", str(args.reference), "--manifest", str(required[-1]),
               "--plan", str(plan_path), "--output", str(output), "--scratch", str(args.scratch)]
    if check_only:
        return command
    # All input and freshness checks precede creation. Partial attempts are never
    # reused automatically; the controller independently rechecks identities.
    with plan_path.open("x") as stream:
        json.dump(plan, stream, indent=2)
        stream.write("\n")
    roots.mkdir()
    for name in ("baseline", "candidate", "stock"):
        variant = roots / name
        variant.mkdir()
        for directory in ("env", "references"):
            (variant / directory).symlink_to(args.reference / directory, target_is_directory=True)
        native = "candidate" if name == "candidate" else "baseline"
        (variant / "candidate").symlink_to(sources[native], target_is_directory=True)
        shutil.copyfile(manifests[native], variant / "source.json")
    args.scratch.mkdir(parents=True, exist_ok=False)
    return command


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    for name in ("run-root", "baseline", "candidate", "reference", "scratch"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--attempt", required=True)
    parser.add_argument("--check-only", action="store_true")
    args = parser.parse_args()
    command = prepare(args, check_only=args.check_only)
    print(json.dumps({"checked_command": command, "check_only": args.check_only}), flush=True)
    if not args.check_only:
        subprocess.run(command, check=True)
