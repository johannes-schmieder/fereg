version 18.0
clear all
set more off

* Private driver for the accepted Windows stata-do controller.  The source
* archive running this file is the trusted controller, not necessarily the
* source being built.  A sealed canonical Git archive below owns the target.
if c(os) != "Windows" {
    exit 9
}

local root = subinstr(`"`c(pwd)'"', char(92), "/", .)
if substr(`"`root'"', strlen(`"`root'"'), 1) == "/" {
    local root = substr(`"`root'"', 1, strlen(`"`root'"') - 1)
}
local controller_run_id : environment WINDOWS_CI_RUN_ID
if !regexm(`"`controller_run_id'"', ///
        "^win-[0-9]{8}T[0-9]{6}Z-[0-9a-f]{8}$") {
    exit 610
}
local status_file `"`root'/windows-ci.status"'
local controller_source_manifest `"`root'/windows-ci-source.json"'
local target_preparation `"`root'/input/feppml-windows-preparation.json"'
local target_archive `"`root'/input/feppml-windows-target-source.tar"'
local target_materializer `"`root'/scripts/prepare_windows_feppml_target.py"'
local target_root `"`root'/target/windows-ci-target-source"'
local target_receipt `"`root'/target/windows-ci-target.receipt"'
local build_program `"`root'/scripts/build_windows.ps1"'
local checksum_verifier `"`root'/scripts/verify_windows_plugin.ps1"'
local platform_capture_helper ///
    `"`root'/scripts/prepare_windows_feppml_platform_capture.ps1"'
local platform_manifest_sealer ///
    `"`root'/scripts/seal_windows_feppml_platform_manifest.py"'
local results_bundler `"`root'/scripts/bundle_windows_feppml_results.py"'
local platform_capture_program ///
    `"`root'/tests/stata/installation/feppml_platform_load_capture.do"'
local built_plugin `"`target_root'/dist/fereg_windows.plugin"'
local built_checksum `"`target_root'/dist/fereg_windows.plugin.sha256"'
local platform_capture `"`root'/feppml-platform-load-capture.json"'
local platform_source_manifest ///
    `"`root'/feppml-windows-platform-source-manifest.json"'
local results_archive `"`root'/feppml-windows-results.zip"'
local private_stage `"`root'/target/windows-ci-private-package"'
local empty_sysdir `"`root'/target/windows-ci-empty-sysdir"'
local built_verification `"`root'/target/windows-ci-built-plugin.verified"'
local staged_verification `"`root'/target/windows-ci-staged-plugin.verified"'
local platform_capture_inputs ///
    `"`root'/target/windows-ci-platform-capture.inputs"'
local platform_capture_verification ///
    `"`root'/target/windows-ci-platform-capture.verified"'

capture erase `"`status_file'"'
capture erase `"`platform_capture'"'
capture erase `"`platform_source_manifest'"'
capture erase `"`results_archive'"'
capture confirm file `"`status_file'"'
if !_rc {
    exit 602
}

foreach required_file in windows-ci-source.json ///
    input/feppml-windows-preparation.json ///
    input/feppml-windows-target-source.tar ///
    scripts/build_windows.ps1 scripts/verify_windows_plugin.ps1 ///
    scripts/prepare_windows_feppml_target.py ///
    scripts/prepare_windows_feppml_platform_capture.ps1 ///
    scripts/seal_windows_feppml_platform_manifest.py ///
    scripts/bundle_windows_feppml_results.py ///
    tests/stata/installation/windows_ci_smoke.do ///
    tests/stata/installation/feppml_platform_load_capture.do ///
    tests/stata/parity_ppmlhdfe/estimate_ppml_hidden_transaction.do ///
    tests/stata/parity_ppmlhdfe/feppml_fail_closed.do {
    confirm file `"`root'/`required_file'"'
}

* Materialization is create-only and verifies the sealed preparation, every
* controller contract file, the exact target archive hash/size, and the Git
* tar's embedded full commit before publishing this line receipt.
capture quietly shell python.exe `"`target_materializer'"' ///
    --preparation `"`target_preparation'"' ///
    --archive `"`target_archive'"' --output `"`target_root'"' ///
    --receipt `"`target_receipt'"'
