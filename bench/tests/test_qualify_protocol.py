"""Mixed released action wires preserve the same serial request and body."""
import copy
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import native_io
import qualify as q
import test_qualify
from test_cmgschur import case_bytes


ARGUMENTS = [b'estimate-ols', b'protocol_major=1', b'protocol_minor=6',
             b'model_major=2', b'model_minor=1', b'result_major=1', b'result_minor=5',
             b'threads=1', b'solver=auto', b'preconditioner=auto',
             b'tolerance=1e-10', b'memory=low', b'include_intercept=1']


def read_case(path):
    with path.open('rb') as stream:
        shape, arguments = native_io.case_header(stream)
        return shape, arguments, stream.read()


class SerialProtocolTransport(unittest.TestCase):
    def test_shared_prepare_changes_only_protocol_and_existing_worker_route_fields(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            source = root / 'input.case'
            source.write_bytes(case_bytes(ARGUMENTS) + b'protocol_minor=6\n')
            original = source.read_bytes()
            source_shape, _, source_body = read_case(source)
            for request in ('preserve', 'auto'):
                for minor in ('6', '7'):
                    path = root / (request + minor)
                    q.prepare_request(source, path, {'request': request, 'action_protocol': '1.' + minor}, 4)
                    shape, arguments, body = read_case(path)
                    fields = dict(a.split(b'=', 1) for a in arguments[1:])
                    expected = dict(a.split(b'=', 1) for a in ARGUMENTS[1:])
                    expected[b'protocol_minor'], expected[b'threads'] = minor.encode(), b'4'
                    self.assertEqual(fields, expected)
                    self.assertEqual(shape, source_shape)
                    self.assertEqual(body, source_body)
            self.assertEqual(source.read_bytes(), original)

    def test_missing_selector_preserves_the_existing_request_bytes(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            source = root / 'input.case'; source.write_bytes(case_bytes(ARGUMENTS))
            for request in ('preserve', 'auto'):
                old, shared = root / ('old-' + request), root / ('shared-' + request)
                if request == 'preserve':
                    native_io.prepare_case(source, old, 4)
                else:
                    q.routing.prepare(source, old, 'baseline', 4)
                q.prepare_request(source, shared, {'request': request}, 4)
                self.assertEqual(old.read_bytes(), shared.read_bytes())

    def test_selector_rejects_nonlegacy_wires_host_facts_and_duplicates_before_writing(self):
        invalid = [
            [a for a in ARGUMENTS if not a.startswith(b'protocol_minor=')],
            [*ARGUMENTS, b'protocol_minor=6'],
            [*ARGUMENTS, b'host_edition=MP'],
            [*ARGUMENTS, b'host_interface=unknown'],
            [*ARGUMENTS, b'host_aliases=1'],
            [b'estimate-ppml', *ARGUMENTS[1:]],
        ]
        for key, value in ((b'protocol_major', b'2'), (b'protocol_minor', b'5'),
                           (b'model_minor', b'2'), (b'result_minor', b'6')):
            invalid.append([key + b'=' + value if a.startswith(key + b'=') else a for a in ARGUMENTS])
        with tempfile.TemporaryDirectory() as name:
            root = Path(name); source = root / 'input.case'; target = root / 'rejected.case'
            for arguments in invalid:
                source.write_bytes(case_bytes(arguments))
                for request in ('auto', 'preserve'):
                    with self.subTest(arguments=arguments, request=request), self.assertRaises(ValueError):
                        q.prepare_request(source, target, {'request': request, 'action_protocol': '1.7'}, 1)
                    self.assertFalse(target.exists())
            for target_protocol in ('1.5', '1.8', '', None, 7, True):
                with self.subTest(target=target_protocol), self.assertRaises(ValueError):
                    native_io.action_protocol_arguments(ARGUMENTS, target_protocol)


class ProtocolQualification(unittest.TestCase):
    def setUp(self):
        test_qualify.Qualification.setUp(self)
        self.source.write_bytes(case_bytes(ARGUMENTS))
        for role, artifact in self.config['artifacts'].items():
            artifact['action_protocol'] = '1.6' if role == 'original' else '1.7'

    def plan(self):
        return test_qualify.Qualification.plan(self)

    def test_plan_binds_each_selector_and_does_not_alias_distinct_requests(self):
        plan = self.plan()
        self.assertEqual(plan['artifacts']['original']['action_protocol'], '1.6')
        aliases = q.artifact_aliases(plan)
        self.assertEqual(aliases['candidate'], aliases['incumbent'])
        self.assertNotEqual(aliases['original'], aliases['candidate'])

    def test_plan_rejects_unknown_selector_or_host_facts_before_execution(self):
        for value in ('1.8', 7, None, True):
            changed = copy.deepcopy(self.config); changed['artifacts']['candidate']['action_protocol'] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                q.plan(self.root / 'config.json', self.root / 'invalid-plan.json', configuration=changed)
        self.source.write_bytes(case_bytes([*ARGUMENTS, b'host_aliases=0']))
        with self.assertRaisesRegex(ValueError, 'absent host facts'):
            self.plan()

    def test_timing_runner_executes_the_same_shared_declared_preparation(self):
        self.plan()
        checked = []
        def execute(command, log, timeout):
            request = Path(command[-2])
            _, arguments, body = read_case(request)
            fields = dict(a.split(b'=', 1) for a in arguments[1:])
            expected = b'6' if request.stem == 'original' else b'7'
            self.assertEqual(fields[b'protocol_minor'], expected)
            self.assertEqual(body, read_case(self.source)[2])
            checked.append((request.stem, fields[b'threads']))
            return test_qualify.Qualification.execute(self, command, log, timeout)
        result = q.run(self.manifest, self.output, execute=execute)
        self.assertEqual(result['status'], 'complete', result['failures'])
        self.assertEqual(len(checked), 20)  # Two transport arms, two workers, one warmup + four calls.
        self.assertEqual(q.report(self.manifest, self.output)['status'], 'smoke_complete')


if __name__ == '__main__':
    unittest.main()
