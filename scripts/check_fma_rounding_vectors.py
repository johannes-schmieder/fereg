#!/usr/bin/env python3
"""Derive and verify kernels.rs FMA witnesses using exact rational arithmetic.

No host floating-point operations are used. The double-rounded comparator is
R53(R64(a*b+c)), where R64 rounds the *exact fused sum* to a 64-bit significand
with an extended exponent range; it does not round the product separately.
Run with Python 3.11+: python3 scripts/check_fma_rounding_vectors.py
"""

from fractions import Fraction
from pathlib import Path
import re


def power_of_two(exponent: int) -> Fraction:
    return Fraction(1 << exponent) if exponent >= 0 else Fraction(1, 1 << -exponent)


def decode(bits: int) -> Fraction:
    exponent = (bits >> 52) & 0x7FF
    significand = bits & ((1 << 52) - 1)
    assert exponent != 0x7FF, "witness inputs must be finite"
    if exponent:
        significand += 1 << 52
    value = significand * power_of_two(max(exponent, 1) - 1023 - 52)
    return -value if bits >> 63 else value


def floor_log2(value: Fraction) -> int:
    assert value > 0
    exponent = value.numerator.bit_length() - value.denominator.bit_length()
    return exponent - int(value < power_of_two(exponent))


def nearest_even(value: Fraction) -> int:
    assert value >= 0
    quotient, remainder = divmod(value.numerator, value.denominator)
    twice = 2 * remainder
    return quotient + int(twice > value.denominator or
                          (twice == value.denominator and quotient % 2 == 1))


def rounded(value: Fraction, precision: int, minimum_exponent: int) -> Fraction:
    if not value:
        return value
    magnitude = abs(value)
    quantum = power_of_two(max(floor_log2(magnitude), minimum_exponent) - precision + 1)
    result = nearest_even(magnitude / quantum) * quantum
    return -result if value < 0 else result


def binary64_bits(value: Fraction) -> int:
    value = rounded(value, 53, -1022)
    if not value:
        return 0
    sign = int(value < 0) << 63
    magnitude = abs(value)
    exponent = floor_log2(magnitude)
    assert exponent <= 1023, "these witnesses have finite intermediate results"
    if exponent < -1022:
        significand = magnitude / power_of_two(-1074)
        assert significand.denominator == 1
        return sign | significand.numerator
    significand = magnitude / power_of_two(exponent - 52)
    assert significand.denominator == 1
    return sign | ((exponent + 1023) << 52) | (significand.numerator - (1 << 52))


def main() -> None:
    source = Path(__file__).resolve().parents[1] / "crates/fereg-core/src/kernels.rs"
    text = source.read_text()
    vectors = text.split("const EXACT_FMA_VECTORS:", 1)[1].split("\n    ];", 1)[0]
    vectors = re.sub(r"//[^\n]*", "", vectors)
    words = [int(word.replace("_", ""), 16) for word in re.findall(r"0x[0-9a-fA-F_]+", vectors)]
    assert len(words) == 36, "expected six input/answer sextuples"
    double_rounding_witnesses = 0
    for index in range(0, len(words), 6):
        a_bits, b_bits, c_bits, expected, nonfused, double = words[index:index + 6]
        a, b, c = map(decode, (a_bits, b_bits, c_bits))
        exact = a * b + c
        derived = (
            binary64_bits(exact),
            binary64_bits(rounded(a * b, 53, -1022) + c),
            binary64_bits(rounded(exact, 64, -16382)),
        )
        assert derived == (expected, nonfused, double), (index // 6, derived)
        assert expected != nonfused, "each vector must expose non-fused arithmetic"
        double_rounding_witnesses += expected != double
        print(f"vector {index // 6}: expected={expected:016x} "
              f"nonfused={nonfused:016x} double={double:016x}")
    assert double_rounding_witnesses == 4


if __name__ == "__main__":
    main()
