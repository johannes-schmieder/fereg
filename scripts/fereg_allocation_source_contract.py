#!/usr/bin/env python3
"""Zero-overhead structural allocation guard for the public fereg route.

The measured F0 harness remains authoritative for bytes and RSS.  This guard is
the independent static layer: it freezes allocation-bearing syntax in the OLS
implementation and the exact adapter/one-shot seams through which public
``fereg`` reaches it.  Deliberate changes require reviewing and regenerating
the contract; an unnoticed data-sized allocation fails repository checks.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import re
import sys
import tomllib
from typing import Any, Iterable, Mapping, Sequence


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CONTRACT = ROOT / "benchmarks/manifests/fereg_allocation_source_contract_v11.json"
CONTRACT_ID = "fereg-allocation-source-contract-v11"
SCHEMA_VERSION = 11

# Keep the watched surface narrow enough to describe the actual OLS seam, but
# closed against a new helper file.  The retained changing-weight session is a
# PPML mechanism and is intentionally excluded from allocation-site hashing;
# the exact one-shot boundaries below prove that fereg cannot enter it.
WATCHED_EXACT_FILES = (
    "crates/fereg-core/build.rs",
    "crates/fereg-core/src/error.rs",
    "crates/fereg-core/src/factorize.rs",
    "crates/fereg-core/src/lib.rs",
    "crates/fereg-core/src/memory.rs",
    "crates/fereg-core/src/metadata.rs",
    "crates/fereg-core/src/options.rs",
    "crates/fereg-core/src/parallel.rs",
    "crates/fereg-core/src/protocol.rs",
    "crates/fereg-core/src/sandwich.rs",
    "crates/fereg-plugin/src/estimate.rs",
    "crates/fereg-plugin/src/ffi.rs",
    "crates/fereg-plugin/src/input.rs",
    "crates/fereg-plugin/src/result.rs",
    "crates/fereg-plugin/src/result_view.rs",
)
WATCHED_CARGO_FILES = (
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    "crates/fereg-core/Cargo.toml",
    "crates/fereg-plugin/Cargo.toml",
)
NATIVE_ROUTE_FILES = (
    {"path": "plugin/stplugin.c", "normalization": "c_tokens"},
    {"path": "plugin/stplugin.h", "normalization": "c_tokens"},
    {"path": "plugin/stata_bridge.c", "normalization": "c_tokens"},
    {"path": "plugin/stata_bridge.h", "normalization": "c_tokens"},
    {"path": "plugin/Makefile", "normalization": "exact_text"},
    {"path": "plugin/exports_linux.map", "normalization": "exact_text"},
    {"path": "plugin/exports_macos.txt", "normalization": "exact_text"},
    {"path": "scripts/build_linux.sh", "normalization": "exact_text"},
    {"path": "scripts/build_macos.sh", "normalization": "exact_text"},
    {"path": "scripts/build_windows.ps1", "normalization": "exact_text"},
)
WATCHED_GLOBS = (
    "crates/fereg-core/src/data/**/*.rs",
    "crates/fereg-core/src/fe/**/*.rs",
    "crates/fereg-core/src/linalg/**/*.rs",
    "crates/fereg-core/src/ols/**/*.rs",
    "crates/fereg-core/src/sandwich/**/*.rs",
    "crates/fereg-plugin/src/**/*.rs",
)
EXCLUDED_SOURCE_PREFIXES: tuple[str, ...] = ()
EXCLUDED_SOURCE_FILES: frozenset[str] = frozenset()
BOUNDARY_ONLY_SOURCE_FILES = frozenset(
    {
        "crates/fereg-core/src/ols/linear_session.rs",
        "crates/fereg-plugin/src/ppml_estimate.rs",
        "crates/fereg-plugin/src/ppml_result.rs",
    }
)
FEREG_SHARED_SOURCE_REGIONS = (
    {
        "path": "crates/fereg-core/src/ols/linear_session.rs",
        "name": "phase_allocation_guard",
    },
)

BOUNDARY_FUNCTIONS = (
    ("crates/fereg-plugin/src/estimate.rs", "estimate_ols_test"),
    ("crates/fereg-core/src/ols/prepared_unadjusted.rs", "fit_prepared_ols_unadjusted_map"),
    (
        "crates/fereg-core/src/ols/prepared_unadjusted.rs",
        "fit_prepared_categorical_ols_unadjusted_map",
    ),
    ("crates/fereg-core/src/ols/prepared_robust.rs", "fit_prepared_ols_robust_map"),
    (
        "crates/fereg-core/src/ols/prepared_robust.rs",
        "fit_prepared_categorical_ols_robust_map",
    ),
    ("crates/fereg-core/src/ols/prepared_clustered.rs", "fit_prepared_ols_clustered_map"),
    (
        "crates/fereg-core/src/ols/prepared_clustered.rs",
        "fit_prepared_categorical_ols_clustered_map",
    ),
    ("crates/fereg-core/src/ols/prepared_fit.rs", "fit_prepared_ols_coefficients_map"),
    (
        "crates/fereg-core/src/ols/prepared_fit.rs",
        "fit_prepared_categorical_ols_coefficients_map",
    ),
    (
        "crates/fereg-core/src/ols/prepared_fit.rs",
        "fit_prepared_ols_coefficients_view_map",
    ),
    (
        "crates/fereg-core/src/ols/prepared_fit.rs",
        "fit_prepared_ols_coefficients_view_map_with_frame",
    ),
    (
        "crates/fereg-core/src/ols/prepared_fit.rs",
        "fit_prepared_weighted_linear_once",
    ),
)
ROUTING_BOUNDARY_FUNCTIONS = (
    ("crates/fereg-plugin/src/ffi.rs", "fereg_dispatch"),
    *BOUNDARY_FUNCTIONS,
)

# These ten functions existed at exact F0 and pure extraction is required to
# preserve their production token bodies.  Later one-shot/session seams are
# frozen by the live companion manifest but cannot be projected backward into
# F0 because they did not yet exist there.
F0_SOURCE_SHA = "2dee5d8c4cd99b51bbaca600348f66f325b58ab4"
F0_PREEXISTING_BOUNDARY_HASHES = {
    ("crates/fereg-plugin/src/estimate.rs", "estimate_ols_test"):
        "4b8ff295a425a1e6cadb5422a026b726ed8300e961fc8eca9770ee4f96f5f2cb",
    ("crates/fereg-core/src/ols/prepared_unadjusted.rs", "fit_prepared_ols_unadjusted_map"):
        "99f6c5a29eec027d043f813cae0b1bb6764d6b2eb254ed56f060d04f2d3f0d8b",
    (
        "crates/fereg-core/src/ols/prepared_unadjusted.rs",
        "fit_prepared_categorical_ols_unadjusted_map",
    ): "e551e3b485cb929c0ece4ae876e9616ea50aa583113f7fcd88dd842825ebab95",
    ("crates/fereg-core/src/ols/prepared_robust.rs", "fit_prepared_ols_robust_map"):
        "127fa026f148f4ce79e022b89b17fe0f3a8a292f0d1987ac88275a5d54be2cf2",
    (
        "crates/fereg-core/src/ols/prepared_robust.rs",
        "fit_prepared_categorical_ols_robust_map",
    ): "83eff57f7aaa0fe594085707a4662ad1ace7f8c8c090245eddd2816c3076c2a8",
    ("crates/fereg-core/src/ols/prepared_clustered.rs", "fit_prepared_ols_clustered_map"):
        "06f222d6c7a9aca1605f45fceaf1b7c6895bf7b694e74abe05210de0a443c427",
    (
        "crates/fereg-core/src/ols/prepared_clustered.rs",
        "fit_prepared_categorical_ols_clustered_map",
    ): "7352e1ed804f454de2a3271b36bffad62087c6153c527ec5069e081bed90b71e",
    ("crates/fereg-core/src/ols/prepared_fit.rs", "fit_prepared_ols_coefficients_map"):
        "2bb68a40720f9989f108aee3782755c315acc62a8f89ed6d5a68ea2dbaf64ebd",
    (
        "crates/fereg-core/src/ols/prepared_fit.rs",
        "fit_prepared_categorical_ols_coefficients_map",
    ): "2bb68a40720f9989f108aee3782755c315acc62a8f89ed6d5a68ea2dbaf64ebd",
    (
        "crates/fereg-core/src/ols/prepared_fit.rs",
        "fit_prepared_ols_coefficients_view_map",
    ): "33b448405a8e6ebbcaa342bc9c59d86acde43bf6aa2068902a67d3560e910832",
}
F0_PREEXISTING_BOUNDARY_LITERAL_HASHES = {
    ("crates/fereg-plugin/src/estimate.rs", "estimate_ols_test"):
        (4, "9ef282b93c2c9d59dfb279cc8de461a3114740a9041e9dbde36305e22c351e9e"),
    ("crates/fereg-core/src/ols/prepared_unadjusted.rs", "fit_prepared_ols_unadjusted_map"):
        (0, "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"),
    (
        "crates/fereg-core/src/ols/prepared_unadjusted.rs",
        "fit_prepared_categorical_ols_unadjusted_map",
    ): (0, "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"),
    ("crates/fereg-core/src/ols/prepared_robust.rs", "fit_prepared_ols_robust_map"):
        (0, "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"),
    (
        "crates/fereg-core/src/ols/prepared_robust.rs",
        "fit_prepared_categorical_ols_robust_map",
    ): (0, "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"),
    ("crates/fereg-core/src/ols/prepared_clustered.rs", "fit_prepared_ols_clustered_map"):
        (0, "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"),
    (
        "crates/fereg-core/src/ols/prepared_clustered.rs",
        "fit_prepared_categorical_ols_clustered_map",
    ): (0, "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"),
    ("crates/fereg-core/src/ols/prepared_fit.rs", "fit_prepared_ols_coefficients_map"):
        (0, "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"),
    (
        "crates/fereg-core/src/ols/prepared_fit.rs",
        "fit_prepared_categorical_ols_coefficients_map",
    ): (0, "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"),
    (
        "crates/fereg-core/src/ols/prepared_fit.rs",
        "fit_prepared_ols_coefficients_view_map",
    ): (0, "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"),
}
F0_ROUTE_LITERAL_BINDINGS = {
    ("crates/fereg-plugin/src/estimate.rs", "ACTION_ESTIMATE_OLS"):
        '"estimate-ols"',
    ("crates/fereg-plugin/src/estimate.rs", "CONFIRMATION"):
        '"fereg-estimation"',
}
F0_FFI_OLS_BRANCH_TOKEN_COUNT = 45
F0_FFI_OLS_BRANCH_SHA256 = (
    "24459b4fdba127bd31d99d2edcd3862134308b6649ca7fbef5bcbc2dcf873482"
)
RETAINED_ROUTE_SYMBOLS = frozenset(
    {
        "CertifiedLinearSession",
        "factor_rank_revealing_retained_qr",
        "new_with_fe_design",
        "new_with_ppml_fe_design",
        "solve_fresh",
        "solve_frozen",
        "solve_full_retained_rank",
        "solve_warm",
        "strictify",
    }
)
RETAINED_ROUTE_ALLOWED_NON_PPML = {
    "crates/fereg-core/src/ols/finite_wls_kernel.rs": frozenset(
        {"factor_rank_revealing_retained_qr"}
    ),
    "crates/fereg-core/src/ols/linear_session.rs": RETAINED_ROUTE_SYMBOLS,
    "crates/fereg-core/src/ols/mod.rs": frozenset({"CertifiedLinearSession"}),
}
RETAINED_ROUTE_PPML_CALLER = "crates/fereg-core/src/ppml/prepared_irls.rs"

# These are architectural route violations, not merely allocation-inventory
# changes.  Token matching avoids comments and string literals.
FORBIDDEN_BOUNDARY_TOKENS = frozenset(
    {
        "CertifiedLinearSession",
        "PpmlMemoryDimensions",
        "PpmlMemoryPlan",
        "fit_prepared_ppml",
        "prepare_ppml",
        "solve_fresh",
        "solve_frozen",
        "solve_warm",
        "strictify",
    }
)
FORBIDDEN_ADAPTER_TOKEN_FRAGMENTS = ("linear_session", "ppml")
FORBIDDEN_BOUNDARY_TOKEN_FRAGMENTS = ("ppml", "session")

_MULTI_PUNCTUATION = (
    "::",
    "->",
    "=>",
    "..=",
    "..",
    "&&",
    "||",
    "==",
    "!=",
    "<=",
    ">=",
    "+=",
    "-=",
    "*=",
    "/=",
)
_ALLOCATION_METHODS = frozenset(
    {
        "append",
        "collect",
        "clone",
        "extend",
        "extend_from_slice",
        "insert",
        "into_boxed_slice",
        "push",
        "push_back",
        "reserve",
        "reserve_exact",
        "resize",
        "resize_with",
        "to_owned",
        "to_vec",
        "try_reserve",
        "try_reserve_exact",
    }
)
_ALLOCATION_ASSOCIATED = frozenset(
    {
        "Box::new",
        "HashMap::with_capacity",
        "HashMap::new",
        "HashSet::with_capacity",
        "HashSet::new",
        "String::from",
        "String::new",
        "String::with_capacity",
        "Vec::from_iter",
        "Vec::new",
        "Vec::with_capacity",
        "VecDeque::new",
        "VecDeque::with_capacity",
    }
)
_ALLOCATION_HELPERS = frozenset(
    {
        "push_reserved",
        "reserve_filled",
        "reserved_empty",
        "reserved_filled",
        "try_boxed_slice",
        "try_collect",
    }
)
_RUST_STRING_LITERAL = r'(?:r\#*"[\s\S]*?"\#*|"(?:\\.|[^"\\])*")'
_PATH_ATTRIBUTE = re.compile(
    rf"#\s*\[\s*path\s*=\s*(?P<value>{_RUST_STRING_LITERAL})\s*\]"
)
_INCLUDE_BINDING = re.compile(
    rf"(?P<kind>include|include_str|include_bytes)\s*!\s*\(\s*"
    rf"(?P<value>{_RUST_STRING_LITERAL})\s*\)"
)
_C_ALLOCATION_FUNCTIONS = frozenset(
    {
        "aligned_alloc",
        "alloca",
        "calloc",
        "malloc",
        "realloc",
        "strdup",
        "strndup",
    }
)


class ContractError(ValueError):
    """The source or frozen structural contract is invalid."""


@dataclass(frozen=True)
class Token:
    value: str
    start: int
    end: int


@dataclass(frozen=True)
class FunctionSpan:
    name: str
    start: int
    body_start: int
    body_end: int


def _sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _canonical_json(value: Any) -> str:
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    )


def _lex(source: str) -> list[Token]:
    """Lex enough Rust to ignore comments/strings and preserve structure."""
    output: list[Token] = []
    index = 0
    length = len(source)
    while index < length:
        char = source[index]
        if char.isspace():
            index += 1
            continue
        if source.startswith("//", index):
            newline = source.find("\n", index + 2)
            index = length if newline < 0 else newline + 1
            continue
        if source.startswith("/*", index):
            depth = 1
            cursor = index + 2
            while cursor < length and depth:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            if depth:
                raise ContractError("unterminated Rust block comment")
            index = cursor
            continue
        if char == "r":
            raw = re.match(r'r(#+)?"', source[index:])
            if raw is not None:
                hashes = raw.group(1) or ""
                close = '"' + hashes
                cursor = source.find(close, index + raw.end())
                if cursor < 0:
                    raise ContractError("unterminated Rust raw string")
                end = cursor + len(close)
                output.append(Token("<string>", index, end))
                index = end
                continue
        if char == '"':
            cursor = index + 1
            escaped = False
            while cursor < length:
                current = source[cursor]
                if escaped:
                    escaped = False
                elif current == "\\":
                    escaped = True
                elif current == '"':
                    cursor += 1
                    break
                cursor += 1
            else:
                raise ContractError("unterminated Rust string")
            output.append(Token("<string>", index, cursor))
            index = cursor
            continue
        # Treat lifetimes as identifiers while leaving character literals
        # opaque.  Both forms are irrelevant to allocation syntax.
        if char == "'":
            lifetime = re.match(r"'[A-Za-z_][A-Za-z0-9_]*", source[index:])
            if (
                lifetime is not None
                and (
                    index + lifetime.end() >= length
                    or source[index + lifetime.end()] != "'"
                )
            ):
                end = index + lifetime.end()
                output.append(Token(lifetime.group(0), index, end))
                index = end
                continue
            cursor = index + 1
            escaped = False
            while cursor < length:
                current = source[cursor]
                if escaped:
                    escaped = False
                elif current == "\\":
                    escaped = True
                elif current == "'":
                    cursor += 1
                    break
                cursor += 1
            output.append(Token("<char>", index, cursor))
            index = cursor
            continue
        identifier = re.match(r"[A-Za-z_][A-Za-z0-9_]*", source[index:])
        if identifier is not None:
            end = index + identifier.end()
            output.append(Token(identifier.group(0), index, end))
            index = end
            continue
        number = re.match(r"[0-9][A-Za-z0-9_.]*", source[index:])
        if number is not None:
            end = index + number.end()
            output.append(Token(number.group(0), index, end))
            index = end
            continue
        matched = next(
            (item for item in _MULTI_PUNCTUATION if source.startswith(item, index)),
            None,
        )
        if matched is not None:
            output.append(Token(matched, index, index + len(matched)))
            index += len(matched)
            continue
        output.append(Token(char, index, index + 1))
        index += 1
    return output


def _matching(tokens: Sequence[Token], start: int, opening: str, closing: str) -> int:
    if start >= len(tokens) or tokens[start].value != opening:
        raise ContractError(f"expected {opening!r} while parsing Rust source")
    depth = 1
    for index in range(start + 1, len(tokens)):
        if tokens[index].value == opening:
            depth += 1
        elif tokens[index].value == closing:
            depth -= 1
            if depth == 0:
                return index
    raise ContractError(f"unclosed Rust delimiter {opening!r}")


def _cfg_expression_value(
    values: Sequence[str],
    start: int,
) -> tuple[bool | None, int]:
    """Evaluate one cfg expression with `test=false`; None means unknown."""
    if start >= len(values):
        return None, start
    operator = values[start]
    if (
        operator in {"all", "any", "not"}
        and start + 1 < len(values)
        and values[start + 1] == "("
    ):
        cursor = start + 2
        arguments: list[bool | None] = []
        if cursor < len(values) and values[cursor] == ")":
            cursor += 1
        else:
            while cursor < len(values):
                argument, cursor = _cfg_expression_value(values, cursor)
                arguments.append(argument)
                if cursor >= len(values):
                    return None, cursor
                if values[cursor] == ",":
                    cursor += 1
                    continue
                if values[cursor] == ")":
                    cursor += 1
                    break
                return None, cursor
        if operator == "not":
            if len(arguments) != 1 or arguments[0] is None:
                return None, cursor
            return not arguments[0], cursor
        if operator == "all":
            if any(value is False for value in arguments):
                return False, cursor
            if all(value is True for value in arguments):
                return True, cursor
            return None, cursor
        if any(value is True for value in arguments):
            return True, cursor
        if all(value is False for value in arguments):
            return False, cursor
        return None, cursor
    if operator == "test":
        return False, start + 1

    # Other predicates (features, targets, unix/windows, values) are unknown.
    cursor = start
    depth = 0
    while cursor < len(values):
        value = values[cursor]
        if value == "(":
            depth += 1
        elif value == ")":
            if depth == 0:
                break
            depth -= 1
        elif value == "," and depth == 0:
            break
        cursor += 1
    return None, cursor


def _cfg_is_provably_false_without_test(
    tokens: Sequence[Token],
    start: int,
    end: int,
) -> bool:
    values = [token.value for token in tokens[start : end + 1]]
    if len(values) < 7 or values[:4] != ["#", "[", "cfg", "("]:
        return False
    if values[-2:] != [")", "]"]:
        return False
    expression = values[4:-2]
    value, consumed = _cfg_expression_value(expression, 0)
    return consumed == len(expression) and value is False


def _item_end(tokens: Sequence[Token], start: int) -> int:
    round_depth = 0
    square_depth = 0
    for index in range(start, len(tokens)):
        value = tokens[index].value
        if value == "(":
            round_depth += 1
        elif value == ")":
            round_depth -= 1
        elif value == "[":
            square_depth += 1
        elif value == "]":
            square_depth -= 1
        elif value in {";", ","} and round_depth == square_depth == 0:
            return index
        elif value == "{" and round_depth == square_depth == 0:
            return _matching(tokens, index, "{", "}")
    raise ContractError("unable to find end of cfg(test) Rust item")


def _without_cfg_test_items(tokens: Sequence[Token]) -> list[Token]:
    removed: list[tuple[int, int]] = []
    index = 0
    while index < len(tokens) - 1:
        if tokens[index].value == "#" and tokens[index + 1].value == "[":
            attribute_end = _matching(tokens, index + 1, "[", "]")
            if _cfg_is_provably_false_without_test(tokens, index, attribute_end):
                item_start = attribute_end + 1
                while (
                    item_start < len(tokens) - 1
                    and tokens[item_start].value == "#"
                    and tokens[item_start + 1].value == "["
                ):
                    item_start = _matching(tokens, item_start + 1, "[", "]") + 1
                removal_start = _item_start_with_attributes(tokens, index)
                removed.append((removal_start, _item_end(tokens, item_start)))
                index = removed[-1][1] + 1
                continue
            index = attribute_end + 1
            continue
        index += 1
    if not removed:
        return list(tokens)
    output: list[Token] = []
    range_index = 0
    for token_index, token in enumerate(tokens):
        while range_index < len(removed) and token_index > removed[range_index][1]:
            range_index += 1
        if range_index < len(removed):
            low, high = removed[range_index]
            if low <= token_index <= high:
                continue
        output.append(token)
    return output


def _production_tokens(source: str) -> list[Token]:
    return _without_cfg_test_items(_lex(source))


def _function_spans(tokens: Sequence[Token]) -> list[FunctionSpan]:
    spans: list[FunctionSpan] = []
    for index, token in enumerate(tokens[:-1]):
        if token.value != "fn" or not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", tokens[index + 1].value):
            continue
        name = tokens[index + 1].value
        cursor = index + 2
        while cursor < len(tokens) and tokens[cursor].value not in {"{", ";"}:
            cursor += 1
        if cursor == len(tokens) or tokens[cursor].value == ";":
            continue
        end = _matching(tokens, cursor, "{", "}")
        spans.append(FunctionSpan(name, index, cursor, end))
    return spans


def _canonical_tokens(tokens: Iterable[Token]) -> str:
    return " ".join(token.value for token in tokens)


def _function_body(tokens: Sequence[Token], name: str) -> list[Token]:
    matches = [span for span in _function_spans(tokens) if span.name == name]
    if len(matches) != 1:
        raise ContractError(
            f"expected exactly one production function {name!r}; found {len(matches)}"
        )
    span = matches[0]
    return list(tokens[span.body_start : span.body_end + 1])


def _ffi_ols_dispatch_branch_tokens(source: str) -> list[Token]:
    body = _function_body(_production_tokens(source), "fereg_dispatch")
    values = [token.value for token in body]
    actions = [
        index for index, value in enumerate(values) if value == "ACTION_ESTIMATE_OLS"
    ]
    if len(actions) != 1:
        raise ContractError(
            f"expected one ACTION_ESTIMATE_OLS dispatch discriminant; found {len(actions)}"
        )
    action = actions[0]
    starts = [
        index
        for index in range(action)
        if values[index : index + 2] == ["else", "if"]
    ]
    if not starts:
        raise ContractError("unable to locate F0 OLS dispatch branch start")
    start = starts[-1]
    try:
        opening = next(
            index for index in range(action, len(body)) if values[index] == "{"
        )
    except StopIteration as error:
        raise ContractError("unable to locate F0 OLS dispatch branch body") from error
    end = _matching(body, opening, "{", "}")
    return body[start : end + 1]


def _call_end(tokens: Sequence[Token], start: int) -> int:
    cursor = start
    while cursor < len(tokens) and tokens[cursor].value not in {"(", "[", ";", "{"}:
        cursor += 1
    if cursor >= len(tokens) or tokens[cursor].value in {";", "{"}:
        return start
    opening = tokens[cursor].value
    closing = ")" if opening == "(" else "]"
    return _matching(tokens, cursor, opening, closing)


def _allocation_sites(tokens: Sequence[Token]) -> list[dict[str, str]]:
    spans = _function_spans(tokens)

    def owner(position: int) -> str:
        matches = [span for span in spans if span.body_start < position < span.body_end]
        if not matches:
            return "<module>"
        return min(matches, key=lambda span: span.body_end - span.body_start).name

    sites: list[dict[str, str]] = []
    values = [token.value for token in tokens]
    index = 0
    while index < len(tokens):
        start: int | None = None
        kind: str | None = None
        if values[index : index + 3] == ["vec", "!", "["]:
            start, kind = index, "vec!"
        elif values[index : index + 2] == ["format", "!"]:
            start, kind = index, "format!"
        elif index + 2 < len(tokens) and values[index + 1] == "::":
            # Admit an optional turbofish (`Vec::<f64>::with_capacity`) while
            # retaining the same structural kind as the inferred-type form.
            method_index = index + 2
            if values[method_index] == "<":
                depth = 1
                method_index += 1
                while method_index < len(tokens) and depth:
                    if values[method_index] == "<":
                        depth += 1
                    elif values[method_index] == ">":
                        depth -= 1
                    method_index += 1
                if (
                    depth == 0
                    and method_index < len(tokens) - 1
                    and values[method_index] == "::"
                ):
                    method_index += 1
            associated = (
                f"{values[index]}::{values[method_index]}"
                if method_index < len(tokens)
                else ""
            )
            if associated in _ALLOCATION_ASSOCIATED:
                start, kind = index, associated
        elif values[index] == "." and index + 1 < len(tokens):
            method = values[index + 1]
            if method in _ALLOCATION_METHODS:
                start, kind = index, f".{method}"
        elif values[index] in _ALLOCATION_HELPERS:
            start, kind = index, values[index]
        if start is None or kind is None:
            index += 1
            continue
        end = _call_end(tokens, start)
        call = _canonical_tokens(tokens[start : end + 1])
        classification = _scale_class(call, owner(start))
        sites.append(
            {
                "function": owner(start),
                "kind": kind,
                "scale_hint": classification,
                "call_sha256": _sha256_text(call),
            }
        )
        index = max(index + 1, end + 1)
    return sorted(
        sites,
        key=lambda item: (
            item["function"],
            item["kind"],
            item["scale_hint"],
            item["call_sha256"],
        ),
    )


def _scale_class(call: str, function: str) -> str:
    words = {word.lower() for word in re.findall(r"[A-Za-z_][A-Za-z0-9_]*", call)}
    words.update(part.lower() for part in function.split("_"))
    row_words = {
        "n",
        "n_active",
        "observation",
        "observations",
        "row",
        "rows",
        "sample",
    }
    column_words = {
        "column",
        "columns",
        "k",
        "rank",
        "stride",
        "width",
    }
    if words & row_words and words & column_words:
        return "observation_by_column"
    if words & row_words:
        return "observation"
    if words & {"cluster", "clusters", "intersection", "intersections"}:
        return "cluster"
    if words & {"category", "categories", "group", "groups", "level", "levels"}:
        return "fe_level"
    return "model_or_fixed"


def _module_bindings(source: str) -> list[str]:
    """Freeze source-graph string payloads that ordinary tokenization hides."""
    bindings = [f"path:{match.group('value')}" for match in _PATH_ATTRIBUTE.finditer(source)]
    bindings.extend(
        f"{match.group('kind')}:{match.group('value')}"
        for match in _INCLUDE_BINDING.finditer(source)
    )
    return sorted(bindings)


def _production_literal_payloads(
    source: str,
    tokens: Sequence[Token],
) -> list[str]:
    """Preserve ordered production literals that can select a compiled route.

    The canonical production-token stream deliberately uses ``<string>`` and
    ``<char>`` placeholders so diagnostic wording does not obscure structural
    diffs.  Literal contents are nevertheless executable structure in Rust:
    action constants, ``cfg`` discriminants, and ``#[path]`` attributes all
    select code without changing that placeholder stream.  Bind every
    production literal separately so those route changes cannot pass silently.
    """
    return [
        source[token.start : token.end]
        for token in tokens
        if token.value in {"<string>", "<char>"}
    ]


def _production_string_constant(source: str, name: str) -> str:
    tokens = _production_tokens(source)
    values = [token.value for token in tokens]
    matches: list[str] = []
    for index in range(len(tokens) - 1):
        if values[index : index + 2] != ["const", name]:
            continue
        cursor = index + 2
        while cursor < len(tokens) and values[cursor] != ";":
            cursor += 1
        if cursor == len(tokens):
            raise ContractError(f"unterminated production constant {name}")
        segment = tokens[index : cursor + 1]
        literals = [
            source[token.start : token.end]
            for token in segment
            if token.value == "<string>"
        ]
        if len(literals) != 1:
            raise ContractError(
                f"production constant {name} must contain exactly one string literal"
            )
        matches.append(literals[0])
    if len(matches) != 1:
        raise ContractError(
            f"expected exactly one production string constant {name}; found {len(matches)}"
        )
    return matches[0]


def _unique_token_sequence(tokens: Sequence[Token], pattern: Sequence[str]) -> int:
    values = [token.value for token in tokens]
    matches = [
        index
        for index in range(len(values) - len(pattern) + 1)
        if values[index : index + len(pattern)] == list(pattern)
    ]
    if len(matches) != 1:
        raise ContractError(
            f"expected one production token sequence {list(pattern)!r}; "
            f"found {len(matches)}"
        )
    return matches[0]


def _item_start_with_attributes(tokens: Sequence[Token], item_start: int) -> int:
    """Walk backward over every contiguous Rust attribute on an item."""
    start = item_start
    while start >= 3 and tokens[start - 1].value == "]":
        depth = 1
        cursor = start - 2
        while cursor >= 0 and depth:
            if tokens[cursor].value == "]":
                depth += 1
            elif tokens[cursor].value == "[":
                depth -= 1
            cursor -= 1
        opening = cursor + 1
        if depth or opening == 0 or tokens[opening - 1].value != "#":
            break
        start = opening - 1
    return start


def _linear_session_phase_guard_tokens(tokens: Sequence[Token]) -> list[Token]:
    """Return the exact session fragment that public one-shot OLS executes."""
    enum_start = _unique_token_sequence(
        tokens,
        ("pub", "enum", "LinearSessionAllocationPhase"),
    )
    start = _item_start_with_attributes(tokens, enum_start)

    drop_impls: list[tuple[int, int]] = []
    for index, token in enumerate(tokens):
        if token.value != "impl":
            continue
        cursor = index + 1
        while cursor < len(tokens) and tokens[cursor].value not in {"{", ";"}:
            cursor += 1
        if cursor >= len(tokens) or tokens[cursor].value != "{":
            continue
        header = {token.value for token in tokens[index:cursor]}
        if {"Drop", "LinearSessionPhaseAllocationGuard"} <= header:
            drop_impls.append((index, _matching(tokens, cursor, "{", "}")))
    if len(drop_impls) != 1:
        raise ContractError(
            "expected exactly one production Drop implementation for "
            "LinearSessionPhaseAllocationGuard"
        )
    end = drop_impls[0][1]
    if end < start:
        raise ContractError("linear-session phase guard region is out of order")
    return list(tokens[start : end + 1])


def _fereg_shared_regions(
    relative: str,
    source: str,
    tokens: Sequence[Token],
) -> list[dict[str, Any]]:
    if relative != "crates/fereg-core/src/ols/linear_session.rs":
        return []
    region_tokens = _linear_session_phase_guard_tokens(tokens)
    sites = _allocation_sites(region_tokens)
    class_counts: dict[str, int] = {}
    for site in sites:
        hint = site["scale_hint"]
        class_counts[hint] = class_counts.get(hint, 0) + 1
    literals = _production_literal_payloads(source, region_tokens)
    return [
        {
            "name": "phase_allocation_guard",
            "allocation_site_count": len(sites),
            "allocation_sites_sha256": _sha256_text(_canonical_json(sites)),
            "scale_hint_counts": dict(sorted(class_counts.items())),
            "production_token_count": len(region_tokens),
            "production_tokens_sha256": _sha256_text(
                _canonical_tokens(region_tokens)
            ),
            "production_literal_count": len(literals),
            "production_literals_sha256": _sha256_text(
                _canonical_json(literals)
            ),
        }
    ]


def _watched_paths(root: Path) -> list[str]:
    paths: set[str] = set()
    for relative in WATCHED_EXACT_FILES:
        if (root / relative).is_file():
            paths.add(relative)
    for pattern in WATCHED_GLOBS:
        for path in root.glob(pattern):
            if path.is_file():
                relative = path.relative_to(root).as_posix()
                if relative in EXCLUDED_SOURCE_FILES or relative.startswith(
                    EXCLUDED_SOURCE_PREFIXES
                ):
                    continue
                paths.add(relative)
    return sorted(paths)


def _cargo_wiring(root: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for relative in WATCHED_CARGO_FILES:
        path = root / relative
        try:
            value = tomllib.loads(path.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
            raise ContractError(f"unable to inspect Cargo wiring {relative}: {error}") from error
        records.append(
            {
                "path": relative,
                "semantic_sha256": _sha256_text(_canonical_json(value)),
            }
        )
    return records


def _c_production_tokens(source: str) -> list[str]:
    """Tokenize C/C headers while ignoring only comments and whitespace."""
    output: list[str] = []
    index = 0
    while index < len(source):
        if source[index].isspace():
            index += 1
            continue
        if source.startswith("//", index):
            newline = source.find("\n", index + 2)
            index = len(source) if newline < 0 else newline + 1
            continue
        if source.startswith("/*", index):
            end = source.find("*/", index + 2)
            if end < 0:
                raise ContractError("unterminated C block comment")
            index = end + 2
            continue
        if source[index] in {'"', "'"}:
            quote = source[index]
            cursor = index + 1
            escaped = False
            while cursor < len(source):
                current = source[cursor]
                if escaped:
                    escaped = False
                elif current == "\\":
                    escaped = True
                elif current == quote:
                    cursor += 1
                    break
                cursor += 1
            else:
                raise ContractError("unterminated C string/character literal")
            output.append(source[index:cursor])
            index = cursor
            continue
        identifier = re.match(r"[A-Za-z_][A-Za-z0-9_]*", source[index:])
        if identifier is not None:
            output.append(identifier.group(0))
            index += identifier.end()
            continue
        number = re.match(r"[0-9][A-Za-z0-9_.]*", source[index:])
        if number is not None:
            output.append(number.group(0))
            index += number.end()
            continue
        punctuation = next(
            (
                item
                for item in (
                    "<<=", ">>=", "...", "->", "++", "--", "&&", "||",
                    "==", "!=", "<=", ">=", "+=", "-=", "*=", "/=", "%=",
                    "&=", "|=", "^=", "<<", ">>", "##",
                )
                if source.startswith(item, index)
            ),
            None,
        )
        if punctuation is not None:
            output.append(punctuation)
            index += len(punctuation)
            continue
        output.append(source[index])
        index += 1
    return output


def _native_route_wiring(root: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for spec in NATIVE_ROUTE_FILES:
        relative = spec["path"]
        normalization = spec["normalization"]
        try:
            source = (root / relative).read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            raise ContractError(f"unable to inspect native route {relative}: {error}") from error
        if normalization == "c_tokens":
            tokens = _c_production_tokens(source)
            allocation_sites = [
                token
                for index, token in enumerate(tokens[:-1])
                if token in _C_ALLOCATION_FUNCTIONS and tokens[index + 1] == "("
            ]
            records.append(
                {
                    "path": relative,
                    "normalization": normalization,
                    "production_token_count": len(tokens),
                    "production_tokens_sha256": _sha256_text(" ".join(tokens)),
                    "allocation_site_count": len(allocation_sites),
                    "allocation_sites_sha256": _sha256_text(
                        _canonical_json(allocation_sites)
                    ),
                }
            )
        elif normalization == "exact_text":
            records.append(
                {
                    "path": relative,
                    "normalization": normalization,
                    "exact_text_sha256": _sha256_text(source),
                }
            )
        else:
            raise ContractError(f"unknown native-route normalization {normalization}")
    return records


def build_contract(root: Path = ROOT) -> dict[str, Any]:
    root = root.resolve()
    source_files: list[dict[str, Any]] = []
    for relative in _watched_paths(root):
        path = root / relative
        source = path.read_text(encoding="utf-8")
        tokens = _production_tokens(source)
        module_bindings = _module_bindings(source)
        excluded = relative in BOUNDARY_ONLY_SOURCE_FILES
        shared_regions = _fereg_shared_regions(relative, source, tokens)
        literal_payloads = (
            [] if excluded else _production_literal_payloads(source, tokens)
        )
        sites = [] if excluded else _allocation_sites(tokens)
        class_counts: dict[str, int] = {}
        for site in sites:
            hint = site["scale_hint"]
            class_counts[hint] = class_counts.get(hint, 0) + 1
        source_files.append(
            {
                "path": relative,
                "allocation_site_count": len(sites),
                "allocation_sites_sha256": _sha256_text(_canonical_json(sites)),
                "scale_hint_counts": dict(sorted(class_counts.items())),
                "allocation_inventory": "boundary_only" if excluded else "guarded",
                "production_token_count": 0 if excluded else len(tokens),
                "production_tokens_sha256": _sha256_text("")
                if excluded
                else _sha256_text(_canonical_tokens(tokens)),
                "structure_inventory": "boundary_only" if excluded else "guarded",
                "fereg_shared_regions": shared_regions,
                "production_literal_count": len(literal_payloads),
                "production_literals_sha256": _sha256_text(
                    _canonical_json(literal_payloads)
                ),
                "module_binding_count": len(module_bindings),
                "module_bindings_sha256": _sha256_text(
                    _canonical_json(module_bindings)
                ),
            }
        )

    boundaries: list[dict[str, Any]] = []
    token_cache: dict[str, list[Token]] = {}
    source_cache: dict[str, str] = {}
    for relative, function in ROUTING_BOUNDARY_FUNCTIONS:
        if relative not in token_cache:
            source_cache[relative] = (root / relative).read_text(encoding="utf-8")
            token_cache[relative] = _production_tokens(source_cache[relative])
        tokens = token_cache[relative]
        body = _function_body(tokens, function)
        canonical = _canonical_tokens(body)
        literals = _production_literal_payloads(source_cache[relative], body)
        boundaries.append(
            {
                "path": relative,
                "function": function,
                "token_count": len(body),
                "body_tokens_sha256": _sha256_text(canonical),
                "literal_count": len(literals),
                "literals_sha256": _sha256_text(_canonical_json(literals)),
            }
        )

    return {
        "schema_version": SCHEMA_VERSION,
        "contract_id": CONTRACT_ID,
        "policy": {
            "watched_exact_files": list(WATCHED_EXACT_FILES),
            "watched_globs": list(WATCHED_GLOBS),
            "watched_cargo_files": list(WATCHED_CARGO_FILES),
            "native_route_files": list(NATIVE_ROUTE_FILES),
            "excluded_source_prefixes": list(EXCLUDED_SOURCE_PREFIXES),
            "excluded_source_files": sorted(EXCLUDED_SOURCE_FILES),
            "boundary_only_source_files": sorted(BOUNDARY_ONLY_SOURCE_FILES),
            "fereg_shared_source_regions": list(FEREG_SHARED_SOURCE_REGIONS),
            "boundary_functions": [
                {"path": path, "function": function}
                for path, function in ROUTING_BOUNDARY_FUNCTIONS
            ],
            "f0_source_sha": F0_SOURCE_SHA,
            "f0_preexisting_boundary_hashes": [
                {"path": path, "function": function, "body_tokens_sha256": digest}
                for (path, function), digest in F0_PREEXISTING_BOUNDARY_HASHES.items()
            ],
            "f0_preexisting_boundary_literal_hashes": [
                {
                    "path": path,
                    "function": function,
                    "literal_count": count,
                    "literals_sha256": digest,
                }
                for (path, function), (count, digest) in
                F0_PREEXISTING_BOUNDARY_LITERAL_HASHES.items()
            ],
            "f0_route_literal_bindings": [
                {"path": path, "constant": constant, "literal": literal}
                for (path, constant), literal in F0_ROUTE_LITERAL_BINDINGS.items()
            ],
            "f0_ffi_ols_branch": {
                "path": "crates/fereg-plugin/src/ffi.rs",
                "function": "fereg_dispatch",
                "token_count": F0_FFI_OLS_BRANCH_TOKEN_COUNT,
                "tokens_sha256": F0_FFI_OLS_BRANCH_SHA256,
            },
            "forbidden_boundary_tokens": sorted(FORBIDDEN_BOUNDARY_TOKENS),
            "forbidden_adapter_token_fragments": list(
                FORBIDDEN_ADAPTER_TOKEN_FRAGMENTS
            ),
            "forbidden_boundary_token_fragments": list(
                FORBIDDEN_BOUNDARY_TOKEN_FRAGMENTS
            ),
        },
        "cargo_wiring": _cargo_wiring(root),
        "native_route_wiring": _native_route_wiring(root),
        "source_files": source_files,
        "boundaries": boundaries,
    }


def _load_contract(path: Path) -> Mapping[str, Any]:
    if set(F0_PREEXISTING_BOUNDARY_LITERAL_HASHES) != set(
        F0_PREEXISTING_BOUNDARY_HASHES
    ):
        raise ContractError("F0 boundary literal pins are incomplete")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"unable to read allocation source contract: {error}") from error
    if not isinstance(value, Mapping):
        raise ContractError("allocation source contract must be a JSON object")
    expected_keys = {
        "schema_version",
        "contract_id",
        "policy",
        "cargo_wiring",
        "native_route_wiring",
        "source_files",
        "boundaries",
    }
    if set(value) != expected_keys:
        raise ContractError("allocation source contract fields are not closed")
    if value["schema_version"] != SCHEMA_VERSION or value["contract_id"] != CONTRACT_ID:
        raise ContractError("allocation source contract identity is invalid")
    expected_policy = {
        "watched_exact_files": list(WATCHED_EXACT_FILES),
        "watched_globs": list(WATCHED_GLOBS),
        "watched_cargo_files": list(WATCHED_CARGO_FILES),
        "native_route_files": list(NATIVE_ROUTE_FILES),
        "excluded_source_prefixes": list(EXCLUDED_SOURCE_PREFIXES),
        "excluded_source_files": sorted(EXCLUDED_SOURCE_FILES),
        "boundary_only_source_files": sorted(BOUNDARY_ONLY_SOURCE_FILES),
        "fereg_shared_source_regions": list(FEREG_SHARED_SOURCE_REGIONS),
        "boundary_functions": [
            {"path": relative, "function": function}
            for relative, function in ROUTING_BOUNDARY_FUNCTIONS
        ],
        "f0_source_sha": F0_SOURCE_SHA,
        "f0_preexisting_boundary_hashes": [
            {"path": path, "function": function, "body_tokens_sha256": digest}
            for (path, function), digest in F0_PREEXISTING_BOUNDARY_HASHES.items()
        ],
        "f0_preexisting_boundary_literal_hashes": [
            {
                "path": path,
                "function": function,
                "literal_count": count,
                "literals_sha256": digest,
            }
            for (path, function), (count, digest) in
            F0_PREEXISTING_BOUNDARY_LITERAL_HASHES.items()
        ],
        "f0_route_literal_bindings": [
            {"path": path, "constant": constant, "literal": literal}
            for (path, constant), literal in F0_ROUTE_LITERAL_BINDINGS.items()
        ],
        "f0_ffi_ols_branch": {
            "path": "crates/fereg-plugin/src/ffi.rs",
            "function": "fereg_dispatch",
            "token_count": F0_FFI_OLS_BRANCH_TOKEN_COUNT,
            "tokens_sha256": F0_FFI_OLS_BRANCH_SHA256,
        },
        "forbidden_boundary_tokens": sorted(FORBIDDEN_BOUNDARY_TOKENS),
        "forbidden_adapter_token_fragments": list(
            FORBIDDEN_ADAPTER_TOKEN_FRAGMENTS
        ),
        "forbidden_boundary_token_fragments": list(
            FORBIDDEN_BOUNDARY_TOKEN_FRAGMENTS
        ),
    }
    if value["policy"] != expected_policy:
        raise ContractError("allocation source contract policy was weakened or changed")
    if (
        not isinstance(value["cargo_wiring"], list)
        or not isinstance(value["native_route_wiring"], list)
        or not isinstance(value["source_files"], list)
        or not isinstance(value["boundaries"], list)
    ):
        raise ContractError("allocation source contract inventories must be arrays")
    return value


def _forbidden_violations(root: Path) -> list[str]:
    violations: list[str] = []
    estimate_relative = "crates/fereg-plugin/src/estimate.rs"
    estimate_tokens = _production_tokens(
        (root / estimate_relative).read_text(encoding="utf-8")
    )
    token_cache = {estimate_relative: estimate_tokens}
    estimate_values = {token.value for token in estimate_tokens}
    for token in sorted(FORBIDDEN_BOUNDARY_TOKENS & estimate_values):
        violations.append(f"public OLS adapter exposes forbidden PPML/session token {token}")
    for token in sorted(estimate_values):
        lowered = token.lower()
        if any(fragment in lowered for fragment in FORBIDDEN_ADAPTER_TOKEN_FRAGMENTS):
            violations.append(
                f"public OLS adapter exposes forbidden PPML/session token {token}"
            )
    for relative, function in BOUNDARY_FUNCTIONS:
        if relative not in token_cache:
            token_cache[relative] = _production_tokens(
                (root / relative).read_text(encoding="utf-8")
            )
        tokens = token_cache[relative]
        body_values = {token.value for token in _function_body(tokens, function)}
        for token in sorted(FORBIDDEN_BOUNDARY_TOKENS & body_values):
            violations.append(
                f"fereg boundary {relative}:{function} exposes forbidden token {token}"
            )
        for token in sorted(body_values):
            lowered = token.lower()
            if any(fragment in lowered for fragment in FORBIDDEN_BOUNDARY_TOKEN_FRAGMENTS):
                violations.append(
                    f"fereg boundary {relative}:{function} exposes forbidden token {token}"
                )
    return violations


def _f0_route_literal_violations(root: Path) -> list[str]:
    violations: list[str] = []
    source_cache: dict[str, str] = {}
    for (relative, constant), expected in F0_ROUTE_LITERAL_BINDINGS.items():
        try:
            if relative not in source_cache:
                source_cache[relative] = (root / relative).read_text(encoding="utf-8")
            observed = _production_string_constant(source_cache[relative], constant)
        except (OSError, UnicodeError, ContractError) as error:
            violations.append(
                f"unable to inspect exact F0 route literal {relative}:{constant}: {error}"
            )
            continue
        if observed != expected:
            violations.append(
                f"pre-existing F0 fereg route literal changed: {relative}:{constant}"
            )
    return violations


def _f0_ffi_ols_branch_violations(root: Path) -> list[str]:
    relative = "crates/fereg-plugin/src/ffi.rs"
    try:
        source = (root / relative).read_text(encoding="utf-8")
        branch = _ffi_ols_dispatch_branch_tokens(source)
    except (OSError, UnicodeError, ContractError) as error:
        return [f"unable to inspect exact F0 FFI OLS branch: {error}"]
    if (
        len(branch) != F0_FFI_OLS_BRANCH_TOKEN_COUNT
        or _sha256_text(_canonical_tokens(branch)) != F0_FFI_OLS_BRANCH_SHA256
    ):
        return ["pre-existing F0 fereg FFI OLS dispatch branch changed"]
    return []


def _retained_session_route_violations(root: Path) -> list[str]:
    """Prove the retained QR/session constructor remains PPML-only.

    The implementation and one crate-private re-export are explicit structural
    exceptions. Every actual production constructor caller must live below the
    core PPML subtree. The tiny phase-allocation guard executed by one-shot OLS
    is checked separately and may not mention any retained-session symbol.
    """
    violations: list[str] = []
    rust_files = sorted(
        {
            *root.glob("crates/fereg-core/src/**/*.rs"),
            *root.glob("crates/fereg-plugin/src/**/*.rs"),
        }
    )
    token_cache: dict[str, list[Token]] = {}
    for path in rust_files:
        relative = path.relative_to(root).as_posix()
        try:
            tokens = _production_tokens(path.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, ContractError) as error:
            violations.append(
                f"unable to inspect retained-session route source {relative}: {error}"
            )
            continue
        token_cache[relative] = tokens
        if relative.startswith("crates/fereg-core/src/ppml/"):
            continue
        allowed = RETAINED_ROUTE_ALLOWED_NON_PPML.get(relative, frozenset())
        values = {token.value for token in tokens}
        for symbol in sorted((RETAINED_ROUTE_SYMBOLS & values) - allowed):
            violations.append(
                f"retained QR/session symbol escaped PPML-only route: "
                f"{relative}:{symbol}"
            )

    finite_relative = "crates/fereg-core/src/ols/finite_wls_kernel.rs"
    finite_values = [
        token.value for token in token_cache.get(finite_relative, [])
    ]
    if finite_values.count("factor_rank_revealing_retained_qr") != 1:
        violations.append("retained QR implementation identity changed")

    mod_relative = "crates/fereg-core/src/ols/mod.rs"
    mod_values = [token.value for token in token_cache.get(mod_relative, [])]
    if mod_values.count("CertifiedLinearSession") != 1:
        violations.append("retained session crate-private re-export identity changed")

    session_relative = "crates/fereg-core/src/ols/linear_session.rs"
    session_tokens = token_cache.get(session_relative)
    if session_tokens is None:
        violations.append("retained session implementation is missing")
    else:
        session_values = [token.value for token in session_tokens]
        retained_calls = sum(
            session_values[index : index + 2]
            == ["factor_rank_revealing_retained_qr", "("]
            for index in range(len(session_values) - 1)
        )
        if retained_calls < 1:
            violations.append("retained QR session call is missing")
        shared_values = {
            token.value for token in _linear_session_phase_guard_tokens(session_tokens)
        }
        for symbol in sorted(RETAINED_ROUTE_SYMBOLS & shared_values):
            violations.append(
                f"public OLS phase guard entered retained session route: {symbol}"
            )

    ppml_tokens = token_cache.get(RETAINED_ROUTE_PPML_CALLER)
    if ppml_tokens is None:
        violations.append("PPML retained-session constructor caller is missing")
    else:
        ppml_values = [token.value for token in ppml_tokens]
        constructor_calls = sum(
            ppml_values[index : index + 4]
            == ["CertifiedLinearSession", "::", "new_with_ppml_fe_design", "("]
            for index in range(len(ppml_values) - 3)
        )
        if constructor_calls < 1:
            violations.append("PPML retained-session constructor call is missing")
    return violations


def audit_repository(
    root: Path = ROOT,
    contract_path: Path = DEFAULT_CONTRACT,
) -> list[str]:
    """Return closed structural violations; an empty list is a passing audit."""
    root = root.resolve()
    expected = _load_contract(contract_path)
    violations = (
        _forbidden_violations(root)
        + _f0_route_literal_violations(root)
        + _f0_ffi_ols_branch_violations(root)
        + _retained_session_route_violations(root)
    )
    try:
        observed = build_contract(root)
    except (OSError, UnicodeError, ContractError) as error:
        return violations + [f"unable to inspect fereg allocation surface: {error}"]

    expected_cargo = {
        item.get("path"): item
        for item in expected["cargo_wiring"]
        if isinstance(item, Mapping)
    }
    observed_cargo = {item["path"]: item for item in observed["cargo_wiring"]}
    if len(expected_cargo) != len(expected["cargo_wiring"]):
        violations.append("Cargo wiring inventory is malformed or duplicated")
    if set(expected_cargo) != set(observed_cargo):
        violations.append("Cargo target/dependency wiring file set changed")
    for relative in sorted(set(expected_cargo) & set(observed_cargo)):
        if expected_cargo[relative] != observed_cargo[relative]:
            violations.append(f"Cargo target/dependency wiring changed: {relative}")

    expected_native = {
        item.get("path"): item
        for item in expected["native_route_wiring"]
        if isinstance(item, Mapping)
    }
    observed_native = {
        item["path"]: item for item in observed["native_route_wiring"]
    }
    if len(expected_native) != len(expected["native_route_wiring"]):
        violations.append("native ABI/build route inventory is malformed or duplicated")
    if set(expected_native) != set(observed_native):
        violations.append("native ABI/build route file set changed")
    for relative in sorted(set(expected_native) & set(observed_native)):
        if expected_native[relative] != observed_native[relative]:
            violations.append(f"native ABI/build route changed: {relative}")
            expected_record = expected_native[relative]
            observed_record = observed_native[relative]
            if (
                expected_record.get("allocation_site_count")
                != observed_record.get("allocation_site_count")
                or expected_record.get("allocation_sites_sha256")
                != observed_record.get("allocation_sites_sha256")
            ):
                violations.append(
                    f"native allocation syntax inventory changed: {relative}"
                )

    expected_files = {
        item.get("path"): item for item in expected["source_files"] if isinstance(item, Mapping)
    }
    observed_files = {
        item["path"]: item for item in observed["source_files"]
    }
    if len(expected_files) != len(expected["source_files"]):
        violations.append("allocation source contract file inventory is malformed or duplicated")
    if set(expected_files) != set(observed_files):
        added = sorted(set(observed_files) - set(expected_files))
        removed = sorted(set(expected_files) - set(observed_files))
        violations.append(
            f"watched fereg allocation source set changed; added={added}, removed={removed}"
        )
    for relative in sorted(set(expected_files) & set(observed_files)):
        expected_file = expected_files[relative]
        observed_file = observed_files[relative]
        if (
            expected_file.get("allocation_site_count")
            != observed_file["allocation_site_count"]
            or expected_file.get("allocation_sites_sha256")
            != observed_file["allocation_sites_sha256"]
            or expected_file.get("scale_hint_counts")
            != observed_file["scale_hint_counts"]
            or expected_file.get("allocation_inventory")
            != observed_file["allocation_inventory"]
        ):
            violations.append(f"allocation syntax inventory changed: {relative}")
        if (
            expected_file.get("production_token_count")
            != observed_file["production_token_count"]
            or expected_file.get("production_tokens_sha256")
            != observed_file["production_tokens_sha256"]
            or expected_file.get("structure_inventory")
            != observed_file["structure_inventory"]
        ):
            violations.append(f"guarded production token structure changed: {relative}")
        if (
            expected_file.get("production_literal_count")
            != observed_file["production_literal_count"]
            or expected_file.get("production_literals_sha256")
            != observed_file["production_literals_sha256"]
        ):
            violations.append(f"production literal payload changed: {relative}")
        if expected_file.get("fereg_shared_regions") != observed_file[
            "fereg_shared_regions"
        ]:
            violations.append(f"fereg-shared source region changed: {relative}")
            expected_regions = expected_file.get("fereg_shared_regions")
            if isinstance(expected_regions, list):
                expected_region_allocations = [
                    (
                        item.get("name"),
                        item.get("allocation_site_count"),
                        item.get("allocation_sites_sha256"),
                        item.get("scale_hint_counts"),
                    )
                    for item in expected_regions
                    if isinstance(item, Mapping)
                ]
                observed_region_allocations = [
                    (
                        item["name"],
                        item["allocation_site_count"],
                        item["allocation_sites_sha256"],
                        item["scale_hint_counts"],
                    )
                    for item in observed_file["fereg_shared_regions"]
                ]
                if expected_region_allocations != observed_region_allocations:
                    violations.append(
                        f"fereg-shared allocation syntax inventory changed: {relative}"
                    )
        if set(expected_file) != set(observed_file):
            violations.append(f"allocation source record fields changed: {relative}")
        if (
            expected_file.get("module_binding_count")
            != observed_file["module_binding_count"]
            or expected_file.get("module_bindings_sha256")
            != observed_file["module_bindings_sha256"]
        ):
            violations.append(f"module-path binding changed: {relative}")

    expected_boundaries = {
        (item.get("path"), item.get("function")): item
        for item in expected["boundaries"]
        if isinstance(item, Mapping)
    }
    observed_boundaries = {
        (item["path"], item["function"]): item for item in observed["boundaries"]
    }
    if len(expected_boundaries) != len(expected["boundaries"]):
        violations.append("allocation source contract boundary inventory is malformed or duplicated")
    if expected_boundaries.keys() != observed_boundaries.keys():
        violations.append("fereg allocation boundary inventory changed")
    for key in sorted(expected_boundaries.keys() & observed_boundaries.keys()):
        if expected_boundaries[key] != observed_boundaries[key]:
            violations.append(f"exact fereg adapter/one-shot boundary changed: {key[0]}:{key[1]}")
    for key, f0_digest in F0_PREEXISTING_BOUNDARY_HASHES.items():
        observed_boundary = observed_boundaries.get(key)
        if (
            observed_boundary is None
            or observed_boundary.get("body_tokens_sha256") != f0_digest
        ):
            violations.append(
                f"pre-existing F0 fereg boundary changed: {key[0]}:{key[1]}"
            )
        expected_literal_count, expected_literal_digest = (
            F0_PREEXISTING_BOUNDARY_LITERAL_HASHES[key]
        )
        if (
            observed_boundary is None
            or observed_boundary.get("literal_count") != expected_literal_count
            or observed_boundary.get("literals_sha256") != expected_literal_digest
        ):
            violations.append(
                f"pre-existing F0 fereg boundary literals changed: "
                f"{key[0]}:{key[1]}"
            )
    return sorted(set(violations))


def _render(root: Path) -> int:
    print(json.dumps(build_contract(root), sort_keys=False, indent=2, ensure_ascii=True))
    return 0


def _audit(root: Path, contract: Path) -> int:
    violations = audit_repository(root, contract)
    if violations:
        print("fereg allocation source contract: FAIL", file=sys.stderr)
        for violation in violations:
            print(f" - {violation}", file=sys.stderr)
        return 1
    print("fereg allocation source contract: PASS")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    audit = subparsers.add_parser("audit")
    audit.add_argument("--root", type=Path, default=ROOT)
    audit.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT)
    render = subparsers.add_parser("render")
    render.add_argument("--root", type=Path, default=ROOT)
    arguments = parser.parse_args(argv)
    if arguments.command == "render":
        return _render(arguments.root)
    return _audit(arguments.root, arguments.contract)


if __name__ == "__main__":
    raise SystemExit(main())
