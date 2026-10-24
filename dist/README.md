# Native distribution

The repository root `fereg.pkg` installs the current Stata files and the
Linux x86-64, Windows x86-64 and universal macOS plugins in this directory.
Both `github install
johannes-schmieder/fereg` and `net install` from the raw `main` URL use this
manifest; no compiler is needed for installation.

`fereg-native.json` binds the plugin hash to its source commit and a digest of
runtime production tokens and artifact inputs. `scripts/check_github_install.py`
checks that the installed binary still matches the runtime on `main`, all
package files exist, and installation includes the licenses and metadata.
Runtime changes require rebuilding and qualifying the binary before pushing.
Documentation and test-only changes can retain the same native binary when
this identity check passes. Build identity may therefore name an earlier commit
than the current `main` head.

`fereg-SOURCE.txt` links the exact corresponding-source archive, including
pinned dependencies, licenses and the offline build configuration. The plugin
checksum file uses a relative filename so no build-host path is distributed.
See [release maintenance](../docs/release_process.md) for the update procedure.
