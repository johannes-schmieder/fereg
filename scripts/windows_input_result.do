* Check one result file of the Windows parallel-input qualification
* (windows_ci_runtime.do). A do-file rather than a program, because the
* fixtures between the checks run `clear all`.
* usage: do windows_input_result.do <file> <expected prefix> [detailed]
* Exits 0 on the expected prefix; with `detailed`, the helper's numeric
* failure code (10000-99999); otherwise 9.
args file prefix detailed
confirm file `"`file'"'
tempname handle
file open `handle' using `"`file'"', read text
file read `handle' line
file close `handle'
display as text `"`line'"'
if substr(`"`line'"', 1, strlen(`"`prefix'"')) == `"`prefix'"' exit
if "`detailed'" != "" & regexm(`"`line'"', "\|code=([0-9]+)$") exit real(regexs(1))
exit 9
