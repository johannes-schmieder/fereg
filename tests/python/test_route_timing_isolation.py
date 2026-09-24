"""Timing exclusions must distinguish new own workers from competing jobs."""
from pathlib import Path
from types import SimpleNamespace
import sys
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'benchmarks/two_fe_dense'))
import route_development as route


class TimingIsolationTests(unittest.TestCase):
    def check_process(self, parents=None, error=None, name='stata-mp', known=False, cpu=(1.0, 1.4), cpu_error=None):
        owner = mock.Mock(pid=100)
        owner.children.return_value = [SimpleNamespace(pid=200)] if known else []
        worker = mock.Mock(info=dict(pid=200, name=name, create_time=1.0))
        worker.parents.side_effect = error
        worker.parents.return_value = [SimpleNamespace(pid=p) for p in (parents or [])]
        worker.cpu_times.side_effect = cpu_error or [SimpleNamespace(user=t, system=0.) for t in cpu]
        with mock.patch.object(route.psutil, 'Process', return_value=owner), \
             mock.patch.object(route.psutil, 'process_iter', return_value=[worker]), \
             mock.patch.object(route.time, 'sleep'), \
             mock.patch.object(route.time, 'monotonic', side_effect=[0., 0.05]):
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

    def test_busy_gui_stata_is_a_competing_fit(self):
        for name in route.GUI_STATA_NAMES:
            with self.subTest(name=name):
                result, worker = self.check_process(parents=[1], name=name)
                self.assertEqual(result[0]['activity_status'], 'cpu_observed')
                self.assertGreater(result[0]['sampled_cpu_seconds'], 0)
                self.assertEqual(worker.cpu_times.call_count, 2)

    def test_idle_gui_does_not_block_but_own_gui_is_never_sampled(self):
        for cpu in [(1., 1.), (1., 1.0005)]:
            result, worker = self.check_process(parents=[1], name='StataMP', cpu=cpu)
            self.assertEqual(result, [])
            self.assertEqual(worker.cpu_times.call_count, 2)
        result, worker = self.check_process(parents=[100, 1], name='StataMP')
        self.assertEqual(result, [])
        worker.cpu_times.assert_not_called()

    def test_unavailable_gui_activity_stays_excluded(self):
        for error in [route.psutil.NoSuchProcess(200), route.psutil.AccessDenied(200)]:
            with self.subTest(error=type(error).__name__):
                result, _ = self.check_process(parents=[1], name='StataMP', cpu_error=error)
                self.assertEqual(result[0]['activity_status'], 'unavailable')

    def test_existing_child_and_unrelated_names_need_no_parent_query(self):
        for args in [dict(known=True), dict(name='unrelated')]:
            result, worker = self.check_process(**args)
            self.assertEqual(result, [])
            worker.parents.assert_not_called()


if __name__ == '__main__':
    unittest.main()
