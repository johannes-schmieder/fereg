#!/usr/bin/env python3
"""Guarded exact-green completion loop for the private stfeols milestone.

The outer process performs synchronization, licensed qualification, receipt
publication, and progress checks without using a model.  Codex is invoked only
for one coherent implementation/repair gate at a time.  Every phase must leave
main clean and synchronized; every accepted source must have its own green
Rust/source plus licensed-Stata receipt.
"""

from __future__ import annotations

import json
import os
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Sequence

ROOT = Path(__file__).resolve().parents[2]
REPOSITORY = "johannes-schmieder/StFixest"
STATA_BIN = "/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp"
MAX_PHASES = int(os.environ.get("STFIXEST_COMPLETION_PHASES", "24"))
COMPLETION_MARKER = ROOT / ".ci/dev/stfeols-complete.json"
REPORT = ROOT / ".ci/dev/stfeols-completion-orchestrator.json"
IGNORED_SOURCE_PREFIXES = (
    ".ci/stata/results/",
    ".ci/stata/full-results/",
    ".ci/dev/",
)
IGNORED_SOURCE_FILES = {".ci/stata/latest.json"}


def log(message: str) -> None:
    print(message, flush=True)


def run(
    command: Sequence[str],
    *,
    check: bool = True,
    env: dict[str, str] | None = None,
    cwd: Path = ROOT,
    capture: bool = False,
) -> subprocess.CompletedProcess[str]:
    log("+ " + " ".join(shlex.quote(part) for part in command))
    result = subprocess.run(
        list(command),
        cwd=cwd,
        env=env,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.STDOUT if capture else None,
        check=False,
    )
    if capture and result.stdout:
        print(result.stdout, end="" if result.stdout.endswith("\n") else "\n", flush=True)
    if check and result.returncode != 0:
        raise RuntimeError(
            f"command failed with rc={result.returncode}: "
            + " ".join(shlex.quote(part) for part in command)
        )
    return result


def capture(command: Sequence[str], *, cwd: Path = ROOT) -> str:
    return subprocess.check_output(list(command), cwd=cwd, text=True).strip()


def git(*args: str, check: bool = True, capture_output: bool = False) -> subprocess.CompletedProcess[str]:
    return run(("git", *args), check=check, capture=capture_output)


def git_capture(*args: str) -> str:
    return capture(("git", *args))


def configure_git() -> None:
    git("config", "user.name", "github-actions[bot]")
    git("config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com")


def sync_main() -> None:
    git("checkout", "main")
    git("fetch", "origin", "main")
    git("reset", "--hard", "origin/main")
    if git_capture("status", "--porcelain"):
        raise RuntimeError("main is not clean after synchronization")


def push_main() -> None:
    for attempt in range(1, 6):
        git("fetch", "origin", "main")
        rebased = git("rebase", "origin/main", check=False)
        if rebased.returncode != 0:
            git("rebase", "--abort", check=False)
            raise RuntimeError("could not rebase current source on main")
        pushed = git("push", "origin", "HEAD:main", check=False)
        if pushed.returncode == 0:
            return
        time.sleep(2 * attempt)
    raise RuntimeError("lost five main update races")


def commit_paths(message: str, paths: Iterable[Path]) -> str:
    relative_existing: list[str] = []
    relative_deleted: list[str] = []
    for path in paths:
        relative = str(path.relative_to(ROOT))
        if path.exists():
            relative_existing.append(relative)
        else:
            relative_deleted.append(relative)
    if relative_existing:
        git("add", "--", *relative_existing)
    if relative_deleted:
        git("add", "-u", "--", *relative_deleted, check=False)
    if git("diff", "--cached", "--quiet", check=False).returncode == 0:
        return git_capture("rev-parse", "HEAD")
    git("commit", "-m", message)
    sha = git_capture("rev-parse", "HEAD")
    push_main()
    return sha


def changed_paths(sha: str) -> list[str]:
    output = capture(
        (
            "git",
            "diff-tree",
            "--no-commit-id",
            "--name-only",
            "-r",
            "--root",
            sha,
        )
    )
    return [line for line in output.splitlines() if line]


def is_source_path(path: str) -> bool:
    if path in IGNORED_SOURCE_FILES:
        return False
    return not any(path.startswith(prefix) for prefix in IGNORED_SOURCE_PREFIXES)


def current_source_sha() -> str:
    for sha in git_capture("rev-list", "HEAD").splitlines():
        if any(is_source_path(path) for path in changed_paths(sha)):
            return sha
    raise RuntimeError("could not locate the current non-evidence source commit")


