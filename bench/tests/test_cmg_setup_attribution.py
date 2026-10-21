"""Attribution patch, parser and binding guards: no native call or timing."""
import copy
import json
import re
import tarfile
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import cmg_setup_attribution as attribution

DATA = Path(__file__).resolve().parent / 'data' / 'cmg_setup_attribution'
Error = attribution.AttributionError


def fixture(arm, mode):
    return (DATA / f'{arm}-{mode}.txt').read_text()


def first_call(arm, mode):
    return attribution.smoke_calls(fixture(arm, mode))[0][3]


def replace_line(text, prefix, edit):
    lines = text.splitlines()
    index = next(i for i, line in enumerate(lines) if line.startswith(prefix))
    lines[index] = edit(lines[index])
    return '\n'.join(lines) + '\n'


def edit_attribution(text, edit):
    return replace_line(text, attribution.PREFIX, edit)


def native_fixture(text):
    """Use the same real diagnostic output with independently recorded route metadata."""
    digest = attribution.fields_of(next(l for l in text.splitlines() if l.startswith('SMOKE_DIGEST|')))
    series = {key: json.loads(digest[key]) for key in ('vertices', 'nonzeros', 'repeats')}
    direct = digest['terminal'] == 'Direct'
    fields = {'phase': 'auto_schur_route', 'installed': '1', 'refusal': 'None',
              'terminal': digest['terminal'], 'levels': len(series['vertices']),
              'hierarchy_vertices': ';'.join(map(str, series['vertices'])),
              'hierarchy_nonzeros': ';'.join(map(str, series['nonzeros'])),
              'repeats': ';'.join(map(str, series['repeats'])), 'terminal_factor': int(direct),
              'terminal_factor_dimension': series['vertices'][-1] - 1 if direct else 0,
              'terminal_factor_nonzeros': int(digest['factor_nonzeros'][5:-1]) if direct else 0,
              'application': 'scalar', 'parallel_operators': 0, 'batch_columns': 1}
    return replace_line(text, 'SMOKE_DIGEST|', lambda _: 'FEREG_SOLVER_TRACE|' +
                        '|'.join(f'{key}={value}' for key, value in fields.items()))


class RealFixtures(unittest.TestCase):
    """Smoke outputs of the four prepared builds (arm x mode)."""

    def test_every_fixture_call_validates(self):
        for arm in attribution.ARMS:
            for mode in attribution.MODES:
                calls = attribution.smoke_calls(fixture(arm, mode))
                self.assertEqual([(g, w) for g, w, _, _ in calls], attribution.SMOKE_CALLS)
                for graph, workers, _, body in calls:
                    with self.subTest(arm=arm, mode=mode, graph=graph, workers=workers):
                        call = attribution.validate_call(body, arm, mode)
                        serial = [s for s in call['sorts'] if s['path'] == 'serial']
                        self.assertTrue(serial)
                        if mode == 'counts':
                            self.assertIsNone(call['exclusive_ms'])
                            key = 'heapsorts' if arm == 'accepted' else 'partitions'
                            self.assertTrue(all(s[key] > 0 for s in serial if s['n'] > 16))
                        else:
                            pieces = call['exclusive_ms']
                            fereg = float(call['profile']['hierarchy_seconds']) * 1e3
                            self.assertAlmostEqual(sum(pieces.values()), fereg, places=6)
                            self.assertTrue(all(value >= 0 for value in pieces.values()), pieces)

    def test_digests_agree_across_arms_and_modes(self):
        digests = {(arm, mode): [d for _, _, d, _ in attribution.smoke_calls(fixture(arm, mode))]
                   for arm in attribution.ARMS for mode in attribution.MODES}
        self.assertEqual(len({tuple(value) for value in digests.values()}), 1)
        self.assertTrue(any('terminal=Direct' in d for d in digests['candidate', 'timers']))

    def test_sorts_are_identical_across_workers(self):
        for arm in attribution.ARMS:
            for mode in attribution.MODES:
                calls = attribution.smoke_calls(fixture(arm, mode))
                for graph in ('random', 'banded'):
                    attribution.require_deterministic(
                        [attribution.validate_call(b, arm, mode) for g, _, _, b in calls if g == graph], mode)
        calls = [attribution.validate_call(b, 'candidate', 'counts')
                 for _, _, _, b in attribution.smoke_calls(fixture('candidate', 'counts'))]
        with self.assertRaisesRegex(Error, 'differ between calls'):
            attribution.require_deterministic(calls, 'counts')

    def test_wrong_arm_identities_are_rejected(self):
        with self.assertRaisesRegex(Error, 'candidate root partition missing'):
            attribution.validate_call(first_call('accepted', 'counts'), 'candidate', 'counts')
        with self.assertRaisesRegex(Error, 'accepted sort is one heapsort'):
            attribution.validate_call(first_call('candidate', 'counts'), 'accepted', 'counts')

    def test_mode_mismatch_is_rejected(self):
        with self.assertRaisesRegex(Error, 'outcome/mode'):
            attribution.validate_call(first_call('candidate', 'counts'), 'candidate', 'timers')