local materialize_rc = _rc
if `materialize_rc' {
    exit `materialize_rc'
}
confirm file `"`target_receipt'"'

tempname target_handle
file open `target_handle' using `"`target_receipt'"', read text
file read `target_handle' target_mode_line
file read `target_handle' target_commit_line
file read `target_handle' target_tree_line
file read `target_handle' target_archive_hash_line
file read `target_handle' target_archive_size_line
file read `target_handle' target_embedded_line
file read `target_handle' target_build_date_line
file read `target_handle' harness_commit_line
file read `target_handle' harness_tree_line
file read `target_handle' target_sentinel
file read `target_handle' target_trailer
local target_receipt_eof = r(eof)
file close `target_handle'

local target_mode = substr(`"`target_mode_line'"', strlen("mode=") + 1, .)
local target_commit = substr(`"`target_commit_line'"', ///
    strlen("target_commit=") + 1, .)
local target_tree = substr(`"`target_tree_line'"', ///
    strlen("target_tree=") + 1, .)
local target_archive_hash = substr(`"`target_archive_hash_line'"', ///
    strlen("target_archive_sha256=") + 1, .)
local target_archive_size = substr(`"`target_archive_size_line'"', ///
    strlen("target_archive_size_bytes=") + 1, .)
local target_embedded = substr(`"`target_embedded_line'"', ///
    strlen("embedded_git_commit=") + 1, .)
local target_build_date = substr(`"`target_build_date_line'"', ///
    strlen("build_date_utc=") + 1, .)
local harness_commit = substr(`"`harness_commit_line'"', ///
    strlen("harness_commit=") + 1, .)
local harness_tree = substr(`"`harness_tree_line'"', ///
    strlen("harness_tree=") + 1, .)
if substr(`"`target_mode_line'"', 1, strlen("mode=")) != "mode=" | ///
   !inlist(`"`target_mode'"', "candidate", "f0-baseline") | ///
   substr(`"`target_commit_line'"', 1, strlen("target_commit=")) != ///
        "target_commit=" | !regexm(`"`target_commit'"', "^[0-9a-f]{40}$") | ///
   substr(`"`target_tree_line'"', 1, strlen("target_tree=")) != ///
        "target_tree=" | !regexm(`"`target_tree'"', "^[0-9a-f]{40}$") | ///
   substr(`"`target_archive_hash_line'"', 1, ///
        strlen("target_archive_sha256=")) != "target_archive_sha256=" | ///
   !regexm(`"`target_archive_hash'"', "^[0-9a-f]{64}$") | ///
   substr(`"`target_archive_size_line'"', 1, ///
        strlen("target_archive_size_bytes=")) != ///
        "target_archive_size_bytes=" | ///
   !regexm(`"`target_archive_size'"', "^[1-9][0-9]*$") | ///
   substr(`"`target_embedded_line'"', 1, ///
        strlen("embedded_git_commit=")) != "embedded_git_commit=" | ///
   `"`target_embedded'"' != substr(`"`target_commit'"', 1, 12) | ///
   substr(`"`target_build_date_line'"', 1, ///
        strlen("build_date_utc=")) != "build_date_utc=" | ///
   !regexm(`"`target_build_date'"', "^[0-9]{4}-[0-9]{2}-[0-9]{2}$") | ///
   substr(`"`harness_commit_line'"', 1, strlen("harness_commit=")) != ///
        "harness_commit=" | !regexm(`"`harness_commit'"', "^[0-9a-f]{40}$") | ///
   substr(`"`harness_tree_line'"', 1, strlen("harness_tree=")) != ///
        "harness_tree=" | !regexm(`"`harness_tree'"', "^[0-9a-f]{40}$") | ///
   (`"`target_mode'"' == "f0-baseline" & ///
        `"`target_commit'"' == `"`harness_commit'"') | ///
   `"`target_sentinel'"' != "FEPPML_WINDOWS_TARGET_MATERIALIZED" | ///
   `"`target_trailer'"' != "" | !`target_receipt_eof' {
    exit 610
}

foreach required_target in Cargo.toml Cargo.lock plugin/stplugin.c ///
    plugin/stata_bridge.c stata/fereg.pkg stata/stata.toc {
    confirm file `"`target_root'/`required_target'"'
}

