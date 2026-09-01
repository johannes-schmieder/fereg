#!/usr/bin/env python3
"""Fail-closed source guard for the retired dense PPML engines.

The old dense PPML fitter, its owned-design IRLS implementation, and the
``FeDesign`` separation runner are useful only as test oracles.  Merely checking
the current plugin call site is too weak: a wrapper, import alias, module rename,
or second production entry point could bypass that check.  This guard instead
checks the Rust module graph and every non-test Rust token in the workspace.

The parser is deliberately small and conservative.  It masks Rust comments and
literals, removes complete items carrying ``#[cfg(test)]``, verifies the frozen
legacy item inventory, and rejects any legacy module or symbol that remains in
the production view.  Rust compilation remains the authoritative syntax/type
check; this file owns only the architectural reachability contract.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
import posixpath
import re
from typing import Mapping, Sequence


CORE = "crates/fereg-core/src"
PPML_MOD = f"{CORE}/ppml/mod.rs"
PPML_IRLS = f"{CORE}/ppml/irls.rs"
PPML_PREPARED_SEPARATION = f"{CORE}/ppml/prepared_separation.rs"
PPML_VCE = f"{CORE}/ppml/vce.rs"
SEPARATION_MOD = f"{CORE}/ppml_separation/mod.rs"
SEPARATION_RUNNER = f"{CORE}/ppml_separation/runner"

CFG_TEST = re.compile(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]")
IDENTIFIER = re.compile(r"\b[A-Za-z_][A-Za-z0-9_]*\b")

# A rename of one of these definitions is itself a contract change.  Keeping
# the exact inventory also prevents a rename from making a symbol scanner blind.
TEST_ONLY_ITEMS: tuple[tuple[str, str, str, bool], ...] = (
    (PPML_MOD, "struct", "PpmlStart", False),
    (PPML_MOD, "struct", "PpmlDiagnostics", False),
    (PPML_MOD, "struct", "PpmlFitStatistics", False),
    (PPML_MOD, "struct", "PpmlResult", False),
    (PPML_MOD, "fn", "fit_ppml", False),
    (PPML_MOD, "fn", "fit_ppml_with_start", False),
    (PPML_MOD, "fn", "final_profile_is_consistent", False),
    (PPML_IRLS, "struct", "ProfileStep", True),
    (PPML_IRLS, "struct", "AcceptedStep", True),
    (PPML_IRLS, "fn", "profile_irls_step", True),
    (PPML_VCE, "struct", "PpmlVceInput", False),
    (PPML_VCE, "fn", "compute_ppml_vce", False),
    (
        PPML_PREPARED_SEPARATION,
        "fn",
        "detect_prepared_ppml_separation",
        False,
    ),
    (f"{SEPARATION_RUNNER}/detect.rs", "fn", "detect_separation", True),
    (
        f"{SEPARATION_RUNNER}/detect.rs",
        "fn",
        "detect_separation_with_multiplicity",
        True,
    ),
)

MODULE_GATES: tuple[tuple[str, str], ...] = (
    (PPML_MOD, "irls"),
    (SEPARATION_MOD, "fe"),
    (SEPARATION_MOD, "ir"),
    (SEPARATION_MOD, "ledger"),
    (SEPARATION_MOD, "mu"),
    (SEPARATION_MOD, "projection"),
    (SEPARATION_MOD, "runner"),
    (SEPARATION_MOD, "subset"),
)

CRATE_PRIVATE_TEST_ONLY_ITEMS = frozenset({(PPML_MOD, "struct", "PpmlStart")})

# Exact token matching avoids false positives for the prepared production
# engine (for example ``prepared_irls`` and ``CertifiedPpmlResult``).
FORBIDDEN_PRODUCTION_IDENTIFIERS = frozenset(
    {
        "irls",
        "runner",
        "PpmlStart",
        "ProfileStep",
        "AcceptedStep",
        "PpmlDiagnostics",
        "PpmlFitStatistics",
        "PpmlResult",
        "fit_ppml",
        "fit_ppml_with_start",
        "final_profile_is_consistent",
        "profile_irls_step",
        "PpmlVceInput",
        "compute_ppml_vce",
        "detect_separation",
        "detect_separation_with_multiplicity",
        "detect_prepared_ppml_separation",
        "fit_ppml_separated",
        "fit_ppml_separated_with_start",
        "SeparatedPpmlResult",
    }
)

GATED_TEST_ONLY_SOURCES = frozenset(
    {
        PPML_IRLS,
        f"{CORE}/ppml_separation/fe.rs",
        f"{CORE}/ppml_separation/ir.rs",
        f"{CORE}/ppml_separation/ledger.rs",
        f"{CORE}/ppml_separation/mu.rs",
        f"{CORE}/ppml_separation/projection.rs",
        f"{CORE}/ppml_separation/subset.rs",
    }
)


@dataclass(frozen=True, order=True)
class Violation:
    path: str
    line: int
    message: str

    def __str__(self) -> str:
        return f"{self.path}:{self.line}: {self.message}"


def _blank(character: str) -> str:
    return "\n" if character == "\n" else " "


def mask_comments_and_literals(source: str) -> str:
    """Return equal-length Rust text with comments and literals blanked."""
    result = list(source)
    index = 0
    length = len(source)
    while index < length:
        if source.startswith("//", index):
            end = source.find("\n", index + 2)
            if end < 0:
                end = length
            for position in range(index, end):
                result[position] = " "
            index = end
            continue
        if source.startswith("/*", index):
            depth = 1
            end = index + 2
            while end < length and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            for position in range(index, min(end, length)):
                result[position] = _blank(source[position])
            index = end
            continue

        raw = re.match(r"(?:br|cr|r)(?P<hashes>#{0,255})\"", source[index:])
        if raw is not None:
            terminator = '"' + raw.group("hashes")
            content = index + raw.end()
            found = source.find(terminator, content)
            end = length if found < 0 else found + len(terminator)
            for position in range(index, end):
                result[position] = _blank(source[position])
            index = end
            continue

        if source[index] == '"':
            end = index + 1
            escaped = False
            while end < length:
                character = source[end]
                end += 1
                if escaped:
                    escaped = False
                elif character == "\\":
                    escaped = True
                elif character == '"':
                    break
            for position in range(index, end):
                result[position] = _blank(source[position])
            index = end
            continue

        # A Rust lifetime (`'a`) is not a character literal.  Mask only the
        # bounded forms that visibly close with a quote.
        if source[index] == "'":
            character_literal = re.match(r"'(?:\\.|[^\\'\n])'", source[index:])
            if character_literal is not None:
                end = index + character_literal.end()
                for position in range(index, end):
                    result[position] = " "
                index = end
                continue
        index += 1
    return "".join(result)


def _matching(source: str, start: int, opening: str, closing: str) -> int | None:
    depth = 0
    for index in range(start, len(source)):
        character = source[index]
        if character == opening:
            depth += 1
        elif character == closing:
            depth -= 1
            if depth == 0:
                return index + 1
    return None


def _skip_space(source: str, index: int) -> int:
    while index < len(source) and source[index].isspace():
        index += 1
    return index


def _test_item_extent(masked: str, attribute_start: int, attribute_end: int) -> tuple[int, int] | None:
    index = _skip_space(masked, attribute_end)
    # Permit derive/allow/etc. between cfg(test) and the item it controls.
    while index < len(masked) and masked[index] == "#":
        bracket = masked.find("[", index + 1)
        if bracket < 0:
            return None
        end = _matching(masked, bracket, "[", "]")
        if end is None:
            return None
        index = _skip_space(masked, end)

    round_depth = 0
    square_depth = 0
    cursor = index
    while cursor < len(masked):
        character = masked[cursor]
        if character == "(":
            round_depth += 1
        elif character == ")":
            round_depth = max(0, round_depth - 1)
        elif character == "[":
            square_depth += 1
        elif character == "]":
            square_depth = max(0, square_depth - 1)
        elif round_depth == 0 and square_depth == 0 and character == ";":
            return attribute_start, cursor + 1
        elif round_depth == 0 and square_depth == 0 and character == "{":
            end = _matching(masked, cursor, "{", "}")
            return None if end is None else (attribute_start, end)
        cursor += 1
    return None


def test_only_spans(
    source: str, *, masked: str | None = None
) -> list[tuple[int, int]]:
    if masked is None:
        masked = mask_comments_and_literals(source)
    spans: list[tuple[int, int]] = []
    for match in CFG_TEST.finditer(masked):
        extent = _test_item_extent(masked, match.start(), match.end())
        if extent is not None:
            spans.append(extent)
    return sorted(spans)


def _inside(position: int, spans: Sequence[tuple[int, int]]) -> bool:
    return any(start <= position < end for start, end in spans)


def production_view(
    source: str,
    *,
    masked: str | None = None,
    spans: Sequence[tuple[int, int]] | None = None,
) -> str:
    if masked is None:
        masked = mask_comments_and_literals(source)
    if spans is None:
        spans = test_only_spans(source, masked=masked)
    output = list(masked)
    for start, end in spans:
        for index in range(start, end):
            output[index] = _blank(output[index])
    return "".join(output)


def _line(source: str, position: int) -> int:
    return source.count("\n", 0, position) + 1


def _definition_pattern(kind: str, name: str) -> re.Pattern[str]:
    visibility = r"(?P<visibility>pub(?:\s*\([^)]*\))?\s+)?"
    qualifiers = r"(?:(?:async|const|unsafe|extern)\s+)*"
    return re.compile(rf"\b{visibility}{qualifiers}{re.escape(kind)}\s+{re.escape(name)}\b")


def _module_pattern(name: str) -> re.Pattern[str]:
    return re.compile(
        rf"\b(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+{re.escape(name)}\s*;"
    )


def _normalized_target(source_path: str, target: str) -> str:
    parent = str(PurePosixPath(source_path).parent)
    return posixpath.normpath(posixpath.join(parent, target))


def _is_legacy_target(source_path: str, target: str) -> bool:
    normalized = _normalized_target(source_path, target)
    legacy_targets = {
        PPML_IRLS,
        *GATED_TEST_ONLY_SOURCES,
        f"{SEPARATION_RUNNER}/mod.rs",
        f"{SEPARATION_RUNNER}/accounting.rs",
        f"{SEPARATION_RUNNER}/detect.rs",
    }
    return normalized in legacy_targets or normalized.startswith(SEPARATION_RUNNER + "/")


def audit_documents(documents: Mapping[str, str]) -> list[Violation]:
    violations: list[Violation] = []
    masked = {
        path: mask_comments_and_literals(source) for path, source in documents.items()
    }
    spans = {
        path: test_only_spans(source, masked=masked[path])
        for path, source in documents.items()
    }

    for path, module in MODULE_GATES:
        source = documents.get(path)
        if source is None:
            violations.append(Violation(path, 1, f"missing legacy module owner for `{module}`"))
            continue
        matches = list(_module_pattern(module).finditer(masked[path]))
        if len(matches) != 1:
            violations.append(
                Violation(path, 1, f"legacy module `{module}` declaration inventory is not exact")
            )
        elif not _inside(matches[0].start(), spans[path]):
            violations.append(
                Violation(
                    path,
                    _line(source, matches[0].start()),
                    f"legacy module `{module}` is not cfg(test)-only",
                )
            )

    for path, kind, name, inherited_test in TEST_ONLY_ITEMS:
        source = documents.get(path)
        if source is None:
            violations.append(Violation(path, 1, f"missing legacy {kind} `{name}`"))
            continue
        matches = list(_definition_pattern(kind, name).finditer(masked[path]))
        if len(matches) != 1:
            violations.append(
                Violation(path, 1, f"legacy {kind} `{name}` inventory is not exact")
            )
        elif not inherited_test and not _inside(matches[0].start(), spans[path]):
            violations.append(
                Violation(
                    path,
                    _line(source, matches[0].start()),
                    f"legacy {kind} `{name}` is not cfg(test)-only",
                )
            )
        if (
            (path, kind, name) in CRATE_PRIVATE_TEST_ONLY_ITEMS
            and matches
            and (matches[0].group("visibility") or "").strip() != "pub(crate)"
        ):
            violations.append(
                Violation(
                    path,
                    _line(source, matches[0].start()),
                    f"legacy {kind} `{name}` is not crate-private",
                )
            )

    gated_paths = set(GATED_TEST_ONLY_SOURCES)
    gated_paths.update(path for path in documents if path.startswith(SEPARATION_RUNNER + "/"))
    for path, source in documents.items():
        if path in gated_paths:
            continue
        view = production_view(source, masked=masked[path], spans=spans[path])
        for match in IDENTIFIER.finditer(view):
            token = match.group(0)
            if token in FORBIDDEN_PRODUCTION_IDENTIFIERS:
                violations.append(
                    Violation(
                        path,
                        _line(source, match.start()),
                        f"legacy PPML token `{token}` is reachable from non-test source",
                    )
                )

        # fereg-core has no production textual Rust includes.  Forbidding them
        # closes concat!/env! path construction, which otherwise could include
        # a gated legacy file under an unrelated module name without leaving a
        # literal path for this source audit to resolve.
        if path.startswith(CORE + "/"):
            for match in re.finditer(r"\binclude\s*!", view):
                violations.append(
                    Violation(
                        path,
                        _line(source, match.start()),
                        "production textual Rust include can bypass cfg(test) module gates",
                    )
                )

        # Reject alternate module names and textual includes of the gated files.
        # These strings are intentionally inspected in the unmasked source; a
        # match is ignored only when the complete containing item is test-only.
        path_attribute = re.compile(
            r"#\s*\[\s*path\s*=\s*\"([^\"]+)\"\s*\]\s*"
            r"(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+[A-Za-z_][A-Za-z0-9_]*"
        )
        include = re.compile(r"\binclude!\s*\(\s*\"([^\"]+)\"")
        for pattern in (path_attribute, include):
            for match in pattern.finditer(source):
                if not _inside(match.start(), spans[path]) and _is_legacy_target(path, match.group(1)):
                    violations.append(
                        Violation(
                            path,
                            _line(source, match.start()),
                            f"legacy PPML source is included through alternate path `{match.group(1)}`",
                        )
                    )

    return sorted(set(violations))


def load_documents(root: Path) -> dict[str, str]:
    documents: dict[str, str] = {}
    for path in sorted((root / "crates").glob("*/src/**/*.rs")):
        documents[path.relative_to(root).as_posix()] = path.read_text(encoding="utf-8")
    return documents


def audit_repository(root: Path) -> list[Violation]:
    return audit_documents(load_documents(root))


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args(argv)
    root = args.root.resolve()
    documents = load_documents(root)
    violations = audit_documents(documents)
    if violations:
        print("PPML legacy boundary: FAIL")
        for violation in violations:
            print(f" - {violation}")
        return 1
    print(
        "PPML legacy boundary: PASS "
        f"files={len(documents)} items={len(TEST_ONLY_ITEMS)} modules={len(MODULE_GATES)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
