"""Economic-label/native-outcome binding; no native execution or timers."""
import copy
import unittest

import routing
import two_fe_policy_labels as labels
from routing_science import route_hierarchy


def action(*, final=0, grant=False, installed=False, epoch=0):
    return {
        'end': dict(final=final, installed=int(installed),
                    build_ns=10 if grant else None,
                    restart_ns=2 if installed else None,
                    startup_ns=3 if installed else None,
                    continuation_ns=4 if installed else None),
        'checkpoints': [dict(build=1, local=16, epoch=epoch)] if grant else [],
    }


def resource(*, final=0, iterations=23, role='force16'):
    return dict(final=str(final), preset=routing.policy('two-remaining-' + role)['injection'],
                iterations_used=iterations, iterations_limit=100,
                memory_bytes_used=10, memory_bytes_limit=100)


def route(*, installed=True, iterations=16):
    result = dict(phase='auto_schur_route', checkpoint='16', samples='1',
                  iterations=str(iterations), installed=str(int(installed)),
                  refusal='None' if installed else 'Some(MemoryBudget)')
    if installed:
        result.update(terminal='Direct', levels='1', hierarchy_vertices='4',
                      hierarchy_nonzeros='10', repeats='1', terminal_factor='1',
                      terminal_factor_dimension='4', terminal_factor_nonzeros='10',
                      application='scalar', parallel_operators='0', batch_columns='1')
    return result


def call():
    installed = route()
    return dict(execution='force16',
                policy_actions=[action(grant=True, installed=True), action(final=1)],
                action_resources=[resource(), resource(final=1)],
                route_trace=[installed], route_hierarchy=[route_hierarchy(installed)])


class NativeOutcomeBindings(unittest.TestCase):
    def test_installation_then_retained_final_needs_only_one_route(self):
        labels.validate_outcomes(call())

    def test_direct_and_initial_certificate_returns_have_no_route(self):
        for role in ('never', 'force16', 'force32'):
            value = dict(execution=role, policy_actions=[action(), action(final=1)],
                         action_resources=[resource(iterations=0, role=role),
                                           resource(final=1, iterations=0, role=role)],
                         route_trace=[], route_hierarchy=[])
            labels.validate_outcomes(value)

    def test_memory_refusal_then_new_final_key_can_install(self):
        refused, installed = route(installed=False), route()
        value = call()
        value.update(policy_actions=[action(grant=True), action(final=1, grant=True, installed=True)],
                     route_trace=[refused, installed],
                     route_hierarchy=[None, route_hierarchy(installed)])
        labels.validate_outcomes(value)
        value['route_trace'][0]['refusal'] = 'Some(Cancelled)'
        with self.assertRaisesRegex(ValueError, 'memory refusal'):
            labels.validate_outcomes(value)

    def test_missing_or_orphan_outcomes_and_hierarchies_fail(self):
        for mutation in ('missing', 'orphan', 'missing_hierarchy', 'wrong_hierarchy'):
            value = call()
            if mutation == 'missing':
                value['route_trace'] = []
                value['route_hierarchy'] = []
            elif mutation == 'orphan':
                value['route_trace'].append(copy.deepcopy(value['route_trace'][0]))
                value['route_hierarchy'].append(copy.deepcopy(value['route_hierarchy'][0]))
            elif mutation == 'missing_hierarchy':
                value['route_hierarchy'] = []
            else:
                value['route_hierarchy'][0]['vertices'] = [5]
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                labels.validate_outcomes(value)

    def test_installation_flag_refusal_and_action_classification_are_bound(self):
        for mutation in ('installed', 'refusal', 'final', 'preset', 'iterations', 'memory'):
            value = call()
            if mutation == 'installed':
                value['policy_actions'][0]['end']['installed'] = 0
            elif mutation == 'refusal':
                value['route_trace'][0]['refusal'] = 'Some(MemoryBudget)'
            elif mutation == 'final':
                value['action_resources'][0]['final'] = '1'
            elif mutation == 'preset':
                value['action_resources'][0]['preset'] = 'routing_two_remaining_never'
            elif mutation == 'iterations':
                value['action_resources'][0]['iterations_used'] = 15
            else:
                value['action_resources'][0]['memory_bytes_used'] = 101
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                labels.validate_outcomes(value)

    def test_correction_checkpoint_has_bounded_global_offset(self):
        value = call()
        value['policy_actions'][0]['checkpoints'][0]['epoch'] = 1
        value['route_trace'][0]['iterations'] = '20'
        labels.validate_outcomes(value)
        for iterations in ('16', '24', '-1', 'not-a-count'):
            value['route_trace'][0]['iterations'] = iterations
            with self.subTest(iterations=iterations), self.assertRaises(ValueError):
                labels.validate_outcomes(value)
        value = call()
        value['route_trace'][0]['iterations'] = '17'
        with self.assertRaisesRegex(ValueError, 'checkpoint'):
            labels.validate_outcomes(value)

    def test_retained_installation_cannot_be_rebuilt_in_final_phase(self):
        value = call()
        value['policy_actions'][1] = action(final=1, grant=True, installed=True)
        value['route_trace'].append(route())
        value['route_hierarchy'].append(route_hierarchy(value['route_trace'][-1]))
        with self.assertRaisesRegex(ValueError, 'new native constructor'):
            labels.validate_outcomes(value)

    def test_never_cannot_have_even_a_refused_constructor(self):
        value = call()
        value['execution'] = 'never'
        value['action_resources'] = [resource(role='never'), resource(role='never', final=1)]
        with self.assertRaisesRegex(ValueError, 'forced checkpoint'):
            labels.validate_outcomes(value)


if __name__ == '__main__':
    unittest.main()
