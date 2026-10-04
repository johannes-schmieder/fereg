"""The isolated timing plugin must propagate SPI failures and reset its state."""
from pathlib import Path
import platform
import subprocess
import tempfile
import unittest
import sys

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from build_clock import build

ROOT=Path(__file__).resolve().parents[2]


class ClockTests(unittest.TestCase):
    @unittest.skipUnless(platform.system() in ['Darwin','Linux'],'Unix CI clock harness')
    def test_clock_scalar_abi_and_failure_state(self):
        with tempfile.TemporaryDirectory() as temp:
            executable=Path(temp)/'clock-test'
            define='APPLEMAC' if platform.system()=='Darwin' else 'OPUNIX'
            subprocess.run(['clang','-std=c11','-O2','-Wall','-Wextra','-Werror','-Wconversion',
                '-Wshadow','-Wstrict-prototypes','-Wmissing-prototypes','-DSYSTEM='+define,'-I'+str(ROOT/'plugin'),
                str(ROOT/'bench/clock.c'),str(ROOT/'plugin/stplugin.c'),str(ROOT/'bench/tests/clock_harness.c'),
                '-o',str(executable)],check=True,capture_output=True,text=True)
            subprocess.run([str(executable)],check=True,capture_output=True,text=True)

    @unittest.skipUnless(platform.system() in ['Darwin','Linux'],'Unix dynamic clock ABI')
    def test_dynamic_plugin_has_only_the_two_stata_exports(self):
        import contextlib,io,json
        with tempfile.TemporaryDirectory() as temp:
            plugin=Path(temp)/'clock.plugin'
            with contextlib.redirect_stdout(io.StringIO()):build(plugin)
            command=['nm','-gU',str(plugin)] if platform.system()=='Darwin' else ['nm','-D','--defined-only',str(plugin)]
            exports={line.split()[-1].lstrip('_') for line in subprocess.check_output(command,text=True).splitlines() if line.split()}
            self.assertEqual(exports,{'pginit','stata_call'})
            inputs=json.loads(plugin.with_suffix('.json').read_text())['inputs']
            self.assertTrue({'plugin/stplugin.c','plugin/stplugin.h','bench/clock.c'} <= set(inputs))
