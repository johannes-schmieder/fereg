from __future__ import annotations

import unittest


def ranges(length: int, threads: int, minimum_chunk: int):
    if length == 0:
        return []
    minimum_chunk = max(1, minimum_chunk)
    useful = max(1, (length + minimum_chunk - 1) // minimum_chunk)
    workers = max(1, min(threads, useful, length))
    base, remainder = divmod(length, workers)
    start = 0
    result = []
    for worker in range(workers):
        width = base + int(worker < remainder)
        result.append(range(start, start + width))
        start += width
    return result


def memory_estimate(n, k, g, c, pool):
    base = n * 8 * 4 + n * g * 4 + n * c * 4
    cross = k * k * 8
    return base + n * k * 16 + cross, base + n * min(pool, k) * 16 + cross


class ParallelAndMemoryOracleTests(unittest.TestCase):
    def test_ranges_cover_observations_once_for_32_threads(self):
        chunks = ranges(1_000_003, 32, 10_000)
        self.assertLessEqual(len(chunks), 32)
        self.assertEqual(chunks[0].start, 0)
        self.assertEqual(chunks[-1].stop, 1_000_003)
        for left, right in zip(chunks, chunks[1:]):
            self.assertEqual(left.stop, right.start)
        self.assertEqual(sum(len(chunk) for chunk in chunks), 1_000_003)

    def test_requested_32_workers_are_exercised_when_work_is_large_enough(self):
        chunks = ranges(100_000, 32, 1_024)
        self.assertEqual(len(chunks), 32)

    def test_small_input_does_not_spawn_empty_workers(self):
        chunks = ranges(3, 32, 1)
        self.assertEqual([len(chunk) for chunk in chunks], [1, 1, 1])

    def test_low_memory_pool_is_smaller_for_wide_model(self):
        fast, low = memory_estimate(30_000_000, 100, 2, 2, 8)
        self.assertLess(low, fast)
        self.assertGreater(fast, 0)


if __name__ == "__main__":
    unittest.main()
