from __future__ import annotations

import copy
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "formal_rust_build_closure", ROOT / "scripts/formal_rust_build_closure.py"
)
assert SPEC is not None and SPEC.loader is not None
CLOSURE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CLOSURE
SPEC.loader.exec_module(CLOSURE)


def _run(*command: str, cwd: Path | None = None) -> str:
    completed = subprocess.run(
        list(command),
        cwd=cwd,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return completed.stdout.strip()


def _write_executable(path: Path, body: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body, encoding="utf-8")
    path.chmod(0o755)


class FormalRustBuildClosureTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.base = Path(self.temporary.name)
        self.run = self.base / "formal-run"
        self.source = self.run / "work/source"
        self.source.mkdir(parents=True)
        self.cache = self.base / "source-cache"
        self.registry_cache = self.cache / "registry/cache/index.crates.test-0000000000000000"
        self.registry_cache.mkdir(parents=True)
        self.git_checkout = self.cache / "git/checkouts/example-0000000000000000/locked"
        self.git_checkout.mkdir(parents=True)
        (self.git_checkout / "src").mkdir()
        (self.git_checkout / "Cargo.toml").write_text(
            '[package]\nname = "git-fixture"\nversion = "0.2.0"\nedition = "2021"\n',
            encoding="utf-8",
        )
        (self.git_checkout / "src/lib.rs").write_text(
            "pub fn exact() -> u64 { 7 }\n", encoding="utf-8"
        )
        _run("git", "init", "-q", cwd=self.git_checkout)
        _run("git", "config", "user.email", "fixture@example.invalid", cwd=self.git_checkout)
        _run("git", "config", "user.name", "Fixture", cwd=self.git_checkout)
        _run("git", "add", "Cargo.toml", "src/lib.rs", cwd=self.git_checkout)
        _run("git", "commit", "-q", "-m", "fixture", cwd=self.git_checkout)
        self.revision = _run("git", "rev-parse", "HEAD", cwd=self.git_checkout)
        (self.git_checkout / ".cargo-ok").write_bytes(b"")
        self.source_key = f"git+https://example.invalid/exact?rev={self.revision}"

        crate_root = self.base / "crate-source/registry-fixture-1.2.3"
        (crate_root / "src").mkdir(parents=True)
        (crate_root / "Cargo.toml").write_text(
            '[package]\nname = "registry-fixture"\nversion = "1.2.3"\nedition = "2021"\n',
            encoding="utf-8",
        )
        (crate_root / "src/lib.rs").write_text(
            "pub fn registry() -> u64 { 11 }\n", encoding="utf-8"
        )
        self.archive = self.registry_cache / "registry-fixture-1.2.3.crate"
        with tarfile.open(self.archive, "w:gz", format=tarfile.PAX_FORMAT) as destination:
            for path in sorted(crate_root.rglob("*")):
                destination.add(
                    path,
                    arcname=path.relative_to(crate_root.parent).as_posix(),
                    recursive=False,
                )
        self.registry_checksum = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        (self.source / "Cargo.toml").write_text(
            '[package]\nname = "workspace-fixture"\nversion = "0.1.0"\nedition = "2021"\n',
            encoding="utf-8",
        )
        (self.source / "Cargo.lock").write_text(
            "# exact fixture lock\n"
            "version = 4\n\n"
            "[[package]]\n"
            'name = "workspace-fixture"\n'
            'version = "0.1.0"\n\n'
            "[[package]]\n"
            'name = "registry-fixture"\n'
            'version = "1.2.3"\n'
            'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
            f'checksum = "{self.registry_checksum}"\n\n'
            "[[package]]\n"
            'name = "git-fixture"\n'
            'version = "0.2.0"\n'
            f'source = "{self.source_key}#{self.revision}"\n',
            encoding="utf-8",
        )
        self.git_vendor_template = self.base / "git-vendor-template/git-fixture-0.2.0"
        (self.git_vendor_template / "src").mkdir(parents=True)
        shutil.copy2(self.git_checkout / "Cargo.toml", self.git_vendor_template / "Cargo.toml")
        shutil.copy2(self.git_checkout / "src/lib.rs", self.git_vendor_template / "src/lib.rs")
        files = {
            relative: hashlib.sha256((self.git_vendor_template / relative).read_bytes()).hexdigest()
            for relative in ("Cargo.toml", "src/lib.rs")
        }
        (self.git_vendor_template / ".cargo-checksum.json").write_bytes(
            CLOSURE.canonical_json_bytes({"files": files, "package": None})
        )
        self.inputs = CLOSURE.SourceCacheInputs(
            cargo_source_home=self.cache,
            registry_archive_roots=(self.registry_cache,),
            git_repositories={self.source_key: self.git_checkout},
            git_executable=Path(shutil.which("git") or "/usr/bin/git"),
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _manual_vendor(self) -> Path:
        vendor = self.run / "work/manual-vendor"
        vendor.mkdir(parents=True)
        lock = CLOSURE.load_cargo_lock(self.source / "Cargo.lock")
        CLOSURE.materialize_registry_package(
            lock["registry"][0], self.archive, vendor / "registry-fixture-1.2.3"
        )
        shutil.copytree(self.git_vendor_template, vendor / "git-fixture-0.2.0")
        return vendor

    def _fake_toolchain(
        self,
    ) -> tuple[Path, Path, dict[str, Path], dict[str, Path]]:
        sysroot = self.base / "toolchain"
        bin_root = sysroot / "bin"
        rustlib = sysroot / "lib/rustlib/x86_64-unknown-linux-gnu/lib"
        rustlib.mkdir(parents=True)
        (rustlib / "libstd-fixture.rlib").write_bytes(b"exact rustlib\n")
        cargo = bin_root / "cargo"
        cargo_body = f"""#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'cargo 1.85.1 (fixture)'
  exit 0
fi
if [ "$1" = "vendor" ]; then
  for arg in "$@"; do dest="$arg"; done
  /bin/mkdir -p "$dest"
  /bin/cp -R '{self.git_vendor_template}' "$dest/git-fixture-0.2.0"
  /bin/mkdir -p "$dest/registry-fixture-1.2.3"
  echo '[source.fixture]'
  exit 0
fi
exit 3
"""
        _write_executable(cargo, cargo_body)
        rustc = bin_root / "rustc"
        _write_executable(
            rustc,
            f"""#!/bin/sh
if [ "$1" = "--print" ] && [ "$2" = "sysroot" ]; then echo '{sysroot}'; exit 0; fi
if [ "$1" = "-vV" ]; then echo 'rustc 1.85.1 (fixture)'; echo 'host: x86_64-unknown-linux-gnu'; exit 0; fi
exit 3
""",
        )
        helper_paths: dict[str, Path] = {}
        discovered = {
            "cc1": bin_root / "cc1",
            "collect2": bin_root / "collect2",
            "as": bin_root / "as",
            "ld": bin_root / "ld",
        }
        for name, path in discovered.items():
            _write_executable(path, f"#!/bin/sh\necho '{name} fixture 1.0'\n")
        c_root = self.base / "c-system"
        c_include = c_root / "include"
        c_programs = c_root / "programs"
        c_libraries = c_root / "libraries"
        for directory in (c_include, c_programs, c_libraries):
            directory.mkdir(parents=True)
        (c_include / "stddef.h").write_text("typedef unsigned long size_t;\n", encoding="ascii")
        (c_programs / "gcc-spec-helper").write_bytes(b"exact gcc helper\n")
        for name in CLOSURE.C_LINK_INPUT_NAMES:
            (c_libraries / name).write_bytes(f"exact {name}\n".encode("ascii"))
        cc = bin_root / "cc"
        _write_executable(
            cc,
            f"""#!/bin/sh
case "$1" in
  -print-prog-name=cc1) echo '{discovered['cc1']}' ;;
  -print-prog-name=collect2) echo '{discovered['collect2']}' ;;
  -print-prog-name=as) echo '{discovered['as']}' ;;
  -print-prog-name=ld) echo '{discovered['ld']}' ;;
  -dumpmachine) echo 'x86_64-unknown-linux-gnu' ;;
  -dumpfullversion) echo '12.3.0' ;;
  -print-sysroot) echo '' ;;
  -dumpspecs) echo '*fixture_specs:'; echo 'exact' ;;
  -print-search-dirs) echo 'install: {c_programs}'; echo 'programs: ={c_programs}'; echo 'libraries: ={c_libraries}' ;;
  -E) echo '#include <...> search starts here:' >&2; echo ' {c_include}' >&2; echo 'End of search list.' >&2 ;;
  -print-file-name=*) name="${{1#-print-file-name=}}"; if [ -f '{c_libraries}'/"$name" ]; then echo '{c_libraries}'/"$name"; else echo "$name"; fi ;;
  --version) echo 'cc fixture 1.0' ;;
  *) exit 3 ;;
esac
""",
        )
        for role in CLOSURE.REQUIRED_HELPER_ROLES:
            if role == "cc":
                helper_paths[role] = cc
            elif role == "linker":
                helper_paths[role] = cc
            elif role == "ar":
                helper_paths[role] = bin_root / "ar"
            elif role == "ranlib":
                helper_paths[role] = bin_root / "ranlib"
            elif role == "nm":
                helper_paths[role] = bin_root / "nm"
            elif role == "strip":
                helper_paths[role] = bin_root / "strip"
            else:
                helper_paths[role] = bin_root / "objcopy"
            if not helper_paths[role].exists():
                _write_executable(
                    helper_paths[role], f"#!/bin/sh\necho '{role} fixture 1.0'\n"
                )
        # This hostile earlier-PATH executable proves the final build PATH does
        # not simply concatenate compiler and build-tool directories.
        _write_executable(bin_root / "python3", "#!/bin/sh\necho HOSTILE-PYTHON\n")
        build_bin = self.base / "build-tools"
        build_paths: dict[str, Path] = {}
        for role in CLOSURE.REQUIRED_BUILD_TOOL_ROLES:
            if role == "bash":
                build_paths[role] = Path("/bin/bash")
            elif role == "nm":
                build_paths[role] = helper_paths["nm"]
            else:
                build_paths[role] = build_bin / role
                _write_executable(
                    build_paths[role], f"#!/bin/sh\necho '{role} fixture 1.0'\n"
                )
        _write_executable(
            build_paths["readelf"],
            f"""#!/bin/sh
case "$1" in
  --version) echo 'readelf fixture 1.0' ;;
  --program-headers) echo '      [Requesting program interpreter: {c_libraries / 'ld-linux-x86-64.so.2'}]' ;;
  --dynamic) echo ' 0x0000000000000001 (NEEDED) Shared library: [libc.so.6]' ;;
  *) exit 3 ;;
esac
""",
        )
        _write_executable(
            build_paths["ldd"],
            f"""#!/bin/sh
if [ "$1" = "--version" ]; then echo 'ldd fixture 1.0'; exit 0; fi
echo 'libc.so.6 => {c_libraries / 'libc.so.6'} (0x00000001)'
echo '{c_libraries / 'ld-linux-x86-64.so.2'} (0x00000002)'
""",
        )
        return cargo, rustc, helper_paths, build_paths

    def _full_receipt(self) -> tuple[dict, Path, dict[str, Path]]:
        cargo, rustc, helpers, build_tools = self._fake_toolchain()
        closure_root = self.run / "work/build-closure"
        target = self.run / "work/target"
        receipt = CLOSURE.prepare_formal_rust_build_closure(
            self.run,
            self.source,
            closure_root,
            target,
            self.inputs,
            cargo=cargo,
            rustc=rustc,
            helpers=helpers,
            build_tools=build_tools,
            slots=16,
            build_variables={
                "FEREG_GIT_COMMIT": "0123456789ab",
                "FEREG_BUILD_DATE": "2026-08-31",
            },
        )
        return receipt, rustc.parent.parent, helpers

    def test_exact_registry_and_git_vendor_validate(self) -> None:
        result = CLOSURE.validate_dependency_vendor(
            self.source / "Cargo.lock",
            self._manual_vendor(),
            self.inputs,
            enforce_cargo_cache_checkout=False,
        )
        self.assertEqual([item["vendor_name"] for item in result["registry_packages"]], ["registry-fixture-1.2.3"])
        self.assertEqual([item["revision"] for item in result["git_packages"]], [self.revision])

    def test_repository_lock_has_the_frozen_cmg_and_within_revisions(self) -> None:
        lock = CLOSURE.load_cargo_lock(ROOT / "Cargo.lock")
        revisions = {
            item["repository_url"]: item["revision"] for item in lock["git"]
        }
        self.assertEqual(
            revisions,
            {
                "https://github.com/johannes-schmieder/CMG": (
                    "90e1fe0b0c14065155532711246ede6678bb4935"
                ),
                "https://github.com/py-econometrics/within": (
                    "b7779cbab7a3116be56aae4389fde1f6e6a99a9f"
                ),
            },
        )
        self.assertEqual(len(lock["registry"]), 51)
        self.assertEqual(len(lock["git"]), 3)

    def test_registry_archive_and_checksum_substitution_fail_closed(self) -> None:
        vendor = self._manual_vendor()
        original = self.archive.read_bytes()
        self.archive.write_bytes(original + b"substitution")
        with self.assertRaisesRegex(CLOSURE.ClosureError, "does not match Cargo.lock"):
            CLOSURE.validate_dependency_vendor(
                self.source / "Cargo.lock", vendor, self.inputs, enforce_cargo_cache_checkout=False
            )
        self.archive.write_bytes(original)
        checksum_path = vendor / "registry-fixture-1.2.3/.cargo-checksum.json"
        checksum = json.loads(checksum_path.read_text(encoding="utf-8"))
        checksum["package"] = "0" * 64
        checksum_path.write_bytes(CLOSURE.canonical_json_bytes(checksum))
        with self.assertRaisesRegex(CLOSURE.ClosureError, "package checksum changed"):
            CLOSURE.validate_dependency_vendor(
                self.source / "Cargo.lock", vendor, self.inputs, enforce_cargo_cache_checkout=False
            )

    def test_dirty_git_and_vendored_git_substitution_fail_closed(self) -> None:
        vendor = self._manual_vendor()
        (self.git_checkout / "src/lib.rs").write_text("pub fn altered() {}\n", encoding="utf-8")
        with self.assertRaisesRegex(CLOSURE.ClosureError, "not clean"):
            CLOSURE.validate_dependency_vendor(
                self.source / "Cargo.lock", vendor, self.inputs, enforce_cargo_cache_checkout=False
            )
        _run("git", "checkout", "--", "src/lib.rs", cwd=self.git_checkout)
        changed = vendor / "git-fixture-0.2.0/src/lib.rs"
        changed.write_text("pub fn substituted() {}\n", encoding="utf-8")
        checksum_path = vendor / "git-fixture-0.2.0/.cargo-checksum.json"
        checksum = json.loads(checksum_path.read_text(encoding="utf-8"))
        checksum["files"]["src/lib.rs"] = hashlib.sha256(changed.read_bytes()).hexdigest()
        checksum_path.write_bytes(CLOSURE.canonical_json_bytes(checksum))
        with self.assertRaisesRegex(CLOSURE.ClosureError, "locked worktree"):
            CLOSURE.validate_dependency_vendor(
                self.source / "Cargo.lock", vendor, self.inputs, enforce_cargo_cache_checkout=False
            )

    def test_git_replacement_refs_and_grafts_fail_closed(self) -> None:
        vendor = self._manual_vendor()
        (self.git_checkout / "src/lib.rs").write_text(
            "pub fn replacement() -> u64 { 99 }\n", encoding="utf-8"
        )
        _run("git", "add", "src/lib.rs", cwd=self.git_checkout)
        _run("git", "commit", "-q", "-m", "replacement", cwd=self.git_checkout)
        replacement = _run("git", "rev-parse", "HEAD", cwd=self.git_checkout)
        _run("git", "checkout", "-q", self.revision, cwd=self.git_checkout)
        _run("git", "replace", self.revision, replacement, cwd=self.git_checkout)
        with self.assertRaisesRegex(CLOSURE.ClosureError, "replacement refs"):
            CLOSURE.validate_dependency_vendor(
                self.source / "Cargo.lock", vendor, self.inputs, enforce_cargo_cache_checkout=False
            )
        _run("git", "pack-refs", "--all", "--prune", cwd=self.git_checkout)
        loose_replace_root = self.git_checkout / ".git/refs/replace"
        if loose_replace_root.is_dir():
            loose_replace_root.rmdir()
        with self.assertRaisesRegex(CLOSURE.ClosureError, "packed replacement refs"):
            CLOSURE.validate_dependency_vendor(
                self.source / "Cargo.lock", vendor, self.inputs, enforce_cargo_cache_checkout=False
            )
        _run("git", "replace", "-d", self.revision, cwd=self.git_checkout)
        loose_replace_root = self.git_checkout / ".git/refs/replace"
        if loose_replace_root.is_dir():
            loose_replace_root.rmdir()
        graft = self.git_checkout / ".git/info/grafts"
        graft.write_text(f"{self.revision} {replacement}\n", encoding="ascii")
        with self.assertRaisesRegex(CLOSURE.ClosureError, "graft file"):
            CLOSURE.validate_dependency_vendor(
                self.source / "Cargo.lock", vendor, self.inputs, enforce_cargo_cache_checkout=False
            )
        graft.unlink()

    def test_actual_toolchain_required_and_helper_rustlib_mutations_fail(self) -> None:
        cargo, rustc, helpers, build_tools = self._fake_toolchain()
        receipt = CLOSURE.capture_toolchain(
            self.base / "probe",
            cargo=cargo,
            rustc=rustc,
            helpers=helpers,
            build_tools=build_tools,
        )
        CLOSURE.verify_toolchain_receipt(receipt, portable=False)
        self.assertTrue(
            receipt["base_image_trust_boundary"]["covered_by_this_receipt"]
        )
        self.assertEqual(
            receipt["base_image_trust_boundary"]["required_post_build_receipt"],
            CLOSURE.LINUX_RUNTIME_RECEIPT_TYPE,
        )
        self.assertEqual(receipt["build_tools"]["bash"]["path"], "/bin/bash")
        self.assertEqual(
            Path(receipt["tool_bin"]["path"], "python3").read_bytes(),
            CLOSURE._tool_wrapper_bytes(str(build_tools["python3"].resolve())),
        )
        self.assertNotIn(
            str((cargo.parent / "python3").resolve()).encode("utf-8"),
            Path(receipt["tool_bin"]["path"], "python3").read_bytes(),
        )
        wrapped_python = subprocess.run(
            [str(Path(receipt["tool_bin"]["path"]) / "python3"), "--version"],
            env={"PATH": receipt["tool_bin"]["path"]},
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        ).stdout.strip()
        self.assertEqual(wrapped_python, "python3 fixture 1.0")
        tampered_bash = copy.deepcopy(receipt)
        tampered_bash["build_tools"]["bash"]["sha256"] = "0" * 64
        with self.assertRaisesRegex(CLOSURE.ClosureError, "executable changed"):
            CLOSURE.verify_toolchain_receipt(tampered_bash, portable=False)
        helpers["ar"].write_text("#!/bin/sh\necho changed\n", encoding="utf-8")
        helpers["ar"].chmod(0o755)
        with self.assertRaisesRegex(CLOSURE.ClosureError, "executable changed"):
            CLOSURE.verify_toolchain_receipt(receipt, portable=False)
        helpers["ar"].write_text("#!/bin/sh\necho 'ar fixture 1.0'\n", encoding="utf-8")
        helpers["ar"].chmod(0o755)
        build_tools["python3"].write_text(
            "#!/bin/sh\necho substituted-python\n", encoding="utf-8"
        )
        build_tools["python3"].chmod(0o755)
        with self.assertRaisesRegex(CLOSURE.ClosureError, "executable changed"):
            CLOSURE.verify_toolchain_receipt(receipt, portable=False)
        build_tools["python3"].write_text(
            "#!/bin/sh\necho 'python3 fixture 1.0'\n", encoding="utf-8"
        )
        build_tools["python3"].chmod(0o755)
        include_header = Path(
            receipt["c_system_closure"]["include_roots"][0]["path"]
        ) / "stddef.h"
        include_original = include_header.read_bytes()
        include_header.write_bytes(b"hostile header\n")
        with self.assertRaisesRegex(CLOSURE.ClosureError, "GCC include root"):
            CLOSURE.verify_toolchain_receipt(receipt, portable=False)
        include_header.write_bytes(include_original)
        libc_record = next(
            item
            for item in receipt["c_system_closure"]["link_inputs"]
            if item["name"] == "libc.so"
        )
        libc_path = Path(libc_record["file"]["path"])
        libc_original = libc_path.read_bytes()
        libc_path.write_bytes(b"hostile libc linker input\n")
        with self.assertRaisesRegex(CLOSURE.ClosureError, "GCC libraries root"):
            CLOSURE.verify_toolchain_receipt(receipt, portable=False)
        libc_path.write_bytes(libc_original)
        rustlib = Path(receipt["rustlib"]["path"]) / "x86_64-unknown-linux-gnu/lib/libstd-fixture.rlib"
        rustlib.write_bytes(b"mutated rustlib\n")
        with self.assertRaisesRegex(CLOSURE.ClosureError, "Rust sysroot changed"):
            CLOSURE.verify_toolchain_receipt(receipt, portable=False)
        proxy = self.base / "proxy/cargo"
        _write_executable(proxy, f"#!/bin/sh\nexec '{cargo}' \"$@\"\n")
        with self.assertRaisesRegex(CLOSURE.ClosureError, "not rustup proxies"):
            CLOSURE.capture_toolchain(
                self.base / "proxy-probe",
                cargo=proxy,
                rustc=rustc,
                helpers=helpers,
                build_tools=build_tools,
            )

    def test_complete_environment_and_portable_replay_are_closed(self) -> None:
        hostile = {
            "CARGO_HOME": os.environ.get("CARGO_HOME"),
            "RUSTFLAGS": os.environ.get("RUSTFLAGS"),
            "CC": os.environ.get("CC"),
        }
        try:
            os.environ["CARGO_HOME"] = "/hostile/cargo"
            os.environ["RUSTFLAGS"] = "-C link-arg=/hostile"
            os.environ["CC"] = "/hostile/cc"
            receipt, _sysroot, _helpers = self._full_receipt()
        finally:
            for key, value in hostile.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value
        self.assertNotIn("RUSTFLAGS", receipt["environment"])
        self.assertEqual(receipt["environment"]["CARGO_NET_OFFLINE"], "true")
        self.assertEqual(receipt["environment"]["CARGO_BUILD_JOBS"], "16")
        CLOSURE.verify_formal_rust_build_closure(
            self.run, receipt, portable=False, cache_inputs=self.inputs
        )
        collection_parent = self.base / "collection"
        collection_parent.mkdir()
        collected = collection_parent / self.run.name
        shutil.copytree(self.run, collected, symlinks=True)
        CLOSURE.verify_formal_rust_build_closure(collected, receipt, portable=True)
        config = collected / receipt["dependencies"]["config"]["path"]
        config.chmod(0o644)
        config.write_text("[net]\noffline = false\n", encoding="utf-8")
        with self.assertRaises(CLOSURE.ClosureError):
            CLOSURE.verify_formal_rust_build_closure(collected, receipt, portable=True)

    def test_portable_vendor_mutation_fails_without_external_cache(self) -> None:
        receipt, _sysroot, _helpers = self._full_receipt()
        collected = self.base / "portable" / self.run.name
        collected.parent.mkdir()
        shutil.copytree(self.run, collected, symlinks=True)
        victim = collected / receipt["dependencies"]["vendor"]["path"] / "git-fixture-0.2.0/src/lib.rs"
        victim.chmod(0o644)
        victim.write_bytes(b"portable substitution\n")
        with self.assertRaisesRegex(CLOSURE.ClosureError, "retained vendor tree changed"):
            CLOSURE.verify_formal_rust_build_closure(collected, receipt, portable=True)

    def test_portable_parent_symlinks_fail_closed(self) -> None:
        receipt, _sysroot, _helpers = self._full_receipt()
        cases = ("work", "cargo-home", "vendor")
        for case in cases:
            with self.subTest(case=case):
                collection_parent = self.base / f"parent-link-{case}"
                collection_parent.mkdir()
                collected = collection_parent / self.run.name
                shutil.copytree(self.run, collected, symlinks=True)
                if case == "work":
                    victim = collected / "work"
                elif case == "cargo-home":
                    victim = collected / receipt["closure_root"] / "cargo-home"
                else:
                    victim = collected / receipt["dependencies"]["vendor"]["path"]
                outside = collection_parent / f"outside-{case}"
                shutil.copytree(victim, outside, symlinks=True)
                for directory in [victim, *victim.rglob("*")]:
                    if directory.is_dir() and not directory.is_symlink():
                        directory.chmod(0o755)
                shutil.rmtree(victim)
                os.symlink(outside, victim)
                with self.assertRaisesRegex(
                    CLOSURE.ClosureError, "symlinked path component"
                ):
                    CLOSURE.verify_formal_rust_build_closure(
                        collected, receipt, portable=True
                    )

    def test_post_build_linux_runtime_closure_local_and_portable(self) -> None:
        receipt, _sysroot, _helpers = self._full_receipt()
        artifact = self.run / "work/artifacts/fereg_linux.plugin"
        artifact.parent.mkdir()
        artifact.write_bytes(b"synthetic ELF artifact bytes\n")
        runtime = CLOSURE.capture_linux_runtime_closure(
            self.run, artifact, receipt["toolchain"]
        )
        self.assertEqual(runtime["needed"], ["libc.so.6"])
        self.assertTrue(
            any(
                Path(item["file"]["path"]).name == "ld-linux-x86-64.so.2"
                for item in runtime["runtime_libraries"]
            )
        )
        CLOSURE.verify_linux_runtime_closure(
            self.run, runtime, receipt["toolchain"], portable=False
        )
        qualified = CLOSURE.verify_qualified_formal_rust_build(
            self.run,
            receipt,
            runtime,
            portable=False,
            cache_inputs=self.inputs,
        )
        self.assertEqual(qualified["status"], "complete")
        collected = self.base / "runtime-collection" / self.run.name
        collected.parent.mkdir()
        shutil.copytree(self.run, collected, symlinks=True)
        CLOSURE.verify_linux_runtime_closure(
            collected, runtime, receipt["toolchain"], portable=True
        )
        collected_artifact = collected / runtime["artifact"]["path"]
        collected_artifact.write_bytes(b"substituted artifact\n")
        with self.assertRaisesRegex(CLOSURE.ClosureError, "runtime artifact bytes changed"):
            CLOSURE.verify_linux_runtime_closure(
                collected, runtime, receipt["toolchain"], portable=True
            )
        with self.assertRaisesRegex(CLOSURE.ClosureError, "unresolved runtime input"):
            CLOSURE._parse_elf_runtime(
                "[Requesting program interpreter: /lib64/ld-linux-x86-64.so.2]",
                "(NEEDED) Shared library: [missing.so]",
                "missing.so => not found",
            )


if __name__ == "__main__":
    unittest.main()