* A stale artifact or receipt must never make a failed build look successful.
foreach stale_file in `"`built_plugin'"' `"`built_checksum'"' ///
    `"`built_verification'"' `"`staged_verification'"' ///
    `"`platform_capture_inputs'"' `"`platform_capture_verification'"' ///
    `"`platform_capture'"' `"`platform_source_manifest'"' ///
    `"`results_archive'"' {
    capture erase `"`stale_file'"'
    capture confirm file `"`stale_file'"'
    if !_rc {
        exit 602
    }
}
capture quietly shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File `"`build_program'"' ///
    -SourceRoot `"`target_root'"' ///
    -ExpectedSourceCommit `"`target_commit'"' ///
    -ExpectedBuildDate `"`target_build_date'"' ///
    -ExpectedMode `"`target_mode'"'
local build_rc = _rc
if `build_rc' {
    exit `build_rc'
}
confirm file `"`built_plugin'"'
confirm file `"`built_checksum'"'

tempname plugin_handle checksum_handle
file open `plugin_handle' using `"`built_plugin'"', read binary
file seek `plugin_handle' eof
local plugin_bytes = r(loc)
file close `plugin_handle'
if `plugin_bytes' <= 0 {
    exit 610
}
file open `checksum_handle' using `"`built_checksum'"', read text
file read `checksum_handle' checksum_line
file read `checksum_handle' checksum_trailer
local checksum_trailer_eof = r(eof)
file close `checksum_handle'
local checksum_line = strtrim(`"`checksum_line'"')
local checksum_words : word count `checksum_line'
local checksum_digest : word 1 of `checksum_line'
local checksum_name : word 2 of `checksum_line'
if `checksum_words' != 2 | strlen(`"`checksum_digest'"') != 64 | ///
   !regexm(`"`checksum_digest'"', "^[0-9a-f]+$") | ///
   `"`checksum_name'"' != "fereg_windows.plugin" | ///
   `"`checksum_trailer'"' != "" | !`checksum_trailer_eof' {
    exit 610
}

capture quietly shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File `"`checksum_verifier'"' ///
    -Plugin `"`built_plugin'"' -Checksum `"`built_checksum'"' ///
    -Receipt `"`built_verification'"'
