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


def thread_environment(threads):
    return {"OMP_NUM_THREADS": str(threads), "RAYON_NUM_THREADS": str(threads),
            "NUMBA_NUM_THREADS": str(threads), "OPENBLAS_NUM_THREADS": "1",
            "MKL_NUM_THREADS": "1", "NUMEXPR_NUM_THREADS": "1",
            "VECLIB_MAXIMUM_THREADS": "1", "OMP_MAX_ACTIVE_LEVELS": "1", "OMP_NESTED": "FALSE"}
