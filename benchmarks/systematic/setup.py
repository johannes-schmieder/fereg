"""Prepare frozen comparator files; acquisition and build precede all timings."""
import shutil
import tarfile
import tomllib
from pathlib import Path

from common import read_json, sha256, write_json


def unpack(archive, destination):
    destination.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive) as source:
        # Upstream test trees contain machine-specific symlinks, none of which
        # belong to the runtime closure. Extract only regular files/directories.
        source.extractall(destination, members=[m for m in source.getmembers() if m.isfile() or m.isdir()], filter="data")


def stage(root):
    candidate = root / "candidate"
    identity = read_json(root / "input/candidate.json")
    if sha256(root / "input/candidate.tar.gz") != identity["sha256"]:
        raise ValueError("candidate archive hash differs")
    if not candidate.exists():
        unpack(root / "input/candidate.tar.gz", candidate)
    receipt = read_json(root / "input/archives/archives.json")
    refs = root / "references"
    refs.mkdir(exist_ok=True)
    tree_roots = {}
    for name, info in receipt.items():
        archive = root / f"input/archives/{name}.tar.gz"
        if sha256(archive) != info["sha256"]:
            raise ValueError(f"archive hash differs: {name}")
        destination = root / "env/sources" / name
        unpack(archive, destination)
        contents = list(destination.iterdir())
        if len(contents) != 1:
            raise ValueError(f"ambiguous archive root: {name}")
        tree = contents[0]
        tree_roots[name] = str(tree)
        if name in ("fixest", "pyfixest"):
            continue
        source = tree / ("build" if name == "gtools" else "src")
        for path in source.iterdir():
            if path.is_file() and path.suffix in (".ado", ".mata", ".mlib", ".sthlp", ".plugin"):
                target = refs / path.name
                if target.exists() and sha256(target) != sha256(path):
                    raise ValueError(f"conflicting comparator file: {path.name}")
                shutil.copyfile(path, target)
    lock = tomllib.loads((candidate / "references.lock.toml").read_text())
    for reference in lock["reference"]:
        name = reference["name"]
        if name == "gtools-gregress":
            name = "gtools"
        if name not in tree_roots:
            continue
        tree = Path(tree_roots[name])
        expected = reference.get("required_capture_sha256", reference.get("captured_component_sha256", {}))
        for file, digest in expected.items():
            if sha256(tree / file) != digest:
                raise ValueError(f"repository reference lock mismatch: {name}/{file}")
    write_json(root / "env/source-roots.json", tree_roots)
    write_json(root / "env/reference-files.json", {p.name: sha256(p) for p in sorted(refs.iterdir()) if p.is_file()})


if __name__ == "__main__":
    import sys
    stage(Path(sys.argv[1]))
