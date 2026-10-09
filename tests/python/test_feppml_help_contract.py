from pathlib import Path
import hashlib
import tempfile
import unittest

from scripts.feppml_help_contract import (
    CANONICAL_HELP_PATH,
    EXPECTED_EXAMPLE_IDS,
    HIDDEN_WRAPPER_PATH,
    PUBLIC_HELP_PATH,
    SHARED_FIXTURE_PATH,
    HelpContractError,
    extract_example_blocks,
    require_public_help,
    require_public_help_bytes,
    validate_canonical_help,
    validate_canonical_help_file,
)


def _example(example_id: str, index: int) -> str:
    prediction = f"__mu_{index}"
    return "\n".join(
        (
            f"{{* example_start - {example_id}}}{{...}}",
            "clear",
            "set obs 8",
            "generate double x = _n / 10",
            "generate double y = 1 + mod(_n, 3)",
            "feppml y x if _n <= 7, vce(robust)",
            'assert `"`e(cmd)\'"\' == "feppml"',
            "assert e(converged) == 1",
            "assert e(N) == 7",
            f"predict double {prediction}, mu",
            f"assert {prediction} > 0 if e(sample)",
            f"assert missing({prediction}) if !e(sample)",
            "{* example_end}{...}",
            f"{{stata fereg_run {example_id} using feppml.sthlp:click to run}}",
        )
    )


def _canonical() -> str:
    sections = ["{smcl}", "{title:Examples}"]
    sections.extend(
        _example(example_id, index)
        for index, example_id in enumerate(EXPECTED_EXAMPLE_IDS, start=1)
    )
    return "\n\n".join(sections) + "\n"


class ParserTests(unittest.TestCase):
    def test_extracts_exact_bodies_and_line_numbers(self) -> None:
        source = (
            "prefix\n"
            "{* example_start - first}{...}\n"
            "display 1\n"
            "display 2\n"
            "{* example_end}{...}\n"
            "suffix\n"
        )
        examples = extract_example_blocks(source)
        self.assertEqual(len(examples), 1)
        self.assertEqual(examples[0].example_id, "first")
        self.assertEqual(examples[0].body, "display 1\ndisplay 2\n")
        self.assertEqual(examples[0].start_line, 2)
        self.assertEqual(examples[0].end_line, 5)

    def test_rejects_malformed_markers(self) -> None:
        malformed = (
            " {* example_start - first}{...}\n",
            "{* example_start-first}{...}\n",
            "{* example_start - First}{...}\n",
            "{* example_start - first}{...} trailing\n",
            "{* example_end }{...}\n",
            "{* example_end}{...} trailing\n",
        )
        for marker in malformed:
            with self.subTest(marker=marker), self.assertRaises(HelpContractError):
                extract_example_blocks(marker)

    def test_rejects_duplicate_nested_empty_or_unbalanced_blocks(self) -> None:
        invalid = {
            "duplicate": (
                "{* example_start - one}{...}\na\n{* example_end}{...}\n"
                "{* example_start - one}{...}\nb\n{* example_end}{...}\n"
            ),
            "nested": (
                "{* example_start - one}{...}\na\n"
                "{* example_start - two}{...}\nb\n"
                "{* example_end}{...}\n{* example_end}{...}\n"
            ),
            "empty": "{* example_start - one}{...}\n \n{* example_end}{...}\n",
            "orphan": "{* example_end}{...}\n",
            "unclosed": "{* example_start - one}{...}\ndisplay 1\n",
        }
        for name, source in invalid.items():
            with self.subTest(name=name), self.assertRaises(HelpContractError):
                extract_example_blocks(source)


