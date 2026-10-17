"""Contract tests for bounded compact-cell discovery; no native builds or jobs."""
import json
import copy
import hashlib
from contextlib import ExitStack
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compact_cells as campaign


class CompactCellsContract(unittest.TestCase):
    def test_six_complete_balanced_deterministic_blocks(self):
        ids = [f'{f}-t{w}' for f in campaign.FAMILIES for w in [1, 4]]
        blocks = campaign.schedule(ids)
        self.assertEqual(blocks, campaign.schedule(ids))
        self.assertEqual(len(blocks), 6)
        for block in blocks:
            self.assertEqual(len(block), 96)
            self.assertEqual({(r['cell'], r['arm']) for r in block},
                             {(cid, arm) for cid in ids for arm in campaign.ARMS})
        for cid in ids:
            positions = {arm: [0] * 4 for arm in campaign.ARMS}
            for block in blocks:
                for index, row in enumerate(r for r in block if r['cell'] == cid):
                    positions[row['arm']][index] += 1
            self.assertTrue(all(max(counts) - min(counts) == 1 for counts in positions.values()))
        self.assertNotEqual([r['cell'] for r in blocks[0]], [r['cell'] for r in blocks[1]])

    def test_bounded_revealed_shapes_include_weighted_and_zero(self):
        for family in campaign.FAMILIES:
            values, weights, intercept = campaign.columns(family)
            self.assertTrue(0 < len(values[0]) <= 100000)
            self.assertTrue(all(len(v) == len(values[0]) for v in values))
            if family == 'weight-dynamic':
                self.assertGreater(weights.max() / weights.min(), 1e6)
            if family == 'inactive-zero':
                self.assertFalse(intercept)
                for value in values[:-2]:
                    self.assertTrue((value[::2] == -value[1::2]).all())
            if family in {'wide', 'low'}:
                self.assertEqual(len(values), 19)

    def test_refuses_changed_frozen_input(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'input'
            path.write_text('a')
            bound = campaign.ref(path)
            self.assertEqual(campaign.checked(bound), path.resolve())
            path.write_text('b')
            with self.assertRaisesRegex(ValueError, 'frozen file changed'):
                campaign.checked(bound)

    def test_native_timeout_is_retained_and_has_fixed_affinity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plan = {'plugin': {'path': '/plugin'}, 'harness': {'path': '/harness'}}
            cell = {'id': 'weak-t1', 'inputs': {'cells_v2': {'path': '/input'}}}
            with patch.object(campaign.subprocess, 'run', side_effect=subprocess.TimeoutExpired('x', 120)) as run:
                row = campaign.native(plan, cell, 'cells_v2', root, 'trial', cpus=[0, 1, 2, 3])
            self.assertEqual(row['status'], 'timeout')
            self.assertEqual(run.call_args.args[0][:3], ['taskset', '-c', '0,1,2,3'])
            self.assertEqual(run.call_args.kwargs['timeout'], 120)
            recorded = json.loads((root / 'outcomes.jsonl').read_text())
            self.assertEqual(recorded['status'], 'timeout')
            self.assertTrue((root / 'trial.log').is_file())

    def test_summary_retains_unfavorable_blocks_without_minimum_selection(self):
        rows = []
        for block, candidate in enumerate([1., 2., 4., 8., 16., 32.]):
            for arm in campaign.ARMS:
                rows.append({'cell': 'weak-t1', 'block': block, 'arm': arm,
                             'phase': 'measure', 'status': 'passed',
                             'seconds': 1. if arm == 'baseline' else candidate})
        result = campaign.summarize(rows)
        self.assertEqual(len(result), 3)
        self.assertEqual(result[0]['paired_blocks'], 6)
        self.assertEqual(result[0]['median_ratio'], 6.)
        self.assertEqual(result[0]['maximum_ratio'], 32.)
        rows[-1]['status'] = 'failed'
        result = campaign.summarize(rows)
        self.assertTrue(all(r['paired_blocks'] == 5 for r in result))
        self.assertEqual(rows[-1]['status'], 'failed')

    def test_smoke_cannot_authorize_measurement(self):
        plan = {'cells': []}
        receipt = {'kind': campaign.KIND + '_science', 'status': 'passed', 'smoke': True,
                   'plan_sha256': campaign.routing.identity(plan), 'fingerprints': {}, 'files': []}
        with self.assertRaisesRegex(ValueError, 'complete frozen science'):
            campaign.validate_science(plan, receipt)

    def test_science_stops_at_first_failed_repeat_without_retry(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'science'
            cell = {'id': 'weak-t1', 'family': 'weak', 'threads': 1,
                    'metadata': {'path': 'meta'}, 'witness': {'path': 'witness'}}
            plan = {'cells': [cell]}
            rows = [{'cell': 'weak-t1', 'arm': 'baseline', 'label': str(i),
                     'target': 'target', 'log': 'log', 'status': 'executed'} for i in range(2)]
            meta = {'case_sha256': 'a', 'body_sha256': 'b'}
            results = [({}, {'science_sha256': str(i)}, Path('columns')) for i in range(2)]
            with patch.dict(campaign.os.environ, {'JOB_ID': '1', 'NSLOTS': '2'}), \
                 patch.object(campaign, 'validate'), patch.object(campaign, 'read', return_value=meta), \
                 patch.object(campaign.routing_oracle, 'load_reference', return_value=({}, {})), \
                 patch.object(campaign.routing_oracle, 'check_reference'), \
                 patch.object(campaign.cmgschur, 'scientific_comparison'), \
                 patch.object(campaign.routing_run, 'resource_diagnostics', return_value={}), \
                 patch.object(Path, 'read_text', return_value=''), \
                 patch.object(campaign, 'native', side_effect=rows) as native, \
                 patch.object(campaign, 'parse_run', side_effect=results):
                result = campaign.science(plan, output, smoke=True)
            self.assertEqual(native.call_count, 2)
            self.assertEqual(result['status'], 'failed')
            self.assertIn('fixed-worker repeat differs', result['failure'])
            self.assertEqual(len(result['records']), 2)

    def test_public_baseline_and_private_arms_are_fixed(self):
        self.assertEqual(campaign.ARMS, ['baseline', 'cells', 'diagonal', 'cells_v2'])
        self.assertEqual(campaign.LIMITS['measurement_budget_s'], 1800)
        self.assertEqual(campaign.LIMITS['seed'], 2026101603)
        self.assertEqual(campaign.LIMITS['workers'], [1, 4])
        self.assertEqual(campaign.LIMITS['call_timeout_s'], 120)
        self.assertTrue(all(value == '1' for value in campaign.LIBRARY_LIMITS.values()))

    def mock_measure(self, output, *, budget=False, warmup_budget=False, bad_boundary=None, fail_science=True):
        cell = {'id': 'weak-t1', 'metadata': {'path': 'meta'}}
        plan = {'cells': [cell], 'blocks': [[{'cell': 'weak-t1', 'arm': a} for a in campaign.ARMS]]}
        receipt = {'records': [{'cell': 'weak-t1', 'arm': a, 'label': 'science'} for a in campaign.ARMS]}
        host = {'affinity': list(range(28)), 'hostname': 'test', 'job_id': '1'}
        config = {k: {} for k in ['job_environment', 'parallel_environment', 'resource_quota',
                                 'queue_configuration']}
        calls, boundaries = [], []

        def native(plan, cell, arm, directory, label, **kwargs):
            calls.append(label)
            return {'cell': cell['id'], 'arm': arm, 'label': label, 'log': 'log',
                    'status': 'executed'}

        def parse(row, meta):
            # Warmup may validly differ from a separate CPU's prerequisite;
            # the first measured mismatch must stop before any second call.
            fingerprint = 'warmup' if row['label'].startswith('warmup') or not fail_science else 'different'
            return {}, {'science_sha256': fingerprint, 'seconds': .1}, Path('columns')

        def configuration(directory, machine, stage):
            boundaries.append(stage)
            if stage == bad_boundary:
                raise ET.ParseError('malformed scheduler XML')
            return config, {}

        with ExitStack() as stack:
            for owner, name, options in [
                (campaign, 'validate', {}), (campaign, 'validate_science', {}),
                (campaign, 'native', {'side_effect': native}),
                (campaign, 'parse_run', {'side_effect': parse}),
                (campaign, 'read', {'return_value': {}}),
                (campaign, 'Observers', {}),
                (campaign, 'check_stable_configuration', {}),
                (campaign.scc.discovery, 'host', {'return_value': host}),
                (campaign.scc, 'configuration', {'side_effect': configuration}),
                (campaign.scc, 'check_configuration', {'return_value': 'queue'}),
                (campaign.scc, 'check_scheduler_snapshot', {}),
                (campaign.cmgschur, 'scientific_comparison', {}),
                (campaign.routing_run, 'resource_diagnostics', {'return_value': {}}),
                (Path, 'read_text', {'return_value': ''}),
            ]:
                mocked = stack.enter_context(patch.object(owner, name, **options))
                if name == 'Observers':
                    mocked.return_value.errors = []
            stack.enter_context(patch.dict(campaign.os.environ, campaign.LIBRARY_LIMITS))
            if budget:
                stack.enter_context(patch.object(campaign.time, 'monotonic_ns',
                                                side_effect=[0, 0, 1801_000_000_000, 1801_000_000_000]))
            if warmup_budget:
                stack.enter_context(patch.object(campaign.time, 'monotonic', side_effect=[0, 0, 0, 0, 0, 1801]))
            result = campaign.measure(plan, receipt, output)
        return result, calls, boundaries

    def test_measurement_science_failure_stops_and_retains_end_configuration(self):
        with tempfile.TemporaryDirectory() as directory:
            result, calls, boundaries = self.mock_measure(Path(directory) / 'measure')
            self.assertEqual(len(calls), 5)  # Four warmups, then one failed measurement.
            self.assertEqual(result['status'], 'failed')
            self.assertIn('same-allocation', result['failure'])
            self.assertEqual(result['records'][-1]['status'], 'failed')
            self.assertEqual(boundaries, ['before', 'after'])
            self.assertEqual(result['summaries'], [])

    def test_measurement_budget_exhaustion_does_not_launch_a_call(self):
        with tempfile.TemporaryDirectory() as directory:
            result, calls, boundaries = self.mock_measure(Path(directory) / 'measure', budget=True)
            self.assertEqual(len(calls), 4)
            self.assertEqual(result['status'], 'failed')
            self.assertEqual(result['failure'], 'measurement budget exhausted')
            self.assertEqual(boundaries, ['before', 'after'])

    def test_final_warmup_validation_overrun_blocks_measurement(self):
        with tempfile.TemporaryDirectory() as directory:
            result, calls, _ = self.mock_measure(Path(directory) / 'measure', warmup_budget=True)
            self.assertEqual(len(calls), 4)
            self.assertEqual(result['status'], 'failed')
            self.assertEqual(result['failure'], 'warmup budget exhausted')
            self.assertTrue(all(row['phase'] == 'warmup' for row in result['records']))

    def test_final_science_validation_overrun_fails_prerequisite(self):
        with tempfile.TemporaryDirectory() as directory:
            cell = {'id': 'weak-t1', 'family': 'weak', 'threads': 1,
                    'metadata': {'path': 'meta'}, 'witness': {'path': 'witness'}}
            plan = {'cells': [cell]}
            rows = [{'cell': 'weak-t1', 'arm': 'baseline', 'label': str(i),
                     'target': 'target', 'log': 'log', 'status': 'executed'} for i in range(2)]
            with patch.dict(campaign.os.environ, {'JOB_ID': '1', 'NSLOTS': '2'}), \
                 patch.object(campaign, 'ARMS', ['baseline']), patch.object(campaign, 'validate'), \
                 patch.object(campaign, 'read', return_value={'case_sha256': 'a', 'body_sha256': 'b'}), \
                 patch.object(campaign.routing_oracle, 'load_reference', return_value=({}, {})), \
                 patch.object(campaign.routing_oracle, 'check_reference'), \
                 patch.object(campaign.cmgschur, 'scientific_comparison'), \
                 patch.object(campaign.routing_run, 'resource_diagnostics', return_value={}), \
                 patch.object(Path, 'read_text', return_value=''), \
                 patch.object(campaign, 'native', side_effect=rows), \
                 patch.object(campaign, 'parse_run', return_value=({}, {'science_sha256': 'same'}, Path('columns'))), \
                 patch.object(campaign.time, 'monotonic', side_effect=[0, 0, 0, 1801, 1801]):
                result = campaign.science(plan, Path(directory) / 'science', smoke=True)
            self.assertEqual(result['status'], 'failed')
            self.assertEqual(result['failure'], 'science budget exhausted')
            self.assertTrue(all(row['status'] == 'executed' for row in result['records']))

    def test_terminal_marker_requires_success(self):
        for status in ['passed', 'failed']:
            with patch.object(sys, 'argv', ['compact_cells.py', 'smoke', '--plan', '/plan', '--output', '/output']), \
                 patch.object(campaign, 'read', return_value=campaign.routing.seal({})), \
                 patch.object(campaign, 'science', return_value={'status': status}), \
                 patch('builtins.print') as output:
                if status == 'passed':
                    campaign.main()
                    output.assert_called_once_with('COMPACT_CELLS_SUCCESS stage=smoke', flush=True)
                else:
                    with self.assertRaisesRegex(ValueError, 'campaign failed'):
                        campaign.main()
                    output.assert_not_called()

    def test_background_malformed_xml_and_missing_fields_fail_closed(self):
        for error in [ET.ParseError('broken XML'), KeyError('missing job field')]:
            with self.subTest(error=type(error).__name__), tempfile.TemporaryDirectory() as directory:
                observer = campaign.Observers(Path(directory), {}, 'queue')
                with patch.object(observer.stop, 'wait', return_value=False), \
                     patch.object(observer, 'census', side_effect=error):
                    observer.loop(60, lambda: observer.census('periodic'))
                self.assertEqual(observer.errors, [str(error)])
                record = json.loads((Path(directory) / 'observer-errors.jsonl').read_text())
                self.assertEqual(record['type'], type(error).__name__)
                with self.assertRaisesRegex(ValueError, 'isolation observer failed'):
                    observer.validate(1, 2)

    def test_malformed_boundary_xml_writes_failure_receipt(self):
        for boundary in ['before', 'after']:
            with self.subTest(boundary=boundary), tempfile.TemporaryDirectory() as directory:
                result, _, _ = self.mock_measure(Path(directory) / 'measure', bad_boundary=boundary,
                                                  fail_science=False)
                self.assertEqual(result['status'], 'failed')
                self.assertEqual(result['failure'], 'malformed scheduler XML')
                self.assertTrue((Path(directory) / 'measure/measurement.json').is_file())

    def test_process_coverage_requires_valid_bracketing_samples_without_gaps(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            def sample(at):
                commands = {}
                for key, fields in [('ps', campaign.scc.PS_FIELDS), ('args', campaign.scc.PS_ARGS)]:
                    body = (f'{os.getpid()} 1 Fri Oct 16 10:00:00 2026 S 00:00:01 0.0 123 python\n'
                            if key == 'ps' else f'{os.getpid()} Fri Oct 16 10:00:00 2026 python compact_cells.py\n')
                    commands[key] = {'argv': ['ps', '-axo', fields], 'returncode': 0, 'locale': 'C',
                                     'timezone': 'UTC', 'stdout': body}
                return {'kind': 'snapshot', 'monotonic_ns': at, 'finished_monotonic_ns': at + 1,
                        'wall_time_ns': at, **commands}
            observer = campaign.Observers(root, {}, 'queue')
            path = root / 'processes.jsonl'
            for at in [1_000_000_000, 5_000_000_000]:
                campaign.scc.append(path, sample(at))
            with patch.object(campaign.scc, 'check_scheduler'):
                observer.validate(2_000_000_000, 4_000_000_000)
                with self.assertRaisesRegex(ValueError, 'bracket'):
                    observer.validate(2_000_000_000, 6_000_000_000)
                campaign.scc.append(path, sample(25_000_000_000))
                with self.assertRaisesRegex(ValueError, 'coverage gap'):
                    observer.validate(2_000_000_000, 24_000_000_000)

    def test_malformed_successful_process_output_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            observer = campaign.Observers(root, {}, 'queue')
            for at in [1, 5]:
                campaign.scc.append(root / 'processes.jsonl', {
                    'kind': 'snapshot', 'monotonic_ns': at, 'finished_monotonic_ns': at + 1,
                    'wall_time_ns': at, 'ps': {'argv': ['ps', '-axo', campaign.scc.PS_FIELDS],
                    'returncode': 0, 'locale': 'C', 'timezone': 'UTC', 'stdout': 'not a parseable ps row'}})
            with patch.object(campaign.scc, 'check_scheduler'):
                with self.assertRaisesRegex(ValueError, 'unparsed ps row'):
                    observer.validate(2, 4)


class FactorStudyContract(unittest.TestCase):
    def test_five_arm_order_and_fixed_diagnostic_counts(self):
        ids = [f'{f}-t{w}' for f in campaign.FAMILIES for w in [1, 4]]
        blocks = campaign.schedule(ids, arms=campaign.FACTOR_ARMS, limits=campaign.FACTOR_LIMITS)
        self.assertEqual(blocks, campaign.schedule(ids, arms=campaign.FACTOR_ARMS, limits=campaign.FACTOR_LIMITS))
        self.assertEqual([len(block) for block in blocks], [120] * 6)
        for cid in ids:
            positions = {arm: [0] * 5 for arm in campaign.FACTOR_ARMS}
            for block in blocks:
                rows = [r for r in block if r['cell'] == cid]
                self.assertEqual({r['arm'] for r in rows}, set(campaign.FACTOR_ARMS))
                for index, row in enumerate(rows):
                    positions[row['arm']][index] += 1
            self.assertTrue(all(sorted(counts) == [1, 1, 1, 1, 2] for counts in positions.values()))
        self.assertNotEqual(campaign.FACTOR_LIMITS['seed'], campaign.LIMITS['seed'])
        self.assertEqual(campaign.FACTOR_LIMITS['data_seed'], campaign.LIMITS['seed'])
        self.assertEqual(campaign.FACTOR_LIMITS['diagnostic_repeats'], 2)
        self.assertEqual(campaign.FACTOR_LIMITS['diagnostic_families'],
                         ['dense', 'repeated', 'hubs', 'high-degree-weak'])
        self.assertEqual(campaign.FACTOR_LIMITS['diagnostic_budget_s'], 1800)

    @staticmethod
    def parent_fixture(root):
        old = root / 'parent'
        old.mkdir()
        frozen, cells = [], []
        executables = {}
        for name in ['generator', 'harness', 'plugin']:
            path = old / name
            path.write_text('original-' + name)
            executables[name] = campaign.ref(path)
            frozen.append(executables[name])
        for family in campaign.FAMILIES:
            source = old / (family + '.case')
            source.write_text(family)
            metadata, witness, arrays = [old / (family + suffix) for suffix in ['.json', '.oracle.json', '.npz']]
            campaign.save(metadata, {'case_sha256': campaign.digest(source),
                                     'body_sha256': campaign.digest(source)})
            witness.write_text('{}')
            arrays.write_text('frozen-independent-witness-' + family)
            frozen.extend(campaign.ref(p) for p in [source, metadata, witness, arrays])
            for workers in [1, 4]:
                cid, inputs = f'{family}-t{workers}', {}
                for arm in campaign.ARMS:
                    path = old / f'{cid}-{arm}.case'
                    path.write_text(f'{family}|{arm}|{workers}')
                    inputs[arm] = campaign.ref(path)
                    frozen.append(inputs[arm])
                cells.append({'id': cid, 'family': family, 'threads': workers,
                              'metadata': campaign.ref(metadata), 'witness': campaign.ref(witness), 'inputs': inputs})
        tool = root / 'tool.py'
        tool.write_text('historical tool')
        frozen.append(campaign.ref(tool))
        parent = {'kind': campaign.KIND, 'limits': campaign.LIMITS, 'arms': campaign.ARMS,
                  'families': campaign.FAMILIES, 'library_limits': campaign.LIBRARY_LIMITS,
                  **executables, 'cells': cells, 'blocks': campaign.schedule([c['id'] for c in cells]),
                  'frozen_files': {r['path']: r['sha256'] for r in frozen}}
        path = old / 'plan.json'
        campaign.save(path, campaign.routing.seal(parent))
        tool.write_text('new tool closure')
        return parent, path, tool

    def test_factor_prepare_reuses_parent_bytes_and_only_creates_new_requests(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            parent, parent_path, tool = self.parent_fixture(root)
            before = {str(p): p.read_bytes() for p in parent_path.parent.iterdir()}
            harness, plugin = root / 'new-harness', root / 'new-plugin'
            harness.write_text('new harness')
            plugin.write_text('new plugin')

            def reference(path, *_):
                arrays = path.with_name(path.name.replace('.oracle.json', '.npz'))
                return {'_arrays_path': arrays}, {p: campaign.digest(p) for p in
                    [path, arrays, Path(campaign.routing_oracle.__file__), campaign.routing_oracle.PARSER_SOURCE]}

            def prepare(source, target, arm, workers):
                self.assertEqual(arm, 'cells_v2_df')
                target.write_bytes(source.read_bytes() + b'|new-df-request')

            def inspect(path):
                return {'body_sha256': hashlib.sha256(path.read_bytes().split(b'|')[0]).hexdigest()}

            with patch.object(campaign, 'tool_files', return_value=[tool]), \
                 patch.object(campaign.routing_oracle, 'load_reference', side_effect=reference), \
                 patch.object(campaign.routing, 'prepare', side_effect=prepare) as preparation, \
                 patch.object(campaign.routing_cases, 'inspect_case', side_effect=inspect), \
                 patch.object(campaign, 'columns', side_effect=AssertionError('must not generate data')), \
                 patch.object(campaign.routing_oracle, 'write_reference', side_effect=AssertionError('must not solve oracle')):
                plan = campaign.prepare_factor(root / 'factor', parent_path, harness, plugin)
                self.assertEqual(preparation.call_count, 24)
                self.assertEqual(len(list((root / 'factor').iterdir())), 25)
                self.assertEqual(plan['parent_plan_sha256'], campaign.routing.identity(parent))
                self.assertEqual(plan['frozen_files'][str(tool.resolve())], campaign.digest(tool))
                for old, new in zip(parent['cells'], plan['cells']):
                    self.assertEqual({a: new['inputs'][a] for a in campaign.ARMS}, old['inputs'])
                    self.assertEqual(new['witness'], old['witness'])
                    self.assertEqual(new['metadata'], old['metadata'])
                campaign.validate(plan)
                for key in ['plugin', 'harness', 'generator']:
                    self.assertIn(parent[key]['path'], plan['parent_dependencies'])
                self.assertEqual(before, {str(p): p.read_bytes() for p in parent_path.parent.iterdir()})
                changed = Path(parent['cells'][0]['inputs']['cells']['path'])
                changed.write_text('changed')
                with self.assertRaisesRegex(ValueError, 'frozen file changed'):
                    campaign.validate(plan)

    def test_cross_checkout_parent_uses_real_loader_and_retains_artifact_guards(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            parent, parent_path, _ = self.parent_fixture(root)
            oracle, np = campaign.routing_oracle, campaign.np
            current_tools = [Path(oracle.__file__).resolve(), oracle.PARSER_SOURCE.resolve()]
            old_tools = parent_path.parent / 'bench'
            old_tools.mkdir()
            for tool in current_tools:
                old = old_tools / tool.name
                old.write_bytes(tool.read_bytes())
                parent['frozen_files'][str(old)] = campaign.digest(old)
                self.assertNotIn(str(tool), parent['frozen_files'])
                self.assertIn(tool, campaign.tool_files())
            grid = np.meshgrid(np.arange(5), np.arange(4), np.arange(3), indexing='ij')
            factors = [values.ravel() for values in grid]
            rng = np.random.default_rng(177)
            x = rng.standard_normal((60, 3))
            y = x @ np.array([.7, -.2, .4]) + np.sin(factors[0]) + rng.normal(size=60) * .2
            reference, arrays = oracle.solve(y, x, factors, np.ones(60))
            for family in campaign.FAMILIES:
                path, archive = [parent_path.parent / (family + suffix) for suffix in ['.oracle.json', '.npz']]
                meta = campaign.read(path.with_name(family + '.json'))
                np.savez_compressed(archive, **arrays)
                bound = {**reference, **meta, 'arrays': {'file': archive.name, 'sha256': campaign.digest(archive)},
                         'producer_sha256': campaign.digest(current_tools[0]),
                         'parser_sha256': campaign.digest(current_tools[1])}
                path.write_text(json.dumps(bound))
                for artifact in [path, archive]:
                    parent['frozen_files'][str(artifact)] = campaign.digest(artifact)
                for cell in parent['cells']:
                    if cell['family'] == family:
                        cell['witness'] = campaign.ref(path)
            parent_path.write_text(json.dumps(campaign.routing.seal(parent)))
            original_files = {p: p.read_bytes() for p in parent_path.parent.rglob('*') if p.is_file()}
            def inspect(path):
                return {'body_sha256': hashlib.sha256(path.read_bytes().split(b'|')[0]).hexdigest()}
            first = parent['cells'][0]
            path = Path(first['witness']['path'])
            archive = path.with_name('weak.npz')
            meta = campaign.read(first['metadata']['path'])
            _, real_bindings = oracle.load_reference(path, meta['case_sha256'], meta['body_sha256'])
            self.assertTrue(all(tool in real_bindings for tool in current_tools))
            with patch.object(campaign.routing_cases, 'inspect_case', side_effect=inspect):
                bindings = campaign.parent_dependencies(parent)
                self.assertTrue(all(str(tool) not in bindings for tool in current_tools))
                self.assertEqual(bindings[str(archive)], campaign.digest(archive))
                self.assertEqual(bindings[str(path)], campaign.digest(path))
                self.assertEqual(original_files, {p: p.read_bytes() for p in original_files})
                for artifact, message in [(path, 'frozen file changed'), (archive, 'oracle arrays changed')]:
                    artifact.write_bytes(original_files[artifact] + b'changed')
                    with self.assertRaisesRegex(ValueError, message):
                        campaign.parent_dependencies(parent)
                    artifact.write_bytes(original_files[artifact])
                for key in ['producer_sha256', 'parser_sha256']:
                    invalid = campaign.read(path)
                    invalid[key] = '0' * 64
                    path.write_text(json.dumps(invalid))
                    changed_parent = copy.deepcopy(parent)
                    for cell in changed_parent['cells']:
                        if cell['family'] == 'weak':
                            cell['witness'] = campaign.ref(path)
                    changed_parent['frozen_files'][str(path)] = campaign.digest(path)
                    with self.assertRaisesRegex(ValueError, 'oracle reference identity mismatch'):
                        campaign.parent_dependencies(changed_parent)
                    path.write_bytes(original_files[path])

    def test_native_trace_isolation_including_host_environment(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plan = {'plugin': {'path': '/plugin'}, 'harness': {'path': '/harness'}}
            cell = {'id': 'dense-t1', 'inputs': {'cells_v2_df': {'path': '/input'}}}
            with patch.dict(os.environ, {key: 'inherited' for key in campaign.routing_run.CLEARED_ENV}), \
                 patch.object(campaign.subprocess, 'run', return_value=Mock(returncode=0)) as run:
                for label, options in [('scored', {}), ('diagnostic', {'traced': True, 'phase_timing': True}),
                                       ('scored-after', {})]:
                    campaign.native(plan, cell, 'cells_v2_df', root, label, **options)
                    command = run.call_args.args[0]
                    for key in campaign.routing_run.CLEARED_ENV:
                        self.assertIn(['-u', key], [command[i:i+2] for i in range(len(command)-1)])
                    self.assertEqual('FEREG_PHASE_TIMING=1' in command, label == 'diagnostic')
                    self.assertEqual('FEREG_SOLVER_TRACE=1' in command, label == 'diagnostic')
                    self.assertEqual(command[-5:], ['/harness', '/plugin', '--replay', '/input', str(root / (label + '.json'))])

    @staticmethod
    def result(iterations=4, fallback=0):
        row = [0.] * 16
        row[2], row[8], row[9], row[10] = iterations, .1, .2, fallback
        return {'seconds': 1., 'science_sha256': 'same', 'solver_rows': [row, list(row)]}

    @staticmethod
    def trace(operator, recurrence, *, iterations=4, passed='true'):
        return (f'FEREG_PHASE|schur|init=0.01|operator={operator}|recurrence={recurrence}'
                f'|reconstruct=0.02|certificate=0.03|iterations={iterations}|passed={passed}\n')

    def test_cumulative_corrections_and_invocation_resets(self):
        log = (self.trace(.2, .3, passed='false')
               + 'FEREG_SOLVER_TRACE|phase=two_fe_pcg_correction|iterations=4|recurrence_tolerance=1e-12\n'
               + self.trace(.4, .5, iterations=6) + self.trace(.1, .2, iterations=2))
        parsed = campaign.phase_diagnostics(log, self.result())
        self.assertEqual([r['cycles'] for r in parsed['schur_invocations']], [2, 1])
        self.assertAlmostEqual(parsed['schur_observed_seconds']['operator'], .5)
        self.assertAlmostEqual(parsed['schur_observed_seconds']['recurrence'], .7)
        self.assertAlmostEqual(parsed['schur_observed_seconds']['init'], .03)
        self.assertAlmostEqual(parsed['schur_observed_seconds']['certificate'], .09)
        self.assertEqual(parsed['rsolver_setup_seconds'], [.1, .1])
        for bad in [log.replace('operator=0.4', 'operator=0.1'), log.replace('recurrence=0.5', 'recurrence=0.1'),
                    log.replace('iterations=6', 'iterations=3')]:
            with self.assertRaisesRegex(ValueError, 'nonmonotonic'):
                campaign.phase_diagnostics(bad, self.result())

    def test_missing_zero_and_failed_trace_are_not_zero_phase_cost(self):
        for iterations, expected in [(0, 'absent_zero_iterations'), (4, 'absent_nonzero_iterations')]:
            parsed = campaign.phase_diagnostics('', self.result(iterations))
            self.assertEqual(parsed['status'], 'missing')
            self.assertEqual(parsed['schur_status'], expected)
            self.assertIsNone(parsed['schur_observed_seconds'])
        parsed = campaign.phase_diagnostics(self.trace(.1, .2, passed='false'), self.result(fallback=1))
        self.assertEqual(parsed['schur_status'], 'partial')
        self.assertIn('fallback work may be untraced', parsed['coverage'])

    def test_diagnostics_have_exact_fixed_count_and_stop_at_first_failure(self):
        cells = [{'id': f'{f}-t{w}', 'family': f, 'threads': w}
                 for f in campaign.FAMILIES for w in [1, 4]]
        plan = {'kind': campaign.FACTOR_KIND, 'cells': cells}
        fingerprints = {c['id'] + '/' + a: 'same' for c in cells for a in campaign.FACTOR_ARMS}
        def native(plan, cell, arm, output, label, **kwargs):
            self.assertTrue(kwargs['traced'] and kwargs['phase_timing'])
            return {'cell': cell['id'], 'arm': arm, 'label': label, 'log': 'log', 'status': 'executed'}
        with patch.object(campaign, 'native', side_effect=native) as run, \
             patch.object(campaign, 'check_prerequisite', return_value=self.result()) as science, \
             patch.object(Path, 'read_text', return_value=self.trace(.1, .2)):
            rows = []
            campaign.collect_diagnostics(plan, {}, Path('/unused'), [0, 1, 2, 3], Mock(errors=[]), fingerprints, rows)
            self.assertEqual(run.call_count, 80)
            self.assertTrue(all(r['status'] == 'passed' and r['phase'] == 'diagnostic' for r in rows))
            self.assertEqual({r['cell'].rsplit('-t', 1)[0] for r in rows}, set(campaign.FACTOR_LIMITS['diagnostic_families']))
            run.reset_mock()
            science.side_effect = ValueError('failed independent science')
            rows = []
            with self.assertRaisesRegex(ValueError, 'failed independent science'):
                campaign.collect_diagnostics(plan, {}, Path('/unused'), [], Mock(errors=[]), fingerprints, rows)
            self.assertEqual(run.call_count, 1)
            self.assertEqual(len(rows), 1)

    def test_factor_pair_summaries_require_complete_five_arm_blocks(self):
        rows = [{'phase': 'measure', 'cell': 'dense-t1', 'block': 0, 'status': 'passed', 'arm': arm,
                 'seconds': index + 1.} for index, arm in enumerate(campaign.FACTOR_ARMS)]
        result = campaign.summarize(rows, arms=campaign.FACTOR_ARMS, reference='cells_v2', candidates=['cells_v2_df'])
        self.assertEqual(result[0]['median_ratio'], 1.25)
        self.assertEqual(result[0]['reference'], 'cells_v2')
        self.assertEqual(campaign.summarize(rows[:-1], arms=campaign.FACTOR_ARMS), [])

    def test_factor_science_counts_exact_parent_controls_and_stops_on_difference(self):
        for different in [False, True]:
            with self.subTest(different=different), tempfile.TemporaryDirectory() as directory:
                cells = [{'id': f'{f}-t{w}', 'family': f, 'threads': w,
                          'metadata': {'path': 'meta'}, 'witness': {'path': 'witness'}}
                         for f in campaign.FAMILIES for w in [1, 4]]
                plan = {'kind': campaign.FACTOR_KIND, 'cells': cells,
                        'parent_plan': {'path': 'parent'}, 'plugin': {'path': 'new'}}
                parent = {'plugin': {'path': 'old'}}
                def read(path):
                    return campaign.routing.seal(parent) if path == 'parent' else {'case_sha256': 'a', 'body_sha256': 'b'}
                def native(plan, cell, arm, output, label, **kwargs):
                    self.assertEqual(plan['plugin']['path'], 'old' if label.startswith('parent-control') else 'new')
                    return {'cell': cell['id'], 'arm': arm, 'label': label, 'log': 'log', 'status': 'executed'}
                def parse(row, meta):
                    result = self.result()
                    if different and row['label'].startswith('parent-control'):
                        result['science_sha256'] = 'different'
                    return {}, result, Path('columns')
                with patch.dict(os.environ, {'JOB_ID': '1', 'NSLOTS': '4'}), \
                     patch.object(campaign, 'validate'), patch.object(campaign, 'read', side_effect=read), \
                     patch.object(campaign, 'native', side_effect=native) as run, \
                     patch.object(campaign, 'parse_run', side_effect=parse), \
                     patch.object(campaign.routing_oracle, 'load_reference', return_value=({}, {})), \
                     patch.object(campaign.routing_oracle, 'check_reference'), \
                     patch.object(campaign.cmgschur, 'scientific_comparison'), \
                     patch.object(campaign.routing_run, 'resource_diagnostics', return_value={}), \
                     patch.object(Path, 'read_text', return_value=''):
                    receipt = campaign.science(plan, Path(directory) / 'science')
                    if different:
                        self.assertEqual(run.call_count, 2)
                        self.assertEqual(receipt['status'], 'failed')
                        self.assertIn('parent/candidate control', receipt['failure'])
                        self.assertEqual(receipt['records'][-1]['status'], 'failed')
                    else:
                        self.assertEqual(run.call_count, 336)
                        self.assertEqual(receipt['status'], 'passed')
                        self.assertEqual(len(receipt['records']), 240)
                        self.assertEqual(receipt['parent_control_count'], 96)
                        campaign.validate_science(plan, receipt)
                        receipt['parent_controls'].pop()
                        with self.assertRaisesRegex(ValueError, 'incomplete parent artifact controls'):
                            campaign.validate_science(plan, receipt)

    def test_factor_measurement_finishes_scored_blocks_before_diagnostics(self):
        with tempfile.TemporaryDirectory() as directory:
            cells = [{'id': f'{f}-t{w}', 'family': f, 'threads': w, 'metadata': {'path': 'meta'}}
                     for f in campaign.FAMILIES for w in [1, 4]]
            plan = {'kind': campaign.FACTOR_KIND, 'cells': cells,
                    'blocks': campaign.schedule([c['id'] for c in cells], arms=campaign.FACTOR_ARMS,
                                                limits=campaign.FACTOR_LIMITS)}
            calls = []
            def native(plan, cell, arm, output, label, **options):
                calls.append(label)
                self.assertEqual(bool(options.get('traced')), label.startswith('diagnostic-'))
                self.assertEqual(bool(options.get('phase_timing')), label.startswith('diagnostic-'))
                return {'cell': cell['id'], 'arm': arm, 'label': label, 'log': 'log', 'status': 'executed'}
            with ExitStack() as stack:
                for owner, name, options in [
                    (campaign, 'validate', {}), (campaign, 'validate_science', {}),
                    (campaign, 'native', {'side_effect': native}),
                    (campaign, 'check_prerequisite', {'return_value': self.result()}),
                    (campaign, 'parse_run', {'return_value': ({}, self.result(), Path('columns'))}),
                    (campaign, 'read', {'return_value': {}}),
                    (campaign, 'Observers', {'return_value': Mock(errors=[])}),
                    (campaign, 'check_stable_configuration', {}),
                    (campaign.scc.discovery, 'host', {'return_value': {'affinity': list(range(28))}}),
                    (campaign.scc, 'configuration', {'return_value': ({}, {})}),
                    (campaign.scc, 'check_configuration', {'return_value': 'queue'}),
                    (campaign.scc, 'check_scheduler_snapshot', {}),
                    (Path, 'read_text', {'return_value': self.trace(.1, .2)}),
                ]:
                    stack.enter_context(patch.object(owner, name, **options))
                stack.enter_context(patch.dict(os.environ, campaign.LIBRARY_LIMITS))
                result = campaign.measure(plan, {}, Path(directory) / 'measure')
            self.assertEqual(result['status'], 'complete', result['failure'])
            self.assertEqual(len(calls), 920)
            self.assertTrue(all(label.startswith('warmup-') for label in calls[:120]))
            self.assertTrue(all(label.startswith('block-') for label in calls[120:840]))
            self.assertTrue(all(label.startswith('diagnostic-') for label in calls[840:]))
            self.assertEqual(len(result['summaries']), 96)
            self.assertEqual(len(result['factor_comparisons']), 48)
            self.assertTrue(all(row['paired_blocks'] == 6 for row in result['summaries']))

    def test_diagnostic_deadline_blocks_dispatch_and_missing_trace_stops(self):
        plan = {'kind': campaign.FACTOR_KIND, 'cells': [{'id': 'dense-t1', 'family': 'dense'}]}
        with patch.object(campaign.time, 'monotonic', side_effect=[0, 1801]), \
             patch.object(campaign, 'native') as native:
            with self.assertRaisesRegex(ValueError, 'diagnostic budget exhausted'):
                campaign.collect_diagnostics(plan, {}, Path('/unused'), [], Mock(errors=[]), {}, [])
            native.assert_not_called()
        rows = []
        with patch.object(campaign, 'native', return_value={'log': 'log', 'status': 'executed'}) as native, \
             patch.object(campaign, 'check_prerequisite', return_value=self.result()), \
             patch.object(Path, 'read_text', return_value=''):
            with self.assertRaisesRegex(ValueError, 'missing diagnostic phase trace'):
                campaign.collect_diagnostics(plan, {}, Path('/unused'), [], Mock(errors=[]),
                                             {'dense-t1/baseline': 'same'}, rows)
            self.assertEqual(native.call_count, 1)
            self.assertEqual(rows[0]['phase_diagnostics']['status'], 'missing')

    def test_untraced_warmup_binds_real_traced_prerequisite_resources(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            metadata, traced_log, untraced_log = root / 'meta.json', root / 'science.log', root / 'warmup.log'
            campaign.save(metadata, {})
            traced_log.write_text('FEREG_SOLVER_TRACE|phase=routing_action|preset=routing_cells_v2_df|'
                                  'final=0|certified=1|iterations=5|native_limit=100|setup_work=6|'
                                  'setup_limit=200|memory_bytes=64|memory_limit=1024\n')
            untraced_log.write_text('NATIVE_SECONDS=1.0\n')
            arm = 'cells_v2_df'
            resources = campaign.routing_run.resource_diagnostics(traced_log.read_text(), arm, False, {})
            original = {'cell': 'dense-t1', 'arm': arm, 'log': str(traced_log),
                        'science_sha256': 'same', 'resources': resources}
            receipt = {'records': [original]}
            cell = {'id': 'dense-t1', 'metadata': {'path': str(metadata)}}
            row = {'cell': cell['id'], 'arm': arm, 'log': str(untraced_log)}
            with patch.object(campaign, 'parse_run', return_value=({}, self.result(), Path('columns'))), \
                 patch.object(campaign.cmgschur, 'scientific_comparison') as science:
                campaign.check_prerequisite(row, cell, receipt, traced=False)
                science.assert_called_once()
                self.assertEqual(row['resources']['action_resources'], resources['action_resources'])
                self.assertEqual(row['resources']['prerequisite_log'], campaign.ref(traced_log))
                self.assertEqual(row['resources']['prerequisite_science_sha256'], 'same')
                self.assertIn('current call untraced', row['resources']['resource_basis'])
                self.assertEqual(original['resources'], resources)
                with self.assertRaisesRegex(ValueError, 'no actual routing_action'):
                    campaign.check_prerequisite(row, cell, receipt, traced=True)
                original['resources'] = {}
                with self.assertRaisesRegex(ValueError, 'prerequisite resource evidence changed'):
                    campaign.check_prerequisite(row, cell, receipt, traced=False)


class StableConfiguration(unittest.TestCase):
    @staticmethod
    def boundary():
        loads = {'arch': 'linux-x64', 'num_proc': '28', 'mem_total': '192000M',
                 'swap_total': '8000M', 'virtual_total': '200000M',
                 'm_topology': 'SCCCCCCCCCCCCCCSCCCCCCCCCCCCCC',
                 'm_topology_inuse': 'SCCCCCCCCCCCCCCSCCCCCCCCCCCCCC',
                 'm_socket': '2', 'm_core': '28', 'sensor_policy': 'fixed'}
        loads.update({key: '1M' if key.endswith(('_free', '_used')) else '0.000000'
                      for key in sorted(campaign.LIVE_HOST_LOAD_FIELDS)})
        text = ('hostname scc-test.scc.bu.edu\nload_scaling NONE\n'
                'complex_values cpu_arch=skylake,scratch_free=876G\n'
                'load_values ' + ','.join(f'{k}={v}' for k, v in loads.items())
                + '\nprocessors 28\nuser_lists NONE\nprojects NONE\n')
        command = {'argv': ['qconf', '-se', 'scc-test'], 'stdout': text, 'stderr': '',
                   'returncode': 0, 'locale': 'C', 'timezone': 'UTC'}
        return {'job_environment': {'NSLOTS': '28', 'PE': 'omp28'},
                'parallel_environment': {'stdout': 'allocation_rule $pe_slots'},
                'resource_quota': {'stdout': 'limit hosts {*} to slots=$num_proc'},
                'queue_configuration': {'complexes': {'stdout': 'slots s INT <= YES YES'},
                                        'queue': {'stdout': 'slots 28'}, 'host': command}}

    @staticmethod
    def replace_host(boundary, before, after):
        record = boundary['queue_configuration']['host']
        record['stdout'] = record['stdout'].replace(before, after)

    def test_all_declared_live_values_can_change_without_mutating_raw_evidence(self):
        before = self.boundary()
        after = copy.deepcopy(before)
        for key in campaign.LIVE_HOST_LOAD_FIELDS:
            self.replace_host(after, key + ('=1M' if key.endswith(('_free', '_used')) else '=0.000000'),
                              key + ('=54321.25G' if key.endswith(('_free', '_used')) else '=12.500000'))
        original = copy.deepcopy((before, after))
        campaign.check_stable_configuration(before, after)
        self.assertEqual((before, after), original)

    def test_every_structural_load_value_and_unknown_sensor_remain_exact(self):
        before = self.boundary()
        _, loads = campaign.host_fields(before['queue_configuration']['host'])
        for key in [*campaign.STRUCTURAL_HOST_LOAD_FIELDS, 'sensor_policy']:
            with self.subTest(key=key):
                after = copy.deepcopy(before)
                self.replace_host(after, f'{key}={loads[key]}', f'{key}=changed')
                with self.assertRaisesRegex(ValueError, 'structural host configuration changed'):
                    campaign.check_stable_configuration(before, after)
        after = copy.deepcopy(before)
        self.replace_host(after, ',cpu=0.000000', '')
        with self.assertRaisesRegex(ValueError, 'structural host configuration changed'):
            campaign.check_stable_configuration(before, after)

    def test_environment_pe_quota_queue_complexes_and_host_policy_stay_exact(self):
        before = self.boundary()
        for key in ['job_environment', 'parallel_environment', 'resource_quota']:
            with self.subTest(key=key):
                after = copy.deepcopy(before)
                after[key]['mutated'] = True
                with self.assertRaisesRegex(ValueError, 'configuration changed'):
                    campaign.check_stable_configuration(before, after)
        for key in ['queue', 'complexes']:
            with self.subTest(key=key):
                after = copy.deepcopy(before)
                after['queue_configuration'][key]['stdout'] += ' policy mutation'
                with self.assertRaisesRegex(ValueError, 'configuration changed'):
                    campaign.check_stable_configuration(before, after)
        for old, new in [('processors 28', 'processors 56'), ('projects NONE', 'projects other'),
                         ('load_scaling NONE', 'load_scaling cpu=2'), ('scratch_free=876G', 'scratch_free=900G')]:
            with self.subTest(policy=old):
                after = copy.deepcopy(before)
                self.replace_host(after, old, new)
                with self.assertRaisesRegex(ValueError, 'structural host configuration changed'):
                    campaign.check_stable_configuration(before, after)

    def test_host_command_metadata_must_remain_exact(self):
        before = self.boundary()
        for key, value in [('argv', ['qconf', '-se', 'other-host']), ('stderr', 'warning'),
                           ('returncode', 1), ('locale', 'other'), ('timezone', 'other')]:
            with self.subTest(key=key):
                after = copy.deepcopy(before)
                after['queue_configuration']['host'][key] = value
                with self.assertRaisesRegex(ValueError, 'command metadata changed'):
                    campaign.check_stable_configuration(before, after)

    def test_malformed_or_duplicate_fields_fail_even_if_both_boundaries_match(self):
        mutations = [('cpu=0.000000', 'cpu=NaN'), ('mem_free=1M', 'mem_free=bad'),
                     ('cpu=0.000000', 'cpu=1M'), ('cpu=0.000000', 'cpu=1e309'),
                     ('cpu=0.000000', 'cpu=0.0,cpu=0.0'), ('arch=linux-x64', 'arch'),
                     ('arch=linux-x64', '=linux-x64'), ('arch=linux-x64', 'arch=a=b'),
                     ('arch=linux-x64', 'arch='), ('arch=linux-x64', 'arch=bad value'),
                     ('processors 28', 'processors 28\nprocessors 28'),
                     ('processors 28', 'malformed_line'),
                     ('cpu_arch=skylake', 'cpu_arch=skylake,cpu_arch=skylake')]
        for old, new in mutations:
            with self.subTest(mutation=new):
                boundary = self.boundary()
                self.replace_host(boundary, old, new)
                with self.assertRaises(ValueError):
                    campaign.check_stable_configuration(boundary, copy.deepcopy(boundary))

    def test_host_wrapping_and_key_order_are_semantic_not_byte_comparisons(self):
        before = self.boundary()
        after = copy.deepcopy(before)
        record = after['queue_configuration']['host']
        lines = record['stdout'].splitlines()
        load = next(line for line in lines if line.startswith('load_values '))
        entries = load.split(None, 1)[1].split(',')
        separator = ', ' + chr(92) + '\n    '
        rewritten = 'load_values ' + separator.join(reversed(entries))
        record['stdout'] = record['stdout'].replace(load, rewritten)
        campaign.check_stable_configuration(before, after)


if __name__ == '__main__':
    unittest.main()
