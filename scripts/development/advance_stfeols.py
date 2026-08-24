#!/usr/bin/env python3
"""Advance the private stfeols development gates on the licensed Mac runner.

This is development infrastructure, not a public estimator path.  It makes one
coherent source commit at a time, qualifies that exact commit with the existing
Rust/C/source and licensed-Stata machinery, and publishes the resulting receipt
before advancing.  Any red receipt stops the driver fail-closed.
"""

from __future__ import annotations

import json
import math
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Iterable, Sequence

ROOT = Path(__file__).resolve().parents[2]
REPO = "johannes-schmieder/StFixest"
REFERENCE_SOURCE_SHA = "e81b942c0dc0b122258489a60c956cdbe0121512"
REFERENCE_PAYLOAD_SHA256 = "1fba54c0adce40ec52964e1edd1b3e57283bc803cb07009a84933b241e630955"
REFERENCE_RECEIPT = (
    ROOT
    / ".ci/reference-probes/fe-interaction"
    / f"{REFERENCE_SOURCE_SHA}.json"
)
STATA_BIN = "/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp"
RECEIPT_EXCLUDES = (
    ".ci/stata/latest.json",
    ".ci/stata/results/",
)


def run(
    command: Sequence[str],
    *,
    cwd: Path = ROOT,
    env: dict[str, str] | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    print("+", " ".join(command), flush=True)
    result = subprocess.run(
        list(command),
        cwd=cwd,
        env=env,
        text=True,
        stdout=sys.stdout,
        stderr=sys.stderr,
        check=False,
    )
    if check and result.returncode != 0:
        raise SystemExit(f"command failed with rc={result.returncode}: {' '.join(command)}")
    return result


def capture(command: Sequence[str], *, cwd: Path = ROOT) -> str:
    return subprocess.check_output(list(command), cwd=cwd, text=True).strip()


def git(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return run(("git", *args), check=check)


def git_capture(*args: str) -> str:
    return capture(("git", *args))


def configure_git() -> None:
    git("config", "user.name", "github-actions[bot]")
    git("config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com")


def sync_main() -> None:
    git("checkout", "main")
    git("pull", "--ff-only", "origin", "main")


def push_main() -> None:
    for attempt in range(1, 5):
        git("fetch", "origin", "main")
        result = git("rebase", "origin/main", check=False)
        if result.returncode != 0:
            git("rebase", "--abort", check=False)
            raise SystemExit("development driver could not rebase current work on main")
        result = git("push", "origin", "HEAD:main", check=False)
        if result.returncode == 0:
            return
        run(("sleep", str(2 * attempt)))
    raise SystemExit("development driver lost four main update races")


def commit_source(message: str, paths: Iterable[Path]) -> str:
    relative = [str(path.relative_to(ROOT)) for path in paths if path.exists()]
    deleted = [
        str(path.relative_to(ROOT))
        for path in paths
        if not path.exists()
    ]
    if relative:
        git("add", "--", *relative)
    if deleted:
        git("add", "-u", "--", *deleted, check=False)
    if git("diff", "--cached", "--quiet", check=False).returncode == 0:
        return git_capture("rev-parse", "HEAD")
    git("commit", "-m", message)
    sha = git_capture("rev-parse", "HEAD")
    push_main()
    return sha


def source_commit_from_head() -> str:
    for sha in git_capture("rev-list", "HEAD").splitlines():
        names = capture(
            (
                "git",
                "diff-tree",
                "--no-commit-id",
                "--name-only",
                "-r",
                "--root",
                sha,
            )
        ).splitlines()
        if any(
            name != RECEIPT_EXCLUDES[0] and not name.startswith(RECEIPT_EXCLUDES[1])
            for name in names
        ):
            return sha
    raise SystemExit("could not identify the current non-receipt source commit")


def receipt_for(sha: str) -> Path:
    return ROOT / ".ci/stata/results" / f"{sha}.json"


def load_receipt(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def receipt_is_green(receipt: dict[str, object]) -> bool:
    return (
        receipt.get("status") == "success"
        and receipt.get("stata_status") == "success"
        and receipt.get("rust_status") == "success"
        and receipt.get("stata_rc") == 0
        and receipt.get("rust_rc") == 0
        and receipt.get("tests_failed") == 0
    )


def qualification_environment(sha: str) -> dict[str, str]:
    env = os.environ.copy()
    env.update(
        {
            "GITHUB_SHA": sha,
            "GITHUB_REF": "refs/heads/main",
            "GITHUB_REF_NAME": "main",
            "STATA_PROFILE": "quick",
            "STATA_BIN": STATA_BIN,
            "CARGO_TERM_COLOR": "always",
            "RUST_BACKTRACE": "1",
        }
    )
    return env


def qualify_exact_source(sha: str) -> dict[str, object]:
    if git_capture("rev-parse", "HEAD") != sha:
        raise SystemExit("exact-source qualification requires HEAD at the source commit")
    shutil.rmtree(ROOT / ".ci/stata/run", ignore_errors=True)
    env = qualification_environment(sha)
    stata = run(("./ci/run_stata_ci.sh", "quick"), env=env, check=False)
    rust = run(("./ci/record_rust_quick.sh",), env=env, check=False)
    if not (ROOT / ".ci/stata/run/receipt.json").exists():
        raise SystemExit("licensed qualification did not produce a Stata receipt")
    if not (ROOT / ".ci/stata/run/rust-quick.status").exists():
        raise SystemExit("licensed qualification did not produce a Rust status")
    run(
        (
            "/usr/bin/python3",
            "ci/augment_ci_receipt.py",
            ".ci/stata/run/receipt.json",
            ".ci/stata/run/rust-quick.status",
        ),
        env=env,
    )
    run(
        (
            "/usr/bin/python3",
            "ci/check_stata_receipt.py",
            ".ci/stata/run/receipt.json",
            "--expect-tested-sha",
            sha,
            "--expect-profile",
            "quick",
        ),
        env=env,
    )
    document = load_receipt(ROOT / ".ci/stata/run/receipt.json")
    if document.get("tested_sha") != sha:
        raise SystemExit("qualification receipt tested a different source SHA")
    destination = receipt_for(sha)
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / ".ci/stata/run/receipt.json", destination)
    staged = [destination]
    if receipt_is_green(document):
        latest = ROOT / ".ci/stata/latest.json"
        shutil.copy2(ROOT / ".ci/stata/run/receipt.json", latest)
        staged.append(latest)
    receipt_sha = commit_source(f"ci(stata): record {sha[:12]} [skip ci]", staged)
    print(
        "QUALIFIED",
        f"source={sha}",
        f"receipt_commit={receipt_sha}",
        f"status={document.get('status')}",
        f"stata_rc={stata.returncode}",
        f"rust_rc={rust.returncode}",
        f"tests_passed={document.get('tests_passed')}",
        f"tests_failed={document.get('tests_failed')}",
        flush=True,
    )
    if not receipt_is_green(document):
        raise SystemExit(f"exact-SHA qualification is red for {sha}")
    return document


def require_current_green() -> tuple[str, dict[str, object]]:
    source_sha = source_commit_from_head()
    path = receipt_for(source_sha)
    if not path.exists():
        raise SystemExit(
            f"current source {source_sha} has no committed exact-SHA receipt; "
            "make a bounded source change or qualify it before advancing"
        )
    document = load_receipt(path)
    if not receipt_is_green(document):
        raise SystemExit(f"current source receipt is red: {path}")
    return source_sha, document


def strip_one_shot_workflows() -> list[Path]:
    changed: list[Path] = []
    workflow = ROOT / ".github/workflows/stata-ci.yml"
    marker = "\n  # BEGIN ONE-SHOT ABSORBED-INTERCEPT SCORE REPAIR\n"
    text = workflow.read_text(encoding="utf-8")
    if marker in text:
        workflow.write_text(text.split(marker, 1)[0] + "\n", encoding="utf-8")
        changed.append(workflow)
    standalone = ROOT / ".github/workflows/repair-prepared-score-intercept.yml"
    if standalone.exists():
        standalone.unlink()
        changed.append(standalone)
    return changed


def apply_score_repair_if_needed() -> bool:
    source = ROOT / "crates/stfixest-core/src/ols/prepared_fit.rs"
    text = source.read_text(encoding="utf-8")
    score_start = text.index("fn certify_scores")
    score_end = text.index("\nfn compensated_add", score_start)
    score = text[score_start:score_end]
    old = (
        "        let denominator = if score_norm > 0.0 {\n"
        "            score_norm\n"
        "        } else {\n"
        "            reference_norm\n"
        "        };\n"
    )
    already = "status == PreparedOlsFitStatus::NormalizedAbsorbedIntercept"
    changed = strip_one_shot_workflows()
    if already in score:
        if changed:
            sha = commit_source("ci: remove completed score-repair machinery", changed)
            qualify_exact_source(sha)
        return False
    if score.count(old) != 1:
        raise SystemExit("prepared score denominator no longer matches the reviewed repair anchor")
    new = (
        "        let denominator =\n"
        "            if status == PreparedOlsFitStatus::NormalizedAbsorbedIntercept {\n"
        "                reference_norm\n"
        "            } else if score_norm > 0.0 {\n"
        "                score_norm\n"
        "            } else {\n"
        "                reference_norm\n"
        "            };\n"
    )
    score = score.replace(old, new)
    text = text[:score_start] + score + text[score_end:]

    test_name = "fn zero_within_norm_column_still_checks_its_raw_score_moment"
    test_start = text.index(test_name)
    test_end = text.index("\n    #[test]", test_start)
    test = text[test_start:test_end]
    regression_token = "roundoff score certificate"
    if regression_token not in test:
        failure_marker = '.expect_err("a deferred intercept must retain its raw score moment")'
        failure_position = test.index(failure_marker)
        insert_at = test.rfind(
            "\n        let mut reservation = InjectedReservation::default();\n",
            0,
            failure_position,
        )
        if insert_at < 0:
            raise SystemExit("could not locate the raw-score regression insertion point")
        insertion = (
            "\n        // A structurally absorbed intercept can acquire a tiny nonzero within\n"
            "        // norm from iterative MAP roundoff. Its score remains a raw intercept\n"
            "        // moment, not a moment normalized by that numerical noise.\n"
            "        let weights = normalized_weights(&prepared).expect(\"weights\");\n"
            "        let rhs = prepared.fit_build_rhs().expect(\"rhs\");\n"
            "        let reference_norm = weighted_matrix_column_norm(&rhs, 1, weights)\n"
            "            .expect(\"reference intercept norm\");\n"
            "        let mut roundoff_residuals = control.residuals().to_vec();\n"
            "        for residual in &mut roundoff_residuals {\n"
            "            *residual += 1e-14;\n"
            "        }\n"
            "        let roundoff = certify_scores(\n"
            "            &prepared,\n"
            "            &roundoff_residuals,\n"
            "            weights,\n"
            "            &[reference_norm],\n"
            "            &[reference_norm * 1e-14],\n"
            "            &[PreparedOlsFitStatus::NormalizedAbsorbedIntercept],\n"
            "            1.0,\n"
            "            PreparedOlsFitConfig::default().score_tolerance,\n"
            "        )\n"
            "        .expect(\"roundoff score certificate\");\n"
            "        assert!(roundoff.passed);\n"
            "        assert!(roundoff.relative_infinity_norm < 1e-12);\n"
        )
        test = test[:insert_at] + insertion + test[insert_at:]
        text = text[:test_start] + test + text[test_end:]
    source.write_text(text, encoding="utf-8")
    changed.append(source)
    run(("cargo", "fmt", "--all"))
    run(
        (
            "cargo",
            "test",
            "-p",
            "stfixest-core",
            "zero_within_norm_column_still_checks_its_raw_score_moment",
        )
    )
    sha = commit_source("core: stabilize absorbed-intercept score certificate", changed)
    qualify_exact_source(sha)
    return True


def stata_hex_number(value: str) -> float:
    mantissa, exponent = value.split("X", 1)
    if mantissa.lstrip("+-").startswith("0."):
        return 0.0
    if any(character in exponent.lower() for character in "abcdef"):
        raise ValueError(f"special Stata hex value is not finite: {value}")
    return float.fromhex(f"{mantissa}p{int(exponent)}")


def load_reference() -> tuple[dict[str, object], dict[str, dict[str, object]]]:
    document = json.loads(REFERENCE_RECEIPT.read_text(encoding="utf-8"))
    payload = document["payload"]
    if payload["status"] != "success":
        raise SystemExit("sealed FE-interaction reference payload is not successful")
    if payload["source_sha"] != REFERENCE_SOURCE_SHA:
        raise SystemExit("sealed FE-interaction reference source identity changed")
    if document["seal"]["payload_sha256"] != REFERENCE_PAYLOAD_SHA256:
        raise SystemExit("sealed FE-interaction reference payload hash changed")
    cases = {case["id"]: case for case in payload["cases"]}
    return document, cases


def extract_data_block(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    marker = (
        "* This fixture consumes the committed sealed receipt; "
        "it does not invoke reghdfe.\n"
    )
    if text.count(marker) != 1:
        raise SystemExit(f"fixture data-block marker is not unique: {path}")
    start = text.index(marker) + len(marker)
    while start < len(text) and text[start] == "\n":
        start += 1
    call = text.index("_stfixest_estimate_ols_test", start)
    block = text[start:call]
    expected = block.find("__stfx_expected")
    if expected >= 0:
        block = block[: block.rfind("\n", 0, expected) + 1]
    return block.rstrip() + "\n"


def fixture_preamble(template: Path, fixture_base: str) -> str:
    text = template.read_text(encoding="utf-8")
    cut = text.index("\nclear\n") + 1
    prefix = text[:cut]
    prefix = prefix.replace(
        "estimate_ols_categorical_interaction_cascade_parity", fixture_base
    )
    prefix = prefix.replace("tuple_second_fe_cascade", "receipt_driven_cases")
    return prefix.rstrip() + "\n\n"


def stata_string(value: str) -> str:
    return value.replace('"', '""')


def matrix_entries(case: dict[str, object], name: str) -> list[dict[str, object]]:
    return [entry for entry in case["matrices"] if entry["name"] == name]


def matrix_names(entries: Sequence[dict[str, object]], axis: str) -> list[str]:
    key_index = "row" if axis == "row" else "column"
    key_name = "row_name" if axis == "row" else "column_name"
    mapping: dict[int, str] = {}
    for entry in entries:
        mapping[int(entry[key_index])] = str(entry[key_name])
    return [mapping[index] for index in sorted(mapping)]


def render_expected_observations(case: dict[str, object]) -> list[str]:
    sample: dict[int, int] = {}
    residuals: dict[int, str] = {}
    for entry in case["observations"]:
        obs_id = round(stata_hex_number(str(entry["obs_id_hex"])))
        if entry["field"] == "sample":
            sample[obs_id] = round(stata_hex_number(str(entry["value_hex"])))
        elif entry["field"] == "residual":
            residuals[obs_id] = str(entry["value_hex"])
    lines = [
        "generate byte __stfx_expected_sample = 0",
        "generate double __stfx_expected_resid = .",
    ]
    retained = [obs_id for obs_id, kept in sorted(sample.items()) if kept == 1]
    for start in range(0, len(retained), 8):
        chunk = retained[start : start + 8]
        ids = ", ".join(str(value) for value in chunk)
        lines.append(
            f"replace __stfx_expected_sample = 1 if inlist(obs_id, {ids})"
        )
    for obs_id in retained:
        value = residuals.get(obs_id)
        if value is None:
            raise SystemExit(f"receipt case {case['id']} lacks residual for retained {obs_id}")
        lines.append(
            f"replace __stfx_expected_resid = {value} if obs_id == {obs_id}"
        )
    return lines


def render_matrix_assertions(case: dict[str, object], name: str) -> list[str]:
    entries = matrix_entries(case, name)
    dimensions = case["matrix_dimensions"][name]
    stata_name = {"b": "__stfx_b", "V": "__stfx_V", "dof_table": "__stfx_D"}[name]
    lines = [
        f"matrix {stata_name} = e({name})",
        f"assert rowsof({stata_name}) == {dimensions['rows']}",
        f"assert colsof({stata_name}) == {dimensions['columns']}",
    ]
    row_names = " ".join(matrix_names(entries, "row"))
    if name == "dof_table":
        # The sealed probe enumerated `: colfullnames` with Stata's `word()`.
        # That splits the single `Num Coefs` stripe into two words, shifts the
        # later labels, and drops `Nested?`.  The matrix values and dimensions
        # are sound; bind the canonical five-column reghdfe stripe explicitly.
        column_names = "Categories Redundant Num Coefs Inexact? Nested?"
    else:
        column_names = " ".join(matrix_names(entries, "column"))
    lines.extend(
        [
            f"local __stfx_names : rowfullnames {stata_name}",
            f'''assert `"`__stfx_names'"' == `"{stata_string(row_names)}"' ''',
            f"local __stfx_names : colfullnames {stata_name}",
            f'''assert `"`__stfx_names'"' == `"{stata_string(column_names)}"' ''',
        ]
    )
    for entry in entries:
        expected = str(entry["value_hex"])
        row = int(entry["row"])
        column = int(entry["column"])
        if name == "V":
            tolerance = f"5e-6 * max(abs({expected}), 1e-10)"
        else:
            tolerance = f"5e-9 * max(abs({expected}), 1)"
        lines.append(
            f"assert abs({stata_name}[{row},{column}] - ({expected})) <= {tolerance}"
        )
    return lines


def expected_status(column_name: str) -> int:
    if column_name == "_cons":
        return 5
    if column_name.startswith("o."):
        return 3
    return 1


def render_case_assertions(case: dict[str, object], *, cluster: bool) -> list[str]:
    lines: list[str] = []
    for name, expected in sorted(case["scalars"].items()):
        lines.append(f"assert e({name}) == ({expected})")
    for name, expected in sorted(case["macros"].items()):
        if name == "cmdline":
            continue
        lines.extend(
            [
                f'''local __stfx_macro `"`e({name})'"' ''',
                f'''assert `"`__stfx_macro'"' == `"{stata_string(str(expected))}"' ''',
            ]
        )
    lines.extend(render_matrix_assertions(case, "b"))
    lines.extend(render_matrix_assertions(case, "V"))
    lines.extend(render_matrix_assertions(case, "dof_table"))
    b_entries = matrix_entries(case, "b")
    column_names = matrix_names(b_entries, "column")
    lines.extend(
        [
            "matrix __stfx_S = e(stfixest_status)",
            f"assert rowsof(__stfx_S) == {len(column_names)}",
            "assert colsof(__stfx_S) == 4",
        ]
    )
    for position, column_name in enumerate(column_names, start=1):
        lines.append(
            f"assert __stfx_S[{position},2] == {expected_status(column_name)}"
        )
    lines.extend(
        [
            "generate byte __stfx_actual_sample = e(sample)",
            "assert __stfx_actual_sample == __stfx_expected_sample",
            "assert missing(__stfx_reference_resid) if __stfx_expected_sample == 0",
            "assert abs(__stfx_reference_resid - __stfx_expected_resid) <= "
            "5e-9 * max(abs(__stfx_expected_resid), 1) if __stfx_expected_sample == 1",
        ]
    )
    if cluster:
        lines.append("assert e(stfixest_certificate)[1,1] == 1")
    return lines


def manual_missing_data_block() -> str:
    return """clear
set obs 12
generate long obs_id = 200 + _n
generate double y = real(word("3.10 3.90 4.40 5.20 5.80 6.50 7.10 7.90 8.40 9.00 . 9.80", _n))
generate double x = real(word("-1.20 -0.70 -0.20 0.30 0.80 1.30 1.80 2.30 -1.00 -0.50 0.00 0.50", _n))
generate int worker = real(word("1 1 1 1 2 2 2 2 3 3 3 3", _n))
generate int firm = real(word("10 10 20 20 10 10 20 20 . 30 30 30", _n))
generate int year = real(word("2020 2020 2021 2021 2020 2020 2021 2021 2020 . 2022 2022", _n))
"""


def render_fixture(
    *,
    fixture_base: str,
    cases: Sequence[tuple[str, str, str]],
    cluster: bool,
) -> str:
    default_template = ROOT / "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_parity.do"
    cascade_template = ROOT / "tests/stata/parity_reghdfe/estimate_ols_categorical_interaction_cascade_parity.do"
    target_block = extract_data_block(default_template)
    cascade_block = extract_data_block(cascade_template)
    builders = {
        "target": target_block,
        "cascade": cascade_block,
        "missing": manual_missing_data_block(),
    }
    _, reference_cases = load_reference()
    output = [fixture_preamble(cascade_template, fixture_base)]
    for case_id, builder, command in cases:
        case = reference_cases[case_id]
        if case["return_code"] != 0:
            raise SystemExit(f"reference case {case_id} is not successful")
        output.append(f"// Sealed receipt case: {case_id}\n")
        output.append(builders[builder])
        output.append("\n".join(render_expected_observations(case)) + "\n")
        output.append(
            f"_stfixest_estimate_ols_test {command} residual(__stfx_reference_resid)\n"
        )
        output.append("\n".join(render_case_assertions(case, cluster=cluster)) + "\n\n")
    output.append(f'display "END_FIXTURE|{fixture_base}|rc=0"\n')
    return "".join(output)


def render_contract(
    *,
    class_name: str,
    fixture_relative: str,
    case_ids: Sequence[str],
) -> str:
    ids = repr(tuple(case_ids))
    return f'''from __future__ import annotations

import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE_SHA = "{REFERENCE_SOURCE_SHA}"
PAYLOAD_SHA256 = "{REFERENCE_PAYLOAD_SHA256}"
CASE_IDS = {ids}
RECEIPT = ROOT / ".ci/reference-probes/fe-interaction" / f"{{SOURCE_SHA}}.json"
FIXTURE = ROOT / "{fixture_relative}"
PROFILES = ROOT / "ci/stata_profiles.json"
README = ROOT / "tests/stata/parity_reghdfe/README.md"


class {class_name}(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.document = json.loads(RECEIPT.read_text(encoding="utf-8"))
        cls.payload = cls.document["payload"]
        cls.cases = {{case["id"]: case for case in cls.payload["cases"]}}

    def test_cases_are_from_the_accepted_sealed_receipt(self) -> None:
        self.assertEqual(self.payload["status"], "success")
        self.assertEqual(self.payload["source_sha"], SOURCE_SHA)
        self.assertEqual(self.document["seal"]["payload_sha256"], PAYLOAD_SHA256)
        self.assertFalse(self.payload["raw_capture_committed"])
        self.assertEqual(set(CASE_IDS), set(CASE_IDS) & self.cases.keys())
        for case_id in CASE_IDS:
            self.assertEqual(self.cases[case_id]["return_code"], 0)

    def test_fixture_binds_every_oracle_family(self) -> None:
        text = FIXTURE.read_text(encoding="utf-8")
        self.assertIn(SOURCE_SHA, text)
        self.assertIn(PAYLOAD_SHA256, text)
        for case_id in CASE_IDS:
            case = self.cases[case_id]
            self.assertIn(case_id, text)
            for name, value in case["scalars"].items():
                self.assertIn(f"e({{name}})", text)
                self.assertIn(value, text)
            for name, value in case["macros"].items():
                if name != "cmdline":
                    self.assertIn(value, text)
            for entry in case["matrices"]:
                self.assertIn(entry["value_hex"], text)
            for entry in case["observations"]:
                if entry["field"] == "residual" and entry["value_hex"] != "+1.0000000000000X+3ff":
                    self.assertIn(entry["value_hex"], text)
        self.assertNotIn("capture noisily reghdfe", text)

    def test_fixture_is_registered_in_all_qualification_profiles(self) -> None:
        profiles = json.loads(PROFILES.read_text(encoding="utf-8"))["profiles"]
        for name in ("quick", "differential", "full"):
            self.assertIn("{fixture_relative}", profiles[name]["fixtures"])

    def test_readme_keeps_the_gate_fail_closed(self) -> None:
        text = README.read_text(encoding="utf-8")
        for case_id in CASE_IDS:
            self.assertIn(case_id, text)
        self.assertIn("does not invoke `reghdfe`", text)
        self.assertIn("does not close `multi_fe_interactions`", text)


if __name__ == "__main__":
    unittest.main()
'''


def register_fixture(relative: str) -> Path:
    profiles_path = ROOT / "ci/stata_profiles.json"
    document = json.loads(profiles_path.read_text(encoding="utf-8"))
    for name in ("quick", "differential", "full"):
        fixtures = document["profiles"][name]["fixtures"]
        if relative not in fixtures:
            fixtures.append(relative)
    profiles_path.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
    return profiles_path


def update_readme(paragraph: str, token: str) -> Path:
    path = ROOT / "tests/stata/parity_reghdfe/README.md"
    text = path.read_text(encoding="utf-8")
    if token in text:
        return path
    anchor = "\nEach committed fixture consumes the sealed receipt"
    if text.count(anchor) != 1:
        raise SystemExit("parity README insertion anchor is not unique")
    text = text.replace(anchor, "\n" + paragraph.rstrip() + "\n" + anchor)
    path.write_text(text, encoding="utf-8")
    return path


def add_remaining_iid_gate() -> bool:
    fixture_relative = (
        "tests/stata/parity_reghdfe/"
        "estimate_ols_categorical_interaction_remaining_iid_parity.do"
    )
    fixture = ROOT / fixture_relative
    if fixture.exists():
        return False
    cases = (
        (
            "missing_interaction_components",
            "missing",
            "y c.x, absorb(worker firm#year) keepsingletons",
        ),
        (
            "repeated_identical_interaction",
            "target",
            "y c.x1, absorb(worker firm#year firm#year)",
        ),
        (
            "interaction_order_year_firm",
            "target",
            "y c.x1, absorb(worker year#firm)",
        ),
        ("dof_none", "target", "y c.x1, absorb(worker firm#year) dof(none)"),
        (
            "dof_firstpair",
            "target",
            "y c.x1, absorb(worker firm#year) dof(firstpair)",
        ),
    )
    fixture.write_text(
        render_fixture(
            fixture_base="estimate_ols_categorical_interaction_remaining_iid_parity",
            cases=cases,
            cluster=False,
        ),
        encoding="utf-8",
    )
    contract = ROOT / "tests/python/test_fe_interaction_remaining_iid_differential_contract.py"
    contract.write_text(
        render_contract(
            class_name="FeInteractionRemainingIidDifferentialContractTests",
            fixture_relative=fixture_relative,
            case_ids=tuple(case[0] for case in cases),
        ),
        encoding="utf-8",
    )
    profiles = register_fixture(fixture_relative)
    readme = update_readme(
        """`estimate_ols_categorical_interaction_remaining_iid_parity.do` consumes the
`missing_interaction_components`, `repeated_identical_interaction`,
`interaction_order_year_firm`, `dof_none`, and `dof_firstpair` cases. It binds
missing-component sample exclusion, repeated and reordered interaction identity,
and the exact none/first-pair absorbed-DoF policies.""",
        "estimate_ols_categorical_interaction_remaining_iid_parity.do",
    )
    sha = commit_source(
        "test(reference): add remaining iid interaction differentials",
        (fixture, contract, profiles, readme),
    )
    qualify_exact_source(sha)
    return True


def add_cluster_gate() -> bool:
    fixture_relative = (
        "tests/stata/parity_reghdfe/"
        "estimate_ols_categorical_interaction_cluster_parity.do"
    )
    fixture = ROOT / fixture_relative
    if fixture.exists():
        return False
    cases = (
        (
            "nested_simple_cluster",
            "target",
            "y c.x1, absorb(worker firm#year) vce(cluster firm)",
        ),
        (
            "identical_interaction_cluster",
            "target",
            "y c.x1, absorb(worker firm#year) vce(cluster firm#year)",
        ),
        (
            "unrelated_interaction_cluster",
            "cascade",
            "y c.x, absorb(worker firm#year) vce(cluster cluster_u#cluster_v)",
        ),
    )
    fixture.write_text(
        render_fixture(
            fixture_base="estimate_ols_categorical_interaction_cluster_parity",
            cases=cases,
            cluster=True,
        ),
        encoding="utf-8",
    )
    contract = ROOT / "tests/python/test_fe_interaction_cluster_differential_contract.py"
    contract.write_text(
        render_contract(
            class_name="FeInteractionClusterDifferentialContractTests",
            fixture_relative=fixture_relative,
            case_ids=tuple(case[0] for case in cases),
        ),
        encoding="utf-8",
    )
    profiles = register_fixture(fixture_relative)
    readme = update_readme(
        """`estimate_ols_categorical_interaction_cluster_parity.do` consumes the
`nested_simple_cluster`, `identical_interaction_cluster`, and
`unrelated_interaction_cluster` cases. It binds cluster identity, nesting,
cluster-aware absorbed and residual DoF, covariance, sample, and residual
transport for simple and declared-interaction clusters.""",
        "nested_simple_cluster",
    )
    sha = commit_source(
        "test(reference): add clustered interaction differentials",
        (fixture, contract, profiles, readme),
    )
    qualify_exact_source(sha)
    return True


def main() -> None:
    os.chdir(ROOT)
    configure_git()
    sync_main()
    apply_score_repair_if_needed()
    source_sha, receipt = require_current_green()
    print(
        f"CURRENT_GREEN source={source_sha} tests_passed={receipt.get('tests_passed')}",
        flush=True,
    )
    add_remaining_iid_gate()
    require_current_green()
    add_cluster_gate()
    final_sha, final_receipt = require_current_green()
    print(
        "INTERACTION_DIFFERENTIAL_SET_GREEN",
        f"source={final_sha}",
        f"tests_passed={final_receipt.get('tests_passed')}",
        flush=True,
    )


if __name__ == "__main__":
    main()
