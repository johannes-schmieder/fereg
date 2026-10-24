"""The report must reject native failures that are independent of capture hashes."""
import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from panel_report import validated_summary


class PanelReportTests(unittest.TestCase):
    def campaign(self):
        cell = dict(id='panel', model='ols', design='panel', rows=10, threads=4,
                    vce='iid', save_fe=True, section='panel', arms=['fereg'])
        return dict(manifest=dict(cells=[cell], policy=dict(repetitions=3, scientific_tolerances={})),
                    comparisons=[], records=[dict(cell='panel', arm='fereg', repetition=rep,
                    status='ok', capture={}, metadata=dict(seconds='1', rss='2', fereg_threads='4'),
                    memory_sweeps=0) for rep in [1, 2, 3]])

    def test_report_requires_exact_reported_objective(self):
        value = self.campaign()
        self.assertTrue(validated_summary(value).eligible.all())
        value['records'][1]['metadata']['rss'] = '2.000000000000001'
        with self.assertRaisesRegex(ValueError, 'eligibility'):
            validated_summary(value)

    def test_report_requires_verified_worker_budget_and_four_core_witness(self):
        for change in ['budget', 'witness']:
            value = copy.deepcopy(self.campaign())
            if change == 'budget':
                value['records'][0]['metadata']['fereg_threads'] = '2'
            else:
                value['manifest']['cells'][0]['threads'] = 1
                for record in value['records']:
                    record['metadata']['fereg_threads'] = '1'
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, 'eligibility'):
                validated_summary(value)


if __name__ == '__main__':
    unittest.main()
