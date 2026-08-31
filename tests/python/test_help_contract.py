from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]
HELP = ROOT / "stata" / "fereg.sthlp"


class HelpContractTests(unittest.TestCase):
    def test_header_footer_and_sources(self) -> None:
        text = HELP.read_text(encoding="utf-8")
        self.assertIn(
            "{.-}\nhelp for {cmd:fereg} "
            "{right:(Johannes F. Schmieder)}\n{.-}",
            text,
        )
        self.assertIn("{title:Sources and acknowledgements}", text)
        author = text[text.index("{title:Author}") :]
        self.assertIn("Johannes F. Schmieder, Boston University, USA", author)
        self.assertIn("https://johannes-schmieder.com/", author)
        self.assertIn("https://github.com/johannes-schmieder", author)
        self.assertIn("mailto:johannes@bu.edu", author)
        self.assertIn("Comments and suggestions are welcome.", author)
        self.assertNotIn("{title:", author[len("{title:Author}") :])

    def test_every_example_is_clickable_and_packaged(self) -> None:
        text = HELP.read_text(encoding="utf-8")
        examples = (
            "basic_two_way",
            "factor_if",
            "weights",
            "clusters",
            "string_fe",
            "interacted_fe",
            "absorbed_slope",
            "slope_only",
            "saved_outputs",
            "disconnected",
            "low_memory",
            "benchmark",
        )
        for example in examples:
            marker = f"{{* example_start - {example}}}{{...}}"
            self.assertEqual(text.count(marker), 1)
            self.assertIn(
                f"fereg_run {example} using fereg.sthlp",
                text,
            )
        self.assertEqual(text.count("{* example_end}{...}"), len(examples))

        runner = (ROOT / "stata" / "fereg_run.ado").read_text(encoding="utf-8")
        package = (ROOT / "stata" / "fereg.pkg").read_text(encoding="utf-8")
        staging = (ROOT / "scripts" / "stage_release.py").read_text(
            encoding="utf-8"
        )
        self.assertIn("program define fereg_run", runner)
        self.assertIn("preserve", runner)
        self.assertIn("capture restore", runner)
        self.assertEqual(package.count("f fereg_run.ado"), 1)
        self.assertIn('"fereg_run.ado"', staging)

        profiles = (ROOT / "ci" / "stata_profiles.json").read_text(
            encoding="utf-8"
        )
        self.assertGreaterEqual(
            profiles.count("tests/stata/documentation/help_examples.do"), 2
        )


if __name__ == "__main__":
    unittest.main()
