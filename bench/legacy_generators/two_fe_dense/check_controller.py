"""Check staged worker identity before any application or timing dispatch."""
import json
from pathlib import Path
import tempfile

from run import stage, validate_controller


def must_reject(folder):
    try:
        validate_controller(folder)
    except RuntimeError as error:
        assert 'fresh benchmark directory' in str(error)
    else:
        raise AssertionError('stale controller accepted')


def check():
    with tempfile.TemporaryDirectory() as temporary:
        folder = stage(Path(temporary))
        validate_controller(folder)
        worker = folder / 'stata_worker.do'
        original_worker = worker.read_text()
        assert "if `dimensions' == 2 local outputvars `outputvars' out_fe2" in original_worker
        stamp = folder / 'source-manifest.json'
        original_stamp = stamp.read_text()
        stamp.unlink()
        must_reject(folder)
        stamp.write_text(original_stamp)
        worker.write_text(original_worker + '\n')
        must_reject(folder)
        worker.write_text(original_worker)
        manifest = json.loads(original_stamp)
        manifest['sources'][next(iter(manifest['sources']))] = '0' * 64
        stamp.write_text(json.dumps(manifest))
        must_reject(folder)
        stamp.write_text(original_stamp)
        validate_controller(folder)
    print('CONTROLLER_IDENTITY_CHECK_PASS fresh, missing, modified worker, stale source')


if __name__ == '__main__':
    check()
