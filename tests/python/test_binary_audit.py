from __future__ import annotations

import contextlib
import importlib.util
import io
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "fereg_audit_binary", ROOT / "scripts" / "audit_binary.py"
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

ELF_HEADER = """\
ELF Header:
  Class:                             ELF64
  Data:                              2's complement, little endian
  Type:                              DYN (Shared object file)
  Machine:                           Advanced Micro Devices X86-64
"""

PE_HEADERS = """\
FILE HEADER VALUES
            8664 machine (x64)
OPTIONAL HEADER VALUES
             20B magic # (PE32+)
            8160 characteristics
                   DLL
"""


class BinaryAuditTests(unittest.TestCase):
    def test_glibc_baseline_accepts_2_28(self):
        text = "Name: GLIBC_2.2.5 Name: GLIBC_2.17 Name: GLIBC_2.28"
        self.assertEqual(MODULE.enforce_glibc_baseline(text), (2, 28))

    def test_glibc_baseline_rejects_newer_symbol(self):
        with self.assertRaisesRegex(RuntimeError, "GLIBC_2.34"):
            MODULE.enforce_glibc_baseline("Name: GLIBC_2.34")

    def test_no_glibc_symbol_is_reported_as_none(self):
        self.assertIsNone(MODULE.enforce_glibc_baseline("not an ELF version table"))

    def test_elf_header_requires_exact_target(self):
        MODULE.validate_elf_header(ELF_HEADER)
        with self.assertRaisesRegex(RuntimeError, "wrong ELF machine"):
            MODULE.validate_elf_header(ELF_HEADER.replace("X86-64", "AArch64"))
        with self.assertRaisesRegex(RuntimeError, "wrong ELF class"):
            MODULE.validate_elf_header(ELF_HEADER.replace("ELF64", "ELF32"))

    def test_pe_header_requires_x64_pe32_plus_dll(self):
        MODULE.validate_pe_headers(PE_HEADERS)
        replacements = (
            ("8664 machine (x64)", "14C machine (x86)", "x86-64"),
            ("20B magic # (PE32+)", "10B magic # (PE32)", "PE32\\+"),
            ("                   DLL", "                   EXECUTABLE_IMAGE", "DLL"),
        )
        for old, new, message in replacements:
            with self.subTest(old=old), self.assertRaisesRegex(RuntimeError, message):
                MODULE.validate_pe_headers(PE_HEADERS.replace(old, new))

    def test_macho_header_requires_matching_64_bit_bundle(self):
        text = "MH_MAGIC_64 X86_64 ALL 0x00 BUNDLE 12 1000 0x0"
        MODULE.validate_macho_header(text, "x86_64")
        with self.assertRaisesRegex(RuntimeError, "arm64 slice"):
            MODULE.validate_macho_header(text, "arm64")
        with self.assertRaisesRegex(RuntimeError, "bundle"):
            MODULE.validate_macho_header(text.replace("BUNDLE", "DYLIB"), "x86_64")

    def test_dependency_parsers_return_library_names_only(self):
        elf = "0x1 (NEEDED) Shared library: [libc.so.6]\n0x1 (NEEDED) Shared library: [libm.so.6]"
        macho = "/tmp/a.plugin:\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1.0.0)"
        pe = "Image has the following dependencies:\n    KERNEL32.dll\n    bcrypt.dll\n\nSummary"
        self.assertEqual(MODULE.parse_elf_dependencies(elf), ["libc.so.6", "libm.so.6"])
        self.assertEqual(MODULE.parse_otool_dependencies(macho), ["/usr/lib/libSystem.B.dylib"])
        self.assertEqual(MODULE.parse_dumpbin_dependencies(pe), ["KERNEL32.dll", "bcrypt.dll"])

    def test_every_forbidden_runtime_family_is_rejected(self):
        dependencies = (
            "libstd-deadbeef12345678.so", "libpython3.12.so", "libjulia.so",
            "libopenblas.so.0", "libmkl_core.so", "libblas.so.3", "liblapack.so.3",
            "libgomp.so.1", "libomp.dylib", "libiomp5.so", "vcomp140.dll",
            "libtbb.so.12", "libstdc++.so.6", "libc++.1.dylib", "msvcp140.dll",
        )
        for dependency in dependencies:
            with self.subTest(dependency=dependency), self.assertRaisesRegex(RuntimeError, "forbidden"):
                MODULE.reject_forbidden_dependencies([dependency])
        MODULE.reject_forbidden_dependencies(
            ["libc.so.6", "/usr/lib/libSystem.B.dylib", "KERNEL32.dll"]
        )

    def test_windows_dynamic_crt_is_rejected(self):
        for dependency in (
            "ucrtbase.dll", "vcruntime140_1.dll", "msvcrt.dll", "msvcp140.dll",
            "concrt140.dll", "api-ms-win-crt-runtime-l1-1-0.dll",
        ):
            with self.subTest(dependency=dependency), self.assertRaisesRegex(
                RuntimeError, "statically CRT-linked"
            ):
                MODULE.enforce_windows_static_crt([dependency])
        MODULE.enforce_windows_static_crt(["KERNEL32.dll", "bcrypt.dll"])

    def test_exports_accept_only_required_names_and_one_leading_underscore(self):
        self.assertEqual(
            MODULE.validate_exports(["_pginit", "_stata_call"], context="test"),
            ["pginit", "stata_call"],
        )
        MODULE.validate_exports(["pginit", "stata_call"], context="test")
        with self.assertRaisesRegex(RuntimeError, "unexpected public exports"):
            MODULE.validate_exports(["pginit", "stata_call", "fereg_dispatch"], context="test")
        with self.assertRaisesRegex(RuntimeError, "missing"):
            MODULE.validate_exports(["pginit"], context="test")
        with self.assertRaisesRegex(RuntimeError, "missing"):
            MODULE.validate_exports(["__pginit", "_stata_call"], context="test")

    def test_platform_export_parsers_ignore_tool_boilerplate(self):
        elf = "pginit T 10 20\nstata_call T 30 40\n"
        macho = "/tmp/a.plugin (for architecture x86_64):\n_pginit\n_stata_call\n"
        pe = "ordinal hint RVA name\n      1    0 00001000 pginit\n      2    1 00002000 stata_call\n  Summary\n"
        self.assertEqual(MODULE.parse_elf_exports(elf), ["pginit", "stata_call"])
        self.assertEqual(MODULE.parse_macho_exports(macho), ["_pginit", "_stata_call"])
        self.assertEqual(MODULE.parse_dumpbin_exports(pe), ["pginit", "stata_call"])

    def test_linux_resolution_rejects_missing_library_and_symbol(self):
        MODULE.validate_linux_resolution("libc.so.6 => /lib/libc.so.6")
        for output in ("libfoo.so => not found", "undefined symbol: missing_ffi (./plugin)"):
            with self.subTest(output=output), self.assertRaisesRegex(RuntimeError, "unresolved"):
                MODULE.validate_linux_resolution(output)

    def test_macho_resolution_rejects_dynamic_lookup(self):
        MODULE.validate_macho_undefineds(
            "(undefined) external _malloc (from libSystem)", architecture="arm64"
        )
        for output in (
            "(undefined) external _missing (dynamically looked up)",
            "(undefined) external _missing",
        ):
            with self.subTest(output=output), self.assertRaisesRegex(RuntimeError, "unresolved"):
                MODULE.validate_macho_undefineds(output, architecture="arm64")

    def test_binary_format_detection_uses_magic_not_host_platform(self):
        prefixes = (
            (b"\x7fELF", "elf"), (b"MZ\x90\x00", "pe"),
            (b"\xca\xfe\xba\xbe", "macho"),
        )
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "artifact.plugin"
            for prefix, expected in prefixes:
                with self.subTest(expected=expected):
                    path.write_bytes(prefix + b"payload")
                    self.assertEqual(MODULE.detect_binary_format(path), expected)
            path.write_bytes(b"text")
            with self.assertRaisesRegex(RuntimeError, "unrecognized"):
                MODULE.detect_binary_format(path)

    def test_end_to_end_linux_audit(self):
        responses = {
            ("readelf", "--file-header"): ELF_HEADER,
            ("readelf", "--dynamic"): "0x1 (NEEDED) Shared library: [libc.so.6]",
            ("nm", "--dynamic"): "pginit T 10 20\nstata_call T 30 40\n",
            ("readelf", "--version-info"): "Name: GLIBC_2.28",
            ("ldd", "-r"): "libc.so.6 => /lib/libc.so.6",
        }
        report = self._mocked_audit(b"\x7fELF", responses)
        self.assertEqual(report["target"], "x86_64-unknown-linux-gnu")
        self.assertEqual(report["newest_glibc"], (2, 28))

    def test_end_to_end_macos_audits_both_slices(self):
        def response(*args: str) -> str:
            if args[0:2] == ("lipo", "-archs"):
                return "x86_64 arm64\n"
            architecture = args[args.index("-arch") + 1]
            if args[0:2] == ("otool", "-hv"):
                return f"MH_MAGIC_64 {architecture.upper()} ALL 0x00 BUNDLE 12 1000 0x0"
            if args[0:2] == ("otool", "-L"):
                return "\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1.0.0)"
            if args[0:2] == ("nm", "-gjU"):
                return "_pginit\n_stata_call\n"
            if args[0:3] == ("nm", "-m", "-u"):
                return "(undefined) external _malloc (from libSystem)"
            raise AssertionError(args)

        report = self._mocked_audit(b"\xca\xfe\xba\xbe", response)
        self.assertEqual(set(report["architectures"]), {"x86_64", "arm64"})
        self.assertEqual(set(report["exports"]), {"x86_64", "arm64"})

    def test_end_to_end_macos_rejects_thin_artifact(self):
        with self.assertRaisesRegex(RuntimeError, "exactly universal"):
            self._mocked_audit(
                b"\xfe\xed\xfa\xcf", {("lipo", "-archs"): "arm64\n"}
            )

    def test_end_to_end_windows_audit(self):
        responses = {
            ("dumpbin", "/HEADERS"): PE_HEADERS,
            ("dumpbin", "/DEPENDENTS"): "Image has the following dependencies:\n KERNEL32.dll\n bcrypt.dll\n",
            ("dumpbin", "/EXPORTS"): "      1    0 00001000 pginit\n      2    1 00002000 stata_call\n",
            ("dumpbin", "/IMPORTS"): "Section contains the following imports: KERNEL32.dll",
        }
        report = self._mocked_audit(b"MZ\x90\x00", responses)
        self.assertEqual(report["target"], "x86_64-pc-windows-msvc")
        self.assertIn("target-native loader test required", report["resolution_check"])

    def test_builds_checksum_only_after_audit_and_loader(self):
        expectations = {
            "build_linux.sh": ("audit_binary.py", "run_actual_unix_plugin_test.sh", "sha256sum"),
            "build_macos.sh": ("audit_binary.py", "run_actual_unix_plugin_test.sh", "shasum -a 256"),
            "build_windows.ps1": ("audit_binary.py", "run_actual_windows_plugin_test.ps1", "Get-FileHash"),
        }
        for filename, ordered_tokens in expectations.items():
            with self.subTest(filename=filename):
                text = (ROOT / "scripts" / filename).read_text(encoding="utf-8")
                positions = [text.index(token) for token in ordered_tokens]
                self.assertEqual(positions, sorted(positions))

    def _mocked_audit(self, magic: bytes, responses):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "artifact.plugin"
            path.write_bytes(magic + b"payload")

            def fake_run(*args: str) -> str:
                if callable(responses):
                    return responses(*args)
                for prefix, output in responses.items():
                    if args[: len(prefix)] == prefix:
                        return output
                raise AssertionError(args)

            with mock.patch.object(MODULE, "require_tool", side_effect=lambda name: name), mock.patch.object(
                MODULE, "run", side_effect=fake_run
            ), contextlib.redirect_stdout(io.StringIO()):
                return MODULE.audit(path)


if __name__ == "__main__":
    unittest.main()
