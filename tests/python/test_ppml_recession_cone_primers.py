"""Exact-rational recession-cone check of the shared PPML separation primers.

The same primers drive the public native fit in
crates/fereg-oracle/tests/ppml_separation_primers.rs. Here the dependency-free
oracle in tests/oracles certifies each primer's first-round separated rows over
the homogeneous span of the intercept, regressors and every FE level dummy.
"""
from __future__ import annotations

import sys
import unittest
from fractions import Fraction
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tests' / 'oracles'))
from ppml_recession_cone_oracle import evaluate_direction, separated_rows  # noqa: E402

PRIMERS = ROOT / 'tests' / 'fixtures' / 'ppml' / 'separation_primers.txt'


def load_primers() -> list[dict]:
    primers: list[dict] = []
    for raw in PRIMERS.read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith('#'):
            continue
        if line.startswith('[') and line.endswith(']'):
            primers.append({'name': line[1:-1], 'regressor': [], 'effect': []})
            continue
        key, value = (part.strip() for part in line.split('=', 1))
        if key in ('regressor', 'effect'):
            primers[-1][key].append(value.split())
        else:
            primers[-1][key] = value.split()
    return primers


def design(primer: dict) -> list[list[Fraction]]:
    rows = len(primer['outcome'])
    columns = [[Fraction(1)] * rows]
    columns += [[Fraction(value) for value in regressor] for regressor in primer['regressor']]
    for effect in primer['effect']:
        codes = [int(code) for code in effect]
        columns += [[Fraction(int(code == level)) for code in codes] for level in range(max(codes) + 1)]
    return [[column[row] for column in columns] for row in range(rows)]


class RecessionConePrimerTests(unittest.TestCase):
    def test_first_round_separated_rows_match_each_primer(self) -> None:
        primers = load_primers()
        self.assertGreaterEqual(len(primers), 6)
        for primer in primers:
            with self.subTest(primer=primer['name']):
                matrix = design(primer)
                outcome = [Fraction(value) for value in primer['outcome']]
                rows, witnesses = separated_rows(matrix, outcome)
                self.assertEqual(list(rows), [int(row) for row in primer['cone']])
                for row, witness in witnesses.items():
                    values = evaluate_direction(matrix, witness)
                    self.assertLess(values[row], 0)
                    for index, (value, y) in enumerate(zip(values, outcome)):
                        if y > 0:
                            self.assertEqual(value, 0, f'row {index}')
                        else:
                            self.assertLessEqual(value, 0, f'row {index}')

    def test_cone_rows_are_a_subset_of_native_drops(self) -> None:
        for primer in load_primers():
            with self.subTest(primer=primer['name']):
                self.assertTrue(set(primer['cone']) <= set(primer['dropped']))


if __name__ == '__main__':
    unittest.main()
