from __future__ import annotations

import math
import random
import unittest

from test_parallel_and_memory_oracles import memory_estimate, ranges
from test_reference_primitives import factorize, factorize_float, interaction_codes


class RandomizedOracleTests(unittest.TestCase):
    def test_integer_factorization_is_permutation_invariant_over_random_cases(self):
        rng = random.Random(0x5F1E57)
        for _ in range(300):
            values = [rng.randint(-50, 50) for _ in range(rng.randint(0, 500))]
            codes, levels = factorize(values)
            mapping = {level: index for index, level in enumerate(levels)}
            self.assertEqual(codes, [mapping[value] for value in values])
            shuffled = list(values)
            rng.shuffle(shuffled)
            shuffled_codes, shuffled_levels = factorize(shuffled)
            self.assertEqual(shuffled_levels, levels)
            self.assertEqual(shuffled_codes, [mapping[value] for value in shuffled])

    def test_string_factorization_is_collision_safe_over_random_cases(self):
        rng = random.Random(0xC0111510)
        alphabet = ["", "#", "a", "a#b", "ab#", "α", "ß", " firm ", "0"]
        for _ in range(200):
            values = [rng.choice(alphabet) + str(rng.randrange(7)) for _ in range(300)]
            codes, levels = factorize(values)
            self.assertEqual(len(levels), len(set(values)))
            for left in range(len(values)):
                for right in range(left + 1, min(left + 8, len(values))):
                    self.assertEqual(codes[left] == codes[right], values[left] == values[right])

    def test_float_factorization_matches_total_order_domain_used_by_core(self):
        rng = random.Random(0xF10A7)
        for _ in range(200):
            values = [rng.randint(-10_000, 10_000) / 16.0 for _ in range(500)]
            values.extend([-0.0, 0.0])
            codes, levels = factorize_float(values)
            self.assertTrue(all(math.isfinite(level) for level in levels))
            self.assertEqual(levels, sorted(set(0.0 if value == 0.0 else value for value in values)))
            self.assertEqual(codes[-2], codes[-1])

    def test_tuple_interactions_are_permutation_invariant_and_exact(self):
        rng = random.Random(0x1A7E2AC7)
        for dimensions in range(1, 7):
            for _ in range(80):
                n = rng.randint(0, 400)
                sources = [[rng.randrange(20 + j) for _ in range(n)] for j in range(dimensions)]
                codes, levels = interaction_codes(*sources)
                tuples = [tuple(source[row] for source in sources) for row in range(n)]
                mapping = {level: index for index, level in enumerate(levels)}
                self.assertEqual(codes, [mapping[value] for value in tuples])
                order = list(range(n))
                rng.shuffle(order)
                permuted = [[source[row] for row in order] for source in sources]
                permuted_codes, permuted_levels = interaction_codes(*permuted)
                self.assertEqual(permuted_levels, levels)
                self.assertEqual(permuted_codes, [mapping[tuples[row]] for row in order])

    def test_range_partition_contract_over_random_sizes_and_thread_counts(self):
        rng = random.Random(0x7A2EAD5)
        for _ in range(5_000):
            length = rng.randrange(0, 2_000_000)
            threads = rng.randrange(1, 129)
            minimum_chunk = rng.randrange(0, 100_000)
            chunks = ranges(length, threads, minimum_chunk)
            if length == 0:
                self.assertEqual(chunks, [])
                continue
            effective_chunk = max(1, minimum_chunk)
            expected_workers = max(1, min(threads, math.ceil(length / effective_chunk), length))
            self.assertEqual(len(chunks), expected_workers)
            self.assertEqual(chunks[0].start, 0)
            self.assertEqual(chunks[-1].stop, length)
            self.assertTrue(all(len(chunk) > 0 for chunk in chunks))
            self.assertTrue(all(left.stop == right.start for left, right in zip(chunks, chunks[1:])))
            widths = [len(chunk) for chunk in chunks]
            self.assertLessEqual(max(widths) - min(widths), 1)

    def test_memory_preflight_monotonicity_and_pool_boundary(self):
        for n in (0, 1, 1_000, 30_000_000):
            previous_fast = -1
            previous_low = -1
            for k in (0, 1, 2, 8, 64, 1000):
                fast, low = memory_estimate(n, k, 3, 2, 8)
                self.assertGreaterEqual(fast, previous_fast)
                self.assertGreaterEqual(low, previous_low)
                self.assertLessEqual(low, fast)
                previous_fast, previous_low = fast, low
            fast_small, low_small = memory_estimate(n, 100, 3, 2, 8)
            fast_large, low_large = memory_estimate(n, 100, 3, 2, 1000)
            self.assertEqual(fast_small, fast_large)
            self.assertLessEqual(low_small, low_large)


if __name__ == "__main__":
    unittest.main()
