"""Synthetic contract witnesses only. Never fit the retained calibration."""
import copy
from fractions import Fraction as F
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import two_fe_policy_gates as g
from routing_science import route_hierarchy


def vector(**changes):
    result = [1]*7 + [5, 6, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1]
    for name, value in changes.items():
        result[g.FEATURES.index(name)] = value
    return tuple(result)


def context(i, x=None, y=None, costs=(100, 80, 90), body=None, pooled=None):
    return g.Context((i,), body or f'{i:064x}', x or vector(), y,
                     tuple(map(F, costs)), tuple(map(F, pooled or costs)), {})


def synthetic_cell(expired=False):
    def action(role, steps, elapsed, checkpoints):
        shape = dict(role=role, start=0, end=3 if role == 'packed' else 0,
                     total=3, block=3, r=4 if role == 'packed' else 1,
                     stride=4, final=int(role == 'final'),
                     tolerance_bits=43 if role == 'final' else 42)
        points = []
        for age, stamp in checkpoints:
            point = dict(shape, epoch=0, local=age, remaining=12000-age,
                         n=1000, p=50, q=40, d=20, workers=4, weighted=0, scaled=0,
                         forecast_h=30, forecast_d=30*shape['r'], classes=1,
                         stamp_ns=stamp, state=age+100, build=0)
            point.update({'history_'+k: None for k in ('width', 'actions', 'steps', 'live', 'original')})
            points.append(point)
        end = dict(shape, steps=steps, live=steps*shape['r'], elapsed_ns=elapsed,
                   build_ns=None, restart_ns=None, startup_ns=None, continuation_ns=None, installed=0)
        return dict(end=end, checkpoints=points)

    def call(arm):
        actions = ([action('packed', 5, 30, []), action('final', 24, 100, [(16, 10)])]
                   if expired else
                   [action('packed', 48, 200, [(16, 10), (32, 40)]), action('final', 7, 50, [])])
        owner = 1 if expired else 0
        grant = 16 if arm == 'force16' else 32 if arm == 'force32' and not expired else None
        routes = []
        if grant:
            a = actions[owner]
            a['checkpoints'] = [p for p in a['checkpoints'] if p['local'] <= grant]
            point = next(p for p in a['checkpoints'] if p['local'] == grant)
            point['build'] = 1
            a['end'].update(steps=grant+5, live=(grant+5)*a['end']['r'], installed=1,
                            build_ns=20, restart_ns=10, startup_ns=5, continuation_ns=15,
                            elapsed_ns=point['stamp_ns']+50)
            if not expired:
                actions[-1]['end']['elapsed_ns'] = 70  # Signed negative suffix saving.
            routes = [dict(phase='auto_schur_route', checkpoint=str(grant), samples='1',
                           iterations=str(grant), installed='1', refusal='None', terminal='Direct',
                           levels='1', hierarchy_vertices='4', hierarchy_nonzeros='10', repeats='1',
                           terminal_factor='1', terminal_factor_dimension='4', terminal_factor_nonzeros='10',
                           application='scalar', parallel_operators='0', batch_columns='1')]
        resources = [dict(final=str(a['end']['final']), preset=g.PRESETS[arm],
                         iterations_used=a['end']['steps'], iterations_limit=16000,
                         memory_bytes_used=100+(2 if arm != 'never' else 0), memory_bytes_limit=1000)
                     for a in actions]
        return dict(execution=arm, policy_actions=actions, action_resources=resources,
                    fingerprint='same' if expired and arm != 'force16' else arm,
                    route_trace=routes, route_hierarchy=[route_hierarchy(r) for r in routes],
                    solver_rows=[], resource_checks={'memory_caps': 'passed', 'iteration_caps': 'passed'},
                    resource_basis='actual traced prerequisite')
    warmups = [call(a) for a in g.ARMS]
    return dict(warmups=warmups, blocks=[dict(status='complete',
                calls=[call(a) for a in (*g.ARMS, *reversed(g.ARMS))])])


