# macOS licensed-Stata CI contract

Development on trusted `codex/**` branches uses `.github/workflows/stata-ci.yml` on the repository-scoped Apple Silicon Mac runner. A normal push runs the licensed-Stata `quick` profile together with Rust, C, and source checks.

For every source checkpoint:

1. Record the full source commit SHA before any receipt-publisher commit.
2. Read `.ci/stata/results/<full-source-sha>.json` from the same branch.
3. Require `tested_sha` to equal that source SHA exactly.
4. Require `status`, `stata_status`, and `rust_status` all to equal `success`.
5. Treat `.ci/stata/latest.json` only as a convenience pointer because receipt publication may append a later `[skip ci]` commit.
6. On failure, inspect `failure_kind`, `process_rc`, `stata_rc`, `rust_rc`, workflow logs, and the uploaded artifact. The explicit Stata status files and PASS sentinel are authoritative because Stata may return shell status zero after a Stata error.
7. Never merge a deliberate or unresolved failing checkpoint into `main`.

The ChatGPT execution sandbox does not provide Stata or Rust. No local Stata or Rust execution is claimed. Hosted qualification is manual/release-only. The private runner must remain restricted to this private repository and trusted branches; it must not run untrusted forks or pull requests or expose credentials, license material, unrelated files, home-directory contents, or confidential research data. Synthetic fixtures are the default.
