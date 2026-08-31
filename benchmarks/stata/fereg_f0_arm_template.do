/* Rendered once per arm and executed in one fresh licensed-Stata process.

   File contract under the rendered output directory:
     capture.machine  lossless science/timed e(), sample, and saved-output data
     READY            resolver/data/science/stabilizers completed
     START            written immediately before the measured command
     DONE             written immediately after the measured command

   The Python renderer replaces every named placeholder and validates all input
   artifact hashes before this file can be executed. */
version 18.0
clear all
set more off
set linesize 255

local profile "@@PROFILE@@"
local arm "@@ARM@@"
local case_id "@@CASE_ID@@"
local requested_threads = @@REQUESTED_THREADS@@
local stata_processors = min(`requested_threads', c(processors_lic), c(processors_mach))
set processors `stata_processors'

@@SYSDIR_SETUP@@
adopath ++ @@PACKAGE_ROOT_QUOTED@@
do @@HELPER_PATH_QUOTED@@

file open __fereg_f0_capture using @@CAPTURE_PATH_QUOTED@@, write text replace
file write __fereg_f0_capture "FEREG_F0_CAPTURE|1|schema|1" _n
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|identity|profile|@@PROFILE@@" _n
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|identity|arm|@@ARM@@" _n
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|identity|case_id|@@CASE_ID@@" _n
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|identity|manifest_sha256|@@MANIFEST_SHA256@@" _n
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|identity|source_commit|@@SOURCE_COMMIT@@" _n
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|identity|plugin_sha256|@@PLUGIN_SHA256@@" _n
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|identity|dataset_sha256|@@DATASET_SHA256@@" _n
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|identity|command_sha256|@@COMMAND_SHA256@@" _n
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|environment|stata_version|`c(stata_version)'" _n
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|environment|stata_flavor|`c(flavor)'" _n
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|environment|os|`c(os)'" _n
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|environment|machine_type|`c(machine_type)'" _n
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|environment|requested_threads|`requested_threads'" _n
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|environment|stata_processors|`stata_processors'" _n

/* Every package member that can affect resolution is checked against its
   exact isolated path before the plugin is loaded. */
@@RESOLVER_CHECKS@@

capture program __fereg_f0_provenance, plugin using(@@PLUGIN_PATH_QUOTED@@)
if _rc {
    file close __fereg_f0_capture
    exit _rc
}
capture noisily plugin call __fereg_f0_provenance, version
if _rc {
    file close __fereg_f0_capture
    exit _rc
}

/* The benchmark dataset is generated and SHA-256 verified by Python before
   this process starts. Stata only consumes the immutable .dta artifact. */
quietly use @@DATASET_PATH_QUOTED@@, clear
if _N != @@EXPECTED_OBSERVATIONS@@ {
    file close __fereg_f0_capture
    exit 459
}
capture confirm numeric variable @@OBS_ID@@
if _rc {
    file close __fereg_f0_capture
    exit 111
}
capture noisily isid @@OBS_ID@@
if _rc {
    file close __fereg_f0_capture
    exit 459
}
@@SAVED_OUTPUT_PRECHECKS@@
quietly _datasignature _all
local input_signature `"`r(datasignature)'"'
quietly __fereg_f0_encode, value(`"`input_signature'"')
local encoded_input_signature `"`r(encoded)'"'
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|dataset|observations|`=_N'" _n
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|dataset|stata_signature|`encoded_input_signature'" _n

/* A complete scientific capture precedes every timing. */
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|phase_begin|science" _n
capture noisily @@COMMAND@@
local science_rc = _rc
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|return_code|science|`science_rc'" _n
if `science_rc' != 0 {
    file close __fereg_f0_capture
    exit `science_rc'
}
__fereg_f0_capture_e, filehandle(__fereg_f0_capture) phase(science) ///
    arm(`arm') caseid(`case_id')
generate byte __fereg_f0_sample = e(sample)
__fereg_f0_capture_sample @@OBS_ID@@ __fereg_f0_sample, ///
    filehandle(__fereg_f0_capture) phase(science) arm(`arm') caseid(`case_id')
@@SAVED_OUTPUT_CAPTURE_SCIENCE@@
drop __fereg_f0_sample
@@SAVED_OUTPUT_CLEANUP@@
ereturn clear
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|phase_end|science" _n

/* Exactly two untimed stabilizing calls use the identical command and data. */
forvalues stabilizer = 1/2 {
    capture quietly @@COMMAND@@
    local stabilizer_rc = _rc
    file write __fereg_f0_capture ///
        "FEREG_F0_CAPTURE|1|stabilizer|`stabilizer'|`stabilizer_rc'" _n
    if `stabilizer_rc' != 0 {
        file close __fereg_f0_capture
        exit `stabilizer_rc'
    }
    @@SAVED_OUTPUT_CLEANUP@@
    ereturn clear
}
quietly _datasignature _all
if `"`r(datasignature)'"' != `"`input_signature'"' {
    file close __fereg_f0_capture
    exit 459
}

tempname marker
file open `marker' using @@READY_PATH_QUOTED@@, write text replace
file write `marker' "phase=READY|arm=@@ARM@@|case_id=@@CASE_ID@@" _n
file close `marker'
sleep 1000
file open `marker' using @@START_PATH_QUOTED@@, write text replace
file write `marker' "phase=START|arm=@@ARM@@|case_id=@@CASE_ID@@" _n
file close `marker'

/* The measured region contains exactly one fereg call. */
timer clear 97
timer on 97
capture quietly @@COMMAND@@
local timed_rc = _rc
timer off 97

file open `marker' using @@DONE_PATH_QUOTED@@, write text replace
file write `marker' "phase=DONE|arm=@@ARM@@|case_id=@@CASE_ID@@|rc=`timed_rc'" _n
file close `marker'
quietly timer list 97
local seconds : display %21x r(t97)
timer clear 97
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|return_code|timed|`timed_rc'" _n
file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|timing_seconds|`seconds'" _n
if `timed_rc' != 0 {
    file close __fereg_f0_capture
    exit `timed_rc'
}

/* Timed results are captured only after DONE, outside the measured region. */
__fereg_f0_capture_e, filehandle(__fereg_f0_capture) phase(timed) ///
    arm(`arm') caseid(`case_id')
generate byte __fereg_f0_sample = e(sample)
__fereg_f0_capture_sample @@OBS_ID@@ __fereg_f0_sample, ///
    filehandle(__fereg_f0_capture) phase(timed) arm(`arm') caseid(`case_id')
@@SAVED_OUTPUT_CAPTURE_TIMED@@
drop __fereg_f0_sample

file write __fereg_f0_capture ///
    "FEREG_F0_CAPTURE|1|capture_complete|arm|@@ARM@@|case_id|@@CASE_ID@@|rc|0" _n
file close __fereg_f0_capture
display as result ///
    "END_FIXTURE|fereg_f0_arm_capture_v1|arm=@@ARM@@|case=@@CASE_ID@@|rc=0"
exit 0