confirm file `"`built_verification'"'
tempname verification_handle
file open `verification_handle' using `"`built_verification'"', read text
file read `verification_handle' built_verification_line
file read `verification_handle' built_verification_trailer
local built_verification_eof = r(eof)
file close `verification_handle'
if `"`built_verification_line'"' != ///
       `"WINDOWS_PLUGIN_SHA256=`checksum_digest'"' | ///
   `"`built_verification_trailer'"' != "" | !`built_verification_eof' {
    exit 610
}

* Stage the package from the target, never from the controller.  Only candidate
* mode receives controller-owned private PPML adapters; F0 remains its exact
* public historical package and runs only the load/OLS/fail-closed contract.
capture mkdir `"`private_stage'"'
if _rc {
    exit 693
}
capture mkdir `"`empty_sysdir'"'
if _rc {
    exit 693
}
tempname package_handle
local package_members
file open `package_handle' using `"`target_root'/stata/fereg.pkg"', read text
file read `package_handle' package_line
while !r(eof) {
    local package_line = strtrim(`"`package_line'"')
    if substr(`"`package_line'"', 1, 2) == "f " {
        local member = strtrim(substr(`"`package_line'"', 3, .))
        local member_words : word count `member'
        local duplicate : list member in package_members
        if `"`member'"' == "" | `member_words' != 1 | `duplicate' | ///
           strpos(`"`member'"', "/") | strpos(`"`member'"', char(92)) | ///
           strpos(`"`member'"', "..") | strpos(`"`member'"', ":") {
            file close `package_handle'
            exit 610
        }
        local package_members `package_members' `member'
        if !strmatch(`"`member'"', "*.plugin") {
            confirm file `"`target_root'/stata/`member'"'
            copy `"`target_root'/stata/`member'"' `"`private_stage'/`member'"'
        }
    }
    file read `package_handle' package_line
}
file close `package_handle'
foreach required_member in fereg.ado _fereg_load.ado feppml.ado ///
    fereg_parse.ado fereg_windows.plugin {
    local present : list required_member in package_members
    if !`present' {
        exit 610
    }
}
copy `"`target_root'/stata/fereg.pkg"' `"`private_stage'/fereg.pkg"'
copy `"`target_root'/stata/stata.toc"' `"`private_stage'/stata.toc"'
copy `"`built_plugin'"' `"`private_stage'/fereg_windows.plugin"'
copy `"`built_checksum'"' `"`private_stage'/fereg_windows.plugin.sha256"'

local private_adapters _fereg_ppml.ado _fereg_estimate_ppml.ado feppml_p.ado
if `"`target_mode'"' == "candidate" {
    foreach private_file of local private_adapters {
        local publicly_shipped : list private_file in package_members
        if `publicly_shipped' {
            exit 610
        }
        copy `"`root'/stata/`private_file'"' `"`private_stage'/`private_file'"'
    }
}

file open `plugin_handle' using `"`private_stage'/fereg_windows.plugin"', read binary
file seek `plugin_handle' eof
local staged_plugin_bytes = r(loc)
file close `plugin_handle'
if `staged_plugin_bytes' != `plugin_bytes' | `staged_plugin_bytes' <= 0 {
    exit 610
}
capture quietly shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File `"`checksum_verifier'"' ///
    -Plugin `"`private_stage'/fereg_windows.plugin"' ///
    -Checksum `"`built_checksum'"' -Receipt `"`staged_verification'"'
confirm file `"`staged_verification'"'
file open `verification_handle' using `"`staged_verification'"', read text
file read `verification_handle' staged_verification_line
file read `verification_handle' staged_verification_trailer
local staged_verification_eof = r(eof)
file close `verification_handle'
if `"`staged_verification_line'"' != ///
       `"WINDOWS_PLUGIN_SHA256=`checksum_digest'"' | ///
   `"`staged_verification_trailer'"' != "" | !`staged_verification_eof' {
    exit 610
}

sysdir set PERSONAL `"`empty_sysdir'"'
sysdir set PLUS `"`empty_sysdir'"'
sysdir set SITE `"`empty_sysdir'"'
sysdir set OLDPLACE `"`empty_sysdir'"'
adopath ++ `"`private_stage'"'
cd `"`empty_sysdir'"'
discard
local staged_resolution_members fereg.pkg stata.toc
foreach package_member of local package_members {
    if !strmatch(`"`package_member'"', "*.plugin") {
        local staged_resolution_members ///
            `staged_resolution_members' `package_member'
    }
}
local staged_resolution_members `staged_resolution_members' fereg_windows.plugin
if `"`target_mode'"' == "candidate" {
    local staged_resolution_members ///
        `staged_resolution_members' `private_adapters'
}
foreach staged_file of local staged_resolution_members {
    findfile `staged_file'
    local resolved = lower(subinstr(`"`r(fn)'"', char(92), "/", .))
    local expected = lower(`"`private_stage'/`staged_file'"')
    if `"`resolved'"' != `"`expected'"' {
        exit 610
    }
}
if `"`target_mode'"' == "f0-baseline" {
    foreach private_file of local private_adapters {
        capture which `private_file'
        if !_rc {
            exit 610
        }
    }
}

* Bind the licensed load to the controller manifest, sealed target receipt,
* exact staged artifact, and exact controller-owned capture program.
capture quietly shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File `"`platform_capture_helper'"' ///
    -Plugin `"`private_stage'/fereg_windows.plugin"' ///
    -Checksum `"`built_checksum'"' ///
    -CaptureProgram `"`platform_capture_program'"' ///
    -SourceManifest `"`controller_source_manifest'"' ///
    -TargetReceipt `"`target_receipt'"' ///
    -Receipt `"`platform_capture_inputs'"'
confirm file `"`platform_capture_inputs'"'
tempname capture_inputs_handle
file open `capture_inputs_handle' using `"`platform_capture_inputs'"', read text
file read `capture_inputs_handle' controller_commit_line
file read `capture_inputs_handle' capture_mode_line
file read `capture_inputs_handle' source_commit_line
file read `capture_inputs_handle' source_tree_line
file read `capture_inputs_handle' source_archive_hash_line
file read `capture_inputs_handle' source_archive_size_line
file read `capture_inputs_handle' embedded_commit_line
file read `capture_inputs_handle' capture_build_date_line
file read `capture_inputs_handle' capture_harness_commit_line
file read `capture_inputs_handle' capture_harness_tree_line
file read `capture_inputs_handle' capture_artifact_hash_line
file read `capture_inputs_handle' capture_artifact_size_line
file read `capture_inputs_handle' capture_program_hash_line
file read `capture_inputs_handle' capture_inputs_sentinel
file read `capture_inputs_handle' capture_inputs_trailer
local capture_inputs_eof = r(eof)
file close `capture_inputs_handle'
local controller_commit = substr(`"`controller_commit_line'"', ///
    strlen("controller_commit=") + 1, .)
local capture_mode = substr(`"`capture_mode_line'"', strlen("mode=") + 1, .)
local source_commit = substr(`"`source_commit_line'"', ///
    strlen("source_commit=") + 1, .)
local source_tree = substr(`"`source_tree_line'"', ///
    strlen("source_tree=") + 1, .)
local source_archive_hash = substr(`"`source_archive_hash_line'"', ///
    strlen("source_archive_sha256=") + 1, .)
local source_archive_size = substr(`"`source_archive_size_line'"', ///
    strlen("source_archive_size_bytes=") + 1, .)
local embedded_commit = substr(`"`embedded_commit_line'"', ///
    strlen("embedded_git_commit=") + 1, .)
local capture_build_date = substr(`"`capture_build_date_line'"', ///
    strlen("build_date_utc=") + 1, .)
local capture_harness_commit = substr(`"`capture_harness_commit_line'"', ///
    strlen("harness_commit=") + 1, .)
local capture_harness_tree = substr(`"`capture_harness_tree_line'"', ///
    strlen("harness_tree=") + 1, .)
local capture_artifact_hash = substr(`"`capture_artifact_hash_line'"', ///
    strlen("artifact_sha256=") + 1, .)
local capture_artifact_size = substr(`"`capture_artifact_size_line'"', ///
    strlen("artifact_size_bytes=") + 1, .)
local capture_program_hash = substr(`"`capture_program_hash_line'"', ///
    strlen("capture_program_sha256=") + 1, .)
if substr(`"`controller_commit_line'"', 1, ///
        strlen("controller_commit=")) != "controller_commit=" | ///
   !regexm(`"`controller_commit'"', "^[0-9a-f]{40}$") | ///
   substr(`"`capture_mode_line'"', 1, strlen("mode=")) != "mode=" | ///
   `"`capture_mode'"' != `"`target_mode'"' | ///
   `"`source_commit'"' != `"`target_commit'"' | ///
   `"`source_tree'"' != `"`target_tree'"' | ///
   `"`source_archive_hash'"' != `"`target_archive_hash'"' | ///
   real(`"`source_archive_size'"') != real(`"`target_archive_size'"') | ///
   `"`embedded_commit'"' != `"`target_embedded'"' | ///
   `"`capture_build_date'"' != `"`target_build_date'"' | ///
   `"`capture_harness_commit'"' != `"`harness_commit'"' | ///
   `"`capture_harness_tree'"' != `"`harness_tree'"' | ///
   `"`capture_artifact_hash'"' != `"`checksum_digest'"' | ///
   real(`"`capture_artifact_size'"') != `plugin_bytes' | ///
   !regexm(`"`capture_program_hash'"', "^[0-9a-f]{64}$") | ///
   `"`capture_inputs_sentinel'"' != ///
        "FEPPML_PLATFORM_CAPTURE_INPUTS_COMPLETE" | ///
   `"`capture_inputs_trailer'"' != "" | !`capture_inputs_eof' {
    exit 610
}

capture noisily do `"`platform_capture_program'"' ///
    `"`private_stage'/fereg_windows.plugin"' "windows-x86_64" ///
    `"`capture_artifact_hash'"' `"`capture_artifact_size'"' ///
    `"`capture_program_hash'"' `"`source_commit'"' ///
    `"`embedded_commit'"' `"`platform_capture'"'
local platform_capture_rc = _rc
if `platform_capture_rc' {
    exit `platform_capture_rc'
}
confirm file `"`platform_capture'"'
capture quietly shell powershell.exe -NoLogo -NoProfile -NonInteractive ///
    -ExecutionPolicy Bypass -File `"`platform_capture_helper'"' ///
    -Plugin `"`private_stage'/fereg_windows.plugin"' ///
    -Checksum `"`built_checksum'"' ///
    -CaptureProgram `"`platform_capture_program'"' ///
    -SourceManifest `"`controller_source_manifest'"' ///
    -TargetReceipt `"`target_receipt'"' ///
    -Capture `"`platform_capture'"' ///
    -Receipt `"`platform_capture_verification'"'
confirm file `"`platform_capture_verification'"'
tempname capture_verification_handle
file open `capture_verification_handle' using ///
    `"`platform_capture_verification'"', read text
file read `capture_verification_handle' platform_capture_verification_line
file read `capture_verification_handle' platform_capture_verification_trailer
local platform_capture_verification_eof = r(eof)
file close `capture_verification_handle'
if substr(`"`platform_capture_verification_line'"', 1, ///
        strlen("FEPPML_PLATFORM_CAPTURE_SHA256=")) != ///
        "FEPPML_PLATFORM_CAPTURE_SHA256=" | ///
   strlen(`"`platform_capture_verification_line'"') != ///
        strlen("FEPPML_PLATFORM_CAPTURE_SHA256=") + 64 | ///
   `"`platform_capture_verification_trailer'"' != "" | ///
   !`platform_capture_verification_eof' {
    exit 610
}

* The source-side manifest is generated from, and seals, the exact target
* receipt, controller manifest, artifact, capture, and capture program.
capture quietly shell python.exe `"`platform_manifest_sealer'"' ///
    --artifact `"`private_stage'/fereg_windows.plugin"' ///
    --capture `"`platform_capture'"' ///
    --capture-program `"`platform_capture_program'"' ///
    --controller-source-manifest `"`controller_source_manifest'"' ///
    --target-receipt `"`target_receipt'"' ///
    --run-id `"`controller_run_id'"' ///
    --output `"`platform_source_manifest'"'
local manifest_rc = _rc
if `manifest_rc' {
    exit `manifest_rc'
}
confirm file `"`platform_source_manifest'"'

* All fixtures are deliberately small.  Both modes prove target OLS and the
* public PPML failure contract; only candidate mode runs trusted private PPML.
capture noisily do `"`root'/tests/stata/installation/windows_ci_smoke.do"'
local smoke_rc = _rc
if `smoke_rc' {
    exit `smoke_rc'
}
if `"`target_mode'"' == "candidate" {
    capture noisily do ///
        `"`root'/tests/stata/parity_ppmlhdfe/estimate_ppml_hidden_transaction.do"'
    local hidden_ppml_rc = _rc
    if `hidden_ppml_rc' {
        exit `hidden_ppml_rc'
    }
}
capture noisily do `"`root'/tests/stata/parity_ppmlhdfe/feppml_fail_closed.do"'
local fail_closed_rc = _rc
if `fail_closed_rc' {
    exit `fail_closed_rc'
}

