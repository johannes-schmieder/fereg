"""ABI harness worker admission, without loading a plugin or running Rust."""
from __future__ import annotations

import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
INVALID = ("", "0", "00", "01", "33", "99", "100", "-1", "+4", " 4", "4 ",
           "4\n", "4.0", "4x", "1e1", "٤", "1٤", "3١", "４", "9" * 1000)


@unittest.skipUnless(sys.platform in ("darwin", "linux"), "Unix actual-plugin harness only")
class ActualPluginThreadsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        output = ROOT / "target/tmp/solver-routing"
        output.mkdir(parents=True, exist_ok=True)
        cls.temporary = tempfile.TemporaryDirectory(prefix="abi-threads-", dir=output)
        cls.directory = Path(cls.temporary.name)
        cls.addClassCleanup(cls.temporary.cleanup)
        driver = cls.directory / "driver.c"
        harness = ROOT / "tests/mock_spi/actual_plugin_loader_test.c"
        driver.write_text('''#include <stdio.h>
#include <string.h>
int harness_main(int argc, char *argv[]);
#define main harness_main
''' + '#include ' + json.dumps(str(harness)) + '''
#undef main
static ST_retcode fake_call(int argc, char *argv[])
{
    int i;
    int found = 0;
    for (i = 0; i < argc; ++i) {
        if (strncmp(argv[i], "threads=", 8) == 0) {
            puts(argv[i]);
            ++found;
        }
    }
    if (found != 1) return 999;
    strcpy(display_buffer,
           "fereg.plugin_version=0.5.0 fereg.action_protocol=1.6 "
           "fereg.model_wire=2.1 fereg.ols_result_wire=1.5 "
           "fereg.ppml_request_wire=1.8 fereg.ppml_result_wire=1.5 "
           "fereg.selftest.overall=pass fereg.ado_plugin_compatible=yes");
    return 0;
}
int main(int argc, char *argv[])
{
    int threads;
    if (argc < 2) return 9;
    if (strcmp(argv[1], "--harness") == 0) return harness_main(argc - 1, argv + 1);
    if (test_thread_limit(getenv("FEREG_TEST_THREADS"), &threads) != 0) return 2;
    if (strcmp(argv[1], "verbose") == 0) return check_selftest_verbose(fake_call, threads);
    if (strcmp(argv[1], "plain") == 0) return check_selftest(fake_call, threads);
    return 9;
}
''')
        cls.binary = cls.directory / "driver"
        flags = ["-std=c11", "-O0", "-Wall", "-Wextra", "-Wpedantic", "-Wconversion",
                 "-Wshadow", "-Wstrict-prototypes", "-Wmissing-prototypes", "-Werror"]
        system = "APPLEMAC" if sys.platform == "darwin" else "OPUNIX"
        libraries = [] if sys.platform == "darwin" else ["-ldl", "-lm"]
        command = [*shlex.split(os.environ.get("CC", "cc")), *flags,
                   f"-DSYSTEM={system}", str(driver), *libraries, "-o", str(cls.binary)]
        result = subprocess.run(command, capture_output=True, text=True, timeout=30)
        if result.returncode:
            raise AssertionError(f"mock C driver compile failed: {result.stderr}")

    def environment(self, value):
        env = dict(os.environ)
        env.pop("FEREG_TEST_THREADS", None)
        if value is not None:
            env["FEREG_TEST_THREADS"] = value
        return env

    def invoke(self, value, *arguments):
        return subprocess.run([str(self.binary), *arguments], env=self.environment(value),
                              capture_output=True, text=True, timeout=5)

    def test_default_and_every_supported_cap_reach_actual_argument_arrays(self):
        for value in (None, *(str(n) for n in range(1, 33))):
            for mode in ("plain", "verbose"):
                with self.subTest(value=value, mode=mode):
                    result = self.invoke(value, mode)
                    cap = 32 if value is None else int(value)
                    expected = cap if mode == "plain" else min(2, cap)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(result.stdout, f"threads={expected}\n")

    def test_invalid_cap_rejected_before_library_loading(self):
        for value in INVALID:
            with self.subTest(value=value):
                result = self.invoke(value, "--harness", "/nonexistent-fereg-test.plugin")
                self.assertEqual(result.returncode, 2)
                self.assertIn("FEREG_TEST_THREADS must", result.stderr)
                self.assertNotIn("dlopen", result.stderr)

    def test_replay_cannot_silently_ignore_or_rewrite_an_abi_cap(self):
        for value in ("1", "4", "32"):
            with self.subTest(value=value):
                result = self.invoke(value, "--harness", "/nonexistent-fereg-test.plugin",
                                     "--replay", "input", "output")
                self.assertEqual(result.returncode, 2)
                self.assertIn("ABI-only", result.stderr)
                self.assertNotIn("dlopen", result.stderr)

    def test_unset_replay_still_reaches_unchanged_loader(self):
        result = self.invoke(None, "--harness", "/nonexistent-fereg-test.plugin",
                             "--replay", "input", "output")
        self.assertEqual(result.returncode, 3)
        self.assertIn("dlopen failed", result.stderr)

    def test_shell_rejects_invalid_cap_before_compilation(self):
        script = ROOT / "tests/mock_spi/run_actual_unix_plugin_test.sh"
        installed = subprocess.run(["locale", "-a"], capture_output=True, text=True,
                                   timeout=5, check=True).stdout.splitlines()
        utf8 = sorted(name for name in installed
                      if name.lower().replace("-", "") in {"c.utf8", "en_us.utf8", "ar_eg.utf8"})
        for language in (None, "C", *utf8):
            for value in INVALID:
                with self.subTest(locale=language or "inherited", value=value):
                    env = self.environment(value)
                    env["CC"] = "/nonexistent-test-compiler"
                    if language is not None:
                        env["LC_ALL"] = language
                    result = subprocess.run(["bash", str(script), "/nonexistent-dir/plugin"],
                                            env=env, capture_output=True, text=True, timeout=5)
                    self.assertEqual(result.returncode, 2, result.stderr)
                    self.assertIn("FEREG_TEST_THREADS must", result.stderr)
                    self.assertNotIn("No such file", result.stderr)

    def test_shell_passes_cap_to_harness_and_preserves_unset_default(self):
        mock_root = self.directory / "shell"
        scripts = mock_root / "tests/mock_spi"
        scripts.mkdir(parents=True)
        script = scripts / "run_actual_unix_plugin_test.sh"
        shutil.copyfile(ROOT / "tests/mock_spi/run_actual_unix_plugin_test.sh", script)
        plugin = mock_root / "unused.plugin"
        plugin.touch()
        compiler = mock_root / "mock-cc"
        compiler.write_text(f"#!{sys.executable}\n" + '''import pathlib, sys
output = pathlib.Path(sys.argv[sys.argv.index('-o') + 1])
output.write_text('#!/bin/sh\\nprintf "%s\\\\n" "${FEREG_TEST_THREADS-unset}"\\n')
output.chmod(0o755)
''')
        compiler.chmod(0o755)
        for value in (None, "1", "4", "32"):
            with self.subTest(value=value):
                env = self.environment(value)
                env["CC"] = str(compiler)
                result = subprocess.run(["bash", str(script), str(plugin)], env=env,
                                        capture_output=True, text=True, timeout=5)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout, (value or "unset") + "\n")


if __name__ == "__main__":
    unittest.main()