def reconstruct(cell):
    return g.reconstruct(cell, platform='Darwin', seed=1, family='synthetic', memory='fast',
                         workers=4, body='a'*64, source=g.SOURCE)


def frozen_grid_rows():
    # Two distinct bodies per value make every intended child admissible.
    # At16, n<=4 must build. At32, the survivors have values5..8: n5
    # should defer, n6..8 should build. Global quartiles cannot cut at5.
    rows = []
    for n in range(1, 9):
        costs = ((1000, 0, 1000) if n <= 4 else
                 (0, 1000, 100) if n == 5 else
                 (10, 1000, 0) if n == 6 else (100, 1000, 0))
        for repeat in range(2):
            rows.append(context(2*n+repeat, vector(n=n), vector(n=n), costs))
    return rows


def retained_run_part(platform):
    binding = dict(result_sha256='a'*64, manifest_sha256='b'*64,
                   execution_sources={'qualify': 'c'*64})
    summary = dict(platform=platform, status='diagnostic_complete', errors=[], native_calls=864,
                   calibration_result_sha256=binding['result_sha256'],
                   manifest_sha256=binding['manifest_sha256'])
    record = dict(platform=platform, warmups=144, recorded=288, native_calls=432)
    cell = dict(status='complete', host_attempt=0,
                warmups=[dict(execution=a) for a in g.ARMS],
                blocks=[dict(index=0, status='complete',
                             calls=[dict(execution=a) for a in (*g.ARMS, *reversed(g.ARMS))])])
    run = dict(platform=platform, status='complete', failures=[], budget_compliant=True,
               replica=0, shard=0, execution_sources=binding['execution_sources'],
               manifest_sha256=binding['manifest_sha256'])
    if platform == 'Darwin':
        cell['attempt'] = 0
        run.update(discarded_attempts=[], retry_cells=[], native_calls_started=864)
    run['cases'] = [copy.deepcopy(cell) for _ in range(48)]
    return run, record, summary, binding


