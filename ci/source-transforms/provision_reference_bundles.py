#!/usr/bin/env python3
"""Provision exact Stata-reference Git bundles, then activate offline capture.

This one-shot transform is the only networked step. It clones exactly the three
public repositories pinned in references.lock.toml, verifies the pinned commits
and required file hashes, creates full-history Git bundles, verifies each bundle
through a clean clone and fsck, and writes only redacted public provenance.
The permanent FE-interaction probe remains offline-only.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path
from typing import Any, Mapping, Sequence

ROOT = Path(__file__).resolve().parents[2]
LOCK = ROOT / "references.lock.toml"
COMPONENTS = ("reghdfe", "ftools", "require")
EXPECTED_REPOSITORIES = {
    "reghdfe": "https://github.com/sergiocorreia/reghdfe",
    "ftools": "https://github.com/sergiocorreia/ftools",
    "require": "https://github.com/sergiocorreia/stata-require",
}
HEX40 = re.compile(r"^[0-9a-f]{40}$")
HEX64 = re.compile(r"^[0-9a-f]{64}$")
MAX_BUNDLE_BYTES = 90 * 1024 * 1024


class ProvisionError(RuntimeError):
    def __init__(self, kind: str, component: str = "none") -> None:
        super().__init__(kind)
        self.kind = kind
        self.component = component if component in {*COMPONENTS, "none"} else "invalid"


def git_env() -> dict[str, str]:
    env = os.environ.copy()
    env.update(
        GIT_CONFIG_NOSYSTEM="1",
        GIT_CONFIG_GLOBAL=os.devnull,
        GIT_TERMINAL_PROMPT="0",
    )
    return env


def run(args: Sequence[str], *, component: str, cwd: Path | None = None) -> str:
    completed = subprocess.run(
        list(args),
        cwd=cwd,
        env=git_env(),
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if completed.returncode:
        raise ProvisionError("public_git_command_failed", component)
    return completed.stdout.strip()


def git_dir(repo: Path, args: Sequence[str], *, component: str) -> str:
    return run(["git", f"--git-dir={repo}", *args], component=component)


def git_c(repo: Path, args: Sequence[str], *, component: str) -> str:
    return run(["git", "-C", str(repo), *args], component=component)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def load_entries() -> dict[str, Mapping[str, Any]]:
    try:
        lock = tomllib.loads(LOCK.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as exc:
        raise ProvisionError("reference_lock_unreadable") from exc
    entries = {
        entry.get("name"): entry
        for entry in lock.get("reference", [])
        if isinstance(entry, Mapping) and entry.get("name") in COMPONENTS
    }
    if set(entries) != set(COMPONENTS):
        raise ProvisionError("reference_lock_inventory_invalid")
    for name in COMPONENTS:
        entry = entries[name]
        files = entry.get("required_capture_files")
        hashes = entry.get("required_capture_sha256")
        if (
            entry.get("repository") != EXPECTED_REPOSITORIES[name]
            or entry.get("checkout_dir") != name
            or not isinstance(entry.get("commit"), str)
            or not HEX40.fullmatch(entry["commit"])
            or not isinstance(files, list)
            or not files
            or len(files) != len(set(files))
            or not isinstance(hashes, Mapping)
            or set(files) != set(hashes)
            or any(not isinstance(value, str) or not HEX64.fullmatch(value) for value in hashes.values())
        ):
            raise ProvisionError("reference_lock_entry_invalid", name)
    return entries


def verify_checkout(checkout: Path, entry: Mapping[str, Any], component: str) -> None:
    commit = entry["commit"]
    if git_c(checkout, ["rev-parse", "--verify", f"{commit}^{{commit}}"], component=component) != commit:
        raise ProvisionError("bundle_commit_mismatch", component)
    git_c(checkout, ["checkout", "--detach", "--quiet", commit], component=component)
    if git_c(checkout, ["status", "--porcelain=v1", "--untracked-files=all"], component=component):
        raise ProvisionError("bundle_checkout_dirty", component)
    for relative, expected in entry["required_capture_sha256"].items():
        path = checkout / relative
        if path.is_symlink() or not path.is_file():
            raise ProvisionError("required_reference_file_missing", component)
        if sha256_file(path) != expected:
            raise ProvisionError("required_reference_file_hash_mismatch", component)
    git_c(checkout, ["fsck", "--full", "--strict"], component=component)


def build_component(work: Path, output: Path, entry: Mapping[str, Any], component: str) -> dict[str, Any]:
    repository = entry["repository"]
    commit = entry["commit"]
    mirror = work / f"{component}.git"
    run(["git", "clone", "--mirror", "--quiet", repository, str(mirror)], component=component)
    origin = git_dir(mirror, ["remote", "get-url", "origin"], component=component).removesuffix(".git").rstrip("/")
    if origin != repository.removesuffix(".git").rstrip("/"):
        raise ProvisionError("reference_origin_mismatch", component)
    if git_dir(mirror, ["rev-parse", "--verify", f"{commit}^{{commit}}"], component=component) != commit:
        raise ProvisionError("reference_commit_missing", component)
    git_dir(mirror, ["fsck", "--full", "--strict"], component=component)

    target = output / component / commit
    target.mkdir(mode=stat.S_IRWXU, parents=True)
    head_ref = f"refs/heads/stfixest-pin-{component}"
    git_dir(mirror, ["update-ref", head_ref, commit], component=component)
    bundle = target / f"{component}-{commit}.bundle"
    git_dir(mirror, ["bundle", "create", str(bundle), head_ref], component=component)
    if not bundle.is_file() or bundle.stat().st_size > MAX_BUNDLE_BYTES:
        raise ProvisionError("bundle_size_invalid", component)
    heads = run(["git", "bundle", "list-heads", str(bundle)], component=component, cwd=ROOT).splitlines()
    if heads != [f"{commit} {head_ref}"]:
        raise ProvisionError("bundle_head_identity_mismatch", component)

    verify = work / f"verify-{component}"
    run(["git", "clone", "--no-checkout", "--quiet", str(bundle), str(verify)], component=component)
    verify_checkout(verify, entry, component)

    bundle_digest = sha256_file(bundle)
    manifest = {
        "schema": 2,
        "status": "success",
        "component": component,
        "source_repository": repository,
        "source_commit": commit,
        "checkout_dir": entry["checkout_dir"],
        "required_files": dict(entry["required_capture_sha256"]),
        "bundle_file": bundle.name,
        "bundle_sha256": bundle_digest,
        "bundle_head_ref": head_ref,
        "provisioning_repository": os.environ.get("GITHUB_REPOSITORY", "unknown"),
        "provisioning_source_sha": os.environ.get("GITHUB_SHA", "unknown"),
        "workflow_run_id": int(os.environ.get("GITHUB_RUN_ID", "0")),
        "workflow_run_attempt": int(os.environ.get("GITHUB_RUN_ATTEMPT", "0")),
        "provisioning_network_policy": "network-allowed-public-source-only",
        "capture_network_policy": "offline-only",
    }
    (target / "manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True, ensure_ascii=True) + "\n",
        encoding="utf-8",
    )
    return {
        "commit": commit,
        "bundle_sha256": bundle_digest,
        "bundle_bytes": bundle.stat().st_size,
        "required_files": len(manifest["required_files"]),
    }


def build_bundles(output: Path) -> dict[str, dict[str, Any]]:
    if not output.is_absolute():
        raise ProvisionError("output_dir_not_absolute")
    shutil.rmtree(output, ignore_errors=True)
    output.mkdir(mode=stat.S_IRWXU, parents=True)
    entries = load_entries()
    work = Path(tempfile.mkdtemp(prefix="stfixest-reference-clones-", dir=output.parent))
    work.chmod(stat.S_IRWXU)
    try:
        return {
            component: build_component(work, output, entries[component], component)
            for component in COMPONENTS
        }
    finally:
        shutil.rmtree(work, ignore_errors=True)


LIVE_MATERIALIZER = '''#!/usr/bin/env python3
"""Materialize the exact lock-pinned Stata reference root from Git bundles.

