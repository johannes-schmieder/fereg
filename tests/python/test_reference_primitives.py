from __future__ import annotations

import math
import random
import unittest


def factorize(values):
    levels = sorted(set(values))
    index = {value: position for position, value in enumerate(levels)}
    return [index[value] for value in values], levels


def factorize_float(values):
    normalized = []
    for value in values:
        if not math.isfinite(value):
            raise ValueError("nonfinite")
        normalized.append(0.0 if value == 0.0 else value)
    return factorize(normalized)


def interaction_codes(*sources):
    if not sources:
        raise ValueError("no sources")
    if len({len(source) for source in sources}) != 1:
        raise ValueError("length mismatch")
    tuples = [tuple(source[row] for source in sources) for row in range(len(sources[0]))]
    return factorize(tuples)


class ReferencePrimitiveTests(unittest.TestCase):
    def test_numeric_factorization_is_row_order_invariant(self):
        values = [20, 10, 20, -1, 99, 10]
        codes, levels = factorize(values)
        self.assertEqual(levels, [-1, 10, 20, 99])
        mapping = dict(zip(values, codes))
        shuffled = list(values)
        random.Random(20260820).shuffle(shuffled)
        shuffled_codes, shuffled_levels = factorize(shuffled)
        self.assertEqual(shuffled_levels, levels)
        self.assertEqual([mapping[value] for value in shuffled], shuffled_codes)

    def test_float_zero_canonicalization_and_nonfinite_rejection(self):
        codes, levels = factorize_float([-0.0, 0.0, 2.5])
        self.assertEqual(codes, [0, 0, 1])
        self.assertEqual(levels, [0.0, 2.5])
        with self.assertRaises(ValueError):
            factorize_float([1.0, float("nan")])

    def test_string_tuple_coding_is_collision_safe(self):
        first, _ = factorize(["a", "a#b", "a"])
        second, _ = factorize(["b#c", "c", "b#c"])
        codes, levels = interaction_codes(first, second)
        self.assertEqual(len(levels), 2)
        self.assertEqual(codes[0], codes[2])
        self.assertNotEqual(codes[0], codes[1])

    def test_tuple_coding_rejects_mismatched_lengths(self):
        with self.assertRaises(ValueError):
            interaction_codes([0, 1], [0])


if __name__ == "__main__":
    unittest.main()
