#!/usr/bin/env python3
"""Validate the frozen future-public ``feppml`` help contract.

The activation help is executable source as well as documentation.  This
module deliberately parses only the exact marker syntax understood by
``fereg_run`` and fails closed on structural ambiguity.  The public-activation
gate can use :func:`require_activated_public_help` to require byte identity
between the reviewed contract and the installed public help.
"""
from __future__ import annotations

import hashlib
from pathlib import Path
import re
from typing import NamedTuple, Sequence


ROOT = Path(__file__).resolve().parents[1]
CANONICAL_HELP_PATH = (
    ROOT / "tests" / "stata" / "documentation" / "feppml_activation.sthlp"
)
PUBLIC_HELP_PATH = ROOT / "stata" / "feppml.sthlp"
SHARED_FIXTURE_PATH = (
    ROOT / "tests" / "stata" / "documentation" / "feppml_help_examples.do"
)
HIDDEN_WRAPPER_PATH = (
    ROOT
    / "tests"
    / "stata"
    / "documentation"
    / "estimate_ppml_hidden_help_examples.do"
)

# This is intentionally explicit and ordered.  Adding, removing, renaming, or
# reordering a documented command requires a source-contract review here.
EXPECTED_EXAMPLE_IDS = (
    "ppml_basic",
    "ppml_absorbed_outputs",
    "ppml_weights",
    "ppml_clusters",
    "ppml_separation",
    "ppml_time_series",
)

START_MARKER = re.compile(
    r"\{\* example_start - ([a-z][a-z0-9_]*)\}\{\.\.\.\}"
)
END_MARKER = "{* example_end}{...}"
MARKER_LIKE = re.compile(r"\{\*\s*example_(?:start|end)\b")
LITERAL_FEPPML = re.compile(
    r"^[ \t]*feppml(?:[ \t]|$)",
    re.MULTILINE,
)
ASSERT_E = re.compile(
    r"^[ \t]*assert\b[^\r\n]*\be\(([A-Za-z][A-Za-z0-9_]*)\)",
    re.MULTILINE,
)
_SAFE_E_CMD = "`\"`e(cmd)'\"'"
ASSERT_CMD = re.compile(
    r"^[ \t]*assert[ \t]+"
    + re.escape(_SAFE_E_CMD)
    + r"[ \t]*==[ \t]*\"feppml\"[ \t]*$",
    re.MULTILINE,
)
ASSERT_CONVERGED = re.compile(
    r"^[ \t]*assert[ \t]+e\(converged\)[ \t]*==[ \t]*1[ \t]*$",
    re.MULTILINE,
)
PREDICT_MU = re.compile(
    r"^[ \t]*predict[ \t]+double[ \t]+([A-Za-z_][A-Za-z0-9_]*)"
    r"[ \t]*,[ \t]*mu[ \t]*$",
    re.MULTILINE,
)
CLICKABLE_FEREG_RUN_PREFIX = re.compile(r"\{stata[ \t]+fereg_run\b")
CLICKABLE_FEREG_RUN = re.compile(
    r"\{stata[ \t]+fereg_run[ \t]+([a-z][a-z0-9_]*)[ \t]+using[ \t]+"
    r"([^\s:}]+):[^}\r\n]+\}"
)


class HelpContractError(ValueError):
    """The future-public help source is incomplete or ambiguous."""


class HelpExample(NamedTuple):
    """One exact executable block extracted from an SMCL help source."""

    example_id: str
    body: str
    start_line: int
    end_line: int


def _decode_help(value: bytes, where: str) -> str:
    try:
        return value.decode("utf-8")
    except UnicodeDecodeError as error:
        raise HelpContractError(f"{where} is not UTF-8: {error}") from error


def _read_help(path: Path, where: str) -> bytes:
    try:
        return path.read_bytes()
    except OSError as error:
        raise HelpContractError(f"cannot read {where} at {path}: {error}") from error


