"""Placement contracts do not need the campaign's scientific Python stack."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('placement', Path(__file__).resolve().parents[2] / 'bench/placement.py')
placement = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(placement)


def topology(entries):
    return '\n\n'.join(f'processor: {cpu}\nphysical id: {socket}\ncore id: {core}'
                       for cpu, socket, core in entries)


class PlacementTests(unittest.TestCase):
    def test_interleaved_sockets_and_hyperthreads(self):
        entries = [(i, i % 2, i // 2) for i in range(8)]
        entries += [(i + 8, socket, core) for i, socket, core in entries]
        self.assertEqual(placement.socket_ordered_cpus(topology(entries), set(range(16))),
                         [0, 2, 4, 6, 1, 3, 5, 7])

    def test_largest_allowed_socket_first_and_sibling_selection(self):
        entries = [(i, i % 2, i // 2) for i in range(8)] + [(8, 0, 0)]
        self.assertEqual(placement.socket_ordered_cpus(topology(entries), {8, 1, 3, 5, 7}),
                         [1, 3, 5, 7, 8])

    def test_missing_topology_is_not_silently_accepted(self):
        with self.assertRaisesRegex(RuntimeError, 'topology unavailable'):
            placement.socket_ordered_cpus('processor: 0\ncore id: 0', {0})

    @patch.object(placement.subprocess, 'check_output', return_value='test lscpu')
    @patch.object(placement.os, 'sched_getaffinity', return_value={0, 2}, create=True)
    def test_memory_nodes_observed_and_bound_for_both_arms(self, affinity, lscpu):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for cpu, node in [(0, 1), (2, 3)]:
                (root / f'cpu{cpu}/node{node}').mkdir(parents=True)
                (root / f'cpu{cpu}/topology').mkdir()
                (root / f'cpu{cpu}/topology/physical_package_id').write_text('5')
            original = Path.read_text
            def read(path, *args, **kwargs):
                return '[madvise] never' if str(path).endswith('transparent_hugepage/enabled') else original(path, *args, **kwargs)
            with patch.object(Path, 'read_text', read):
                observed = placement.placement_metadata([0, 2], root)
            self.assertEqual(observed['memory_nodes'], [1, 3])
            self.assertEqual(observed['sockets'], [5])
            self.assertEqual(observed['thp'], '[madvise] never')
            self.assertEqual(observed['lscpu'], 'test lscpu')
            for arm in ['stata-mp', 'Rscript']:
                self.assertEqual(placement.bound_command([arm, 'fit'], observed),
                                 ['numactl', '--physcpubind=0,2', '--membind=1,3', '--', arm, 'fit'])
            with self.assertRaisesRegex(RuntimeError, 'CPU mask'):
                placement.placement_metadata([4], root)
            (root / 'cpu0/node1').rmdir()
            with self.assertRaisesRegex(RuntimeError, 'NUMA node unavailable'):
                placement.placement_metadata([0], root)


if __name__ == '__main__':
    unittest.main()
