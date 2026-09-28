"""Two current-artifact admission probes, isolated from all headline clocks."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import sys
from types import SimpleNamespace


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--run-root', type=Path, required=True)
    parser.add_argument('--scratch', type=Path, required=True)
    args = parser.parse_args()
    root = args.run_root
    bindings = json.loads((root / 'deployment.json').read_text())
    for name, expected in bindings.items():
        assert hashlib.sha256((root / name).read_bytes()).hexdigest() == expected, name
    registration = json.loads((root / 'registration.json').read_text())
    reference = Path(registration['original_run'])
    manifest = reference / 'input/manifest.json'
    assert hashlib.sha256(manifest.read_bytes()).hexdigest() == registration['manifest_sha256']
    sys.path.insert(0, str(root / 'benchmarks/performance'))
    import diagnose
    assert int(os.environ['NSLOTS']) == 4
    args.scratch.mkdir()
    diagnose.write_json(root / 'launch.json', dict(job_id=os.environ['JOB_ID'],
        hostname=socket.gethostname(), slots=4, cpu_affinity=sorted(os.sched_getaffinity(0)),
        available_bytes=diagnose.available_linux_memory(), performance_eligible=False))
    diagnose.run(SimpleNamespace(plan=root / 'plan.json', output=root / 'output',
        scratch=args.scratch, source=Path(registration['accepted_source']),
        source_manifest=root / 'base-source.json', manifest=manifest, refs=reference / 'references'))


if __name__ == '__main__':
    main()