def receipt_path(sha: str) -> Path:
    return ROOT / ".ci/stata/results" / f"{sha}.json"


def read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def receipt_green(document: dict[str, object], sha: str) -> bool:
    return (
        document.get("tested_sha") == sha
        and document.get("status") == "success"
        and document.get("stata_status") == "success"
        and document.get("rust_status") == "success"
        and document.get("stata_rc") == 0
        and document.get("rust_rc") == 0
        and document.get("tests_failed") == 0
    )


def current_receipt() -> tuple[str, dict[str, object] | None]:
    sha = current_source_sha()
    path = receipt_path(sha)
    return sha, read_json(path) if path.is_file() else None


def qualification_env(sha: str, profile: str = "quick") -> dict[str, str]:
    env = os.environ.copy()
    env.update(
        {
            "GITHUB_SHA": sha,
            "GITHUB_REF": "refs/heads/main",
            "GITHUB_REF_NAME": "main",
            "STATA_PROFILE": profile,
            "STATA_BIN": STATA_BIN,
            "CARGO_TERM_COLOR": "always",
            "RUST_BACKTRACE": "1",
        }
    )
    return env


def publish_receipt(source_sha: str, document: dict[str, object]) -> str:
    destination = receipt_path(source_sha)
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / ".ci/stata/run/receipt.json", destination)
    paths = [destination]
    if receipt_green(document, source_sha):
        latest = ROOT / ".ci/stata/latest.json"
        shutil.copy2(ROOT / ".ci/stata/run/receipt.json", latest)
        paths.append(latest)
    return commit_paths(f"ci(stata): record {source_sha[:12]} [skip ci]", paths)


def qualify_current_source() -> tuple[str, dict[str, object], str]:
    source_sha = current_source_sha()
    if git_capture("rev-parse", "HEAD") != source_sha:
        # Evidence-only commits may sit above the source.  Qualify the source tree
        # represented by current HEAD; GITHUB_SHA remains the bound source SHA.
        log(f"Qualifying source {source_sha} from evidence-bearing HEAD {git_capture('rev-parse', 'HEAD')}")
    shutil.rmtree(ROOT / ".ci/stata/run", ignore_errors=True)
    env = qualification_env(source_sha)
    stata = run(("./ci/run_stata_ci.sh", "quick"), check=False, env=env)
    rust = run(("./ci/record_rust_quick.sh",), check=False, env=env)
    if not (ROOT / ".ci/stata/run/receipt.json").is_file():
        raise RuntimeError("licensed qualification produced no Stata receipt")
    if not (ROOT / ".ci/stata/run/rust-quick.status").is_file():
        raise RuntimeError("source qualification produced no Rust status")
    run(
        (
            "/usr/bin/python3",
            "ci/augment_ci_receipt.py",
            ".ci/stata/run/receipt.json",
            ".ci/stata/run/rust-quick.status",
        ),
        env=env,
    )
    run(
        (
            "/usr/bin/python3",
            "ci/check_stata_receipt.py",
            ".ci/stata/run/receipt.json",
            "--expect-tested-sha",
            source_sha,
            "--expect-profile",
            "quick",
        ),
        env=env,
    )
    document = read_json(ROOT / ".ci/stata/run/receipt.json")
    receipt_commit = publish_receipt(source_sha, document)
    log(
        "QUALIFICATION "
        f"source={source_sha} receipt_commit={receipt_commit} "
        f"status={document.get('status')} stata_process_rc={stata.returncode} "
        f"rust_process_rc={rust.returncode} tests_passed={document.get('tests_passed')} "
        f"tests_failed={document.get('tests_failed')}"
    )
    return source_sha, document, receipt_commit


def completion_valid(source_sha: str) -> bool:
    if not COMPLETION_MARKER.is_file():
        return False
    try:
        document = read_json(COMPLETION_MARKER)
    except (OSError, ValueError, json.JSONDecodeError):
        return False
    required: dict[str, object] = {
        "schema_version": 1,
        "source_sha": source_sha,
        "public_activation": True,
        "full_scientific_qualification": "success",
        "licensed_stata": "success",
        "rust_source": "success",
        "frozen_projection": "success",
        "benchmark_gate": "success",
        "public_transaction": "success",
        "performance": "accepted",
    }
    return all(document.get(key) == value for key, value in required.items())


