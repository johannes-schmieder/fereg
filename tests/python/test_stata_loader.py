from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
LOADER = (ROOT / "stata/_fereg_load.ado").read_text(encoding="utf-8")


def expected_plugin(os_name: str, machine_type: str) -> str | None:
    if os_name == "Windows":
        return "fereg_windows.plugin"
    if os_name == "MacOSX":
        return "fereg_macos.plugin"
    if os_name == "Unix" and machine_type.startswith("Mac"):
        return "fereg_macos.plugin"
    if os_name == "Unix":
        return "fereg_linux.plugin"
    return None


class StataLoaderPlatformTests(unittest.TestCase):
    def test_required_platform_matrix(self):
        cases = {
            ("Unix", "Mac (Apple Silicon)"): "fereg_macos.plugin",
            ("Unix", "Macintosh (Intel 64-bit)"): "fereg_macos.plugin",
            ("MacOSX", "Macintosh (Intel 64-bit)"): "fereg_macos.plugin",
            ("Unix", "PC (64-bit x86-64)"): "fereg_linux.plugin",
            ("Windows", "PC (64-bit x86-64)"): "fereg_windows.plugin",
            ("Plan9", "unknown"): None,
        }
        for platform, plugin in cases.items():
            with self.subTest(platform=platform):
                self.assertEqual(expected_plugin(*platform), plugin)

    def test_loader_uses_machine_type_for_unix_mac_before_linux(self):
        self.assertIn("c(machine_type)", LOADER)
        mac_condition = 'else if `"`os\'"\' == "Unix" & strmatch(`"`machine_type\'"\', "Mac*")'
        linux_condition = 'else if `"`os\'"\' == "Unix" {'
        self.assertIn(mac_condition, LOADER)
        self.assertIn(linux_condition, LOADER)
        self.assertLess(LOADER.index(mac_condition), LOADER.index(linux_condition))

    def test_unknown_operating_system_fails_closed(self):
        self.assertIn("does not recognize operating system/machine type", LOADER)
        self.assertNotIn('else {\n        local plugin_name "fereg_linux.plugin"', LOADER)


if __name__ == "__main__":
    unittest.main()