class Rejections(unittest.TestCase):
    def setUp(self):
        self.counts = first_call('candidate', 'counts')
        self.timers = first_call('candidate', 'timers')

    def rejects(self, text, message, mode='counts', arm='candidate'):
        with self.assertRaisesRegex(Error, message):
            attribution.validate_call(text, arm, mode)

    def test_missing_or_duplicate_lines(self):
        line = next(l for l in self.counts.splitlines() if l.startswith(attribution.PREFIX))
        self.rejects(self.counts + line + '\n', 'one attribution line')
        self.rejects(self.counts.replace(line + '\n', ''), 'one attribution line')
        profile = next(l for l in self.counts.splitlines() if l.startswith(attribution.PROFILE_PREFIX))
        self.rejects(self.counts.replace(profile + '\n', ''), 'one profile line')
        self.rejects(self.counts.replace('completed_phases=6', 'completed_phases=5'), 'six phases')

    def test_overflow_and_unknown_phase_fields(self):
        for field in ('sort_overflow', 'unknown_phases', 'level_overflow'):
            text = edit_attribution(self.counts, lambda l: l.replace(f'|{field}=0|', f'|{field}=1|'))
            self.rejects(text, field)

    def test_sort_record_shape(self):
        self.rejects(edit_attribution(self.counts, lambda l: l.replace(',queries:', ',polls:', 1)), 'keys changed')
        self.rejects(edit_attribution(self.counts, lambda l: l.replace('path:serial', 'path:other', 1)), 'path')
        self.rejects(edit_attribution(self.counts, lambda l: l.replace('|sorts=', '|sorts=9', 1)), 'fields changed')
        self.rejects(edit_attribution(self.counts, lambda l: l + '|extra=1'), 'fields changed')
        self.rejects(edit_attribution(self.counts, lambda l: l.replace('complete:1', 'complete:0', 1)),
                     'incomplete')

    def test_uncounted_paths_and_counters(self):
        hosted = edit_attribution(self.counts, lambda l: l.replace('path:hosted,', 'path:hosted,', 1).replace(
            'complete:1,cmp:0', 'complete:1,cmp:5', 1))
        self.rejects(hosted, 'not counted')
        queries = edit_attribution(self.counts, lambda l: _set_first_serial(l, 'queries', 10 ** 9))
        self.rejects(queries, 'cadence')

    def test_contraction_levels_must_be_in_order(self):
        text = edit_attribution(self.counts, lambda l: l.replace('site:contraction,level:0', 'site:contraction,level:7', 1))
        self.rejects(text, 'one per level')

    def test_counts_build_must_not_time_and_timers_must_not_count(self):
        self.rejects(edit_attribution(self.counts, lambda l: l.replace('|timers=', '|timers=hierarchy_total@0:5:1', 1)),
                     'recorded timers')
        self.rejects(edit_attribution(self.timers, lambda l: _set_first_serial(l, 'cmp', 3)), 'counted operations',
                     mode='timers')

    def test_timer_structure_and_nesting(self):
        self.rejects(edit_attribution(self.timers, lambda l: _drop_timer(l, 'hierarchy_total@0')),
                     'hierarchy_total missing', mode='timers')
        self.rejects(edit_attribution(self.timers, lambda l: _duplicate_timer(l, 'finalize_total@0')),
                     'duplicate timer', mode='timers')
        self.rejects(edit_attribution(self.timers, lambda l: _scale_timer(l, 'build_with_total@0', 1000)),
                     'nested timers inconsistent', mode='timers')
        self.rejects(edit_attribution(self.timers, lambda l: l.replace('|timers=', '|timers=bogus_phase@0:1:1;', 1)),
                     'unknown timer phase', mode='timers')

    def test_every_expected_phase_is_required_on_direct_and_iterative_routes(self):
        for _, _, _, body in attribution.smoke_calls(fixture('candidate', 'timers')):
            native = native_fixture(body)
            expected = attribution.validate_call(native, 'candidate', 'timers')
            for phase in expected['timers']:
                with self.subTest(phase=phase, terminal=expected['hierarchy']['terminal']):
                    self.rejects(edit_attribution(native, lambda l: _drop_timer(l, phase)),
                                 'missing|coverage|terminal_factor|disagree', mode='timers')

    def test_timer_cannot_move_to_another_level(self):
        call = attribution.validate_call(self.timers, 'candidate', 'timers')
        phase = attribution.CONTRACTION
        first = call['timers'][phase + '@0']['ns']
        canonical = next(s['canonical_ns'] for s in call['sorts'] if s['site'] == 'contraction')
        delta = first - canonical + 1
        text = edit_attribution(self.timers, lambda l: _set_timer(l, phase + '@0', first - delta))
        text = edit_attribution(text, lambda l: _set_timer(l, phase + '@1',
                                                        call['timers'][phase + '@1']['ns'] + delta))
        self.rejects(text, 'canonical within contraction at level 0', mode='timers')

    def test_sub_microsecond_negative_remainder_is_rejected(self):
        call = attribution.validate_call(self.timers, 'candidate', 'timers')
        parent = round(float(call['profile']['hierarchy_seconds']) * 1e9)
        text = edit_attribution(self.timers, lambda l: _set_timer(l, 'build_with_total@0', parent + 500))
        self.rejects(text, 'nested timers inconsistent', mode='timers')

    def test_profile_duration_is_finite_and_nonnegative_in_both_modes(self):
        for mode in attribution.MODES:
            for value in ('nan', 'inf', '-inf', '-0.001'):
                text = replace_line(first_call('candidate', mode), attribution.PROFILE_PREFIX,
                                    lambda l: re.sub(r'hierarchy_seconds=[^|]+', 'hierarchy_seconds=' + value, l))
                self.rejects(text, 'finite and nonnegative', mode=mode)

    def test_terminal_factor_must_agree_with_independent_route(self):
        text = native_fixture(self.timers)
        self.rejects(edit_attribution(text, lambda l: l.replace('|timers=', '|timers=terminal_factor@0:0:1;', 1)),
                     'terminal_factor timer disagrees', mode='timers')
        body = next(b for g, _, _, b in attribution.smoke_calls(fixture('candidate', 'timers')) if g == 'banded')
        native = native_fixture(body)
        self.assertEqual(attribution.validate_call(native, 'candidate', 'timers')['hierarchy']['terminal_factor'], 1)


