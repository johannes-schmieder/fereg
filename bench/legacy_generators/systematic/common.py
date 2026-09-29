"""Shared campaign identities and enumeration; no estimation code."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent


def read_json(path):
    return json.loads(Path(path).read_text())


def write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + "\n")
    temporary.replace(path)


def sha256(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as handle:
        for block in iter(lambda: handle.read(2**20), b""):
            digest.update(block)
    return digest.hexdigest()


def cells(manifest):
    """Deduplicate observation/core intersections, without duplicating figures."""
    from light import enabled, cells as light_cells
    if enabled(manifest):
        return light_cells(manifest)
    result = []
    for model in manifest["arms"]:
        for design, spec in manifest["designs"].items():
            settings = {(n, 10, t, "main") for n in manifest["sizes"] for t in (4, 28)}
            settings |= {(manifest["scaling_n"], 10, t, "main") for t in manifest["threads"]}
            settings |= {(manifest["scaling_n"], k, t, "width") for k in (4, 50) for t in (4, 28)}
            if model == "ppml":
                settings |= {(manifest["scaling_n"], 10, t, "zeros") for t in (4, 28)}
            for n, k, threads, section in sorted(settings):
                result.append(dict(model=model, design=design, suite=spec["suite"],
                                   n=n, k=k, threads=threads, section=section,
                                   effects=spec["effects"]))
    return result


def cell_id(cell):
    return "{model}-{design}-n{n}-k{k}-t{threads}-{section}".format(**cell)


def dataset_id(cell, seed):
    # Thread requests never change the generated dataset or random stream.
    return "{model}-{design}-n{n}-k{k}-{section}-s{seed}".format(**cell, seed=seed)


def arm_order(arms, repetition):
    # For each comparator, candidate comes first in three of six warm rounds.
    offset = ((repetition - 1) // 2) % len(arms)
    rotated = arms[offset:] + arms[:offset]
    return rotated if repetition % 2 else list(reversed(rotated))


def thread_environment(threads):
    return {"OMP_NUM_THREADS": str(threads), "RAYON_NUM_THREADS": str(threads),
            "NUMBA_NUM_THREADS": str(threads), "OPENBLAS_NUM_THREADS": "1",
            "MKL_NUM_THREADS": "1", "NUMEXPR_NUM_THREADS": "1",
            "OMP_MAX_ACTIVE_LEVELS": "1", "OMP_NESTED": "FALSE"}


def manifest_for(root):
    path = Path(root) / 'input/manifest.json'
    return read_json(path if path.exists() else HERE / 'manifest.json')
