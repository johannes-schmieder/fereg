"""Routing subsets keep the ordinary gates and expose their limited scope."""
import copy
import unittest
from unittest.mock import patch
import test_qualify
import qualify as q


class RoutingSubset(unittest.TestCase):
    def setUp(self):
        test_qualify.Qualification.setUp(self)
        self.config.update(coverage='routing-subset', blocks=6)

    plan = test_qualify.Qualification.plan
    execute = test_qualify.Qualification.execute

    def test_plan_accepts_generic_subset_and_binds_all_execution_checks(self):
        self.config['cases'].append(dict(self.config['cases'][0], id='second', family='slopes'))
        value = self.plan()
        self.assertEqual([c['family'] for c in value['cases']], ['no-fe', 'slopes'])
        self.assertEqual(value['execution_sources'].keys(), q.execution_sources().keys())
        self.assertEqual(value['threads'], [1, 4])

    def test_unknown_duplicate_campaign_and_one_worker_are_rejected(self):
        original = copy.deepcopy(self.config)
        variants = []
        for family in ('unknown-family', next(iter(q.CAMPAIGN_FAMILIES))):
            changed = copy.deepcopy(original)
            changed['cases'][0]['family'] = family
            variants.append(changed)
        changed = copy.deepcopy(original)
        changed['cases'].append(dict(changed['cases'][0], id='duplicate'))
        variants.append(changed)
        changed = copy.deepcopy(original); changed['threads'] = [1]; variants.append(changed)
        for changed in variants:
            self.config = changed
            with self.subTest(config=changed), self.assertRaisesRegex(ValueError, 'unique known routing'):
                self.plan()

    def test_block_and_real_fixture_requirements_still_apply(self):
        for blocks in (2, 5):
            self.config['blocks'] = blocks
            with self.subTest(blocks=blocks), self.assertRaisesRegex(ValueError, 'balanced blocks'):
                self.plan()
        self.config['blocks'] = 6
        q.write(self.source.with_suffix('.json'), {'small': True})
        with self.assertRaisesRegex(ValueError, 'small tooling fixtures'):
            self.plan()

    def test_passing_report_is_explicitly_limited_and_retains_both_guards(self):
        self.plan()
        result = q.run(self.manifest, self.output, execute=self.execute)
        self.assertEqual(result['status'], 'complete')
        summary = q.report(self.manifest, self.output)
        self.assertEqual(summary['status'], 'subset_passed', summary)
        self.assertEqual(len(summary['comparisons']), 4)
        self.assertEqual({r['reference'] for r in summary['comparisons']}, {'original', 'incumbent'})
        self.assertFalse(summary['activation_qualified'])
        self.assertEqual(summary['selected_families'], ['no-fe'])
        self.assertEqual(set(summary['omitted_families']), q.FAMILIES - {'no-fe'})
        self.assertEqual(self.oracle.call_count, 2)

    def test_failed_science_and_adverse_timings_cannot_pass_subset(self):
        self.plan()
        with patch.object(q.science, 'check_oracle', side_effect=ValueError('science failure')):
            result = q.run(self.manifest, self.output, execute=self.execute)
        self.assertNotEqual(result['status'], 'complete')
        summary = q.report(self.manifest, self.output)
        self.assertEqual(summary['status'], 'needs_discussion')
        self.assertFalse(summary['activation_qualified'])
        # The generic reporting guard is exercised on a complete independent mock round.
        self.output = self.root / 'target/tmp/adverse'
        q.run(self.manifest, self.output, execute=self.execute)
        with patch.object(q, 'score', return_value={'ratio': 2., 'excess_s': .01}):
            summary = q.report(self.manifest, self.output)
        self.assertEqual(summary['status'], 'needs_discussion')
        self.assertTrue(all(row['status'] == 'regression' for row in summary['comparisons']))


if __name__ == '__main__':
    unittest.main()
