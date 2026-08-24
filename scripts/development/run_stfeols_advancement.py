#!/usr/bin/env python3
"""Race-safe entrypoint for the exact-green stfeols advancement driver."""

from __future__ import annotations

import os

import advance_stfeols as advance


def ensure_current_source_is_green() -> tuple[str, dict[str, object]]:
    source_sha = advance.source_commit_from_head()
    path = advance.receipt_for(source_sha)
    if not path.exists():
        advance.qualify_exact_source(source_sha)
    else:
        document = advance.load_receipt(path)
        if not advance.receipt_is_green(document):
            advance.qualify_exact_source(source_sha)
    return advance.require_current_green()


def main() -> None:
    os.chdir(advance.ROOT)
    advance.configure_git()
    advance.sync_main()
    advance.apply_score_repair_if_needed()
    ensure_current_source_is_green()
    advance.add_remaining_iid_gate()
    ensure_current_source_is_green()
    advance.add_cluster_gate()
    source_sha, receipt = ensure_current_source_is_green()
    print(
        "INTERACTION_DIFFERENTIAL_SET_GREEN",
        f"source={source_sha}",
        f"tests_passed={receipt.get('tests_passed')}",
        flush=True,
    )


if __name__ == "__main__":
    main()