All three public reference repositories are carried as digest-bound,
full-history Git bundles in this private repository. Capture is offline-only:
this resolver never searches the machine, never fetches, and never prints a
selected path. It clones each bundle into runner-temporary storage, restores
the public origin, and applies the unchanged clean-checkout and locked-file
preflight.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Mapping, Sequence

from ols_reference import load_reference_lock, preflight_reference_root
from resolve_reference_root import (
    LOCK,
    ROOT,
    ResolverError,
    _locked_capture_entries,
    _prepare_materialization_parent,
    write_private_path,
)

COMPONENTS = ("reghdfe", "ftools", "require")
BUNDLE_ROOT = ROOT / "ci/reference-bundles/stata-require"
MATERIALIZED_PREFIX = "stfixest-bundled-reference-"
HEX40 = re.compile(r"^[0-9a-f]{40}$")
HEX64 = re.compile(r"^[0-9a-f]{64}$")


class BundledResolverError(RuntimeError):
    def __init__(self, kind: str, component: str = "none") -> None:
        super().__init__(kind)
        self.kind = kind
        self.component = component if component in {*COMPONENTS, "none"} else "invalid"


def git_env() -> dict[str, str]:
    env = os.environ.copy()
    env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull, GIT_TERMINAL_PROMPT="0")
    return env


