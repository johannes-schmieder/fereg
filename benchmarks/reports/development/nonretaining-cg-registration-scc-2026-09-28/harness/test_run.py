import unittest
from run import probe_records


def lines(kind):
    threads = [1, 4] if kind == 'direct' else [1, 8]
    cases = [1, 4, 64, 65, 129] if kind == 'direct' else [0, 1, 2, 3]
    return [f'FEREG_CG_PROBE|{kind}|{t}|{w}|{c}|true|1e-16|3|Some(123)'
            for t in threads for w in range(4) for c in cases]


class Evidence(unittest.TestCase):
    def test_exact_case_coverage(self):
        for kind, count in [('direct', 40), ('cmg', 32)]:
            rows = probe_records('\n'.join(lines(kind)), kind)
            self.assertEqual(len(rows), count)
            self.assertTrue(all(row['passed'] for row in rows))

    def test_missing_duplicate_and_unregistered_cases_fail(self):
        rows = lines('direct')
        for altered in [rows[:-1], rows[:-1] + [rows[0]], rows + [rows[0]],
                        [rows[0].replace('|1|0|1|', '|2|0|1|')] + rows[1:]]:
            with self.assertRaises(ValueError):
                probe_records('\n'.join(altered), 'direct')

    def test_failure_is_not_upgraded(self):
        rows = lines('cmg')
        rows[7] = rows[7].replace('|true|1e-16|3|Some(123)', '|false|ERROR|InvalidData|failed current certificate')
        parsed = probe_records('\n'.join(rows), 'cmg')
        self.assertEqual(sum(not r['passed'] for r in parsed), 1)
        self.assertEqual(parsed[7]['error'], 'ERROR')

    def test_test_runner_prefix_and_bad_boolean(self):
        rows = lines('direct')
        rows[0] = 'test fe::test ... ' + rows[0]
        self.assertEqual(len(probe_records('\n'.join(rows), 'direct')), 40)
        rows[0] = rows[0].replace('|true|', '|unknown|')
        with self.assertRaises(ValueError):
            probe_records('\n'.join(rows), 'direct')


if __name__ == '__main__':
    unittest.main()