def _set_first_serial(line, key, value):
    fields = line.split('|')
    for index, field in enumerate(fields):
        if field.startswith('sort') and 'path:serial' in field:
            parts = [f'{key}:{value}' if part.startswith(key + ':') else part for part in field.split(',')]
            fields[index] = ','.join(parts)
            break
    return '|'.join(fields)


def _timer_items(line):
    head, rest = line.split('|timers=', 1)
    timers, tail = (rest.split('|', 1) + [''])[:2]
    return head, timers.split(';'), tail


def _join(head, items, tail):
    return head + '|timers=' + ';'.join(items) + ('|' + tail if tail else '')


def _drop_timer(line, name):
    head, items, tail = _timer_items(line)
    return _join(head, [item for item in items if not item.startswith(name + ':')], tail)


def _duplicate_timer(line, name):
    head, items, tail = _timer_items(line)
    return _join(head, items + [item for item in items if item.startswith(name + ':')], tail)


def _scale_timer(line, name, factor):
    head, items, tail = _timer_items(line)
    scaled = []
    for item in items:
        if item.startswith(name + ':'):
            label, ns, calls = item.rsplit(':', 2)
            item = f'{label}:{int(ns) * factor}:{calls}'
        scaled.append(item)
    return _join(head, scaled, tail)