class Gates(unittest.TestCase):
    def test_explicit_mac_and_linux_retained_schemas(self):
        for platform in ('Darwin', 'Linux'):
            g.validate_run_part(*retained_run_part(platform))
        for key in ('discarded_attempts', 'retry_cells', 'native_calls_started'):
            mac = retained_run_part('Darwin')
            del mac[0][key]
            with self.subTest(platform='Darwin', key=key), self.assertRaises(ValueError):
                g.validate_run_part(*mac)
            linux = retained_run_part('Linux')
            linux[0][key] = 864 if key == 'native_calls_started' else []
            with self.subTest(platform='Linux', key=key), self.assertRaises(ValueError):
                g.validate_run_part(*linux)

    def test_retained_schema_rejects_failure_reuse_and_missing_or_extra_calls(self):
        mutations = (
            lambda r, p, s, b: r.update(failures=['native failure']),
            lambda r, p, s, b: r.update(budget_compliant=False),
            lambda r, p, s, b: s.update(errors=['unverified']),
            lambda r, p, s, b: s.update(native_calls=863),
            lambda r, p, s, b: s.update(calibration_result_sha256='d'*64),
            lambda r, p, s, b: s.update(manifest_sha256='d'*64),
            lambda r, p, s, b: r.update(execution_sources={'qualify': 'd'*64}),
            lambda r, p, s, b: r['cases'][0].update(host_attempt=1),
            lambda r, p, s, b: r['cases'][0].update(reused=False),
            lambda r, p, s, b: r['cases'][0].update(attempt=0),
            lambda r, p, s, b: r['cases'][0]['warmups'].pop(),
            lambda r, p, s, b: r['cases'][0]['blocks'][0]['calls'].append(dict(execution='never')),
            lambda r, p, s, b: r['cases'][0]['blocks'][0]['calls'][1].update(execution='never'),
            lambda r, p, s, b: p.update(native_calls=431),
        )
        for index, mutation in enumerate(mutations):
            value = retained_run_part('Linux')
            mutation(*value)
            with self.subTest(mutation=index), self.assertRaises(ValueError):
                g.validate_run_part(*value)

    def test_distinct_quartile_cuts_ignore_frequency(self):
        values = [vector(n=n) for n in range(1, 9)]
        gates = g.candidate_gates(values + [values[0]]*50)
        self.assertEqual({a.cut for a in gates if a.kind == 2}, {2, 4, 6})
        self.assertEqual(len(gates), 8)
        maximum = [tuple((i % 2) if k in g.BOOLEANS else i+1 for k in range(21))
                   for i in range(8)]
        self.assertEqual(len(g.candidate_gates(maximum)), 92)

    def test_features_use_actual_future_chunks_not_stride(self):
        p = synthetic_cell()['warmups'][0]['policy_actions'][0]['checkpoints'][0]
        self.assertEqual(g.features(p)[17:], (0, 0, 0, 1))
        p.update(role='outcome', r=1, total=17, block=10, stride=18)
        self.assertEqual(g.features(p)[17:], (2, 17, 10, 1))
        p.update(role='final')
        self.assertEqual(g.features(p)[17:], (0, 0, 0, 0))
        for key, val in (('forecast_h', None), ('classes', 0), ('workers', 0), ('n', 1 << 128)):
            bad = dict(p, **{key: val})
            with self.subTest(key=key), self.assertRaises(ValueError):
                g.features(bad)

    def test_joint_reachability_and_diversity(self):
        a = g.Gate(2, 0, 2, 1)
        rows = [context(i, vector(n=i), vector(n=i), costs=(100, 0, 20)) for i in range(1, 5)]
        self.assertTrue(g.admissible(a, rows, 16))
        reached = rows[2:]
        self.assertFalse(g.admissible(g.Gate(2, 0, 3, 1), reached, 32))
        self.assertTrue(g.admissible(g.BUILD, reached, 32))
        duplicated = [context(i, vector(n=i), body='a'*64, costs=(100, 0, 100)) for i in range(1, 5)]
        result = g.fit(duplicated, total_cells=4)
        self.assertEqual(g.decode(bytes.fromhex(result['encoded_hex'])), (g.DEFER, g.DEFER))

    def test_exact_joint_loss_not_two_separate_losses(self):
        rows = [context(1, vector(n=1), vector(n=1), (100, 0, 90)),
                context(2, vector(n=1), vector(n=1), (100, 0, 90)),
                context(3, vector(n=2), vector(n=2), (100, 100, 0)),
                context(4, vector(n=2), vector(n=2), (100, 100, 0))]
        result = g.fit(rows, total_cells=6)
        gates = g.decode(bytes.fromhex(result['encoded_hex']))
        self.assertEqual([g.programme(gates, r) for r in rows], [1, 1, 2, 2])
        self.assertEqual(result['costs']['pooled']['empirical_regret_ns'], 0)
        self.assertEqual(result['coverage']['no_episode'], 2)
        self.assertIsNone(result['coverage']['no_episode_treatment_regret'])

    def test_ties_prefer_structural_then_constant_then_lexical(self):
        progress = g.Gate(2, 7, 5, 1)
        structural = g.Gate(2, 16, 0, 1)
        self.assertLess(g.tie_key((structural, g.DEFER)), g.tie_key((progress, g.DEFER)))
        rows = [context(i, costs=(20, 20, 20)) for i in (1, 2)]
        result = g.fit(rows, total_cells=2)
        self.assertEqual(g.decode(bytes.fromhex(result['encoded_hex'])), (g.DEFER, g.DEFER))
        rows = [context(i, costs=(20, F(1, 3), 20)) for i in (1, 2)]
        result = g.fit(rows, total_cells=2)
        self.assertEqual(g.decode(bytes.fromhex(result['encoded_hex'])), (g.BUILD, g.DEFER))
        self.assertEqual(result['costs']['pooled']['selected_ns'], F(2, 3))

    def test_same_model_unpooled_audit_never_reselects(self):
        rows = [context(i, costs=(10, 15, 30), pooled=(20, 15, 20)) for i in (1, 2)]
        result = g.fit(rows, total_cells=2)
        self.assertEqual(g.decode(bytes.fromhex(result['encoded_hex'])), (g.BUILD, g.DEFER))
        self.assertEqual(result['costs']['unpooled']['selected_ns'], 30)
        self.assertEqual(result['costs']['unpooled']['empirical_regret_ns'], 5)
        with patch.object(g, 'fit', side_effect=AssertionError('no second fit')):
            self.assertEqual(g.audit((g.BUILD, g.DEFER), rows, total_cells=2)['costs'], result['costs'])

    def test_wire_roundtrip_and_reject_all_noncanonical_fields(self):
        gates = (g.Gate(2, 0, (1 << 127)+3, 0), g.BUILD)
        wire = g.encode(gates)
        self.assertEqual(len(wire), 128)
        self.assertEqual(g.decode(wire), gates)
        for n in ((1 << 127)+2, (1 << 127)+3, (1 << 127)+4):
            values = vector(n=n)
            self.assertEqual(gates[0].build(values), n > gates[0].cut)
            self.assertEqual(g.decode(wire)[0].build(values), gates[0].build(values))
        for index in (0, 8, 10, 12, 16, 17, 32, 67, 97, 98, 112):
            bad = bytearray(wire); bad[index] ^= 1
            if index >= 64:
                bad[32:64] = hashlib.sha256(bad[64:]).digest()
            with self.subTest(index=index), self.assertRaises(ValueError):
                g.decode(bytes(bad))
        with self.assertRaises(ValueError):
            g.decode(wire[:-1])
        for values in ((3, 0, 0, 0), (2, 21, 0, 0), (0, 1, 0, 0), (2, 9, 1, 0)):
            with self.assertRaises(ValueError):
                g.Gate(*values)

    def test_common_origin_and_signed_suffix_and_warmup_exclusion(self):
        cell = synthetic_cell()
        row, good, missing = reconstruct(cell)
        self.assertFalse(missing)
        self.assertEqual(row.raw, (F(240), F(120), F(150)))
        self.assertEqual(row.metrics[16]['observed_suffix_gain'], -20)
        # Force32 local-origin cost is120; the policy must also charge16->32.
        self.assertEqual(row.raw[2] - row.metrics[32]['build_programme_ns'], 30)
        cell['warmups'][0]['policy_actions'][0]['end']['elapsed_ns'] += 10000
        self.assertEqual(reconstruct(cell)[0].raw, row.raw)

    def test_expiration_pooling_own_limits_and_two_transport_bytes(self):
        cell = synthetic_cell(expired=True)
        row, _, missing = reconstruct(cell)
        self.assertIsNone(row.x32)
        self.assertEqual(row.pooled, (90, 50, 90))
        self.assertEqual(len(missing), 1)
        n, f = cell['warmups'][0], cell['warmups'][2]
        f['action_resources'][0]['memory_bytes_limit'] = 102
        g.validate_pool(n, f, source=g.SOURCE)
        for field in ('memory_bytes_used', 'iterations_used', 'iterations_limit'):
            bad = copy.deepcopy(f); bad['action_resources'][0][field] += 1
            with self.subTest(field=field), self.assertRaises(ValueError):
                g.validate_pool(n, bad, source=g.SOURCE)
        for key in ('fingerprint', 'resource_basis'):
            bad = copy.deepcopy(f); bad[key] = 'changed'
            with self.assertRaises(ValueError):
                g.validate_pool(n, bad, source=g.SOURCE)
        with self.assertRaises(ValueError):
            g.validate_pool(n, f, source='unbound')

    def test_alignment_partition_repeat_and_missing_origin_fail_closed(self):
        for mutation in ('state', 'prefix', 'partition', 'missing', 'repeat'):
            cell = synthetic_cell()
            calls = [cell['warmups'][2], *[c for c in cell['blocks'][0]['calls'] if c['execution']=='force32']]
            for call in calls:
                if mutation == 'state': call['policy_actions'][0]['checkpoints'][0]['state'] += 1
                elif mutation == 'partition': call['policy_actions'][0]['end']['build_ns'] += 1
                elif mutation == 'missing': del call['policy_actions'][0]['checkpoints'][0]
                elif mutation == 'prefix': call['policy_actions'][0]['checkpoints'][0]['stamp_ns'] = 1000
            if mutation == 'repeat': calls[1]['policy_actions'][0]['end']['steps'] += 1
            with self.subTest(mutation=mutation), self.assertRaises(ValueError): reconstruct(cell)

    def test_shape_decoder_rejects_ambiguous_nodes(self):
        self.assertEqual(g.unpack([['a']], {'0': [1]}), {'a': 1})
        for node in ({'00': [1]}, {'1': [1]}, {'0': []}, {'0': [1], '1': [2]}):
            with self.assertRaises(ValueError): g.unpack([['a']], node)

    def test_collector_rejects_changed_outcome_before_any_fit(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp)/g.OUTCOME
            path.parent.mkdir(parents=True)
            path.write_text('{}')
            with patch.object(g, 'fit', side_effect=AssertionError('collection must not fit')):
                with self.assertRaisesRegex(ValueError, 'evidence hash changed'):
                    g.collect(temp)

    def test_stage32_cuts_are_frozen_before_reachability_filter(self):
        rows = frozen_grid_rows()
        all_cuts = {x.cut for x in g.candidate_gates([r.x32 for r in rows]) if x.kind == 2}
        late_cuts = {x.cut for x in g.candidate_gates([r.x32 for r in rows[8:]]) if x.kind == 2}
        self.assertEqual(all_cuts, {2, 4, 6})
        self.assertEqual(late_cuts, {5, 6, 7})
        actual = g.fit(rows, total_cells=16)
        gates = g.decode(bytes.fromhex(actual['encoded_hex']))
        self.assertEqual(gates, (g.Gate(2, 0, 4, 1), g.Gate(2, 0, 6, 0)))
        self.assertEqual([g.programme(gates, row) for row in rows], [1]*8 + [0]*4 + [2]*4)
        self.assertEqual(actual['costs']['pooled']['selected_ns'], 20)
        self.assertEqual(actual['costs']['pooled']['empirical_regret_ns'], F(5, 4))
        # The tempting reached-only cut5 achieves zero but is outside the
        # frozen grid. This quantifies why the prohibited mutation matters.
        illegal = (g.Gate(2, 0, 4, 1), g.Gate(2, 0, 5, 0))
        illegal_report = g.audit(illegal, rows, total_cells=16)
        self.assertEqual(illegal_report['costs']['pooled']['selected_ns'], 0)

    def test_leaf_reports_missing_components_and_signed_means(self):
        rows = [context(i, costs=(100, 80, 100)) for i in (1, 2)]
        rows[0].metrics[16] = dict(B=F(10), observed_suffix_gain=F(-20), remaining_steps=F(7))
        rows[1].metrics[16] = dict(B=None, observed_suffix_gain=F(10), remaining_steps=F(8))
        report = g.audit((g.BUILD, g.DEFER), rows, total_cells=2)
        leaf = report['leaves'][0]['means']
        self.assertEqual(leaf['B'], dict(mean=10, available=1, unavailable=1))
        self.assertEqual(leaf['observed_suffix_gain']['mean'], -5)
        self.assertEqual(leaf['remaining_steps']['mean'], F(15, 2))
        self.assertEqual(leaf['fitted_continuation_programme_ns']['mean'], 100)
        self.assertEqual(g.json_value(F(1, 3)), {'numerator': 1, 'denominator': 3})


if __name__ == '__main__':
    unittest.main()
