"""The executable stage binder preserves the frozen prospective model/arms."""
import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import routing
import routing_stage
from test_routing_incremental import incremental_spec


class RoutingStageTests(unittest.TestCase):
    def config(self):
        spec = incremental_spec()
        config = {key: {'sha256': spec['arms'][arm]['sha256']}
                  for key, arm in [('baseline_plugin', 'baseline'), ('incumbent_plugin', 'incumbent'),
                                   ('candidate_plugin', 'incremental-many')]}
        config.update(harness={'sha256': spec['harness_sha256']},
                      sampling_admission={'sha256': spec['sampled_qualification']['1']},
                      candidate_source_identity=spec.get('candidate_source_identity', {}),
                      incremental=spec['incremental'], prior_manifests=spec['prior_manifests'])
        return config, {'family': 'many', 'panels': spec['panels']}

    def test_binder_preserves_models_and_both_public_references(self):
        config, prepared = self.config()
        before = copy.deepcopy(prepared)
        result = routing_stage.manifest(config, prepared)
        self.assertEqual(prepared, before)
        self.assertEqual(result['panels'], before['panels'])
        self.assertEqual(result['protections'], ['baseline', 'incumbent'])
        self.assertEqual(result['candidates'], ['incumbent', 'incremental-many'])
        self.assertIsNone(result['arms']['incumbent']['policy'])
        self.assertEqual(result['arms']['cmgschur-k1']['cmgsamples'], 1)
        self.assertEqual(result['arms'][routing.COMPACT_IT]['injection'], 'it_coefficients')

    def test_no_relabeling_or_reused_holdout(self):
        config, prepared = self.config()
        with self.assertRaises(ValueError):
            routing_stage.manifest(config, {**prepared, 'family': 'two'})
        prepared['panels']['C'][0]['body_sha256'] = prepared['panels']['B'][0]['body_sha256']
        with self.assertRaises(ValueError):
            routing_stage.manifest(config, prepared)

    def test_stage_seeds_are_distinct_before_execution(self):
        seeds = [s for values in routing_stage.SEEDS.values() for s in values.values()]
        self.assertEqual(len(seeds), len(set(seeds)))
        self.assertEqual(set(routing_stage.SEEDS), {'many', 'two'})


if __name__ == '__main__':
    unittest.main()
