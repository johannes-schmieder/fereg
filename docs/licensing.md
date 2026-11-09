# Licensing

fereg is licensed under MIT; see [`LICENSE`](../LICENSE).

On 2026-11-03 Johannes Schmieder confirmed official permission from CMG's
developers, communicated through Ioannis Koutis, to release CMG under MIT.
Johannes selected MIT for CMG, fevc, and fereg.

The MIT grant for the pinned CMG Rust dependency is reproduced in
[`licenses/CMG-MIT.txt`](../licenses/CMG-MIT.txt) and the package's combined
third-party notices. CMG 0.3.0 carries that license and MIT Cargo metadata upstream.
Source packaging preserves the dependency unchanged and verifies its pinned
source inventory in `licenses/CMG-SOURCE.json`.

Preserve third-party licenses and copyright notices. StataCorp's SPI sources,
the MIT numerical dependencies, and MIT/Apache-2.0 Rayon sources retain their
own notices. Johannes confirmed the required Stata/SPI native-distribution
permissions on 2026-10-13.

Native releases include checksums, a dependency audit, matching source with
locked dependencies, license notices, and an offline Cargo configuration.
Report the platforms actually included and tested. GitHub testing remains
paused; see [release process](release_process.md) and
[installation](installation.md).
