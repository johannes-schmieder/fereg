from __future__ import annotations

import copy
import hashlib
import io
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import ppml_reference_closure as closure  # noqa: E402


def _entry(name: str, payload: bytes, *, commit_digit: str) -> dict[str, object]:
    relative = f"src/{name}.ado"
    entry: dict[str, object] = {
        "name": name,
        "commit": commit_digit * 40,
        "git_tree": ("f" if commit_digit != "f" else "e") * 40,
        "pin_status": "resolved",
        "checkout_dir": name,
        "capture_ado_subdir": "src",
        "required_capture_files": [relative],
        "required_capture_sha256": {relative: hashlib.sha256(payload).hexdigest()},
        "source_archive_format": "tar.gz" if name == "ppmlhdfe" else "git-archive-tar",
        "source_archive_scope": "src",
        "source_archive_size_bytes": 1,
        "source_archive_sha256": commit_digit * 64,
    }
    return entry


def _tar_gz(path: Path, members: list[tuple[str, bytes]]) -> None:
    with tarfile.open(path, "w:gz", format=tarfile.PAX_FORMAT) as archive:
        roots = sorted({name.split("/", 1)[0] for name, _ in members})
        for root in roots:
            info = tarfile.TarInfo(root + "/")
            info.type = tarfile.DIRTYPE
            info.mode = 0o755
            info.mtime = 0
            archive.addfile(info)
        for name, payload in members:
            info = tarfile.TarInfo(name)
            info.size = len(payload)
            info.mode = 0o644
            info.mtime = 0
            archive.addfile(info, io.BytesIO(payload))


