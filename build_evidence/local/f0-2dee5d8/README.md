# Local F0 candidate evidence

This directory records the exact-SHA local qualification of the proposed
internal `fereg` baseline:

- source commit: `2dee5d8c4cd99b51bbaca600348f66f325b58ab4`
- source tree: `cc10b50077f162cab8be10702a9627b99ee5eea2`
- deterministic `git archive` SHA-256:
  `41dab4eff5cd2242b2e02877fddcd81fb8f011478c80db8119b7dd34b383ee18`
- original raw augmented full-profile receipt SHA-256:
  `4c18f1b7561aed1ac5e5488c37c4d6a6c1d981990dadbe4bc1fb68eb37d39da5`
- sanitized tracked receipt SHA-256:
  `5813d66caa6f6b1d1a4369e56c9d90a435f6973f2b62fcfc169b3e91ba5a4b71`
- universal macOS plugin SHA-256:
  `633fcd1a0b91617ea5c1aff10f17facf0990bd6ca72893cb81f0c100dfeb2298`

The source was checked out as a detached worktree at the exact commit. The
licensed `full` Stata profile passed all 35 fixtures, including the interactive
Break fixture. `ci/record_rust_quick.sh` then passed the repository, Python,
C/mock-SPI, Rust workspace, and plugin tests, and `ci/augment_ci_receipt.py`
bound both results in `receipt.json`.

The tracked receipt replaces the absolute Stata executable path with the stable
token `platform-default-stata-mp`. No scientific, artifact, runner, fixture, or
return-code field changed. The original raw receipt is kept outside Git; its
hash above preserves provenance without recording a machine-specific path.

The separate receipt-machinery suite also passed 17 tests with the one
platform-specific Linux/SCC case skipped on macOS:
`python3 -m unittest discover -s ci/tests -p 'test_*.py' -v`.

This evidence establishes the local candidate only. The F0 identity must not
be marked sealed until the registered direct-regression, SCC/Linux, and
platform evidence required by `docs/fereg_regression_contract.md` also exists.
