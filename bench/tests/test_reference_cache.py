"""Reference caching cannot suppress actual corruption or change CSV meaning."""
import csv
import io
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compare import ReferenceCache, accuracy, coefficients, sample


class ReferenceCacheTests(unittest.TestCase):
    def test_every_comparison_reads_actual_and_reference_is_reused(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            actual, reference = root/'actual', root/'reference'
            for directory in [actual, reference]:
                directory.mkdir()
                (directory/'coefficients.csv').write_text('name,value\nx,1\n')
                (directory/'sample.csv').write_text('obs_id\n1\n2\n')
            rec = dict(status='ok', directory=str(actual))
            ref = dict(status='ok', directory=str(reference))
            policy = dict(coefficient_atol=1e-10, coefficient_rtol=1e-7)
            cache = ReferenceCache()
            import compare
            with patch.object(compare, 'coefficients', wraps=coefficients) as co, patch.object(compare, 'sample', wraps=sample) as sa:
                for _ in range(4):
                    self.assertEqual(accuracy(rec, ref, policy, cache), ('pass', 0.0))
                self.assertEqual((co.call_count, sa.call_count), (5, 5))
                (actual/'sample.csv').write_text('obs_id\n1\n3\n')
                self.assertEqual(accuracy(rec, ref, policy, cache)[0], 'disagreement')
            (actual/'sample.csv').write_text('obs_id\n1\n2\n')
            (reference/'sample.csv').write_text('obs_id\n1\n3\n')
            self.assertEqual(accuracy(rec, ref, policy, cache)[0], 'unavailable')

    def test_retention_budget_and_eviction_do_not_change_results(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            cache = ReferenceCache(maximum_bytes=1600)
            for i in range(20):
                directory = root/str(i)
                directory.mkdir()
                (directory/'sample.csv').write_text('obs_id\n1\n2\n')
                self.assertEqual(cache.read(directory, 'sample.csv', sample), (1, 2))
                self.assertLessEqual(cache.retained_bytes, cache.maximum_bytes)
            self.assertLess(len(cache.entries), 20)
            disabled = ReferenceCache(0)
            self.assertEqual(disabled.read(directory, 'sample.csv', sample), (1, 2))
            self.assertFalse(disabled.entries)

    def test_parser_matches_dict_reader_named_field_semantics(self):
        captures = ['obs_id\n1\n\n2\n', 'other,obs_id\na,1\nb,2\n',
                    'obs_id,obs_id\n7,1\n8,2\n', 'obs_id\n1,extra\n', '',
                    'obs_id\n"2"\n', 'obs_id\ninvalid\n', 'wrong\n1\n',
                    'other,obs_id\n1\n']
        with tempfile.TemporaryDirectory() as temp:
            for capture in captures:
                (Path(temp)/'sample.csv').write_text(capture)
                try:
                    expected = tuple(int(row['obs_id']) for row in csv.DictReader(io.StringIO(capture)))
                except (ValueError, KeyError, TypeError) as error:
                    with self.assertRaises(type(error)):
                        sample(temp)
                else:
                    self.assertEqual(sample(temp), expected)

    def test_mutation_during_first_read_is_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp)/'sample.csv'
            path.write_text('obs_id\n1\n')
            def changing(directory):
                result = sample(directory)
                path.write_text('obs_id\n2\n')
                return result
            with self.assertRaises(ValueError):
                ReferenceCache().read(temp, 'sample.csv', changing)


if __name__ == '__main__':
    unittest.main()