def run(args: Sequence[str], *, component: str, cwd: Path | None = None) -> str:
    completed = subprocess.run(
        list(args), cwd=cwd, env=git_env(), check=False,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
    )
    if completed.returncode:
        raise BundledResolverError("local_git_command_failed", component)
    return completed.stdout.strip()


def git_c(repo: Path, args: Sequence[str], *, component: str) -> str:
    return run(["git", "-C", str(repo), *args], component=component)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def required_hashes(entry: Mapping[str, Any], component: str) -> dict[str, str]:
    files = entry.get("required_capture_files")
    hashes = entry.get("required_capture_sha256")
    if (
        not isinstance(files, list) or not files or len(files) != len(set(files))
        or not isinstance(hashes, Mapping) or set(files) != set(hashes)
        or any(not isinstance(value, str) or not HEX64.fullmatch(value) for value in hashes.values())
    ):
        raise BundledResolverError("bundle_lock_files_invalid", component)
    return {path: hashes[path] for path in files}


def inventory(component: str, entry: Mapping[str, Any]) -> tuple[Path, Path]:
    commit = entry["commit"]
    directory = BUNDLE_ROOT / component / commit
    manifest = directory / "manifest.json"
    bundle = directory / f"{component}-{commit}.bundle"
    for path in (BUNDLE_ROOT, directory, manifest, bundle):
        if path.is_symlink():
            raise BundledResolverError("bundle_inventory_symlink", component)
    if not directory.is_dir() or not manifest.is_file() or not bundle.is_file():
        raise BundledResolverError("bundle_inventory_absent", component)
    return manifest, bundle


def validate_manifest(component: str, entry: Mapping[str, Any], manifest_path: Path, bundle: Path) -> str:
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise BundledResolverError("bundle_manifest_invalid", component) from exc
    expected = {
        "schema": 2,
        "status": "success",
        "component": component,
        "source_repository": entry["repository"],
        "source_commit": entry["commit"],
        "checkout_dir": entry["checkout_dir"],
        "required_files": required_hashes(entry, component),
        "bundle_file": bundle.name,
        "provisioning_repository": "johannes-schmieder/StFixest",
        "provisioning_network_policy": "network-allowed-public-source-only",
        "capture_network_policy": "offline-only",
    }
    if any(manifest.get(key) != value for key, value in expected.items()):
        raise BundledResolverError("bundle_manifest_identity_mismatch", component)
    if manifest.get("bundle_sha256") != sha256_file(bundle):
        raise BundledResolverError("bundle_digest_mismatch", component)
    if not isinstance(manifest.get("provisioning_source_sha"), str) or not HEX40.fullmatch(manifest["provisioning_source_sha"]):
        raise BundledResolverError("bundle_provisioning_identity_invalid", component)
    if not isinstance(manifest.get("workflow_run_id"), int) or not isinstance(manifest.get("workflow_run_attempt"), int):
        raise BundledResolverError("bundle_provisioning_identity_invalid", component)
    head_ref = manifest.get("bundle_head_ref")
    if head_ref != f"refs/heads/stfixest-pin-{component}":
        raise BundledResolverError("bundle_head_identity_mismatch", component)
    heads = run(["git", "bundle", "list-heads", str(bundle)], component=component, cwd=ROOT).splitlines()
    if heads != [f"{entry['commit']} {head_ref}"]:
        raise BundledResolverError("bundle_head_identity_mismatch", component)
    return head_ref


