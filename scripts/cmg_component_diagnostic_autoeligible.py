#!/usr/bin/env python3
"""Run the CMG component diagnostic at the current auto-CMG shape boundary."""

from pathlib import Path

import cmg_component_diagnostic as diagnostic


diagnostic.N = 99_000
diagnostic.WORKERS = 33_000
diagnostic.FIRMS = 3_300
diagnostic.PERIODS = 3
diagnostic.SEED = 20260902
diagnostic.BALANCED_COMPONENT_COUNTS = (1, 10, 100, 1_100)
diagnostic.MATRIX_MIXED_COMPONENT_COUNTS = (10, 100, 1_000)
diagnostic.SWEEP_MIXED_COMPONENT_COUNTS = (200, 400, 600, 800, 1_100)
diagnostic.MATRIX_CONTROLS = (0, 4)
diagnostic.INCLUDE_THREAD_SUBSET = False
diagnostic.DEFAULT_OUTPUT = (
    Path(__file__).resolve().parents[1]
    / "benchmarks/reports/development/cmg-component-autoeligible-v1"
)


if __name__ == "__main__":
    raise SystemExit(diagnostic.main())
