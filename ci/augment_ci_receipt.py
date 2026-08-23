#!/usr/bin/env python3
"""Add the normal-push Rust result to an existing Stata receipt."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys


def read_status(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        key, separator, value = line.partition("=")
        if not separator or not key:
            raise ValueError(f"malformed Rust status line: {line!r}")
        values[key] = value
    return values


def write_json_atomic(path: Path, payload: dict[str, object]) -> None:
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    temporary.replace(path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("receipt", type=Path)
    parser.add_argument("rust_status", type=Path)
    args = parser.parse_args()

    try:
        receipt = json.loads(args.receipt.read_text(encoding="utf-8"))
        status = read_status(args.rust_status)
        rust_rc = int(status["rust_rc"])
    except (OSError, ValueError, KeyError, json.JSONDecodeError) as exc:
        print(f"cannot augment CI receipt: {exc}", file=sys.stderr)
        return 2
    if status.get("completed") != "1" or status.get("rust_status") not in {
        "success",
        "failure",
    }:
        print("cannot augment CI receipt: incomplete Rust status", file=sys.stderr)
        return 2

    receipt["stata_status"] = receipt.get("stata_status", receipt.get("status"))
    receipt["rust_status"] = status["rust_status"]
    receipt["rust_rc"] = rust_rc
    if rust_rc != 0:
        receipt["status"] = "failure"
        if receipt["stata_status"] == "success":
            receipt["failure_kind"] = "rust_error"
            receipt["failure_detail"] = f"quick Rust/C/source checks returned {rust_rc}"
    write_json_atomic(args.receipt, receipt)
    print(
        f"CI_RECEIPT_AUGMENTED stata_status={receipt['stata_status']} "
        f"rust_status={receipt['rust_status']} status={receipt['status']}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
