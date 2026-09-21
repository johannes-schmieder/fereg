"""Timing exclusions must distinguish new own workers from competing jobs."""
from pathlib import Path
from types import SimpleNamespace
import sys
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'benchmarks/two_fe_dense'))
import route_development as route


class TimingIsolationTests(unittest.TestCase):
    def check_process(self, parents=None, error=None, name='stata-mp', known=False):
        owner = mock.Mock(pid=100)
        owner.children.return_value = [SimpleNamespace(pid=200)] if known else []
        worker = mock.Mock(info=dict(pid=200, name=name, create_time=1.0))
        worker.parents.side_effect = error
        worker.parents.return_value = [SimpleNamespace(pid=p) for p in (parents or [])]
        with mock.patch.object(route.psutil, 'Process', return_value=owner), \
             mock.patch.object(route.psutil, 'process_iter', return_value=[worker]):
            result = route.foreign_processes()
        return result, worker

    def test_new_grandchild_after_snapshot_is_ours(self):
        result, worker = self.check_process(parents=[150, 100, 1])
        self.assertEqual(result, [])
        worker.parents.assert_called_once_with()

    def test_competing_job_keeps_ancestry(self):
        result, _ = self.check_process(parents=[300, 1])
        self.assertEqual(result[0]['parent_pids'], [300, 1])
        self.assertEqual(result[0]['ancestry_status'], 'resolved')

    def test_unverifiable_process_stays_excluded(self):
        for error in [route.psutil.NoSuchProcess(200), route.psutil.AccessDenied(200)]:
            with self.subTest(error=type(error).__name__):
                result, _ = self.check_process(error=error)
                self.assertEqual(result[0]['ancestry_status'], 'unavailable')

    def test_existing_child_and_unrelated_names_need_no_parent_query(self):
        for args in [dict(known=True), dict(name='unrelated')]:
            result, worker = self.check_process(**args)
            self.assertEqual(result, [])
            worker.parents.assert_not_called()


if __name__ == '__main__':
    unittest.main()