def codex_command(prompt: str, last_message: Path) -> list[str]:
    help_result = run(("codex", "exec", "--help"), capture=True)
    help_text = help_result.stdout or ""
    base = ["codex", "exec"]
    if "--full-auto" in help_text:
        base.append("--full-auto")
    if "--cd" in help_text:
        base.extend(("--cd", str(ROOT)))
    elif "-C" in help_text:
        base.extend(("-C", str(ROOT)))
    if "--output-last-message" in help_text:
        base.extend(("--output-last-message", str(last_message)))
    if "--skip-git-repo-check" in help_text:
        base.append("--skip-git-repo-check")
    # Prefer extra-high reasoning through a normal CLI config override, but retry
    # without it if this installed CLI/config does not recognize the key.
    configured = list(base)
    if "--config" in help_text or "-c" in help_text:
        configured.extend(("-c", 'model_reasoning_effort="xhigh"'))
    configured.append(prompt)
    return configured


def run_codex_phase(prompt: str, phase: int) -> int:
    if shutil.which("codex") is None:
        raise RuntimeError("Codex CLI is unavailable on the licensed runner")
    with tempfile.TemporaryDirectory(prefix="stfixest-codex-") as temporary:
        last_message = Path(temporary) / "last-message.txt"
        command = codex_command(prompt, last_message)
        result = run(command, check=False)
        if result.returncode != 0 and "model_reasoning_effort" in " ".join(command):
            retry = [part for part in command if part not in ("-c", 'model_reasoning_effort="xhigh"')]
            log(f"Phase {phase}: retrying installed CLI without optional reasoning config")
            result = run(retry, check=False)
        return result.returncode


def phase_prompt(
    *,
    phase: int,
    source_sha: str,
    receipt: dict[str, object] | None,
    repair: bool,
) -> str:
    receipt_summary = (
        "no exact-SHA receipt exists"
        if receipt is None
        else json.dumps(
            {
                key: receipt.get(key)
                for key in (
                    "status",
                    "failure_kind",
                    "failure_detail",
                    "stata_status",
                    "stata_rc",
                    "rust_status",
                    "rust_rc",
                    "tests_passed",
                    "tests_failed",
                )
            },
            sort_keys=True,
        )
    )
    mode = (
        "Repair the current red exact-SHA gate before doing anything else."
        if repair
        else "Implement exactly one coherent next formally required gate."
    )
    return f"""You are phase {phase} of guarded completion work in the private repository
{REPOSITORY}, running on its licensed Apple Silicon Stata/MP 18 machine.

Work directly on main. Never create a branch. Inspect current main and recent
history yourself. Read AGENTS.md, STATUS.md, current exact-SHA receipts, the
frozen OLS manifest/receipts, and all relevant implementation and tests before
editing. The current source identity is {source_sha}; receipt state is:
{receipt_summary}

{mode}

Non-negotiable policy:
- Preserve every scientific, sample, VCE, absorbed/residual DoF, weight,
  omission/status, solver-certificate, allocation, transaction/rollback, and
  fail-closed contract. Never weaken tolerances or skip assertions to make a
  test pass.
- Pinned reghdfe comparison is necessary but never replaces independent
  mathematical/oracle checks.
- Public stfeols and estimate-ols must remain disabled until the complete OLS
  1.0 surface, frozen projection, full licensed-Stata/Rust qualification,
  public eclass transaction and rollback, and scientific-first speed/RSS
  benchmark gate are all genuinely complete. stfepois/estimate-ppml remain
  disabled independently.
- Use licensed Stata only on this runner. Do not commit raw logs, local paths,
  license identity, secrets, restricted data, or agent transcripts.
- Use maximum reasoning care for math/architecture and ordinary effort for
  mechanical monitoring. Make small coherent commits directly on main and push
  after meaningful changes. Update STATUS.md only from exact-green evidence.
- For ordinary implementation/debugging choices, use best judgment. Ask the
  user only for a genuinely scientific/API/architectural ambiguity.
- Follow STATUS.md's live order and do not skip blockers. If interaction
  differentials remain red, close them first; then reseal the frozen hidden OLS
  projection; reconcile/activate final-sample categorical rebasing; implement
  the remaining frozen blockers and public parser/prediction/saved-output/
  postestimation surface; qualify the public transaction; run full independent
  parity and the frozen scientific-first performance/RSS gate; only then enable
  stfeols.

Run all relevant Rust, Python, C/loader, and licensed Stata tests for the one
coherent gate. Leave the checkout clean, commit and push the source change, and
bind acceptance to the exact tested source SHA. The outer orchestrator will
independently re-run quick licensed qualification after this phase, so do not
spend a model turn polling CI.

Only when stfeols is fully complete, create .ci/dev/stfeols-complete.json with
exactly the accepted source SHA and these claims, all backed by committed
receipts/evidence:
{{
  "schema_version": 1,
  "source_sha": "<accepted source SHA>",
  "public_activation": true,
  "full_scientific_qualification": "success",
  "licensed_stata": "success",
  "rust_source": "success",
  "frozen_projection": "success",
  "benchmark_gate": "success",
  "public_transaction": "success",
  "performance": "accepted"
}}
Never create the marker for a hidden-only, partial, unbenchmarked, or red state.
Stop this phase after one coherent exact-source gate is implemented and pushed,
or at a genuine ambiguity.
"""


