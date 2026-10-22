"""Counterfactual matching must reject changed science and retain missing labels."""
import copy
from fractions import Fraction
import unittest
from unittest.mock import patch

import two_fe_policy_labels as labels


def projection(role, *, total=2, start=0, end=0, block=2, width=1, history=None):
    shape = dict(role=role, start=start, end=end, total=total, block=block,
                 r=width, stride=3, tolerance_bits=42 if role != 'final' else 43,
                 final=int(role == 'final'))
    point = dict(shape, epoch=0, local=16, remaining=100, n=1000, p=50, q=40,
                 d=20, workers=4, weighted=0, scaled=0, forecast_h=70,
                 forecast_d=70*width, classes=1, stamp_ns=10, state=19, build=0)
    for name, value in zip(('width', 'actions', 'steps', 'live', 'original'),
                           history or (None,)*5):
        point['history_'+name] = value
    finish = dict(shape, steps=40, live=40*width, elapsed_ns=200, build_ns=None,
                  restart_ns=None, startup_ns=None, continuation_ns=None, installed=0)
    return dict(end=finish, checkpoints=[point])


def cell():
    def call(control):
        first = projection('packed', end=2, width=3)
        final = projection('final', total=0, block=0)
        if control != 'never':
            first['checkpoints'][0].update(build=1, local=16 if control == 'force16' else 32)
            first['end'].update(elapsed_ns=110, build_ns=30, restart_ns=20,
                                startup_ns=10, continuation_ns=40, installed=1)
            final['end']['elapsed_ns'] = 80
        else:
            second = dict(first['checkpoints'][0], local=32, stamp_ns=20)
            first['checkpoints'].append(second)
        # Final checkpoints deliberately absent; the complete final interval is
        # still part of each first intervention's future suffix.
        final['checkpoints'] = []
        return dict(execution=control, policy_actions=[first, final],
                    fingerprint=control, route_trace=[], route_hierarchy=[], action_resources=[])
    controls = ['never', 'force16', 'force32']
    return dict(warmups=[call(c) for c in controls], blocks=[dict(status='complete',
                calls=[call(c) for c in controls + list(reversed(controls))])])


def match(value):
    # Outcome/route binding is tested independently with actual trace fixtures.
    # These focused witnesses isolate complete-call pairing and label provenance.
    with patch.object(labels, 'validate_outcomes', create=True):
        return labels.paired_cell(value, platform='Darwin', seed=2026102201,
                                  family='weak', memory='fast', workers=4)