def clone_bundle(component: str, bundle: Path, destination: Path, entry: Mapping[str, Any]) -> None:
    run(["git", "clone", "--no-checkout", "--quiet", str(bundle), str(destination)], component=component)
    commit = entry["commit"]
    if git_c(destination, ["rev-parse", "--verify", f"{commit}^{{commit}}"], component=component) != commit:
        raise BundledResolverError("materialized_commit_mismatch", component)
    git_c(destination, ["checkout", "--detach", "--quiet", commit], component=component)
    git_c(destination, ["remote", "set-url", "origin", entry["repository"]], component=component)
    if git_c(destination, ["status", "--porcelain=v1", "--untracked-files=all"], component=component):
        raise BundledResolverError("materialized_checkout_dirty", component)
    git_c(destination, ["fsck", "--full", "--strict"], component=component)


def materialize_bundled_reference_root(parent: Path) -> tuple[Path, int]:
    try:
        materialize_parent = _prepare_materialization_parent(parent)
        lock = load_reference_lock(LOCK)
        entries = _locked_capture_entries(lock)
    except ResolverError as exc:
        raise BundledResolverError(exc.kind) from exc
    selected: dict[str, tuple[Path, Mapping[str, Any]]] = {}
    for component in COMPONENTS:
        entry = entries[component]
        if not isinstance(entry.get("commit"), str) or not HEX40.fullmatch(entry["commit"]):
            raise BundledResolverError("reference_commit_invalid", component)
        manifest, bundle = inventory(component, entry)
        validate_manifest(component, entry, manifest, bundle)
        selected[component] = (bundle, entry)
    target = Path(tempfile.mkdtemp(prefix=MATERIALIZED_PREFIX, dir=materialize_parent))
    target.chmod(stat.S_IRWXU)
    try:
        for component in COMPONENTS:
            bundle, entry = selected[component]
            clone_bundle(component, bundle, target / component, entry)
        preflight_reference_root(target, lock, ROOT)
        return target, len(selected)
    except Exception:
        shutil.rmtree(target, ignore_errors=True)
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--materialize-parent", type=Path, required=True)
    args = parser.parse_args()
    try:
        selected, present = materialize_bundled_reference_root(args.materialize_parent)
        write_private_path(args.output, selected)
    except BundledResolverError as exc:
        print(f"BUNDLED_REFERENCE_ROOT status=failure kind={exc.kind} component={exc.component} bundles_present=0")
        return 1
    except Exception:
        print("BUNDLED_REFERENCE_ROOT status=failure kind=preflight_or_materialization_failed component=none bundles_present=0")
        return 1
    print(f"BUNDLED_REFERENCE_ROOT status=success components={','.join(COMPONENTS)} bundles_present={present}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
'''


LIVE_TEST = '''from __future__ import annotations

import hashlib
import json
import subprocess
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RESOLVER = ROOT / "scripts/materialize_bundled_reference_root.py"
PROBE_WORKFLOW = ROOT / ".github/workflows/fe-interaction-reference-probe.yml"
BUNDLE_ROOT = ROOT / "ci/reference-bundles/stata-require"
LOCK = ROOT / "references.lock.toml"
COMPONENTS = ("reghdfe", "ftools", "require")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


class BundledReferenceRootContractTests(unittest.TestCase):
    def test_resolver_is_offline_finite_and_non_enumerating(self) -> None:
        text = RESOLVER.read_text(encoding="utf-8")
        for token in (
            'COMPONENTS = ("reghdfe", "ftools", "require")',
            'BUNDLE_ROOT = ROOT / "ci/reference-bundles/stata-require"',
            "inventory(", "validate_manifest(", "clone_bundle(",
            "preflight_reference_root(target, lock, ROOT)",
        ):
            self.assertIn(token, text)
        for forbidden in (
            "repository_candidates", "_find_unique_local_store", ".rglob(",
            ".glob(", ".iterdir(", "os.listdir", "os.scandir", "os.walk",
            "urllib", "requests", "curl", '"fetch"', "git fetch",
        ):
            self.assertNotIn(forbidden, text)

    def test_bundle_identity_and_integrity_are_exactly_verified(self) -> None:
        text = RESOLVER.read_text(encoding="utf-8")
        for token in (
            "required_capture_files", "required_capture_sha256", "bundle_sha256",
            "sha256_file(bundle)", '"capture_network_policy": "offline-only"',
            '"bundle", "list-heads"', 'f"{commit}^{{commit}}"',
            '"fsck", "--full", "--strict"',
            '"status", "--porcelain=v1", "--untracked-files=all"',
            '"remote", "set-url", "origin"',
        ):
            self.assertIn(token, text)

    def test_repository_contains_all_lock_pinned_bundles(self) -> None:
        lock = tomllib.loads(LOCK.read_text(encoding="utf-8"))
        entries = {entry["name"]: entry for entry in lock["reference"] if entry.get("name") in COMPONENTS}
        self.assertEqual(set(entries), set(COMPONENTS))
        for component in COMPONENTS:
            entry = entries[component]
            commit = entry["commit"]
            directory = BUNDLE_ROOT / component / commit
            manifest_path = directory / "manifest.json"
            bundle = directory / f"{component}-{commit}.bundle"
            self.assertTrue(manifest_path.is_file(), component)
            self.assertTrue(bundle.is_file(), component)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            self.assertEqual(manifest["schema"], 2)
            self.assertEqual(manifest["status"], "success")
            self.assertEqual(manifest["component"], component)
            self.assertEqual(manifest["source_repository"], entry["repository"])
            self.assertEqual(manifest["source_commit"], commit)
            self.assertEqual(manifest["required_files"], entry["required_capture_sha256"])
            self.assertEqual(manifest["capture_network_policy"], "offline-only")
            self.assertEqual(manifest["bundle_sha256"], sha256_file(bundle))
            completed = subprocess.run(
                ["git", "bundle", "list-heads", str(bundle)], cwd=ROOT, check=False,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
            )
            self.assertEqual(completed.returncode, 0, component)
            self.assertEqual(completed.stdout.strip(), f"{commit} refs/heads/stfixest-pin-{component}")

    def test_only_private_path_and_redacted_counts_are_emitted(self) -> None:
        text = RESOLVER.read_text(encoding="utf-8")
        self.assertIn("write_private_path(args.output, selected)", text)
        self.assertIn("bundles_present=", text)
        self.assertIn("component=", text)
        for forbidden in ("print(selected", "print(bundle", "print(destination", "repr(selected"):
            self.assertNotIn(forbidden, text)

    def test_probe_workflow_attempts_bundled_resolver_before_fallback(self) -> None:
        text = PROBE_WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("scripts/materialize_bundled_reference_root.py", text)
        self.assertIn("tests/python/test_bundled_reference_root_contract.py", text)
        self.assertIn("ci/reference-bundles/stata-require/**", text)
        self.assertIn("BUNDLED_REFERENCE_OUTCOME", text)
        self.assertIn("scripts/resolve_reference_root.py", text)
        self.assertNotIn("echo $STFIXEST_REFERENCE_ROOT", text)
        self.assertNotIn("cat $REFERENCE_ROOT_FILE", text)


if __name__ == "__main__":
    unittest.main()
'''


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise ProvisionError(f"status_patch_{label}_count_{count}")
    return text.replace(old, new, 1)


def replace_block(text: str, pattern: str, replacement: str, label: str) -> str:
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.DOTALL)
    if count != 1:
        raise ProvisionError(f"status_patch_{label}_count_{count}")
    return updated


def patch_status(path: Path, source_sha: str) -> None:
    text = path.read_text(encoding="utf-8")
    if "## Current consolidated main state\n" in text and f"triggered from `{source_sha}`" in text:
        return
    old_header = (
        "**Last updated:** 2026-08-22\n"
        "**Branch / baseline:** `main` through `2e32dc1`; the active candidate remains the `d731b327` implementation with its receipt sealed by `9e2f23e`\n"
        "**Active milestone:** complete the ten blocked OLS semantic cases, beginning with native numeric categorical FE interactions\n"
        "**Release state:** development only — not a production or cross-platform release\n"
    )
    new_header = (
        "**Last updated:** 2026-08-23\n"
        "**Branch / baseline:** consolidated `main`; last pre-handover exact-green source `8ddaa790a2c3be615eecb9d2da2d20634f91f38f`, recorded by receipt commit `7032b64b4f02711a3466298430d5f40f86c3e9bd`\n"
        "**Active milestone:** close native numeric categorical absorbed FE interactions against the exact pinned `reghdfe` reference, then reseal the frozen OLS projection\n"
        "**Release state:** development only — public estimators remain fail-closed; no production or cross-platform release claim\n"
    )
    text = replace_once(text, old_header, new_header, "header")
    current = f'''## Current consolidated main state

- Only `main` is used for StFixest development. The former categorical and OSX
  continuation lines were consolidated by `4bb5f1a38fca46b86cc7d52be9d8e013b173a0a4`;
  obsolete StFixest branches were then deleted.
- Exact-SHA quick CI for source `8ddaa790a2c3be615eecb9d2da2d20634f91f38f`
  is green: overall, licensed-Stata, and Rust statuses are `success`, Stata
  returned zero, and all 12 quick fixtures passed.
- Current main contains substantial hidden categorical-interaction support:
  collision-free compact tuple factorization; canonical partition reuse;
  final-sample topology; iterative singleton and frequency-weight behavior;
  certified MAP; prepared fit, absorbed DoF, iid, HC1, and clustered inference;
  hidden/native result transport; Stata-side hidden transport; independent
  explicit-dummy oracles; and a licensed-Stata categorical quick fixture.
- This does **not** close `multi_fe_interactions`. No successful source-bound
  pinned-`reghdfe` FE-interaction receipt has yet been accepted. The probe for
  consolidation source `4bb5f1a...` stopped before Stata because no exact
  offline reference root was available.
- The one-shot provisioning checkpoint triggered from `{source_sha}` adds
  digest-bound full-history Git bundles for the pinned `reghdfe`, `ftools`, and
  `stata-require` commits. The permanent probe must materialize those bundles
  offline and produce a successful redacted receipt before parity is claimed.
- Final-sample categorical rebasing remains staged rather than live. Its module,
  source transform, validation workflow, and prior validation receipts are
  preserved, but activation must be reconciled with `main` and rerun after the
  reference gate.
- `stfeols`, `stfepois`, public `estimate-ols`, and public `estimate-ppml`
  remain deliberately disabled.

'''
    text = replace_once(text, "## Milestone exit criteria\n", current + "## Milestone exit criteria\n", "milestone")
    text = replace_once(
        text,
        "## Completed, evidence-backed baseline\n",
        "## Historical evidence-backed baseline (pre-categorical consolidation)\n\n"
        "The rows below are retained as checkpoint-specific provenance. They describe\n"
        "the cited historical commits and are not the live current-main inventory; the\n"
        "consolidated state above and active checkpoint below control current work.\n",
        "historical",
    )
    active = '''## Active checkpoint — categorical interaction reference closure

The hidden native implementation for numeric categorical absorbed interactions,
beginning with the frozen `firm#year` case, is now substantial and passes the
normal quick lane. The remaining closure is external and differential: prove
that the exact pinned reference environment can be materialized offline, run
the licensed-Stata probe, and compare the hidden StFixest result to `reghdfe`
for sample identity, coefficient/status layout, covariance, absorbed and
residual DoF, fit statistics, singleton and weight behavior, constant handling,
interaction identity, and contracted observation outputs.

Independent explicit-dummy Rust oracles remain mandatory. A successful
`reghdfe` comparison is necessary but cannot replace independent mathematical
checks. Public dispatch stays disabled throughout this gate.

'''
    text = replace_block(text, r"## Active checkpoint — first blocked semantic case\n.*?(?=## Current handoff / resume point)", active, "active")
    handoff = '''## Current handoff / resume point

First inspect the source-bound FE-interaction receipt produced after the exact
reference bundles are committed. Accept it only when the payload is successful,
source-bound, redacted, and the probe actually reached pinned `reghdfe` in
licensed Stata. Then add the hidden-StFixest versus `reghdfe` differential
family and reseal the frozen 40-case projection. After that, reconcile and
activate final-sample categorical rebasing on `main`; do not bypass the staged
validation contract.

'''
    text = replace_block(text, r"## Current handoff / resume point\n.*?(?=## Current blockers — exact frozen manifest inventory)", handoff, "handoff")
    text = replace_once(
        text,
        "The active candidate executes 30 of the frozen 40 semantic cases. These ten\n"
        "cases remain explicit blockers:\n",
        "The historical sealed candidate executed 30 of the frozen 40 semantic cases.\n"
        "Current main adds hidden categorical-interaction implementation, but the frozen\n"
        "projection has not been resealed; these ten cases therefore remain formally open:\n",
        "blockers",
    )
    text = replace_once(
        text,
        "1. `multi_fe_interactions` — absorbed categorical interaction; next priority.\n",
        "1. `multi_fe_interactions` — hidden implementation present; pinned-`reghdfe` differential and resealed projection still required.\n",
        "first_blocker",
    )
    text = replace_once(
        text,
        "## Next evidence to record\n",
        "## Next evidence to record\n\n"
        "Current order of operations: accept the exact offline bundle/probe receipt; add\n"
        "the hidden StFixest versus pinned-`reghdfe` differential family; reseal the frozen\n"
        "projection; then activate and revalidate final-sample rebasing. Historical bullets\n"
        "below remain useful detail but do not supersede this order.\n",
        "next_evidence",
    )
    path.write_text(text, encoding="utf-8")


def write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content if content.endswith("\n") else content + "\n", encoding="utf-8")


def apply_repository(repository: Path, stage: Path, source_sha: str) -> None:
    if not repository.is_absolute() or not repository.is_dir():
        raise ProvisionError("apply_repository_invalid")
    if not stage.is_absolute() or not stage.is_dir():
        raise ProvisionError("apply_stage_invalid")
    if not HEX40.fullmatch(source_sha):
        raise ProvisionError("apply_source_sha_invalid")
    for component in COMPONENTS:
        if not (stage / component).is_dir():
            raise ProvisionError("apply_bundle_inventory_incomplete", component)
    destination = repository / "ci/reference-bundles/stata-require"
    shutil.rmtree(destination, ignore_errors=True)
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(stage, destination)
    write_text(repository / "scripts/materialize_bundled_reference_root.py", LIVE_MATERIALIZER)
    write_text(repository / "tests/python/test_bundled_reference_root_contract.py", LIVE_TEST)
    patch_status(repository / "STATUS.md", source_sha)
    write_text(
        repository / "ci/reference-probes/triggers/bundled-reference-v1.txt",
        "schema=2\ncomponents=reghdfe,ftools,require\n"
        f"provisioning_source_sha={source_sha}\n"
        "capture_network_policy=offline-only\n",
    )
    (repository / ".github/workflows/import-stata-require-bundle.yml").unlink(missing_ok=True)
    (repository / "ci/source-transforms/provision_reference_bundles.py").unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    build = commands.add_parser("build")
    build.add_argument("--output-dir", type=Path, required=True)
    apply = commands.add_parser("apply")
    apply.add_argument("--repository", type=Path, required=True)
    apply.add_argument("--bundle-stage", type=Path, required=True)
    apply.add_argument("--source-sha", required=True)
    templates = commands.add_parser("write-templates")
    templates.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "build":
            summary = build_bundles(args.output_dir)
            canonical = json.dumps(summary, sort_keys=True, separators=(",", ":")).encode()
            print(
                "REFERENCE_BUNDLE_PROVISION status=success components="
                + ",".join(COMPONENTS)
                + " summary_sha256="
                + hashlib.sha256(canonical).hexdigest()
            )
        elif args.command == "apply":
            apply_repository(args.repository, args.bundle_stage, args.source_sha)
            print("REFERENCE_BUNDLE_APPLY status=success")
        else:
            args.output_dir.mkdir(parents=True, exist_ok=True)
            write_text(args.output_dir / "materialize_bundled_reference_root.py", LIVE_MATERIALIZER)
            write_text(args.output_dir / "test_bundled_reference_root_contract.py", LIVE_TEST)
            print("REFERENCE_BUNDLE_TEMPLATES status=success")
    except ProvisionError as exc:
        print(f"REFERENCE_BUNDLE_PROVISION status=failure kind={exc.kind} component={exc.component}")
        return 1
    except Exception:
        print("REFERENCE_BUNDLE_PROVISION status=failure kind=unexpected component=none")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
