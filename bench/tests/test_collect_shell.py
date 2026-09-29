"""Exercise the collector's job-list stdin boundary without network access."""
import csv
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from run import load_suite, selected_cells, selected_threads


class CollectShellTests(unittest.TestCase):
    def test_every_job_is_collected_without_consuming_manifest_stdin(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixture, binaries = root / 'fixture', root / 'bin'
            fixture.mkdir()
            binaries.mkdir()
            (fixture / 'summaries').mkdir()
            (fixture / 'jobs.tsv').write_text('setup\t10\t1\nbenchmark\t11\t1\n')
            (fixture / 'tasks.tsv').write_text('all all\n')
            (fixture / 'source.json').write_text(json.dumps({'commit': 'a' * 40}))
            suite = load_suite()
            (fixture / 'suite.toml').write_text((Path(__file__).resolve().parents[1] / 'suite.toml').read_text())
            rows = [dict(source_sha='a' * 40, dirty=False, tier='smoke', platform='linux',
                         engine='legacy', solver='auto', cell=cell['id'], threads=threads,
                         arm=arm, status='ok', accuracy='pass', repetitions=5, partial=False)
                    for cell in selected_cells(suite, 'smoke')
                    for threads in selected_threads(suite, 'smoke', cell, None)
                    for arm in suite['comparators'][cell['model']]]
            with (fixture / 'summaries/task-1.csv').open('w') as stream:
                writer = csv.DictWriter(stream, fieldnames=list(rows[0]))
                writer.writeheader()
                writer.writerows(rows)
            stubs = {
                'ssh': '''import sys
if '-n' not in sys.argv and 'qacct' in sys.argv[-1]:
    sys.stdin.read()
if 'qacct' in sys.argv[-1]:
    print('=====\\nfailed 0\\nexit_status 0\\nslots 28\\ntaskid 1')
else:
    print('FEREG_BENCH_COMPLETE 16 groups 0 command failures')
''',
                'rsync': '''import os, shutil, sys
from pathlib import Path
fixture = Path(os.environ['COLLECT_FIXTURE'])
destination = Path(sys.argv[-1])
for source in sys.argv[1:-1]:
    if source.startswith('-'):
        continue
    path = fixture / Path(source.rstrip('/')).name
    if source.endswith('/'):
        shutil.copytree(path, destination, dirs_exist_ok=True)
    else:
        shutil.copyfile(path, destination / path.name)
'''}
            for name, script in stubs.items():
                path = binaries / name
                path.write_text(f'#!{sys.executable}\n' + script)
                path.chmod(0o700)
            environment = dict(os.environ, PATH=str(binaries) + os.pathsep + os.environ['PATH'],
                               FEREG_PYTHON_BIN=sys.executable, COLLECT_FIXTURE=str(fixture))
            destination, summary = root / 'collected', root / 'summary.csv'
            script = Path(__file__).resolve().parents[1] / 'scc/collect.sh'
            subprocess.run(['bash', str(script), '/projectnb/welfgr/fereg/dev/test',
                            str(destination), str(summary)], env=environment, check=True,
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            self.assertTrue((destination / 'setup-qacct.txt').is_file())
            self.assertTrue((destination / 'benchmark-qacct.txt').is_file())
            self.assertEqual(stat.S_IMODE(destination.stat().st_mode), 0o700)
            with summary.open() as stream:
                self.assertEqual(len(list(csv.DictReader(stream))), 16)


if __name__ == '__main__':
    unittest.main()