def write_report(
    *,
    status: str,
    phase: int,
    source_sha: str,
    detail: str,
    receipt: dict[str, object] | None,
) -> None:
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    document = {
        "schema_version": 1,
        "updated_at": datetime.now(timezone.utc).isoformat(),
        "status": status,
        "phase": phase,
        "source_sha": source_sha,
        "detail": detail,
        "receipt_status": None if receipt is None else receipt.get("status"),
        "tests_passed": None if receipt is None else receipt.get("tests_passed"),
        "tests_failed": None if receipt is None else receipt.get("tests_failed"),
        "model_transcript_committed": False,
        "raw_logs_committed": False,
    }
    REPORT.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
    commit_paths("ci(dev): record stfeols completion progress [skip ci]", (REPORT,))


def main() -> None:
    os.chdir(ROOT)
    configure_git()
    sync_main()
    for phase in range(1, MAX_PHASES + 1):
        sync_main()
        source_sha, receipt = current_receipt()
        if receipt is not None and receipt_green(receipt, source_sha) and completion_valid(source_sha):
            write_report(
                status="complete",
                phase=phase - 1,
                source_sha=source_sha,
                detail="completion marker and exact-SHA receipt are valid",
                receipt=receipt,
            )
            log(f"STFEOLS_COMPLETE source={source_sha}")
            return

        repair = receipt is None or not receipt_green(receipt, source_sha)
        prompt = phase_prompt(
            phase=phase,
            source_sha=source_sha,
            receipt=receipt,
            repair=repair,
        )
        before = source_sha
        rc = run_codex_phase(prompt, phase)
        if rc != 0:
            write_report(
                status="codex_failure",
                phase=phase,
                source_sha=before,
                detail=f"Codex phase returned rc={rc}",
                receipt=receipt,
            )
            raise RuntimeError(f"Codex phase {phase} failed with rc={rc}")

        # Codex may have pushed directly, or may have left local commits.  Push
        # any clean local commits, but never auto-commit an unknown dirty tree.
        if git_capture("status", "--porcelain"):
            write_report(
                status="dirty_worktree",
                phase=phase,
                source_sha=before,
                detail="Codex left uncommitted files",
                receipt=receipt,
            )
            raise RuntimeError(f"Codex phase {phase} left a dirty worktree")
        push_main()
        sync_main()
        after = current_source_sha()
        if after == before:
            write_report(
                status="no_progress",
                phase=phase,
                source_sha=after,
                detail="phase made no source commit",
                receipt=receipt,
            )
            raise RuntimeError(f"Codex phase {phase} made no source progress")

        qualified_sha, qualified, _ = qualify_current_source()
        if qualified_sha != after:
            raise RuntimeError("qualification source identity changed unexpectedly")
        green = receipt_green(qualified, qualified_sha)
        write_report(
            status="accepted_gate" if green else "red_gate",
            phase=phase,
            source_sha=qualified_sha,
            detail=(
                "one coherent gate accepted exact-green"
                if green
                else "exact-SHA qualification is red; next phase must repair it"
            ),
            receipt=qualified,
        )
        # A red gate is intentionally fed into the next phase rather than
        # silently accepted or abandoned.

    sync_main()
    source_sha, receipt = current_receipt()
    write_report(
        status="phase_cap",
        phase=MAX_PHASES,
        source_sha=source_sha,
        detail="guarded phase cap reached before completion",
        receipt=receipt,
    )
    raise RuntimeError("guarded phase cap reached before stfeols completion")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:  # noqa: BLE001 - top-level evidence path
        print(f"STFEOLS_ORCHESTRATOR_FAILURE: {error}", file=sys.stderr, flush=True)
        raise