def _set_timer(line, name, value):
    head, items, tail = _timer_items(line)
    return _join(head, [f'{name}:{value}:{item.rsplit(":", 1)[1]}' if item.startswith(name + ':') else item
                        for item in items], tail)


class PatchSpecification(unittest.TestCase):
    def test_archive_preserves_executables_and_normalizes_other_metadata(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name, mode in (('build.sh', 0o755), ('source.rs', 0o644)):
                (root / name).write_text(name)
                (root / name).chmod(mode)
            first = attribution.deterministic_archive(root, ['source.rs', 'build.sh'], root / 'one.tar')
            second = attribution.deterministic_archive(root, ['build.sh', 'source.rs'], root / 'two.tar')
            self.assertEqual(first, second)
            with tarfile.open(root / 'one.tar') as archive:
                self.assertEqual([(m.name, m.mode, m.mtime) for m in archive],
                                 [('build.sh', 0o755, 0), ('source.rs', 0o644, 0)])

    def test_anchor_must_occur_exactly_once(self):
        self.assertEqual(attribution.apply_edit('a b c', 'b', 'x', 'test'), 'a x c')
        with self.assertRaisesRegex(Error, 'occurs 0 times'):
            attribution.apply_edit('a c', 'b', 'x', 'test')
        with self.assertRaisesRegex(Error, 'occurs 2 times'):
            attribution.apply_edit('b b', 'b', 'x', 'test')

    def test_module_mode_is_exclusive(self):
        source = attribution.MODULE.read_text()
        for anchor in attribution.MODULE_ANCHORS:
            self.assertEqual(source.count(anchor), 1)
        counts, timers = attribution.module_source('counts'), attribution.module_source('timers')
        self.assertIn('COUNTS: bool = true', counts)
        self.assertIn('TIMERS: bool = false', counts)
        self.assertIn('TIMERS: bool = true', timers)
        self.assertIn('COUNTS: bool = false', timers)
        with self.assertRaisesRegex(Error, 'mode'):
            attribution.module_source('both')

    def test_every_edit_targets_a_pinned_file(self):
        for arm in attribution.ARMS:
            for name, anchor, replacement in attribution.edits(arm):
                self.assertIn(name, attribution.PRISTINE[arm])
                self.assertNotEqual(anchor, replacement)

    def test_pristine_identity_mismatch_fails_closed(self):
        files = {name: 'not the pinned source' for name in attribution.PRISTINE['accepted']}
        with self.assertRaisesRegex(Error, 'pristine identity mismatch'):
            attribution.patch_files(files, files, 'accepted', 'counts')
        with self.assertRaisesRegex(Error, 'file set'):
            attribution.patch_files({}, {}, 'candidate', 'counts')

    def test_root_profile_patch_is_the_reviewed_one(self):
        self.assertEqual(attribution.file_sha256(attribution.PROFILE_PATCH), attribution.PROFILE_PATCH_SHA256)

    def test_canonical_sorts_use_the_counted_comparator(self):
        for arm in attribution.ARMS:
            replacements = ''.join(r for _, _, r in attribution.edits(arm))
            self.assertNotIn('compare_raw_edges(&values', replacements.replace('counted_compare_raw_edges', ''))
            self.assertNotIn(', compare_raw_edges,', replacements)


class Revisions(unittest.TestCase):
    def test_wrong_revision_or_existing_output_fails_closed(self):
        import subprocess
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary) / 'repo'
            repo.mkdir()
            subprocess.run(['git', 'init', '-q', str(repo)], check=True)
            with self.assertRaisesRegex(Error, 'does not hold commit|rev-parse failed'):
                attribution.export(repo, attribution.ARMS['accepted']['cmg'], Path(temporary) / 'out')
            with self.assertRaisesRegex(Error, 'does not hold commit|rev-parse failed'):
                attribution.prepare('accepted', 'counts', repo, repo, Path(temporary) / 'prepared')
            (Path(temporary) / 'exists').mkdir()
            with self.assertRaisesRegex(Error, 'exists'):
                attribution.prepare('accepted', 'counts', repo, repo, Path(temporary) / 'exists')
        with self.assertRaisesRegex(Error, 'unknown'):
            attribution.prepare('public', 'counts', Path('.'), Path('.'), Path('/nonexistent/x'))