class Matching(unittest.TestCase):
    def test_observed_memory_limits_vary_but_each_actual_bound_still_applies(self):
        value = cell()
        for ordinal, call in enumerate([*value['warmups'], *value['blocks'][0]['calls']]):
            call['action_resources'] = []
            for action in call['policy_actions']:
                action['end'].update(installed=0, build_ns=None, restart_ns=None,
                                     startup_ns=None, continuation_ns=None)
                for point in action['checkpoints']:
                    point['build'] = 0
                call['action_resources'].append(dict(
                    final=str(action['end']['final']),
                    preset='routing_two_remaining_' + call['execution'],
                    iterations_used=40, iterations_limit=100,
                    memory_bytes_used=100, memory_bytes_limit=1000 + ordinal))

        def actual_pair(candidate):
            return labels.paired_cell(candidate, platform='Darwin', seed=2026102201,
                                      family='weak', memory='fast', workers=4)

        usable, unavailable = actual_pair(value)
        self.assertFalse(usable)
        self.assertEqual([entry['reason'] for entry in unavailable], ['no_intervention'] * 2)
        for field in ('memory_bytes_used', 'iterations_used', 'iterations_limit'):
            changed = copy.deepcopy(value)
            changed['blocks'][0]['calls'][0]['action_resources'][0][field] += 1
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'repeat changed'):
                actual_pair(changed)
        value['blocks'][0]['calls'][0]['action_resources'][0]['memory_bytes_limit'] = 99
        with self.assertRaisesRegex(ValueError, 'resource bound'):
            actual_pair(value)

    def test_means_use_all_recorded_calls_and_the_complete_later_suffix(self):
        value = cell()
        force = [c for c in value['blocks'][0]['calls'] if c['execution'] == 'force16']
        force[1]['policy_actions'][0]['end'].update(build_ns=32, elapsed_ns=112)
        force[1]['policy_actions'][1]['end']['elapsed_ns'] = 81
        usable, unavailable = match(value)
        self.assertFalse(unavailable)
        self.assertEqual(len(usable), 2)
        first = usable[0]['labels']
        self.assertEqual(first['B'], 31)
        self.assertEqual(first['I'], 190)
        self.assertEqual(first['observed_suffix_gain'], Fraction(239, 2))

    def test_preintervention_state_mismatch_is_retained_without_replacement(self):
        value = cell()
        for call in [*value['warmups'], *value['blocks'][0]['calls']]:
            if call['execution'] == 'force16':
                call['policy_actions'][0]['checkpoints'][0]['state'] = 20
        usable, unavailable = match(value)
        self.assertEqual([o['control'] for o in usable], ['force32'])
        self.assertEqual(unavailable[0]['reason'], 'pre_intervention_state_mismatch')

    def test_refusal_and_missing_incumbent_checkpoint_remain_unavailable(self):
        for refusal in (True, False):
            value = cell()
            for call in [*value['warmups'], *value['blocks'][0]['calls']]:
                if refusal and call['execution'] == 'force16':
                    call['policy_actions'][0]['end'].update(installed=0, restart_ns=None,
                        startup_ns=None, continuation_ns=None)
                if not refusal and call['execution'] == 'never':
                    call['policy_actions'][0]['checkpoints'] = call['policy_actions'][0]['checkpoints'][1:]
            usable, unavailable = match(value)
            self.assertEqual(len(usable), 1)
            self.assertEqual(unavailable[0]['reason'], 'constructor_refused' if refusal
                             else 'missing_incumbent_checkpoint')

    def test_non_timing_repeat_and_worker_drift_fail(self):
        for field in ('state', 'workers'):
            value = cell()
            value['blocks'][0]['calls'][0]['policy_actions'][0]['checkpoints'][0][field] += 1
            with self.subTest(field=field), self.assertRaises(ValueError):
                match(value)

    def test_actual_low_schedule_and_zero_history_exclude_current_work(self):
        actions = [projection('outcome', total=5),
                   projection('regressor', total=5, end=2, width=2),
                   projection('regressor', total=5, start=2, end=4, width=2,
                              history=(2, 1, 0, 0, 2)),
                   projection('regressor', total=5, start=4, end=5,
                              history=(2, 2, 40, 80, 4)),
                   projection('final', total=0, block=0)]
        actions[1]['end'].update(steps=0, live=0)
        labels.validate_actions(actions, 'low', 4)
        changed = copy.deepcopy(actions)
        changed[2]['checkpoints'][0]['history_steps'] = 40
        with self.assertRaisesRegex(ValueError, 'history'):
            labels.validate_actions(changed, 'low', 4)
        with self.assertRaisesRegex(ValueError, 'range'):
            labels.validate_actions([actions[0], *actions[2:]], 'low', 4)

    def test_one_attempt_per_accuracy_allows_final_key_only(self):
        actions = [projection('outcome'), projection('regressor', end=2, width=2),
                   projection('final', total=0, block=0)]
        actions[0]['checkpoints'][0]['build'] = 1
        actions[2]['checkpoints'][0]['build'] = 1
        labels.validate_actions(actions, 'low', 4)
        actions[1]['checkpoints'][0]['build'] = 1
        with self.assertRaisesRegex(ValueError, 'opportunity'):
            labels.validate_actions(actions, 'low', 4)


if __name__ == '__main__':
    unittest.main()
