"""Active repository status and public PPML entry-point contracts."""
from __future__ import annotations

from pathlib import PurePosixPath
import re


def status_paths(tracked_paths: list[str]) -> list[str]:
    # Receipts may embed the source tree they measured. They are immutable
    # evidence, and untracked research trees are outside repository policy.
    archived = ("benchmarks/reports/", "build_evidence/", ".ci/")
    reserved = {"handoff.md", "implementation_status.md", "local_codex_prompt.md", "validation.md"}
    return sorted(path for path in tracked_paths
                  if not path.startswith(archived)
                  and PurePosixPath(path).suffix.lower() == ".md"
                  and ("status" in PurePosixPath(path).name.lower()
                       or PurePosixPath(path).name.lower() in reserved))


def status_errors(source: str) -> list[str]:
    # STATUS is deliberately mutable; validate its role and navigability,
    # without requiring an obsolete set of checkpoint section titles.
    errors = []
    if not source.startswith("# fereg 0.5.0 beta status\n"):
        errors.append("STATUS.md has the wrong title")
    for field in ("Last updated", "Branch", "Mode", "Release state"):
        if not re.search(rf"^\*\*{field}:\*\*\s+\S", source, re.MULTILINE):
            errors.append(f"STATUS.md is missing its {field} field")
    if not re.search(r"^## \S", source, re.MULTILINE):
        errors.append("STATUS.md must contain a checkpoint section")
    if not re.search(r"^- \[ \] \S", source, re.MULTILINE):
        errors.append("STATUS.md must record the remaining beta gates")
    return errors


def public_ppml_wrapper_is_exact(source: str) -> bool:
    executable = tuple(line.strip().lower() for line in source.splitlines()
                       if line.strip() and not line.lstrip().startswith(("*", "//")))
    # ADR 0041 removed sortpreserve; no extra command, option, or second call
    # is permitted at the public boundary.
    return executable == ("program define feppml, eclass", "version 18.0",
                          "_fereg_ppml `0'", "end")