class Binding(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        (self.root / 'source.tar').write_bytes(b'archive')
        (self.root / 'plugin').write_bytes(b'plugin')
        (self.root / 'build.log').write_text('build')
        self.source = {
            'schema': 1, 'arm': 'candidate', 'hidden_arm': 'setup_hidden', 'mode': 'timers',
            'fereg_revision': attribution.ARMS['candidate']['fereg'],
            'cmg_revision': attribution.ARMS['candidate']['cmg'],
            'tool_sha256': attribution.file_sha256(attribution.__file__),
            'module_sha256': attribution.file_sha256(attribution.MODULE),
            'profile_patch_sha256': attribution.PROFILE_PATCH_SHA256,
            'source_archive': {'path': 'source.tar', 'sha256': attribution.sha256(b'archive')},
        }
        (self.root / 'attribution-source.json').write_text(json.dumps(self.source))

    def test_bind_writes_the_profile_helper_source_format(self):
        record = attribution.bind(self.root, self.root / 'plugin', self.root / 'build.log', 'a' * 64)
        self.assertEqual(set(record), {'arm', 'revision', 'baseline_sha256', 'plugin_sha256', 'patch_sha256',
                                       'source_archive', 'build_log'})
        self.assertEqual(record['arm'], 'setup_hidden')
        self.assertEqual(record['revision'], attribution.ARMS['candidate']['fereg'])
        self.assertEqual(record['patch_sha256'], attribution.PROFILE_PATCH_SHA256)
        self.assertEqual(record['source_archive'], self.source['source_archive'])
        self.assertEqual(json.loads((self.root / 'profile-source.json').read_text()), record)

    def test_bind_rejects_a_changed_archive(self):
        (self.root / 'source.tar').write_bytes(b'other')
        with self.assertRaisesRegex(Error, 'archive changed'):
            attribution.bind(self.root, self.root / 'plugin', self.root / 'build.log', 'a' * 64)

    def test_attribution_source_must_match_the_bound_profile(self):
        sys.path.insert(0, str(Path(attribution.__file__).parent))
        identity = {'arm': 'setup_hidden', 'base_revision': attribution.ARMS['candidate']['fereg'],
                    'patch_sha256': attribution.PROFILE_PATCH_SHA256,
                    'source_archive_sha256': attribution.sha256(b'archive')}
        path = self.root / 'attribution-source.json'
        self.assertEqual(attribution.check_attribution_source(path, identity)[0], 'candidate')
        for key, value in [('arm', 'accepted'), ('cmg_revision', '0' * 40), ('tool_sha256', '0' * 64),
                           ('module_sha256', '0' * 64), ('profile_patch_sha256', '0' * 64)]:
            changed = copy.deepcopy(self.source)
            changed[key] = value
            path.write_text(json.dumps(changed))
            with self.subTest(key=key), self.assertRaises(Error):
                attribution.check_attribution_source(path, identity)
        path.write_text(json.dumps(self.source))
        with self.assertRaisesRegex(Error, 'archive differs'):
            attribution.check_attribution_source(path, {**identity, 'source_archive_sha256': '0' * 64})
        with self.assertRaisesRegex(Error, 'another arm'):
            attribution.check_attribution_source(path, {**identity, 'arm': 'accepted_hidden'})


class RunLoop(unittest.TestCase):
    """The 18-call loop with every native/binding boundary mocked: the
    harness subprocess replays one real fixture call per invocation."""

    def test_run_records_eighteen_validated_calls(self):
        self.run_fixture()

    def test_second_call_shape_change_stops_immediately_and_keeps_failing_output(self):
        self.run_fixture(change_second_call=True)

    def run_fixture(self, change_second_call=False):
        import cmg_setup_profile as profile
        import qualify as q
        import routing
        body = native_fixture(next(b for g, w, _, b in attribution.smoke_calls(
            fixture('candidate', 'timers')) if g == 'banded'))
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = attribution.sha256(b'archive')
            source = {'arm': 'candidate', 'hidden_arm': 'setup_hidden', 'mode': 'timers',
                      'fereg_revision': attribution.ARMS['candidate']['fereg'],
                      'cmg_revision': attribution.ARMS['candidate']['cmg'],
                      'tool_sha256': attribution.file_sha256(attribution.__file__),
                      'module_sha256': attribution.file_sha256(attribution.MODULE),
                      'profile_patch_sha256': attribution.PROFILE_PATCH_SHA256,
                      'source_archive': {'path': 'source.tar', 'sha256': archive}}
            (root / 'attribution-source.json').write_text(json.dumps(source))
            for name in ('harness', 'high-degree-weak', 'weak-low'):
                (root / name).write_text(name)
            identity = {'arm': 'setup_hidden', 'base_revision': source['fereg_revision'],
                        'patch_sha256': attribution.PROFILE_PATCH_SHA256, 'source_archive_sha256': archive}
            manifest = {'harness': 'harness', 'cases': [{'id': n, 'input': n} for n in ('high-degree-weak', 'weak-low')]}
            base = {'cases': [{'id': f'{n}-t{w}', 'warmups': [{'execution': 'setup_hidden', 'fingerprint': 'f'}]}
                              for n in ('high-degree-weak', 'weak-low') for w in (1, 4, 8)]}
            guard = mock.Mock()
            guard.bind.return_value = '0' * 64

            native_calls = []

            def harness(command, stdout, **_):
                native_calls.append(command)
                text = body
                if change_second_call and len(native_calls) == 2:
                    text = edit_attribution(body, lambda l: _set_first_serial(l, 'n', 123456))
                stdout.write(text)
                Path(command[-1]).write_text('{}')
                return types.SimpleNamespace(returncode=0)

            def native(result, log, meta):
                columns = Path(str(result) + '.columns')
                columns.write_text('')
                return None, columns, {'fingerprint': 'f'}

            args = types.SimpleNamespace(repository=Path(attribution.__file__).parents[1], task_root=root,
                                         output=root / 'out', arm='setup_hidden', revision=source['fereg_revision'],
                                         source=root / 'profile-source.json', attribution=root / 'attribution-source.json',
                                         plugin=root / 'plugin', plan=None, baseline=None)
            with mock.patch.object(profile, 'load_context', return_value=(root / 'plan', manifest, base, root / 'plugin',
                                                                          identity, guard)), \
                    mock.patch.object(profile, 'check_call', return_value={'route_trace': [], 'route_hierarchy': []}), \
                    mock.patch.object(q, 'metadata', return_value=None), \
                    mock.patch.object(q, 'read_native', side_effect=native), \
                    mock.patch.object(routing, 'prepare', side_effect=lambda src, dst, *a: Path(dst).write_text('case')), \
                    mock.patch.object(attribution.subprocess, 'run', side_effect=harness):
                if change_second_call:
                    with self.assertRaisesRegex(Error, 'differ between calls'):
                        attribution.run(args)
                else:
                    attribution.run(args)
            results = json.loads((root / 'out' / 'results.json').read_text())
            if change_second_call:
                self.assertEqual(len(native_calls), 2)
                self.assertEqual(len(results['calls']), 2)
                self.assertEqual(results['status'], 'running')
                self.assertTrue((root / 'out' / 'high-degree-weak-t1-1.json').exists())
                self.assertTrue((root / 'out' / 'high-degree-weak-t1-1.log').exists())
                return
            self.assertEqual(results['status'], 'complete')
            self.assertEqual(len(results['calls']), 18)
            self.assertEqual(results['identity']['attribution_mode'], 'timers')
            self.assertTrue(all(call['attribution']['exclusive_ms'] for call in results['calls']))
            self.assertEqual(sorted(p.name for p in (root / 'out').iterdir() if p.suffix == '.case'), [])


if __name__ == '__main__':
    unittest.main()
