#!/usr/bin/env python3
"""Audit a built Stata plugin for exports and forbidden dynamic dependencies."""
from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

FORBIDDEN = ("openblas", "mkl", "libgomp", "libomp", "tbb", "python", "julia", "rustc")
WINDOWS_CRT_FORBIDDEN = ("vcruntime", "msvcp", "ucrtbase", "api-ms-win-crt")
MAX_GLIBC = (2, 28)


def parse_glibc_versions(text: str) -> list[tuple[int, int]]:
    return sorted(
        {
            (int(major), int(minor))
            for major, minor in re.findall(r"GLIBC_(\d+)\.(\d+)", text)
        }
    )


def enforce_glibc_baseline(text: str) -> tuple[int, int] | None:
    versions = parse_glibc_versions(text)
    if not versions:
        return None
    newest = max(versions)
    if newest > MAX_GLIBC:
        raise RuntimeError(
            f"Linux plugin requires GLIBC_{newest[0]}.{newest[1]}, "
            f"newer than the supported GLIBC_{MAX_GLIBC[0]}.{MAX_GLIBC[1]} baseline"
        )
    return newest


def run(*args: str) -> str:
    completed = subprocess.run(args, check=True, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    return completed.stdout


def require_tool(name: str) -> str:
    path = shutil.which(name)
    if path is None:
        raise RuntimeError(f"required audit tool not found: {name}")
    return path


def audit(path: Path) -> dict[str, object]:
    if not path.is_file() or path.stat().st_size == 0:
        raise RuntimeError(f"plugin does not exist or is empty: {path}")
    file_output = run(require_tool("file"), str(path)) if shutil.which("file") else "unknown"
    lower = file_output.lower()
    report: dict[str, object] = {"path": str(path), "size": path.stat().st_size, "file": file_output.strip()}
    if "elf" in lower:
        if "x86-64" not in lower and "x86_64" not in lower:
            raise RuntimeError(f"Linux plugin is not x86-64: {file_output.strip()}")
        dependencies = run(require_tool("ldd"), str(path))
        symbols = run(require_tool("nm"), "-D", "--defined-only", str(path))
        versions = run(require_tool("readelf"), "--version-info", str(path))
        newest_glibc = enforce_glibc_baseline(versions)
        report.update(
            platform="linux",
            dependencies=dependencies,
            exports=symbols,
            symbol_versions=versions,
            newest_glibc=newest_glibc,
        )
    elif "mach-o" in lower or sys.platform == "darwin":
        architectures = run(require_tool("lipo"), "-archs", str(path))
        dependencies = run(require_tool("otool"), "-L", str(path))
        symbols = run(require_tool("nm"), "-gU", str(path))
        if set(architectures.split()) != {"x86_64", "arm64"}:
            raise RuntimeError(f"macOS plugin is not universal: {architectures.strip()}")
        report.update(platform="macos", architectures=architectures, dependencies=dependencies, exports=symbols)
    elif "pe32" in lower or sys.platform == "win32":
        dumpbin = require_tool("dumpbin")
        dependencies = run(dumpbin, "/DEPENDENTS", str(path))
        symbols = run(dumpbin, "/EXPORTS", str(path))
        headers = run(dumpbin, "/HEADERS", str(path))
        lower_headers = headers.lower()
        if "8664 machine" not in lower_headers and "machine (x64)" not in lower_headers:
            raise RuntimeError("Windows plugin is not an x86-64 PE image")
        report.update(
            platform="windows",
            dependencies=dependencies,
            exports=symbols,
            headers=headers,
        )
        lower_dependencies = dependencies.lower()
        dynamic_crt = [name for name in WINDOWS_CRT_FORBIDDEN if name in lower_dependencies]
        if dynamic_crt:
            raise RuntimeError(f"Windows plugin is not statically CRT-linked: {', '.join(dynamic_crt)}")
    else:
        raise RuntimeError(f"unrecognized native binary format: {file_output.strip()}")

    dependency_and_export_text = (
        str(report.get("dependencies", ""))
        + "\n"
        + str(report.get("exports", ""))
    ).lower()
    found = [name for name in FORBIDDEN if name in dependency_and_export_text]
    if found:
        raise RuntimeError(f"forbidden dynamic dependencies or symbols found: {', '.join(found)}")
    exports = str(report.get("exports", ""))
    for symbol in ("pginit", "stata_call"):
        if symbol not in exports:
            raise RuntimeError(f"required exported symbol missing: {symbol}")
    print(json.dumps(report, indent=2, sort_keys=True))
    return report


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: audit_binary.py PATH_TO_PLUGIN")
    audit(Path(sys.argv[1]).resolve())
