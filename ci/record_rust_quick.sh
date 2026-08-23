#!/bin/bash
set -uo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(git rev-parse --show-toplevel)"
run_dir="$repo_root/.ci/stata/run"
log_file="$run_dir/rust-quick.log"
status_file="$run_dir/rust-quick.status"
mkdir -p "$run_dir"

"$script_dir/run_rust_quick.sh" >"$log_file" 2>&1
rust_rc=$?
/bin/cat "$log_file"

if [[ $rust_rc -eq 0 ]]; then
  rust_status=success
else
  rust_status=failure
fi
printf 'schema_version=1\nrust_status=%s\nrust_rc=%s\ncompleted=1\n' \
  "$rust_status" "$rust_rc" >"$status_file"
exit "$rust_rc"