class CanonicalContractTests(unittest.TestCase):
    def test_accepts_complete_contract(self) -> None:
        examples = validate_canonical_help(_canonical())
        self.assertEqual(
            tuple(example.example_id for example in examples),
            EXPECTED_EXAMPLE_IDS,
        )

    def test_inventory_and_order_are_closed(self) -> None:
        source = _canonical()
        missing = source.replace(_example(EXPECTED_EXAMPLE_IDS[-1], len(EXPECTED_EXAMPLE_IDS)), "")
        with self.assertRaisesRegex(HelpContractError, "inventory or order"):
            validate_canonical_help(missing)

        first = _example(EXPECTED_EXAMPLE_IDS[0], 1)
        second = _example(EXPECTED_EXAMPLE_IDS[1], 2)
        reordered = source.replace(first, "__FIRST__").replace(second, first)
        reordered = reordered.replace("__FIRST__", second)
        with self.assertRaisesRegex(HelpContractError, "inventory or order"):
            validate_canonical_help(reordered)

    def test_every_block_requires_literal_command_and_eclass_assertions(self) -> None:
        mutations = {
            "literal": ("feppml y x if _n <= 7", "local command feppml\n`command' y x if _n <= 7"),
            "cmd": ('assert `"`e(cmd)\'"\' == "feppml"', "display e(cmd)"),
            "converged": ("assert e(converged) == 1", "display e(converged)"),
            "additional": ("assert e(N) == 7", "display e(N)"),
        }
        source = _canonical()
        for name, (old, new) in mutations.items():
            changed = source.replace(old, new, 1)
            with self.subTest(name=name), self.assertRaises(HelpContractError):
                validate_canonical_help(changed)

    def test_every_block_requires_mu_prediction_assertions(self) -> None:
        source = _canonical()
        mutations = (
            ("predict double __mu_1, mu", "predict double __mu_1, xb"),
            ("assert __mu_1 > 0 if e(sample)", "assert __mu_1 > 0"),
            (
                "assert missing(__mu_1) if !e(sample)",
                "assert missing(__mu_1)",
            ),
        )
        for old, new in mutations:
            with self.subTest(old=old), self.assertRaises(HelpContractError):
                validate_canonical_help(source.replace(old, new, 1))

    def test_clickable_link_inventory_target_and_syntax_are_closed(self) -> None:
        source = _canonical()
        mutations = (
            (
                "{stata fereg_run ppml_basic using feppml.sthlp:click to run}",
                "",
            ),
            (
                "fereg_run ppml_basic using feppml.sthlp",
                "fereg_run ppml_basic using feppml_public.sthlp",
            ),
            (
                "{stata fereg_run ppml_basic using feppml.sthlp:click to run}",
                "{stata fereg_run ppml_basic using feppml.sthlp}",
            ),
        )
        for old, new in mutations:
            with self.subTest(old=old), self.assertRaises(HelpContractError):
                validate_canonical_help(source.replace(old, new, 1))

    def test_stata_extraction_line_limit_is_closed(self) -> None:
        source = _canonical() + ("x" * 245) + "\n"
        with self.assertRaisesRegex(HelpContractError, "at most 244"):
            validate_canonical_help(source)


class PublicHelpBoundaryTests(unittest.TestCase):
    def test_current_public_help_matches_its_reviewed_hash(self) -> None:
        require_public_help()

    def test_public_help_requires_exact_full_bytes(self) -> None:
        canonical = _canonical().encode("utf-8")
        examples = require_public_help_bytes(canonical, canonical)
        self.assertEqual(
            tuple(example.example_id for example in examples),
            EXPECTED_EXAMPLE_IDS,
        )
        with self.assertRaisesRegex(HelpContractError, "differs from canonical bytes"):
            require_public_help_bytes(canonical + b"\n", canonical)

    def test_file_entry_point_requires_the_reviewed_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            public = Path(temporary) / "public.sthlp"
            public.write_bytes(_canonical().encode("utf-8"))
            reviewed = hashlib.sha256(public.read_bytes()).hexdigest()
            require_public_help(public, reviewed)
            public.write_bytes(public.read_bytes().replace(b"Examples", b"Examplez"))
            with self.assertRaises(HelpContractError):
                require_public_help(public, reviewed)


class SharedFixtureTests(unittest.TestCase):
    def test_stata_helper_program_names_fit_the_identifier_limit(self) -> None:
        source = SHARED_FIXTURE_PATH.read_text(encoding="utf-8")
        helper_names = ("_fereg_ppml_help_run", "_fereg_ppml_help_inner")
        for helper_name in helper_names:
            with self.subTest(helper_name=helper_name):
                self.assertLessEqual(len(helper_name), 32)
                self.assertIn(f"program define {helper_name}\n", source)
        self.assertNotIn("_fereg_feppml_run_help_example", source)

    def test_invalid_utf8_fails_closed(self) -> None:
        with self.assertRaisesRegex(HelpContractError, "not UTF-8"):
            require_public_help_bytes(b"\xff", b"\xff")


class RepositorySourceContractTests(unittest.TestCase):
    def test_canonical_public_help_contract(self) -> None:
        examples = validate_canonical_help_file()
        self.assertEqual(
            tuple(example.example_id for example in examples),
            EXPECTED_EXAMPLE_IDS,
        )

    def test_documentation_fixtures_use_the_canonical_source(self) -> None:
        self.assertEqual(
            CANONICAL_HELP_PATH.relative_to(CANONICAL_HELP_PATH.parents[1]).as_posix(),
            "stata/feppml.sthlp",
        )
        shared = SHARED_FIXTURE_PATH.read_text(encoding="utf-8")
        wrapper = HIDDEN_WRAPPER_PATH.read_text(encoding="utf-8")
        self.assertIn('local feppml_help_source "feppml.sthlp"', shared)
        for example_id in EXPECTED_EXAMPLE_IDS:
            self.assertIn(example_id, shared)
        self.assertIn("$FEREG_PPML_TEST_COMMAND", shared)
        self.assertIn("_fereg_ppml", wrapper)
        self.assertIn('global FEREG_PPML_HELP_SOURCE "feppml.sthlp"', wrapper)
        self.assertIn(SHARED_FIXTURE_PATH.name, wrapper)


if __name__ == "__main__":
    unittest.main()