class PpmlReferenceClosureTests(unittest.TestCase):
    def test_git_reference_archive_executes_the_exact_pinned_commit(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            checkout = Path(raw) / "reghdfe"
            source = checkout / "src"
            source.mkdir(parents=True)
            payload = b"program reghdfe\nend\n"
            (source / "reghdfe.ado").write_bytes(payload)
            subprocess.run(["git", "init", "-q", str(checkout)], check=True)
            subprocess.run(
                ["git", "-C", str(checkout), "add", "src/reghdfe.ado"],
                check=True,
            )
            subprocess.run(
                [
                    "git", "-C", str(checkout),
                    "-c", "user.name=qualification-test",
                    "-c", "user.email=qualification-test@example.invalid",
                    "commit", "-q", "-m", "fixture",
                ],
                check=True,
            )
            commit = subprocess.check_output(
                ["git", "-C", str(checkout), "rev-parse", "HEAD"], text=True
            ).strip()
            tree = subprocess.check_output(
                ["git", "-C", str(checkout), "rev-parse", "HEAD^{tree}"], text=True
            ).strip()
            identity = closure.ReferenceIdentity(
                name="reghdfe",
                commit=commit,
                tree=tree,
                checkout=checkout,
                ado_dir=source,
            )

            archive = closure._git_archive(
                identity,
                {"capture_ado_subdir": "src", "git_tree": tree},
            )

            with tarfile.open(fileobj=io.BytesIO(archive), mode="r:") as handle:
                member = handle.extractfile("src/reghdfe.ado")
                self.assertIsNotNone(member)
                self.assertEqual(member.read(), payload)

            entry = _entry("reghdfe", payload, commit_digit="a")
            entry.update(
                {
                    "commit": commit,
                    "git_tree": tree,
                    "source_archive_sha256": hashlib.sha256(archive).hexdigest(),
                    "source_archive_size_bytes": len(archive),
                }
            )
            closure._stage_git_reference(identity, entry, Path(raw) / "staged")
            wrong_tree = copy.deepcopy(entry)
            wrong_tree["git_tree"] = "0" * 40
            with self.assertRaisesRegex(closure.ClosureError, "git_tree_mismatch"):
                closure._stage_git_reference(
                    identity, wrong_tree, Path(raw) / "wrong-tree"
                )
            changed = copy.deepcopy(entry)
            changed["source_archive_sha256"] = "f" * 64
            with self.assertRaisesRegex(
                closure.ClosureError, "git_archive_digest_mismatch"
            ):
                closure._stage_git_reference(
                    identity, changed, Path(raw) / "must-not-stage"
                )

    def test_repository_lock_closes_all_four_reference_identities(self) -> None:
        lock = tomllib.loads((ROOT / "references.lock.toml").read_text(encoding="utf-8"))
        entries = closure.locked_entries(lock)
        self.assertEqual(set(entries), set(closure.COMPONENTS))
        self.assertEqual(
            entries["ppmlhdfe"]["source_archive_sha256"],
            "366ddb71ebbe53bb4d7e0d4066e0a419ec6f0b689d0004be2da23e0c295d953e",
        )
        self.assertEqual(entries["ppmlhdfe"]["source_archive_size_bytes"], 5_079_754)
        self.assertEqual(len(entries["ppmlhdfe"]["required_capture_files"]), 10)
        expected_archives = {
            "reghdfe": (
                "b8c2c5dcfa23fe59ff1ca72d2f7fedf507763757ea2ad52fe301a5fe686a01bd",
                768_000,
            ),
            "ftools": (
                "d112fc4b662945c1f76c8462a0d6c904ddc336813f24fb8c83c3aad3a0f49680",
                256_000,
            ),
            "require": (
                "d190c105f703b12d696b9aa426cd70c6c05dbd7102e62a1fcf59f0ae30616cf1",
                51_200,
            ),
        }
        for name, (digest, size) in expected_archives.items():
            self.assertEqual(entries[name]["source_archive_sha256"], digest)
            self.assertEqual(entries[name]["source_archive_size_bytes"], size)
        for entry in entries.values():
            self.assertRegex(str(entry["git_tree"]), r"^[0-9a-f]{40}$")
            self.assertRegex(str(entry["source_archive_sha256"]), r"^[0-9a-f]{64}$")
            self.assertGreater(entry["source_archive_size_bytes"], 0)

    def test_ppml_archive_golden_stage_and_tamper_rejection(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            temporary = Path(raw)
            payload = b"program ppmlhdfe\nend\n"
            entry = _entry("ppmlhdfe", payload, commit_digit="a")
            archive = temporary / "ppmlhdfe.tar.gz"
            root = f"ppmlhdfe-{entry['commit']}"
            _tar_gz(archive, [(f"{root}/src/ppmlhdfe.ado", payload)])
            entry.update(
                {
                    "source_archive_root": root,
                    "source_archive_size_bytes": archive.stat().st_size,
                    "source_archive_sha256": closure.sha256_file(archive),
                }
            )

            identity = closure.stage_ppmlhdfe_archive(
                archive, entry, temporary / "staged"
            )
            self.assertEqual(identity.sha256, entry["source_archive_sha256"])
            self.assertEqual(
                (temporary / "staged/src/ppmlhdfe.ado").read_bytes(), payload
            )

            tampered = temporary / "tampered.tar.gz"
            tampered.write_bytes(archive.read_bytes() + b"tamper")
            with self.assertRaisesRegex(
                closure.ClosureError, "ppmlhdfe_archive_digest_mismatch"
            ):
                closure.stage_ppmlhdfe_archive(
                    tampered, entry, temporary / "must-not-stage"
                )

    def test_archive_traversal_is_rejected_even_when_digest_is_locked(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            temporary = Path(raw)
            payload = b"safe"
            entry = _entry("ppmlhdfe", payload, commit_digit="b")
            archive = temporary / "malicious.tar.gz"
            root = f"ppmlhdfe-{entry['commit']}"
            _tar_gz(
                archive,
                [
                    (f"{root}/src/ppmlhdfe.ado", payload),
                    (f"{root}/../escape", b"unsafe"),
                ],
            )
            entry.update(
                {
                    "source_archive_root": root,
                    "source_archive_size_bytes": archive.stat().st_size,
                    "source_archive_sha256": closure.sha256_file(archive),
                }
            )
            with self.assertRaisesRegex(closure.ClosureError, "archive_path_unsafe"):
                closure.stage_ppmlhdfe_archive(
                    archive, entry, temporary / "must-not-stage"
                )
            self.assertFalse((temporary / "escape").exists())

    def test_bundle_and_receipt_are_deterministic_and_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            temporary = Path(raw)
            runtime = temporary / "runtime"
            entries: dict[str, dict[str, object]] = {}
            sources: dict[str, closure.SourceArchive] = {}
            for index, name in enumerate(closure.COMPONENTS, start=1):
                payload = f"program {name}\nend\n".encode("ascii")
                entry = _entry(name, payload, commit_digit=str(index))
                component = runtime / name / "src"
                component.mkdir(parents=True)
                (component / f"{name}.ado").write_bytes(payload)
                entries[name] = entry
                sources[name] = closure.SourceArchive(str(index) * 64, index * 10)
                entry.update(
                    {
                        "source_archive_sha256": sources[name].sha256,
                        "source_archive_size_bytes": sources[name].size_bytes,
                    }
                )
            entries["ppmlhdfe"].update(
                {
                    "source_archive_sha256": sources["ppmlhdfe"].sha256,
                    "source_archive_size_bytes": sources["ppmlhdfe"].size_bytes,
                }
            )

            first = temporary / "first.tar"
            second = temporary / "second.tar"
            first_identity = closure.write_deterministic_bundle(runtime, first)
            second_identity = closure.write_deterministic_bundle(runtime, second)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            self.assertEqual(first_identity, second_identity)

            lock_sha = "a" * 64
            document = closure._receipt_document(
                lock_sha256=lock_sha,
                entries=entries,
                sources=sources,
                bundle=first_identity,
            )
            comparator = closure.verify_receipt_document(
                document, first, lock_sha, entries
            )
            self.assertEqual(comparator["bundle_sha256"], first_identity.sha256)

            extracted = temporary / "extracted"
            closure._extract_archive_bytes(
                first.read_bytes(), extracted, allowed_roots=set(closure.COMPONENTS)
            )
            closure.verify_runtime_root(extracted, entries)

            shadow = extracted / "ppmlhdfe/src/ppmlhdfe_shadow.ado"
            shadow.write_text("program ppmlhdfe_shadow\nend\n", encoding="ascii")
            with self.assertRaisesRegex(
                closure.ClosureError, "runtime_file_inventory_invalid"
            ):
                closure.verify_runtime_root(extracted, entries)
            shadow.unlink()

            changed = copy.deepcopy(document)
            changed["payload"]["comparator"]["references"]["require"]["tree"] = "0" * 40
            with self.assertRaisesRegex(closure.ClosureError, "receipt_seal_mismatch"):
                closure.verify_receipt_document(changed, first, lock_sha, entries)

            first.chmod(0o600)
            data = bytearray(first.read_bytes())
            data[-1] ^= 1
            first.write_bytes(data)
            with self.assertRaisesRegex(closure.ClosureError, "bundle_identity_mismatch"):
                closure.verify_receipt_document(document, first, lock_sha, entries)


if __name__ == "__main__":
    unittest.main()
