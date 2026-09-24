import importlib.util
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / "benchmarks/scc/paper_akm_runner.py"


def load_runner():
    spec = importlib.util.spec_from_file_location("paper_akm_runner", RUNNER)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class PaperAkmScalingMemoryContract(unittest.TestCase):
    def test_large_single_cell_mode_is_named_benchmark_canary(self):
        source = RUNNER.read_text(encoding="utf-8")
        self.assertIn('choices=("benchmark-canary", "production")', source)
        self.assertNotIn('mode == "smoke"', source)

    def test_cell_matrix_has_expected_coverage(self):
        runner = load_runner()
        cells = runner.measurement_cells()
        self.assertEqual(len(cells), 23)
        self.assertEqual({arm for _, _, arm in cells}, {"fereg", "reghdfe", "areg"})
        self.assertEqual(
            {threads for n, threads, arm in cells if n == 8_000_000 and arm == "fereg"},
            {1, 2, 4, 8, 16},
        )
        for arm in ("reghdfe", "areg"):
            self.assertEqual(
                {threads for n, threads, cell_arm in cells if n == 8_000_000 and cell_arm == arm},
                {1, 2, 4},
            )

    def test_preflights_cover_every_fereg_n_thread_cell(self):
        runner = load_runner()
        measured = {(n, threads) for n, threads, arm in runner.measurement_cells() if arm == "fereg"}
        self.assertEqual(set(runner.preflight_cells()), measured)

    def test_stata_measurement_uses_one_command_and_external_sentinels(self):
        source = (ROOT / "benchmarks/stata/paper_akm_measure.do").read_text(encoding="utf-8")
        self.assertIn("sleep 1000", source)
        self.assertIn("/READY", source)
        self.assertIn("/START", source)
        self.assertIn("/DONE", source)
        self.assertIn("threads(`requested_threads')", source)
        self.assertIn("dfabsorb noabsorbtest", source.replace("///\n        ", ""))

if __name__ == "__main__":
    unittest.main()
