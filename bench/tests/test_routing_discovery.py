"""Prospective SCC screen: exact identities, fail-stop collection, no promotion."""
import copy
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import routing
import routing_discovery as discovery
from test_routing import h, manifest, records


def spec42():
    spec = manifest((1, 2))
    cell = spec['panels']['A'][0]
    spec['panels']['A'] = [{**cell, 'id': f'A-{i}', 'case_sha256': h(f'case-{i}'),
                           'body_sha256': h(f'body-{i}')} for i in range(42)]
    return spec


def producer(spec, prerequisite=False):
    value = records(spec, 'A')
    value.update(platform='Linux-test', tool_sha256={'test': h('source')},
                 purpose='scientific_prerequisites' if prerequisite else 'policy_tournament',
                 repetitions=1 if prerequisite else 3, warmups=0 if prerequisite else 1,
                 timeout_s=120.0, budget_s=14400.0)
    if prerequisite:
        value['records'] = [r for r in value['records'] if r['repetition'] == 0]
    return value


def save_json(path, value):
    path.write_text(json.dumps(value))
    return discovery.reference(path)


class DiscoveryTests(unittest.TestCase):
    def setUp(self):
        self.spec = spec42()
        self.tools = {'test': h('source')}
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.plan = {'kind': discovery.KIND + '_plan', 'design': discovery.DESIGN,
                     'tournament': routing.seal(self.spec), 'producer_tools': self.tools,
                     'plugins': {a: 'unused' for a in self.spec['arms']},
                     'library_limits': discovery.LIBRARY_LIMITS,
                     'output_root': str(self.root)}
        self.machine = {'hostname': 'test', 'job_id': '42', 'slots': 28, 'affinity': list(range(28)),
                        'machine': 'x86_64', 'library_limits': discovery.LIBRARY_LIMITS}
        self.original = producer(self.spec, True)
        self.proof = {'kind': discovery.KIND + '_prerequisites', 'status': 'ready',
                      'host': self.machine, 'plan_sha256': routing.identity(self.plan),
                      'records': save_json(self.root / 'prerequisites-records.json', self.original),
                      'wrappers': {}}
        for cell in self.spec['panels']['A']:
            cid = cell['id']
            self.proof['wrappers'][cid] = save_json(self.root / (cid + '-wrapper.json'), {'checked': cid})

    def timed(self):
        value = producer(self.spec)
        for row in [*value['prerequisites'], *value['records']]:
            row['witness_sha256'] = self.proof['wrappers'][row['cell']]['sha256']
        return value

    def test_exact_full_grid_and_A_only_manifest_metadata(self):
        discovery.validate_spec(self.spec)
        for change in ['cell', 'candidate', 'ablation']:
            bad = copy.deepcopy(self.spec)
            if change == 'cell':
                bad['panels']['A'].pop()
            elif change == 'candidate':
                bad['candidates'].pop()
            else:
                bad['purpose'] = 'ablation_only'
            with self.subTest(change=change), self.assertRaises(ValueError):
                discovery.validate_spec(bad)

    def test_freeze_binds_only_A_files_and_detects_later_changes(self):
        spec = copy.deepcopy(self.spec)
        harness = self.root / 'harness'
        harness.write_bytes(b'\x7fELFharness')
        baseline, candidate = self.root / 'baseline.so', self.root / 'candidate.so'
        baseline.write_bytes(b'\x7fELFbaseline')
        candidate.write_bytes(b'\x7fELFcandidate')
        spec['harness_sha256'] = discovery.digest(harness)
        plugins = {}
        for arm in spec['arms']:
            plugins[arm] = str(baseline if arm == 'baseline' else candidate)
            spec['arms'][arm]['sha256'] = discovery.digest(Path(plugins[arm]))
        cases, witnesses = {}, {}
        for cell in spec['panels']['A']:
            cid = cell['id']
            case, witness = self.root / (cid + '.case'), self.root / (cid + '-ready.json')
            case.write_text(cid)
            cell['case_sha256'] = discovery.digest(case)
            case.with_suffix('.json').write_text(json.dumps({'oracle': 'test'}))
            witness.write_text('{"status":"ready"}')
            cases[cid], witnesses[cid] = str(case), str(witness)
        manifest_path = self.root / 'manifest.json'
        discovery.write(manifest_path, spec)
        source_check, generator = self.root / 'checks.json', self.root / 'receipt.py'
        source_check.write_text('{"focused_checks":"passed"}')
        generator.write_text('# independently reviewed test fixture')
        config = {'manifest': str(manifest_path), 'harness': str(harness), 'plugins': plugins,
                  'cases': cases, 'witnesses': witnesses, 'source_checks': [str(source_check)],
                  'receipt_generator': str(generator), 'output_root': str(self.root / 'campaign')}
        def evidence(path, *_args, **_kwargs):
            return {}, {path: discovery.digest(path)}
        with patch.object(discovery.routing, 'scratch', side_effect=lambda p: p.resolve()), \
             patch.object(discovery.routing_run, 'evidence', side_effect=evidence):
            plan = discovery.freeze(config)
        discovery.validate_plan(plan)
        self.assertEqual(set(plan['cases']), {c['id'] for c in spec['panels']['A']})
        self.assertFalse(any('B-cell' in p or 'C-cell' in p for p in plan['frozen_files']))
        candidate.write_bytes(b'\x7fELFchanged')
        with self.assertRaisesRegex(ValueError, 'frozen file changed'):
            discovery.validate_plan(plan)

    def test_point_scorer_matches_legacy_A_without_bootstrap_claim(self):
        spec = manifest((1, 2))
        raw = records(spec, 'A', times={a: .85 + .025 * i for i, a in enumerate(spec['candidates'])})
        times, outcomes, _ = routing.observations(spec, 'A', spec['candidates'], raw, None)
        with patch('time_exact.BOOTSTRAP_DRAWS', 100):
            old = routing.evaluate(spec, 'A', raw)['ranking']
        new = discovery.score(spec, times, outcomes)
        keys = ['arm', 'baseline_violations', 'worst_internal_normalized', 'geomean_baseline_ratio']
        self.assertEqual([{k: r[k] for k in keys} for r in old], [{k: r[k] for k in keys} for r in new])
        self.assertNotIn('bootstrap', json.dumps(new))

    def test_absolute_allowance_and_paired_ratios_are_preserved(self):
        spec = manifest()
        times, outcomes, _ = routing.observations(spec, 'A', spec['candidates'], records(spec, 'A'), None)
        cid = spec['panels']['A'][0]['id']
        for arm in spec['arms']:
            times[cid, arm] = [.0001, .001, .01] * 3
        times[cid, 'h4-k0'] = [.0011, .002, .011] * 3
        row = next(r for r in discovery.score(spec, times, outcomes) if r['arm'] == 'h4-k0')
        protected = row['comparisons'][0]
        self.assertEqual(protected['upper_allowance'], 2.)
        self.assertEqual(protected['median_ratio'], 2.)
        self.assertEqual(row['baseline_violations'], 0)

    def test_new_failure_and_cross_round_fingerprint_changes_block(self):
        expected = discovery.prerequisite_status(self.spec, self.original)
        exact = discovery.expected_fingerprints(self.proof, self.original, expected)
        raw = self.timed()
        discovery.round_data(self.plan, self.spec, raw, expected, exact)
        for key in ['science_sha256', 'witness_sha256', 'prepared_sha256', 'platform', 'status']:
            bad = copy.deepcopy(raw)
            if key == 'platform':
                bad[key] = 'Darwin'
            elif key == 'status':
                bad['records'][0]['status'] = 'blocked'
            else:
                bad['records'][0][key] = h('changed')
            with self.subTest(key=key), self.assertRaises(ValueError):
                discovery.round_data(self.plan, self.spec, bad, expected, exact)

    def test_optional_linux_failure_is_carried_with_origin_bound_and_no_calls(self):
        cid, arm = self.spec['panels']['A'][0]['id'], 'lsmr'
        original = copy.deepcopy(self.original)
        before = next(r for r in original['prerequisites'] if (r['cell'], r['arm']) == (cid, arm))
        before.update(status='unavailable', failure={'kind': 'timeout', 'dispatched': True,
            'call_limit_s': 120.0, 'elapsed_s': 120.1, 'log_sha256': h('timeout-log'),
            'prepared_sha256': before['prepared_sha256']})
        after = next(r for r in original['records'] if (r['cell'], r['arm']) == (cid, arm))
        after.update(status='not_run', blocked_by_sha256=routing.observation_identity(before))
        expected = discovery.prerequisite_status(self.spec, original)
        exact = discovery.expected_fingerprints(self.proof, original, expected)
        raw = self.timed()
        carried = next(r for r in raw['prerequisites'] if (r['cell'], r['arm']) == (cid, arm))
        carried.clear()
        carried.update(before, observation_source='prior_prerequisite', prior_proof={'test': 'proof'})
        for row in raw['records']:
            if (row['cell'], row['arm']) == (cid, arm):
                row.update(status='not_run', blocked_by_sha256=routing.observation_identity(carried))
        times = discovery.round_data(self.plan, self.spec, raw, expected, exact)
        self.assertIsNone(times[cid, arm])
        ranking = discovery.score(self.spec, times, expected)
        self.assertTrue(all(any(c['status'] == 'availability_dominance' for c in r['comparisons']) for r in ranking))
        carried['failure'] = {**carried['failure'], 'log_sha256': h('new failure')}
        for row in raw['records']:
            if (row['cell'], row['arm']) == (cid, arm):
                row['blocked_by_sha256'] = routing.observation_identity(carried)
        with self.assertRaisesRegex(ValueError, 'original Linux failure'):
            discovery.round_data(self.plan, self.spec, raw, expected, exact)

    def test_mac_prerequisites_and_missing_candidate_science_cannot_carry(self):
        discovery.validate_prerequisite_identity(self.plan, self.original)
        bad = copy.deepcopy(self.original)
        bad['platform'] = 'Darwin'
        with self.assertRaisesRegex(ValueError, 'Mac evidence'):
            discovery.validate_prerequisite_identity(self.plan, bad)
        bad = copy.deepcopy(self.original)
        bad['records'][1]['science_pass'] = False
        with self.assertRaisesRegex(ValueError, 'prerequisite failure'):
            discovery.prerequisite_status(self.spec, bad)

    def collect_with(self, broken_round=None):
        calls = []
        def invoke(plan, spec, wrappers, output):
            calls.append(output.name)
            output.mkdir()
            result = self.timed()
            if len(calls) == broken_round:
                result['records'][0]['status'] = 'blocked'
                result['status'] = 'complete_with_failed_cells'
            save_json(output / 'records.json', result)
            return result
        with patch.object(discovery, 'validate_plan', return_value=self.spec), \
             patch.object(discovery, 'host', return_value=self.machine), \
             patch.object(discovery.routing_run, 'evidence'), \
             patch.object(discovery, 'invoke', side_effect=invoke):
            result = discovery.collect(self.plan, self.proof, self.root / 'collection')
        return result, calls

    def test_three_complete_rounds_and_no_timing_selection_during_collection(self):
        with patch.object(discovery, 'score', side_effect=AssertionError('early ranking')):
            result, calls = self.collect_with()
        self.assertEqual(calls, ['round-1', 'round-2', 'round-3'])
        self.assertEqual(result['status'], 'complete')
        self.assertEqual(len(result['rounds']), 3)
        self.assertGreater(result['collection_pid'], 0)
        self.assertLess(result['started_monotonic_ns'], result['completed_monotonic_ns'])
        self.assertLess(result['started_wall_time_ns'], result['completed_wall_time_ns'])

    def test_failed_first_round_is_retained_and_never_retried(self):
        result, calls = self.collect_with(broken_round=1)
        self.assertEqual(calls, ['round-1'])
        self.assertEqual(result['status'], 'incomplete')
        self.assertLess(result['started_monotonic_ns'], result['completed_monotonic_ns'])
        self.assertTrue(discovery.checked(result['rounds'][0]).is_file())
        self.assertFalse((self.root / 'collection/round-2').exists())
        with self.assertRaises(FileExistsError):
            self.collect_with()

    def test_nine_slots_are_aggregated_only_after_complete_receipt(self):
        result, _ = self.collect_with()
        with patch.object(discovery, 'validate_plan', return_value=self.spec), \
             patch.object(discovery.routing_run, 'evidence'), \
             patch.object(discovery, 'isolation') as gate:
            out = discovery.aggregate(self.plan, self.proof, result, {'external': 'test'})
            gate.assert_called_once()
            self.assertEqual(out['status'], 'discovery_only')
            self.assertFalse(out['promotion'])
            self.assertEqual(len(out['shortlist']), 4)
            self.assertNotIn('stage', out)
            with self.assertRaises(ValueError):
                routing.required_candidates(self.spec, 'B', out)
            for refs in [[result['rounds'][0]] * 3, list(reversed(result['rounds']))]:
                bad = {**result, 'rounds': refs}
                with self.assertRaisesRegex(ValueError, 'distinct prospectively frozen'):
                    discovery.aggregate(self.plan, self.proof, bad, {})
            result['rounds'].pop()
            with self.assertRaisesRegex(ValueError, 'incomplete discovery'):
                discovery.aggregate(self.plan, self.proof, result, {})

    def test_missing_or_changed_library_limits_refuse_before_any_native_call(self):
        for limits in [{}, {**discovery.LIBRARY_LIMITS, 'OMP_NUM_THREADS': '2'}]:
            with patch.object(discovery, 'validate_plan', return_value=self.spec), \
                 patch.object(discovery, 'host', return_value={**self.machine, 'library_limits': limits}), \
                 patch.object(discovery, 'invoke') as call:
                with self.assertRaisesRegex(ValueError, 'library thread limits'):
                    discovery.collect(self.plan, self.proof, self.root / 'collection')
                call.assert_not_called()

    def receipt(self, collection):
        generator = self.root / 'receipt-generator.py'
        generator.write_text('# reviewed source\n')
        self.plan['receipt_generator'] = discovery.reference(generator)
        raw = self.root / 'scheduler.raw'
        raw.write_text('external observation\n')
        return {'kind': discovery.KIND + '_isolation', 'plan_sha256': routing.identity(self.plan),
                'collection_sha256': routing.identity(collection), 'host': self.machine,
                'rounds': collection['rounds'], 'generator': self.plan['receipt_generator'],
                'raw_evidence': {key: discovery.reference(raw) for key in discovery.EVIDENCE},
                'reservation': {'num_proc': 28, 'slots': 28, 'pe_allocation_rule': '$pe_slots',
                                'rqs_enabled': True, 'rqs_all_queues': True, 'rqs_limit': 'num_proc', 'single_host': True},
                'accounting': {'failed': 0, 'exit_status': 0}, 'observed_compute_overlap': [],
                'observation_complete': True}

    def test_external_receipt_requires_bindings_configuration_and_process_evidence(self):
        collection = {'host': self.machine, 'rounds': [{'sha256': h(str(i)), 'path': str(i)} for i in range(3)]}
        receipt = self.receipt(collection)
        verified = {'schema': 1, 'status': 'verified', 'plan_sha256': routing.identity(self.plan),
                    'collection_sha256': routing.identity(collection), 'receipt_sha256': routing.identity(receipt)}
        with patch.object(discovery.subprocess, 'run', return_value=SimpleNamespace(returncode=0, stdout=json.dumps(verified))) as run:
            discovery.isolation(self.plan, collection, receipt)
            self.assertEqual(run.call_args.kwargs['timeout'], 30)
            self.assertEqual(json.loads(run.call_args.kwargs['input'])['receipt'], receipt)
        for stdout, code in [('{}', 0), (json.dumps(verified), 1), ('noise\n' + json.dumps(verified), 0)]:
            with patch.object(discovery.subprocess, 'run', return_value=SimpleNamespace(returncode=code, stdout=stdout)):
                with self.assertRaises(ValueError):
                    discovery.isolation(self.plan, collection, receipt)
        for change in ['missing', 'quota', 'accounting', 'process', 'incomplete', 'round', 'generator']:
            bad = copy.deepcopy(receipt)
            if change == 'missing':
                bad['raw_evidence'].pop('scheduler')
            elif change == 'quota':
                bad['reservation']['rqs_all_queues'] = False
            elif change == 'accounting':
                bad['accounting']['exit_status'] = 1
            elif change == 'process':
                bad['observed_compute_overlap'] = ['other build']
            elif change == 'incomplete':
                bad['observation_complete'] = False
            elif change == 'round':
                bad['rounds'].pop()
            else:
                bad['generator']['sha256'] = h('different source')
            with self.subTest(change=change), self.assertRaises(ValueError):
                discovery.isolation(self.plan, collection, bad)

    def test_bound_file_changes_refuse_and_sealed_output_cannot_overwrite(self):
        path = self.root / 'source'
        path.write_text('original')
        ref = discovery.reference(path)
        path.write_text('changed')
        with self.assertRaisesRegex(ValueError, 'frozen file changed'):
            discovery.checked(ref)
        target = self.root / 'sealed.json'
        discovery.write(target, {'kind': 'test'})
        self.assertEqual(discovery.load(target), {'kind': 'test'})
        with self.assertRaises(FileExistsError):
            discovery.write(target, {})


if __name__ == '__main__':
    unittest.main()
