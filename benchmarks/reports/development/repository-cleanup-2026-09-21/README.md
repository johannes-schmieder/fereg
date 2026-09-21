# Repository cleanup and publication checkpoint

The unreferenced `cmg_container_memory.rs` draft is preserved byte-for-byte
with its historical CMG report, outside active source. No Rust module refers
to it. Six uncommitted inventory additions only indexed that orphan draft;
they are removed, restoring those manifests exactly to their committed bytes.
Historical inventory versions and scientific receipts remain unchanged.

Six regenerable LaTeX auxiliary/output/log files were removed. Original
machine-specific run requests and the original stopped-campaign evidence ZIP
remain local and unchanged; separate portable exports preserve numerical
contents and original hashes. Private data, installed environments and raw
licensed logs remain ignored.

The pre-cleanup repository checker reports allocation source/inventory drift
against older snapshots after recent optimization work. This is recorded in
`repository-check-before-cleanup.txt`; no historical pin was recaptured to
turn that result green. The post-cleanup checker reports the exact same 102 findings. All 733 staged JSON
and 98 Python files pass parse checks; 51 shell scripts pass syntax checks. Existing repository byte-preservation
attributes also cover Veneto result CSVs and replayable patches. This
checkpoint preserves development work, not release qualification. No estimator
or licensed benchmark is rerun for this archival cleanup.
