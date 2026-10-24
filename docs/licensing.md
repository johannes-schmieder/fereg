# Dependency and native-distribution decision

Johannes approved retaining the pinned within and GPL-3.0-only CMG dependencies
and a GPLv3 distribution for fereg. On 2026-10-13, when asked about the outstanding
Stata/SPI distribution restriction, he confirmed: “The required native-distribution
permissions have been obtained.” This resolves the previously recorded permission
gate and accompanies his instruction to publish fereg under `fe-tools` on GitHub.

The current fereg source and package metadata use GPL-3.0-only. Preserve
third-party licenses and copyright notices: this project's license does not
relicense StataCorp's SPI sources, the MIT numerical dependencies, or the
MIT/Apache-2.0 Rayon sources. Earlier fereg MIT notices remain in
[THIRD_PARTY_NOTICES](../THIRD_PARTY_NOTICES.md).

Native beta assets must include their checksums and dependency audit, plus a
matching corresponding-source archive with the exact Cargo.lock dependency
sources, their licenses, build scripts, and an offline Cargo configuration.
Only publish the platforms actually included and tested. Checks on one platform
do not establish another platform's behavior or general performance.
GitHub testing remains paused. The packaging and installation procedures are in
[release process](release_process.md) and [installation](installation.md).
