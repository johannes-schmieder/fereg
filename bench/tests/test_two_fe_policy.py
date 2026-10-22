"""Calibration control, finite-budget and interval-retention regressions."""
import copy
import unittest

import two_fe_policy as policy
import qualify
import routing


def action():
    return dict(role='packed', start=0, end=2, total=2, block=2,
                r=3, stride=3, tolerance_bits=42, final=0)


def checkpoint():
    return dict(action(), epoch=0, local=16, remaining=100, n=1000, p=50, q=40, d=20,
                workers=4, weighted=0, scaled=0, forecast_h=70, forecast_d=120, classes=1,
                history_width=None, history_actions=None, history_steps=None,
                history_live=None, history_original=None, stamp_ns=10, state=19, build=1)


def ending():
    return dict(action(), steps=19, live=45, elapsed_ns=125,
                build_ns=30, restart_ns=35, startup_ns=20, continuation_ns=30, installed=1)


def line(kind, fields):
    return 'FEREG_POLICY_LABEL|' + kind + '|' + '|'.join(
        f"{k}={'none' if v is None else v}" for k, v in fields.items())


class Calibration(unittest.TestCase):
    def test_distinct_forced_controls_never_share_a_comparison_series(self):
        manifest = {'diagnostic': policy.MODE}
        names = [qualify.comparison_key(manifest, 'never', target) for target in ('force16', 'force32')]
        self.assertEqual(names, ['never->force16', 'never->force32'])
        self.assertEqual(qualify.comparison_key({}, 'original', 'candidate'), 'original')

    def test_cross_worker_comparison_never_pairs_different_seed_or_memory_inputs(self):
        cell = dict(id='s2026102201-weak-fast-t1', family='weak')
        self.assertTrue(qualify.same_source_case(cell, dict(cell, id='s2026102201-weak-fast-t4')))
        self.assertFalse(qualify.same_source_case(cell, dict(cell, id='s2026103201-weak-fast-t4')))
        self.assertFalse(qualify.same_source_case(cell, dict(cell, id='s2026102201-weak-low-t4')))

    def test_exact_budget_is_one_mirrored_block_and_retained_warmup(self):
        for platform in ('Darwin', 'Linux'):
            fixed = policy.fixed(platform)
            recorded = qualify.orders(list(policy.ARMS), fixed['blocks'], fixed['seed'])
            self.assertEqual(len(recorded), 1)
            self.assertEqual(recorded[0][:3], list(reversed(recorded[0][3:])))
            self.assertEqual(len(policy.CASE_IDS)*len(fixed['threads'])*(3+len(recorded[0])), 864)

    def test_frozen_controls_cannot_be_separate_binaries_or_add_sampling(self):
        config = dict(policy.fixed('Linux'), platform='Linux', artifacts={
            role: dict(path='binary', revision='1'*40, build='pinned', request=request, families=['two'])
            for role, request in policy.ARMS.items()}, cases=[
                dict(id=name, family=name.split('-', 1)[1].rsplit('-', 1)[0],
                     memory_limit_bytes=None, oracle=dict(kind='routing')) for name in policy.CASE_IDS],
            files={'binary': 'a'*64})
        policy.validate(config)
        for field, value in [('blocks', 2), ('replicas', 2), ('seed', 4), ('timeout_s', 121),
                             ('recovery', {}), ('continuation', {})]:
            changed = copy.deepcopy(config)
            changed[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                policy.validate(changed)
        changed = copy.deepcopy(config)
        changed['artifacts']['force16']['path'] = 'other'
        changed['files']['other'] = 'b'*64
        with self.assertRaisesRegex(ValueError, 'same actual binary'):
            policy.validate(changed)
        for role, request in policy.ARMS.items():
            self.assertEqual(routing.policy(request)['injection'], 'routing_two_remaining_' + role)

    def test_partition_retains_unknowns_and_requires_exact_closure(self):
        log = line('checkpoint', checkpoint()) + '\n' + line('action_end', ending())
        parsed = policy.parse_labels(log)
        self.assertIsNone(parsed[0]['checkpoints'][0]['history_width'])
        self.assertEqual(parsed[0]['end']['startup_ns'], 20)
        for field in ('elapsed_ns', 'build_ns', 'restart_ns', 'startup_ns', 'continuation_ns'):
            end = ending()
            end[field] += 1
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'partition'):
                policy.parse_labels(line('checkpoint', checkpoint())+'\n'+line('action_end', end))
        end = ending()
        end['startup_ns'] = None
        with self.assertRaisesRegex(ValueError, 'disjoint'):
            policy.parse_labels(line('checkpoint', checkpoint())+'\n'+line('action_end', end))

    def test_refusal_direct_and_missing_tail_are_distinct(self):
        end = ending()
        end.update(installed=0, restart_ns=None, startup_ns=None, continuation_ns=None)
        refused = policy.parse_labels(line('checkpoint', checkpoint())+'\n'+line('action_end', end))
        self.assertEqual(refused[0]['end']['build_ns'], 30)
        end.update(build_ns=None)
        direct = policy.parse_labels(line('action_end', end))
        self.assertEqual(direct[0]['checkpoints'], [])
        with self.assertRaisesRegex(ValueError, 'incomplete'):
            policy.parse_labels(line('checkpoint', checkpoint()))
        with self.assertRaisesRegex(ValueError, 'outcome'):
            policy.parse_labels(line('checkpoint', checkpoint())+'\n'+line('action_end', end))

    def test_mismatched_state_identity_duplicate_fields_and_bad_cadence_reject(self):
        base = line('checkpoint', checkpoint())
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            policy.parse_labels(base+'|local=16')
        point = checkpoint()
        point['local'] = 17
        with self.assertRaisesRegex(ValueError, 'cadence'):
            policy.parse_labels(line('checkpoint', point))
        end = ending()
        end['r'] = 2
        with self.assertRaisesRegex(ValueError, 'outside'):
            policy.parse_labels(base+'\n'+line('action_end', end))
        complete = base+'\n'+line('action_end', ending())
        with self.assertRaisesRegex(ValueError, 'twice'):
            policy.parse_labels(complete+'\n'+complete)


if __name__ == '__main__':
    unittest.main()
