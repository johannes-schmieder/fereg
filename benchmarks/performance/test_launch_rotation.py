import argparse
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from launch_rotation import prepare


class LaunchRotationTests(unittest.TestCase):
    def fixture(self, root, attempt="rotation03"):
        (root / "diagnostic02").mkdir(exist_ok=True)
        (root / "diagnostic02/complete.json").write_text('{"all_fits_passed": true}')
        plugins = {}
        for name in ("baseline", "candidate"):
            source = root / name
            (source / "dist").mkdir(parents=True, exist_ok=True)
            (source / "core.rs").write_text(name)
            (source / "dist/fereg_linux.plugin").write_text(name)
            digest = hashlib.sha256(name.encode()).hexdigest()
            manifest = "source-manifest.json" if name == "baseline" else "candidate01-source.json"
            (root / manifest).write_text(json.dumps({"files": {"core.rs": digest}}))
            plugins[name] = digest
        ref = root / "reference"
        for directory in ("env", "input", "references"):
            (ref / directory).mkdir(parents=True, exist_ok=True)
        (ref / "env/frozen.json").write_text("{}")
        (ref / "input/manifest.json").write_text("{}")
        (root / f"{attempt}-registration.json").write_text(json.dumps({"plugins": plugins}))
        harness = root / f"{attempt}-harness/performance"
        harness.mkdir(parents=True)
        (harness / "rotations.py").touch()
        return argparse.Namespace(run_root=root, attempt=attempt, baseline=root / "baseline",
            candidate=root / "candidate", reference=ref, scratch=root / f"{attempt}-scratch")

    def test_fresh_paths_and_check_only_match_real_launch(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            args = self.fixture(root)
            (root / "rotations01").mkdir()
            (root / "rotation02-roots").mkdir()
            before = set(root.iterdir())
            command = prepare(args, check_only=True)
            self.assertEqual(set(root.iterdir()), before)
            self.assertEqual(command[command.index("--output") + 1], str(root / "rotation03-results"))
            self.assertEqual(command, prepare(args))
            self.assertEqual((root / "rotation03-roots/candidate/candidate").resolve(), args.candidate)
            self.assertFalse((root / "rotation03-results").exists())
            with self.assertRaises(FileExistsError):
                prepare(args)

    def test_existing_output_rejected_before_any_mutation(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            args = self.fixture(root)
            (root / "rotation03-results").mkdir()
            with self.assertRaises(FileExistsError):
                prepare(args)
            self.assertFalse((root / "rotation03-plan.json").exists())
            self.assertFalse(args.scratch.exists())

    def test_identity_mismatch_and_failed_diagnostics_reject(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            args = self.fixture(root)
            (args.candidate / "core.rs").write_text("changed")
            with self.assertRaisesRegex(ValueError, "source changed"):
                prepare(args)
            (args.candidate / "core.rs").write_text("candidate")
            (root / "diagnostic02/complete.json").write_text('{"all_fits_passed": false}')
            with self.assertRaisesRegex(ValueError, "adjudication"):
                prepare(args)
            self.assertFalse((root / "rotation03-plan.json").exists())


if __name__ == "__main__":
    unittest.main()