* Package only the verifier's closed result inventory.  The accepted backend
* must independently hash, return, and bind these exact ZIP bytes to its PASS.
capture quietly shell python.exe `"`results_bundler'"' ///
    --artifact `"`private_stage'/fereg_windows.plugin"' ///
    --checksum `"`built_checksum'"' ///
    --capture `"`platform_capture'"' ///
    --target-receipt `"`target_receipt'"' ///
    --source-manifest `"`platform_source_manifest'"' ///
    --built-verification `"`built_verification'"' ///
    --staged-verification `"`staged_verification'"' ///
    --capture-inputs `"`platform_capture_inputs'"' ///
    --capture-verification `"`platform_capture_verification'"' ///
    --output `"`results_archive'"'
local results_bundle_rc = _rc
if `results_bundle_rc' {
    exit `results_bundle_rc'
}
confirm file `"`results_archive'"'
file open `plugin_handle' using `"`results_archive'"', read binary
file seek `plugin_handle' eof
local results_archive_bytes = r(loc)
file close `plugin_handle'
if `results_archive_bytes' <= 0 {
    exit 610
}

* This exact marker is the only success publication and is written last.
tempname status_handle
file open `status_handle' using `"`status_file'"', write text
file write `status_handle' "WINDOWS_CI=PASS"
file close `status_handle'
