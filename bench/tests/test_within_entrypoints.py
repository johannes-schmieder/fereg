"""Pure framing and scientific failure gates; no native process or build."""
from copy import deepcopy
import hashlib
import io
from pathlib import Path
import struct
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_within_entrypoints as driver


def field(name, value):
    name = name.encode()
    return struct.pack('<Q', len(name)) + name + struct.pack('<Q', len(value)) + value


def valid_payload():
    return {'entrypoint': 'controlled_default', 'rows': 8192, 'slopes': 0,
            'mode': 'off', 'workers': 1, 'weighted': True, 'tolerance': 1e-12,
            'witness': 1e-8, 'rhs': 4, 'warmups': 1, 'maximum_error': 1e-9,
            'input_fnv64': '0123456789abcdef', 'science_fnv64': 'fedcba9876543210',
            'iterations': [11] * 4, 'recurrence_residuals': [1e-13] * 4,
            'controlled_stop_reasons': ['NormalEquationTolerance'] * 4,
            'build_ns': None, 'elapsed_ns': None, 'total_ns': None,
            'exact_fields': {'coefficients': {'bytes': 8, 'sha256': '0' * 64}}}


class WithinEntrypointGates(unittest.TestCase):
    def test_framing_hashes_payload_and_rejects_truncation_or_duplicates(self):
        frame = field('x', b'abc')
        self.assertEqual(driver.read_fields(io.BytesIO(frame)),
                         {'x': {'bytes': 3, 'sha256': hashlib.sha256(b'abc').hexdigest(), 'hex': '616263'}})
        for malformed in [frame[:-1], frame[:3], frame + frame]:
            with self.subTest(malformed=malformed), self.assertRaises(RuntimeError):
                driver.read_fields(io.BytesIO(malformed))

    def test_forward_witness_does_not_accept_nonfinite_or_excess_error(self):
        for value in [1.000001e-8, float('nan'), float('inf'), -1e-12]:
            payload = valid_payload()
            payload['maximum_error'] = value
            with self.subTest(value=value), self.assertRaisesRegex(RuntimeError, 'forward witness'):
                driver.validate(payload, driver.cells()[0], 'api')

    def test_science_mode_rejects_timers_wrong_entrypoint_and_wrong_stops(self):
        for key, value, message in [('build_ns', 1, 'timer mode'),
                                    ('entrypoint', 'legacy', 'entrypoint mismatch'),
                                    ('controlled_stop_reasons', ['MaxIterations'] * 4, 'stop metadata')]:
            payload = valid_payload()
            payload[key] = value
            with self.subTest(key=key), self.assertRaisesRegex(RuntimeError, message):
                driver.validate(payload, driver.cells()[0], 'api')

    def test_signatures_and_sha_fields_must_both_match(self):
        science = driver.validate(valid_payload(), driver.cells()[0], 'api')
        for key in ['input_fnv64', 'science_fnv64', 'exact_fields']:
            other = deepcopy(science)
            other[key] = '0' * 16 if key != 'exact_fields' else {'coefficients': {'bytes': 8, 'sha256': '1' * 64}}
            with self.subTest(key=key):
                self.assertFalse(driver.same(science, other))
        payload = valid_payload()
        del payload['exact_fields']
        with self.assertRaisesRegex(RuntimeError, 'missing exact scientific fields'):
            driver.validate(payload, driver.cells()[0], 'api')


if __name__ == '__main__':
    unittest.main()
