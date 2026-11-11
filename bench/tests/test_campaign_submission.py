"""Revision campaign staging, candidate plugin admission and the task wrapper."""
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest

HERE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HERE))


class CandidatePlugin(unittest.TestCase):
    def test_setup_admits_only_a_candidate_with_this_frontend(self):
        text = (HERE/'scc/setup.sh').read_text()
        start = text.index('CAMPAIGN_PLUGIN:-build}" == candidate')
        snippet = text[text.index("<<'PY'\n", start)+7:text.index('\nPY\n', start)]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package, source = root/'package', root/'source'
            for tree in [package/'dist', package/'stata', source/'dist', source/'stata']:
                tree.mkdir(parents=True)
            (package/'dist/fereg_linux.plugin').write_bytes(b'candidate')
            (package/'dist/fereg_linux.plugin.sha256').write_text(
                hashlib.sha256(b'candidate').hexdigest()+'  fereg_linux.plugin\n')
            (source/'dist/fereg_linux.plugin').write_bytes(b'stale')
            for tree in [package, source]:
                (tree/'stata/fereg.ado').write_text('program fereg\nend\n')
            (package/'stata/fereg.sthlp').write_text('newer help')
            run = lambda: subprocess.run([sys.executable, '-c', snippet, str(package), str(source)],
                                         capture_output=True, text=True)
            result = run()
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((source/'dist/fereg_linux.plugin').read_bytes(), b'candidate')
            (source/'dist/fereg_linux.plugin').write_bytes(b'stale')
            (source/'stata/fereg.ado').write_text('program fereg\n  version 19\nend\n')
            self.assertNotEqual(run().returncode, 0)
            self.assertEqual((source/'dist/fereg_linux.plugin').read_bytes(), b'stale')


class Staging(unittest.TestCase):
    def test_stage_records_inputs_launchers_and_candidate(self):
        from campaign_submit import LAUNCHERS, stage
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root/'source.tar'
            with tarfile.open(archive, 'w') as tar:
                for name in LAUNCHERS:
                    data = (HERE/'scc'/name).read_bytes()
                    info = tarfile.TarInfo('bench/scc/'+name)
                    info.size = len(data)
                    tar.addfile(info, io.BytesIO(data))
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            (root/'source.json').write_text(json.dumps(dict(commit='c'*40, dirty=False, archive_sha256=digest)))
            package = root/'package'
            (package/'dist').mkdir(parents=True)
            (package/'dist/fereg_linux.plugin').write_bytes(b'candidate')
            run = root/'run'
            stage(run, archive, root/'source.json', HERE/'campaign_revision.json', 'prefix', 'candidate', package)
            listed = dict(reversed(line.split('  ', 1)) for line in (run/'input/SHA256SUMS').read_text().splitlines())
            self.assertEqual(set(listed), {'input/source.tar', 'input/source.json', 'input/policy.json', *LAUNCHERS})
            self.assertTrue(all(hashlib.sha256((run/n).read_bytes()).hexdigest() == v for n, v in listed.items()))
            # Setup writes the frozen and real-panel inputs; staging must not.
            self.assertFalse((run/'input/frozen-inputs.json').exists())
            context = json.loads((run/'context.json').read_text())
            self.assertEqual(context['candidate_plugin_sha256'], hashlib.sha256(b'candidate').hexdigest())
            self.assertEqual(context['launcher_sha256']['campaign_tasks.sh'], listed['campaign_tasks.sh'])
            with self.assertRaises(RuntimeError):
                stage(root/'other', archive, root/'source.json', None, 'prefix', 'candidate', None)
            with self.assertRaises(FileExistsError):
                stage(run, archive, root/'source.json', None, 'prefix')

    def test_task_wrapper_runs_each_task_without_relabelling_the_allocation(self):
        text = (HERE/'scc/campaign_tasks.sh').read_text()
        subprocess.run(['bash', '-n', str(HERE/'scc/campaign_tasks.sh')], check=True)
        self.assertIn('--task "$task"', text)
        self.assertNotIn('SGE_TASK_ID=', text)
        self.assertIn('#$ -pe omp28 28', text)
        self.assertIn('#$ -l num_proc=28', text)


class Collection(unittest.TestCase):
    def test_production_sentinels_match_whole_lines(self):
        from campaign_collect import production_sentinels
        with tempfile.TemporaryDirectory() as directory:
            logs = Path(directory)
            for task in range(1, 13):
                (logs/f'production.{task}.sge').write_text(f'fits\nCAMPAIGN_JOB_COMPLETE job=77 task={task}\n')
            context = dict(production_job='77')
            # task=1 is a prefix of task=10, 11 and 12; each still matches one log.
            self.assertTrue(production_sentinels(logs, context, 12))
            (logs/'production.12.sge').write_text('fits\n')
            self.assertFalse(production_sentinels(logs, context, 12))
            (logs/'production.12.sge').write_text('CAMPAIGN_JOB_COMPLETE job=77 task=12\n')
            (logs/'production.13.sge').write_text('CAMPAIGN_JOB_COMPLETE job=77 task=12\n')
            self.assertFalse(production_sentinels(logs, context, 12))


if __name__ == '__main__':
    unittest.main()