def extract_example_blocks(text: str) -> tuple[HelpExample, ...]:
    """Extract exact example blocks, rejecting all ambiguous marker structure."""

    lines = text.splitlines(keepends=True)
    records: list[HelpExample] = []
    seen: set[str] = set()
    active_id: str | None = None
    active_start = 0
    active_body_start = 0

    for line_number, source_line in enumerate(lines, start=1):
        marker_line = source_line.rstrip("\r\n")
        start = START_MARKER.fullmatch(marker_line)
        is_end = marker_line == END_MARKER

        if MARKER_LIKE.search(marker_line) and start is None and not is_end:
            raise HelpContractError(
                f"malformed example marker on line {line_number}; markers must be exact"
            )

        if start is not None:
            example_id = start.group(1)
            if active_id is not None:
                raise HelpContractError(
                    f"nested example {example_id!r} on line {line_number} "
                    f"inside {active_id!r}"
                )
            if example_id in seen:
                raise HelpContractError(f"duplicate example id {example_id!r}")
            seen.add(example_id)
            active_id = example_id
            active_start = line_number
            active_body_start = line_number
            continue

        if is_end:
            if active_id is None:
                raise HelpContractError(f"orphan example end marker on line {line_number}")
            body = "".join(lines[active_body_start : line_number - 1])
            if not body.strip():
                raise HelpContractError(f"example {active_id!r} is empty")
            records.append(
                HelpExample(active_id, body, active_start, line_number)
            )
            active_id = None

    if active_id is not None:
        raise HelpContractError(
            f"example {active_id!r} opened on line {active_start} is unclosed"
        )
    return tuple(records)


def _prediction_assertions(body: str, variable: str) -> tuple[bool, bool]:
    escaped = re.escape(variable)
    positive = re.compile(
        rf"^[ \t]*assert[ \t]+{escaped}[ \t]*>[ \t]*0[ \t]+"
        rf"if[ \t]+e\(sample\)[ \t]*$",
        re.MULTILINE,
    )
    missing = re.compile(
        rf"^[ \t]*assert[ \t]+missing\([ \t]*{escaped}[ \t]*\)[ \t]+"
        rf"if[ \t]+(?:![ \t]*e\(sample\)|e\(sample\)[ \t]*==[ \t]*0)[ \t]*$",
        re.MULTILINE,
    )
    return positive.search(body) is not None, missing.search(body) is not None


def _validate_example_semantics(examples: Sequence[HelpExample]) -> None:
    for example in examples:
        body = example.body
        if LITERAL_FEPPML.search(body) is None:
            raise HelpContractError(
                f"example {example.example_id!r} has no literal feppml command"
            )
        if ASSERT_CMD.search(body) is None:
            raise HelpContractError(
                f"example {example.example_id!r} does not safely assert "
                "the string e(cmd)==\"feppml\""
            )
        if ASSERT_CONVERGED.search(body) is None:
            raise HelpContractError(
                f"example {example.example_id!r} does not assert e(converged)==1"
            )
        asserted_e = {
            name for name in ASSERT_E.findall(body)
            if name not in {"cmd", "converged", "sample"}
        }
        if not asserted_e:
            raise HelpContractError(
                f"example {example.example_id!r} has no additional e() assertion"
            )

        predictions = PREDICT_MU.findall(body)
        if len(predictions) != 1:
            raise HelpContractError(
                f"example {example.example_id!r} must contain exactly one "
                "literal 'predict double ..., mu' command"
            )
        positive, missing = _prediction_assertions(body, predictions[0])
        if not positive or not missing:
            raise HelpContractError(
                f"example {example.example_id!r} must assert positive mu in "
                "e(sample) and missing mu outside e(sample)"
            )


