#!/usr/bin/env python3
"""Bounded LSMR reuse inventory using unchanged strict OLS capture/comparison."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from scripts import ols_arithmetic_development as dev

MANIFEST = ROOT / "benchmarks/manifests/ols_lsmr_reuse_development_v1.json"
BASE_COMMAND = dev.command


def scopes():
    result = []
    for entry in json.loads(MANIFEST.read_text())["scopes"]:
        s = dict(entry)
        if set(s) != {"design", "k", "threads", "memory", "poolsize", "guard"}:
            raise ValueError("unexpected LSMR scope fields")
        if not isinstance(s["poolsize"], int) or s["poolsize"] <= 0:
            raise ValueError("invalid LSMR scope poolsize")
        s["id"] = (f'{s["design"]}_k{s["k"]}_t{s["threads"]}_{s["memory"]}'
                   f'_p{s["poolsize"]}' + ("_" + s["guard"] if s["guard"] else ""))
        result.append(s)
    if len({s["id"] for s in result}) != len(result):
        raise ValueError("duplicate LSMR scope")
    return result


def command(scope, reference=False):
    value = BASE_COMMAND(scope, reference)
    return value if reference else value + f' poolsize({scope["poolsize"]})'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("list", "prepare", "run", "compare"))
    parser.add_argument("--work", type=Path)
    parser.add_argument("--reference", type=Path)
    parser.add_argument("--source", type=Path, default=ROOT)
    parser.add_argument("--baseline-source", type=Path)
    parser.add_argument("--label")
    parser.add_argument("--scope", action="append")
    parser.add_argument("--trace", action="store_true")
    args = parser.parse_args()
    dev.MANIFEST, dev.scopes, dev.command = MANIFEST, scopes, command
    if args.action == "list":
        print(json.dumps(scopes(), indent=2))
    elif args.action == "prepare":
        dev.prepare(args.work, args.reference)
    elif args.action == "compare":
        dev.compare(args)
    else:
        dev.run(args)


if __name__ == "__main__":
    main()
