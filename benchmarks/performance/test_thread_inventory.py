"""Synthetic evidence tests; no estimators or numerical libraries are invoked."""
import copy
import csv
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import thread_inventory as inventory


def row(request, selected, rep, seconds):
    return dict(cell=dict(model='ppml', design='dense', n=100, k=2, threads=request, section='main'),
                phase='warm', repetition=rep, status='pass', eligible_references=['fixest_strict'],
                requested_threads=request, selected_threads=selected, thread_metadata_valid=True,
                seconds=seconds, input_sha256=f'input-{rep}', sample_sha256='sample',
                host='node-a', job_id='123', task_id='1', converged=1, stata_processors=4)


class ThreadInventoryTests(unittest.TestCase):
    def test_requested_selected_and_stata_threads_are_separate(self):
        for model, prefix in [('ols', 'fereg'), ('ppml', 'feppml')]:
            receipt = dict(cell=dict(model=model), metadata={prefix + '_threads': 8.,
                           prefix + '_requested_threads': 28., 'stata_processors': 4.})
            result = inventory.thread_fields(receipt, 28)
            self.assertTrue(result['thread_metadata_valid'])
            self.assertEqual((result['requested_threads'], result['selected_threads'], result['stata_processors']), (28, 8, 4))
            for bad in (None, 0, -1, 1.5, 29, float('inf'), float('nan'), True):
                receipt['metadata'][prefix + '_threads'] = bad
                self.assertFalse(inventory.thread_fields(receipt, 28)['thread_metadata_valid'])
            receipt['metadata'][prefix + '_threads'] = 8
            del receipt['metadata'][prefix + '_requested_threads']
            self.assertIsNone(inventory.thread_fields(receipt, 28)['selected_threads'])

    def test_capped_request_is_not_more_native_cores(self):
        rows = [row(t, 8, rep, 1 if t == 8 else 1.1) for t in (8, 28) for rep in (1, 2)]
        result = inventory.scaling_inventory(rows, (1, 2))[0]
        self.assertEqual(result['transitions'], dict(same_native_team=2))
        self.assertTrue(result['slower_on_every_repetition'])
        self.assertTrue(result['alternating_confirmation_required'])
        self.assertFalse(result['causal_scaling_established'])

    def test_cross_node_ratios_remain_observational(self):
        rows = [row(t, t, rep, 1 / t) for t in (4, 8) for rep in (1, 2)]
        rows[-1]['host'] = 'node-b'
        result = inventory.scaling_inventory(rows, (1, 2))[0]
        self.assertEqual(result['observational_higher_over_lower']['values'], [.5, .5])
        self.assertFalse(result['pairs'][1]['same_node_task'])
        self.assertFalse(result['causal_scaling_established'])

    def test_missing_failed_or_incomparable_repetition_blocks_summary(self):
        original = [row(t, t, rep, 1 / t) for t in (4, 8) for rep in (1, 2)]
        for change in [dict(status='command_timeout'), dict(eligible_references=[]), dict(seconds=0),
                       dict(seconds=float('inf')), dict(input_sha256='other'), dict(sample_sha256=None),
                       dict(converged=0), dict(thread_metadata_valid=False)]:
            rows = copy.deepcopy(original)
            rows[-1].update(change)
            result = inventory.scaling_inventory(rows, (1, 2))[0]
            self.assertFalse(result['all_repetitions_eligible'], change)
            self.assertIsNone(result['observational_higher_over_lower'], change)
            self.assertTrue(result['pairs'][1]['reasons'], change)
        result = inventory.scaling_inventory(original[:-1], (1, 2))[0]
        self.assertIn('higher:missing', result['pairs'][1]['reasons'])

    def test_repeated_seed_keeps_both_clocks(self):
        rows = [row(4, 4, 1, 2), row(8, 8, 1, 1), row(4, 4, 2, 1), row(8, 8, 2, 2)]
        for entry in rows:
            entry['input_sha256'] = 'same-seed'
        result = inventory.scaling_inventory(rows, (1, 2))[0]
        self.assertEqual(result['observational_higher_over_lower']['values'], [.5, 2.])
        self.assertFalse(result['slower_on_every_repetition'])
        self.assertTrue(result['alternating_confirmation_required'])
        with self.assertRaisesRegex(ValueError, 'duplicate native repetition'):
            inventory.scaling_inventory(rows + rows[:1], (1, 2))
        with self.assertRaisesRegex(ValueError, 'unregistered native repetition'):
            inventory.scaling_inventory(rows, (1,))

    def fixture(self, root):
        run, report = root / 'run', root / 'report'
        report.mkdir()
        manifest = dict(arms=dict(ppml=['feppml', 'fixest']), warm_repetitions=2)
        frozen = dict(candidate=dict(commit='test'), runtime={}, controller={
            name: hashlib.sha256((inventory.SYSTEMATIC / name).read_bytes()).hexdigest()
            for name in ('common.py', 'light.py')})
        frozen['fingerprint'] = hashlib.sha256(json.dumps(frozen, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
        def write(path, value):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(value))
        for base, name in [(run, 'input/manifest.json'), (report, 'manifest.json')]:
            write(base / name, manifest)
        for base, name in [(run, 'env/frozen.json'), (report, 'provenance.json')]:
            write(base / name, frozen)
        registered, pairs = [], []
        for t in (4, 8):
            for rep in (1, 2):
                item = row(t, t, rep, 1 / t)
                cell = item['cell']
                registered.append(dict(cell=cell, repetition=rep))
                path = run / 'results' / inventory.cell_id(cell) / f'warm-{rep}' / 'feppml/result.json'
                write(path, dict(item, arm='feppml', measurement_fingerprint=frozen['fingerprint'],
                    metadata=dict(feppml_threads=t, feppml_requested_threads=t, stata_processors=4, converged=1)))
                pairs.append(dict(cell, repetition=rep, phase='warm', comparator='fixest', eligible='True',
                                  status='pass', failed_checks=''))
        write(report / 'completeness.json', dict(status='complete', expected_groups=4, complete_groups=4, integrity_errors=[]))
        with (report / 'paired-ratios.csv').open('w') as stream:
            writer = csv.DictWriter(stream, fieldnames=list(pairs[0]))
            writer.writeheader(); writer.writerows(pairs)
        return run, report, registered

    def test_collector_binds_and_preserves_frozen_report(self):
        with tempfile.TemporaryDirectory() as temp:
            run, report, registered = self.fixture(Path(temp))
            before = {p: p.read_bytes() for p in Path(temp).rglob('*') if p.is_file()}
            with patch.object(inventory, 'groups', return_value=registered):
                result = inventory.collect(run, report)
            self.assertEqual(len(result['applications']), 4)
            self.assertEqual(result['scaling'][0]['observational_higher_over_lower']['median'], .5)
            self.assertEqual({p: p.read_bytes() for p in before}, before)
            self.assertEqual(len(result['bindings']), 10)

    def test_collector_rejects_missing_pairs_and_identity_drift(self):
        for mutation, message in [('pair', 'every registered comparison'), ('receipt', 'native receipt identity'),
                                  ('manifest', 'manifest mismatch'), ('completeness', 'inconsistent report completeness')]:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temp:
                run, report, registered = self.fixture(Path(temp))
                if mutation == 'pair':
                    path = report / 'paired-ratios.csv'
                    path.write_text('\n'.join(path.read_text().splitlines()[:-1]) + '\n')
                else:
                    path = (next((run / 'results').rglob('result.json')) if mutation == 'receipt' else
                            run / 'input/manifest.json' if mutation == 'manifest' else report / 'completeness.json')
                    value = json.loads(path.read_text())
                    value.update({'measurement_fingerprint': 'wrong'} if mutation == 'receipt' else
                                 {'warm_repetitions': 3} if mutation == 'manifest' else {'complete_groups': 3})
                    path.write_text(json.dumps(value))
                with patch.object(inventory, 'groups', return_value=registered):
                    with self.assertRaisesRegex(ValueError, message):
                        inventory.collect(run, report)


if __name__ == '__main__':
    unittest.main()
