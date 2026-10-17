"""Prepare-only C2 contracts; no native estimation, timing, or cluster jobs."""
import copy
import hashlib
import math
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import occurrence_confirmation as confirmation

campaign = confirmation.campaign


class OccurrenceConfirmation(unittest.TestCase):
    def test_fixed_new_seeds_shapes_workers_and_balanced_abba_order(self):
        self.assertEqual(confirmation.ARMS, ['baseline', 'cells_s'])
        self.assertEqual(confirmation.LIMITS['seed'], 2026101723)
        self.assertEqual(confirmation.LIMITS['data_seed'], 2026101724)
        self.assertEqual(confirmation.LIMITS['workers'], [1, 4])
        self.assertEqual(confirmation.LIMITS['blocks'], 12)
        self.assertEqual(confirmation.LIMITS['warmups'], 1)
        self.assertEqual(confirmation.LIMITS['science_repeats'], 2)
        for key in ['measurement_budget_s', 'science_budget_s', 'warmup_budget_s']:
            self.assertEqual(confirmation.LIMITS[key], 1800)
        self.assertEqual(confirmation.LIMITS['call_timeout_s'], 120)
        self.assertEqual([(c['family'], c['generator'], c['shape'], c['workers'], c['firms'], c['periods'])
                          for c in confirmation.CASES], [
            ('short2', 'synthetic', 'short', 3000, 300, 2),
            ('long40', 'synthetic', 'long', 3000, 300, 40),
            ('weak40', 'synthetic', 'weak', 3000, 300, 40),
            ('hubs', 'high_degree_case', 'd60-stress', 1500, 300, 64),
            ('dense', 'high_degree_case', 'd30-mixed', 1500, 300, 32),
            ('repeated', 'synthetic', 'long', 1500, 300, 20),
            ('wide', 'additional_case', 'wide', 1500, 300, 20),
            ('low', 'additional_case', 'wide', 1500, 300, 20)])
        ids = [f"{c['family']}-t{w}" for c in confirmation.CASES for w in [1, 4]]
        blocks = confirmation.schedule(ids)
        self.assertEqual(blocks, confirmation.schedule(ids))
        self.assertEqual([len(block) for block in blocks], [16] * 12)
        self.assertTrue(all({r['cell'] for r in block} == set(ids) for block in blocks))
        self.assertEqual(sum(len(r['arms']) for block in blocks for r in block), 768)
        for cid in ids:
            orders = [r['arms'] for block in blocks for r in block if r['cell'] == cid]
            self.assertEqual(orders.count(['baseline', 'cells_s', 'cells_s', 'baseline']), 6)
            self.assertEqual(orders.count(['cells_s', 'baseline', 'baseline', 'cells_s']), 6)
        self.assertEqual(confirmation.EXECUTION['warmup_calls'], 32)
        self.assertEqual(confirmation.EXECUTION['scientific_calls'], 64)
        self.assertEqual(confirmation.EXECUTION['scored_calls'], 768)
        analysis = confirmation.ANALYSIS
        self.assertEqual(analysis['upper_tail_alpha'], .025)
        self.assertEqual(analysis['upper_median_order_statistic'], 10)
        self.assertLessEqual(sum(math.comb(12, k) for k in range(10, 13)) / 2**12, .025)
        self.assertGreater(sum(math.comb(12, k) for k in range(9, 13)) / 2**12, .025)
        self.assertEqual(analysis['pass_rule'], 'E_(10) <= 0 in every one of the 16 settings; ties pass')
        self.assertEqual(analysis['block_excess'], 'E = N - B - max(0.05 * B, 0.001)')
        self.assertTrue(all(analysis[key] == 'none' for key in
                            ['exclusions', 'extensions', 'family_drops', 'alternate_finalists']))
        self.assertEqual(confirmation.ORACLE_CAPS, {name: getattr(campaign.routing_oracle, name)
                                                   for name in confirmation.ORACLE_CAPS})

    def test_columns_use_each_new_seed_and_repeat_only_the_repeated_case(self):
        for index, spec in enumerate(confirmation.CASES):
            values = [campaign.np.arange(3) for _ in range(5)]
            with self.subTest(family=spec['family']), \
                 patch.object(campaign.cases, spec['generator'], return_value=values) as generator:
                actual = confirmation.columns(spec)
            generator.assert_called_once_with(spec['shape'], spec['workers'], spec['firms'],
                                              spec['periods'], 2026101724 + index)
            self.assertEqual(len(actual[0]), 12 if spec['family'] == 'repeated' else 3)
            self.assertTrue(all(campaign.np.array_equal(a, campaign.np.repeat(b, 4)
                                  if spec['family'] == 'repeated' else b) for a, b in zip(actual, values)))

    @staticmethod
    def discovery(root):
        artifacts = {}
        for key in ['generator', 'harness', 'plugin']:
            path = root / key
            path.write_text(key)
            artifacts[key] = campaign.ref(path)
        discovery = {'kind': campaign.OCCURRENCE_KIND, 'native_affinity_cores':5, **artifacts}
        source = root / 'discovery.json'
        campaign.save(source, campaign.routing.seal(discovery))
        tool = root / 'tool.py'
        tool.write_text('frozen confirmation tool')
        return discovery, source, tool

    def prepare_fixture(self, root, *, failed_witness=False):
        discovery, source, tool = self.discovery(root)
        calls = {'witnesses': [], 'requests': []}
        def write_case(generator, path, values):
            self.assertEqual(str(generator), discovery['generator']['path'])
            path.write_text(path.stem)
        def inspect(path, *args):
            return {'body_sha256': hashlib.sha256(path.read_bytes().split(b'|')[0]).hexdigest()}
        def witness(source, target):
            calls['witnesses'].append(source.stem)
            if failed_witness and len(calls['witnesses']) == 3:
                raise ValueError('independent oracle rank certificate failed')
            target.write_text('independent witness')
            target.with_suffix('.npz').write_text('independent arrays')
        def load(path, *args):
            return {}, {p.resolve(): campaign.digest(p) for p in [path, path.with_suffix('.npz')]}
        def prepare(source, target, arm, workers):
            calls['requests'].append((source.stem, arm, workers))
            target.write_bytes(source.read_bytes() + f'|{arm}|{workers}'.encode())
        def low(source, target, **kwargs):
            self.assertEqual(kwargs, {'low': True})
            target.write_bytes(source.read_bytes() + b'|low')
        with patch.object(campaign, 'validate'), \
             patch.object(confirmation, 'tool_files', return_value=[tool]), \
             patch.object(confirmation, 'columns', return_value=[]), \
             patch.object(campaign.cases, 'write_case', side_effect=write_case), \
             patch.object(campaign.cases, 'graph_summary', return_value={}), \
             patch.object(campaign.routing_cases, 'inspect_case', side_effect=inspect), \
             patch.object(campaign.routing_cases, 'copy_import', side_effect=low) as low_request, \
             patch.object(campaign.routing_oracle, 'write_reference', side_effect=witness), \
             patch.object(campaign.routing_oracle, 'load_reference', side_effect=load), \
             patch.object(campaign.routing, 'prepare', side_effect=prepare), \
             patch.object(campaign, 'native', side_effect=AssertionError('C2 candidate calls prohibited')):
            if failed_witness:
                with self.assertRaisesRegex(ValueError, 'independent oracle rank'):
                    confirmation.prepare(root / 'c2', source)
                self.assertEqual(calls['witnesses'], ['short2', 'long40', 'weak40'])
                self.assertEqual(len(calls['requests']), 8)
                self.assertFalse((root / 'c2/plan.json').exists())
                return
            plan = confirmation.prepare(root / 'c2', source)
            self.assertEqual(calls['witnesses'], [c['family'] for c in confirmation.CASES])
            self.assertEqual(len(calls['requests']), 32)
            self.assertEqual(low_request.call_count, 1)
            self.assertEqual(plan['discovery_plan_sha256'], campaign.routing.identity(discovery))
            self.assertEqual(len(plan['cells']), 16)
            self.assertEqual(plan['prepared_wall_ns'] > 0, True)
            confirmation.validate(plan)
            for mutation in ['budget', 'case', 'order', 'arm', 'oracle-cap', 'freeze', 'artifact', 'binding', 'analysis', 'execution', 'file']:
                changed = copy.deepcopy(plan)
                if mutation == 'budget':
                    changed['limits']['measurement_budget_s'] += 1
                elif mutation == 'case':
                    changed['cases'][0]['periods'] = 3
                elif mutation == 'order':
                    changed['blocks'][0][0]['arms'] = ['baseline', 'cells_s']
                elif mutation == 'arm':
                    changed['cells'][0]['inputs']['cells_s_lanes'] = changed['cells'][0]['inputs']['cells_s']
                elif mutation == 'oracle-cap':
                    changed['oracle_caps']['MAX_ROWS'] += 1
                elif mutation == 'freeze':
                    changed['prepared_wall_ns'] = 0
                elif mutation == 'artifact':
                    changed['plugin'] = changed['harness']
                elif mutation == 'binding':
                    del changed['frozen_files'][changed['cells'][0]['source']['path']]
                elif mutation == 'analysis':
                    changed['analysis']['upper_median_order_statistic'] = 9
                elif mutation == 'execution':
                    changed['execution']['scored_calls'] = 767
                else:
                    Path(changed['cells'][0]['source']['path']).write_text('changed source')
                with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                    confirmation.validate(changed)

    def test_preparation_freezes_all_dependencies_without_any_native_estimation(self):
        with tempfile.TemporaryDirectory() as directory:
            self.prepare_fixture(Path(directory))

    def test_invalid_witness_stops_without_resizing_retry_or_replacement(self):
        with tempfile.TemporaryDirectory() as directory:
            self.prepare_fixture(Path(directory), failed_witness=True)


if __name__ == '__main__':
    unittest.main()
