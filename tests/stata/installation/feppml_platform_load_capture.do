version 18.0

* This fixture proves only that one exact native artifact can be loaded and
* execute the installation selftest in licensed Stata.  The calling
* platform driver computes and rechecks every SHA-256 value; this program
* publishes the Stata-owned version/flavor and terminal completion record.
args plugin_path platform artifact_sha256 artifact_size_bytes ///
    capture_program_sha256 source_commit embedded_git_commit capture_path

foreach required in plugin_path platform artifact_sha256 artifact_size_bytes ///
    capture_program_sha256 source_commit embedded_git_commit capture_path {
    if `"``required''"' == "" {
        exit 198
    }
}

if !inlist(`"`platform'"', "macos-universal", "linux-x86_64", ///
    "windows-x86_64") {
    exit 198
}
local host_is_mac = (`"`c(os)'"' == "MacOSX") | ///
    (`"`c(os)'"' == "Unix" & strmatch(`"`c(machine_type)'"', "Mac*"))
local host_is_linux = (`"`c(os)'"' == "Unix") & ///
    !strmatch(`"`c(machine_type)'"', "Mac*")
if (`"`platform'"' == "macos-universal" & !`host_is_mac') | ///
   (`"`platform'"' == "linux-x86_64" & !`host_is_linux') | ///
   (`"`platform'"' == "windows-x86_64" & `"`c(os)'"' != "Windows") {
    exit 9
}

if strlen(`"`artifact_sha256'"') != 64 | ///
   !regexm(`"`artifact_sha256'"', "^[0-9a-f]+$") | ///
   strlen(`"`capture_program_sha256'"') != 64 | ///
   !regexm(`"`capture_program_sha256'"', "^[0-9a-f]+$") {
    exit 198
}
if strlen(`"`source_commit'"') != 40 | ///
   !regexm(`"`source_commit'"', "^[0-9a-f]+$") | ///
   `"`embedded_git_commit'"' != substr(`"`source_commit'"', 1, 12) {
    exit 198
}
local artifact_size = real(`"`artifact_size_bytes'"')
if missing(`artifact_size') | `artifact_size' <= 0 | ///
   `artifact_size' != floor(`artifact_size') {
    exit 198
}

local normalized_plugin = subinstr(`"`plugin_path'"', char(92), "/", .)
local expected_plugin = cond(`"`platform'"' == "macos-universal", ///
    "fereg_macos.plugin", cond(`"`platform'"' == "linux-x86_64", ///
    "fereg_linux.plugin", "fereg_windows.plugin"))
if substr(`"`normalized_plugin'"', ///
    strlen(`"`normalized_plugin'"') - strlen(`"`expected_plugin'"') + 1, .) ///
    != `"`expected_plugin'"' {
    exit 198
}

confirm file `"`plugin_path'"'
capture confirm file `"`capture_path'"'
if !_rc {
    exit 602
}

capture program drop __feppml_platform_load_probe
program define __feppml_platform_load_probe, plugin using(`"`plugin_path'"')
capture noisily plugin call __feppml_platform_load_probe, selftest ///
    "ado=0.5.0" "protocol_major=1" "protocol_minor=6" ///
    "stata=`c(stata_version)'" "threads=1" "verbose=1"
local plugin_load_rc = _rc
if `plugin_load_rc' {
    exit `plugin_load_rc'
}

local stata_version `"`c(stata_version)'"'
local stata_flavor `"`c(flavor)'"'
if `"`stata_version'"' == "" | `"`stata_flavor'"' == "" {
    exit 9
}
if !regexm(`"`stata_version'"', "^[0-9.]+$") | ///
   !regexm(`"`stata_flavor'"', "^[A-Za-z0-9 -]+$") {
    exit 9
}

tempname capture_handle
file open `capture_handle' using `"`capture_path'"', write text
file write `capture_handle' ///
    "{" (char(34)) "schema_version" (char(34)) ":1," ///
    (char(34)) "evidence_type" (char(34)) ":" (char(34)) ///
    "feppml-licensed-stata-load-v1" (char(34)) "," ///
    (char(34)) "platform" (char(34)) ":" (char(34)) ///
    `"`platform'"' (char(34)) "," ///
    (char(34)) "artifact_sha256" (char(34)) ":" (char(34)) ///
    `"`artifact_sha256'"' (char(34)) "," ///
    (char(34)) "artifact_size_bytes" (char(34)) ":`artifact_size_bytes'," ///
    (char(34)) "stata_version" (char(34)) ":" (char(34)) ///
    `"`stata_version'"' (char(34)) "," ///
    (char(34)) "stata_flavor" (char(34)) ":" (char(34)) ///
    `"`stata_flavor'"' (char(34)) "," ///
    (char(34)) "license_status" (char(34)) ":" (char(34)) ///
    "licensed" (char(34)) "," ///
    (char(34)) "plugin_load_status" (char(34)) ":" (char(34)) ///
    "success" (char(34)) "," ///
    (char(34)) "plugin_load_rc" (char(34)) ":0," ///
    (char(34)) "loaded_plugin_sha256" (char(34)) ":" (char(34)) ///
    `"`artifact_sha256'"' (char(34)) "," ///
    (char(34)) "capture_program_sha256" (char(34)) ":" (char(34)) ///
    `"`capture_program_sha256'"' (char(34)) "," ///
    (char(34)) "source_commit" (char(34)) ":" (char(34)) ///
    `"`source_commit'"' (char(34)) "," ///
    (char(34)) "embedded_git_commit" (char(34)) ":" (char(34)) ///
    `"`embedded_git_commit'"' (char(34)) "," ///
    (char(34)) "mock_only" (char(34)) ":false," ///
    (char(34)) "scientific_claim" (char(34)) ":false," ///
    (char(34)) "complete_sentinel" (char(34)) ":" (char(34)) ///
    "FEPPML_LICENSED_STATA_LOAD_COMPLETE" (char(34)) "}" _n
file close `capture_handle'

display as result "FEPPML_LICENSED_STATA_LOAD_COMPLETE"