def _validate_clickable_links(text: str) -> None:
    prefixes = tuple(CLICKABLE_FEREG_RUN_PREFIX.finditer(text))
    links = tuple(CLICKABLE_FEREG_RUN.finditer(text))
    if len(prefixes) != len(links):
        raise HelpContractError("a clickable fereg_run link is malformed")
    observed = tuple(match.group(1) for match in links)
    if observed != EXPECTED_EXAMPLE_IDS:
        raise HelpContractError(
            "clickable fereg_run example inventory or order changed; "
            f"expected={EXPECTED_EXAMPLE_IDS!r}, observed={observed!r}"
        )
    wrong_targets = [
        match.group(2) for match in links if match.group(2) != "feppml.sthlp"
    ]
    if wrong_targets:
        raise HelpContractError(
            "clickable fereg_run links must target installed feppml.sthlp"
        )


def validate_canonical_help(text: str) -> tuple[HelpExample, ...]:
    """Validate and return the complete future-public example inventory."""

    if not text.startswith("{smcl}"):
        raise HelpContractError("canonical feppml help must begin with {smcl}")
    for line_number, source_line in enumerate(text.splitlines(), start=1):
        encoded_length = len(source_line.encode("utf-8"))
        if encoded_length > 244:
            raise HelpContractError(
                f"canonical feppml help line {line_number} is {encoded_length} "
                "UTF-8 bytes; fereg_run can extract at most 244"
            )
    examples = extract_example_blocks(text)
    observed = tuple(example.example_id for example in examples)
    if observed != EXPECTED_EXAMPLE_IDS:
        raise HelpContractError(
            "canonical feppml example inventory or order changed; "
            f"expected={EXPECTED_EXAMPLE_IDS!r}, observed={observed!r}"
        )
    _validate_example_semantics(examples)
    _validate_clickable_links(text)
    return examples


def validate_canonical_help_file(
    path: Path = CANONICAL_HELP_PATH,
) -> tuple[HelpExample, ...]:
    """Read and validate the canonical activation help source."""

    value = _read_help(path, "canonical feppml help")
    return validate_canonical_help(_decode_help(value, "canonical feppml help"))


def validate_preactivation_public_help(text: str) -> None:
    """Require the currently installed public help to describe a closed stub."""

    if MARKER_LIKE.search(text) or END_MARKER in text:
        raise HelpContractError(
            "preactivation public feppml help must not contain example markers"
        )
    normalized = " ".join(text.split())
    required = (
        "Every invocation returns {cmd:r(498)}",
        "without posting {cmd:e()} results.",
    )
    for statement in required:
        if statement not in normalized:
            raise HelpContractError(
                f"preactivation public feppml help lacks fail-closed statement {statement!r}"
            )


def validate_preactivation_public_help_file(path: Path = PUBLIC_HELP_PATH) -> None:
    """Read and validate the current installed fail-closed help."""

    value = _read_help(path, "public feppml help")
    validate_preactivation_public_help(_decode_help(value, "public feppml help"))


def require_activated_public_help_bytes(
    public_help: bytes, canonical_help: bytes
) -> tuple[HelpExample, ...]:
    """Require activated public help to be the reviewed canonical bytes."""

    examples = validate_canonical_help(
        _decode_help(canonical_help, "canonical feppml help")
    )
    if public_help != canonical_help:
        public_hash = hashlib.sha256(public_help).hexdigest()
        canonical_hash = hashlib.sha256(canonical_help).hexdigest()
        raise HelpContractError(
            "activated public feppml help differs from canonical bytes; "
            f"public_sha256={public_hash}, canonical_sha256={canonical_hash}"
        )
    return examples


def require_activated_public_help(
    public_path: Path = PUBLIC_HELP_PATH,
    canonical_path: Path = CANONICAL_HELP_PATH,
) -> tuple[HelpExample, ...]:
    """File-oriented activation-gate entry point with exact byte comparison."""

    public_help = _read_help(public_path, "activated public feppml help")
    canonical_help = _read_help(canonical_path, "canonical feppml help")
    return require_activated_public_help_bytes(public_help, canonical_help)
