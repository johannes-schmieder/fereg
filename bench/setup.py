#!/usr/bin/env python3
"""Stage exact comparator sources in a private run root, outside timed commands."""
from __future__ import annotations
import argparse
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tomllib
import urllib.error
import urllib.request
from generate import sha256

ROOT = Path(__file__).resolve().parents[1]


def download(reference, destination):
    if destination.exists() and sha256(destination) == reference["source_archive_sha256"]:
        return
    errors = []
    for url in [reference["source_archive_url"], reference.get("source_archive_fallback_url")]:
        if not url:
            continue
        try:
            with urllib.request.urlopen(url, timeout=120) as response, destination.open("wb") as output:
                shutil.copyfileobj(response, output)
            if sha256(destination) != reference["source_archive_sha256"]:
                raise ValueError(f"source archive digest differs: {reference['name']}")
            return
        except (urllib.error.URLError, OSError) as error:
            errors.append(str(error))
    raise RuntimeError(f"could not acquire {reference['name']}: {errors}")


def stage_stata(root, lock):
    refs = root / "references"
    refs.mkdir(parents=True, exist_ok=True)
    sources = root / "sources"
    sources.mkdir(exist_ok=True)
    for name in ["reghdfe", "ftools", "require", "ppmlhdfe"]:
        ref = lock[name]
        archive = root / "archives" / (name + ".tar.gz")
        if name != "ppmlhdfe":
            bundle = ROOT / "ci/reference-bundles/stata-require" / name / ref["commit"] / f"{name}-{ref['commit']}.bundle"
            checkout = sources / name
            if not checkout.exists():
                subprocess.run(["git", "clone", "--no-checkout", str(bundle), str(checkout)], check=True)
            payload = subprocess.check_output(["git", "-C", str(checkout), "archive", "--format=tar", ref["commit"], "--", "src"])
            import hashlib
            if hashlib.sha256(payload).hexdigest() != ref["source_archive_sha256"]:
                raise ValueError(f"bundle archive digest differs: {name}")
            source = tarfile.open(fileobj=io.BytesIO(payload))
            prefix = "src/"
        else:
            download(ref, archive)
            source = tarfile.open(archive)
            prefix = ref["source_archive_root"] + "/src/"
        with source:
            for member in source.getmembers():
                if not member.isfile() or not member.name.startswith(prefix):
                    continue
                relative = member.name[len(prefix):]
                if "/" in relative or Path(relative).suffix not in {".ado", ".mata", ".mlib", ".sthlp", ".pkg", ".toc"}:
                    continue
                destination = sources / name / "src" / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                with source.extractfile(member) as stream:
                    payload = stream.read()
                if destination.exists() and destination.read_bytes() != payload:
                    raise ValueError(f"conflicting staged reference file: {relative}")
                destination.write_bytes(payload)
                if destination.suffix in {".ado", ".mata", ".mlib", ".sthlp"}:
                    runtime = refs / relative
                    if runtime.exists() and runtime.read_bytes() != payload:
                        raise ValueError(f"conflicting runtime reference: {relative}")
                    runtime.write_bytes(payload)
        for path, expected in ref.get("required_capture_sha256", {}).items():
            if sha256(sources / name / path) != expected:
                raise ValueError(f"staged reference digest differs: {name}/{path}")
    return refs


def stage_r(root, reference):
    archive = root / "archives/fixest.tar.gz"
    download(reference, archive)
    library = root / "r-library"
    library.mkdir(exist_ok=True)
    identity = library / "fereg-fixest-source.sha256"
    if identity.exists() and identity.read_text().strip() == reference["source_archive_sha256"]:
        subprocess.run(["Rscript", "-e", f'.libPaths(c({json.dumps(str(library))},.libPaths())); stopifnot(as.character(packageVersion("fixest"))=="0.14.2")'], check=True)
        return library
    expression = (f'.libPaths(c({json.dumps(str(library))},.libPaths())); '
                  'deps<-c("Rcpp","dreamerr","stringmagic","Formula","numDeriv","sandwich"); '
                  'missing<-deps[!vapply(deps,requireNamespace,logical(1),quietly=TRUE)]; '
                  f'if(length(missing)) install.packages(missing,lib={json.dumps(str(library))},repos="https://cran.r-project.org")')
    subprocess.run(["Rscript", "-e", expression], check=True)
    subprocess.run(["R", "CMD", "INSTALL", "--library=" + str(library), str(archive)], check=True,
                   env={**os.environ, "R_LIBS_USER": str(library)})
    identity.write_text(reference["source_archive_sha256"] + "\n")
    return library


def setup(root):
    root = Path(root).resolve()
    root.mkdir(parents=True, exist_ok=True)
    (root / "archives").mkdir(exist_ok=True)
    lock = {r["name"]: r for r in tomllib.loads((ROOT / "references.lock.toml").read_text())["reference"]}
    refs = stage_stata(root, lock)
    library = stage_r(root, lock["fixest"])
    metadata = {"references": str(refs), "r_library": str(library),
                "source_hashes": {name: lock[name]["source_archive_sha256"] for name in ["reghdfe","ftools","require","ppmlhdfe","fixest"]}}
    (root / "environment.json").write_text(json.dumps(metadata, indent=2) + "\n")
    return metadata


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    setup(parser.parse_args().root)
