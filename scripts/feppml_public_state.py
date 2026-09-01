#!/usr/bin/env python3
"""Classify the exact public ``feppml`` command state.

This module is committed before activation and must not change in the
activation commit.  Repository and Python source contracts use it to preserve
the complete preactivation fail-closed boundary while accepting only the
reviewed one-call public wrapper after activation.
"""
from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Literal


ROOT = Path(__file__).resolve().parents[1]
PREACTIVATION = "preactivation_stub"
ACTIVATED = "activated"
PublicState = Literal["preactivation_stub", "activated"]
FROZEN_PREACTIVATION_ADO_SHA256 = (
    "a82654f05651ce61908d0983a46e11d362667259d42a78498c2d1b98285258e1"
)
FROZEN_PREACTIVATION_ADO = '''*! fereg feppml fail-closed stub 0.5.0 24aug2026
program define feppml, eclass sortpreserve
    version 18.0
    syntax varlist(fv ts numeric min=1) [if] [in] [fw aw pw iw], ///
        [ABSorb(string asis) VCE(string asis) DOF(string asis) ///
         OFFset(varname numeric) EXPosure(varname numeric) ///
         SEParation(string asis) KEEPsingletons ///
         RESIDuals(name) SAVEFEs(string asis) ///
         SOLVer(string) PREConditioner(string) THREADS(integer 0) ///
         MEMory(string) POOLSize(integer 10) TOLerance(real 1e-8) ///
         ITERate(integer 100) DETERMINISTIC TIMEIT VERBose(integer 0)]

    marksample touse, novarlist
    gettoken depvar indepvars : varlist

    if `"`offset'"' != "" & `"`exposure'"' != "" {
        di as error "offset() and exposure() are mutually exclusive"
        exit 198
    }
    if `threads' < 0 | `poolsize' <= 0 | `tolerance' <= 0 | `iterate' <= 0 {
        di as error "threads(), poolsize(), tolerance(), or iterate() is invalid"
        exit 198
    }

    di as error "feppml estimation is not implemented in fereg 0.5.0"
    di as error "PPML IRLS and separation are outside the 0.5.0 OLS beta scope."
    di as error "No partial or uncertified PPML estimate has been posted."
    exit 498
end
'''
if hashlib.sha256(FROZEN_PREACTIVATION_ADO.encode("utf-8")).hexdigest() != (
    FROZEN_PREACTIVATION_ADO_SHA256
):
    raise RuntimeError("embedded preactivation feppml ado bytes changed")


class PublicStateError(ValueError):
    """The public command is neither of the two reviewed states."""


def executable_lines(source: str) -> tuple[str, ...]:
    """Return normalized noncomment Stata source lines."""

    return tuple(
        line.strip()
        for line in source.splitlines()
        if line.strip() and not line.lstrip().startswith(("*", "//"))
    )


def classify_public_ado(source: str) -> PublicState:
    """Require the exact activated wrapper or the complete closed stub."""

    lines = executable_lines(source)
    activated = (
        "program define feppml, eclass sortpreserve",
        "version 18.0",
        "_fereg_ppml `0'",
        "end",
    )
    if lines == activated:
        return ACTIVATED

    if source == FROZEN_PREACTIVATION_ADO:
        return PREACTIVATION
    raise PublicStateError(
        "stata/feppml.ado is neither the complete fail-closed stub nor the "
        "exact activated one-call wrapper"
    )


def public_state(root: Path = ROOT) -> PublicState:
    """Classify the public ado file in ``root``."""

    try:
        source = (root / "stata/feppml.ado").read_text(encoding="utf-8")
    except OSError as error:
        raise PublicStateError(f"cannot read public feppml ado: {error}") from error
    return classify_public_ado(source)
