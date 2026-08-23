#!/usr/bin/env python3
"""Wire the verified offline Git bundle into the pinned reference probe."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/fe-interaction-reference-probe.yml"
BUNDLE_COMMIT = "f4760b86e55cf73b82041ce59cf20e8e8224afb0"
BUNDLE_DIR = ROOT / "ci/reference-bundles/stata-require" / BUNDLE_COMMIT


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


manifest = BUNDLE_DIR / "manifest.json"
bundle = BUNDLE_DIR / f"stata-require-{BUNDLE_COMMIT}.bundle"
if not manifest.is_file() or manifest.is_symlink():
    raise SystemExit("verified stata-require bundle manifest is absent or symlinked")
if not bundle.is_file() or bundle.is_symlink():
    raise SystemExit("verified stata-require Git bundle is absent or symlinked")

text = WORKFLOW.read_text(encoding="utf-8")
path_marker = "      - scripts/resolve_reference_root.py\n"
path_insert = (
    "      - scripts/materialize_bundled_reference_root.py\n"
    "      - scripts/resolve_reference_root.py\n"
)
text = replace_once(text, path_marker, path_insert, "bundled resolver workflow trigger")

contract_marker = "      - tests/python/test_reference_root_resolver_contract.py\n"
contract_insert = (
    "      - tests/python/test_reference_root_resolver_contract.py\n"
    "      - tests/python/test_bundled_reference_root_contract.py\n"
    "      - ci/reference-bundles/stata-require/**\n"
)
text = replace_once(text, contract_marker, contract_insert, "bundled resolver contract trigger")

start_marker = "      - name: Resolve or materialize offline pinned reference root\n"
end_marker = "      - name: Run isolated pinned reference probe\n"
start = text.find(start_marker)
end = text.find(end_marker, start + 1)
if start < 0 or end < 0 or end <= start:
    raise SystemExit("reference-root workflow step boundaries are inconsistent")
new_step = '''      - name: Resolve or materialize offline pinned reference root
        id: reference_root
        if: steps.python.outcome == 'success'
        continue-on-error: true
        shell: bash
        env:
          PROBE_PYTHON: ${{ steps.python.outputs.bin }}
        run: |
          set -euo pipefail
          root_file="$RUNNER_TEMP/stfixest-reference-root.path"
          materialize_parent="$RUNNER_TEMP/stfixest-reference-materialized"
          bundled_resolver="scripts/materialize_bundled_reference_root.py"
          fallback_resolver="scripts/resolve_reference_root.py"
          rm -f "$root_file"
          rm -rf "$materialize_parent"
          mkdir -m 700 "$materialize_parent"
          "$PROBE_PYTHON" -m py_compile \
            "$bundled_resolver" \
            "$fallback_resolver" \
            scripts/ols_reference.py

          bundled_outcome="failure"
          if "$PROBE_PYTHON" "$bundled_resolver" \
            --output "$root_file" \
            --materialize-parent "$materialize_parent"; then
            bundled_outcome="success"
          else
            rm -f "$root_file"
            rm -rf "$materialize_parent"
            mkdir -m 700 "$materialize_parent"
            "$PROBE_PYTHON" "$fallback_resolver" \
              --output "$root_file" \
              --materialize-parent "$materialize_parent"
          fi
          chmod 600 "$root_file"
          echo "file=$root_file" >> "$GITHUB_OUTPUT"
          echo "bundled_outcome=$bundled_outcome" >> "$GITHUB_OUTPUT"

'''
text = text[:start] + new_step + text[end:]

env_marker = "          REFERENCE_ROOT_OUTCOME: ${{ steps.reference_root.outcome }}\n"
env_insert = (
    "          REFERENCE_ROOT_OUTCOME: ${{ steps.reference_root.outcome }}\n"
    "          BUNDLED_REFERENCE_OUTCOME: ${{ steps.reference_root.outputs.bundled_outcome }}\n"
)
text = replace_once(text, env_marker, env_insert, "bundled resolver finalizer outcome")

payload_marker = '''                  "reference_root_outcome": os.environ.get("REFERENCE_ROOT_OUTCOME", "unknown"),
                  "probe_outcome": os.environ.get("PROBE_OUTCOME", "unknown"),
'''
payload_insert = '''                  "reference_root_outcome": os.environ.get("REFERENCE_ROOT_OUTCOME", "unknown"),
                  "bundled_reference_outcome": os.environ.get(
                      "BUNDLED_REFERENCE_OUTCOME", "unknown"
                  ),
                  "probe_outcome": os.environ.get("PROBE_OUTCOME", "unknown"),
'''
text = replace_once(text, payload_marker, payload_insert, "bundled resolver failure payload")

WORKFLOW.write_text(text, encoding="utf-8")
print("activated offline bundled-reference probe path")
