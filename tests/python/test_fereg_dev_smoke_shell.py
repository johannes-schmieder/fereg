import pathlib
import subprocess
import tempfile
import unittest


class SmokeShellInitializationTests(unittest.TestCase):
    def test_sourced_smoke_initializes_under_inherited_nounset(self):
        repo = pathlib.Path(__file__).resolve().parents[2]
        source = (repo / "benchmarks/scc/fereg_dev_smoke_job.sh").read_text()
        prefix = source.split(': "${RUN_ROOT:?', 1)[0]
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            startup = root / "bashrc"
            startup.write_text('test -z "$BASHRCSOURCED"\nBASHRCSOURCED=1\n')
            entry = root / "entry.sh"
            entry.write_text(prefix.replace("source /etc/bashrc", 'source "' + str(startup) + '"') +
                             '\n[[ "$-" == *u* ]]\n[[ "$BASHRCSOURCED" == 1 ]]\n')
            for inherited in ("set -eo pipefail", "set -euo pipefail"):
                result = subprocess.run(["bash", "-c", inherited + '; unset BASHRCSOURCED; source "$1"',
                                         "bash", str(entry)], capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
