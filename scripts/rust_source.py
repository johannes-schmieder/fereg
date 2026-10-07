"""Rust source token and item boundaries used by the complexity ratchet."""
from __future__ import annotations
from dataclasses import dataclass
import re
from typing import Sequence

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
