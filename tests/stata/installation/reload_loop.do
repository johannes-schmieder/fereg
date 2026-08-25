version 18.0
clear all
set more off
forvalues repetition = 1/100 {
    capture program drop __fereg_plugin
    capture noisily fereg, selftest threads(2)
    local selftest_rc = _rc
    local plugin_name "`r(plugin_name)'"
    if `selftest_rc' != 0 {
        exit `selftest_rc'
    }
    if (c(os) == "MacOSX" | ///
        (c(os) == "Unix" & strmatch(c(machine_type), "Mac*"))) & ///
        "`plugin_name'" != "fereg_macos.plugin" {
        di as error "reload selected wrong macOS plugin: `plugin_name'"
        exit 9
    }
    else if c(os) == "Windows" & ///
        "`plugin_name'" != "fereg_windows.plugin" {
        di as error "reload selected wrong Windows plugin: `plugin_name'"
        exit 9
    }
    else if c(os) == "Unix" & !strmatch(c(machine_type), "Mac*") & ///
        "`plugin_name'" != "fereg_linux.plugin" {
        di as error "reload selected wrong Linux plugin: `plugin_name'"
        exit 9
    }
}
di as result "reload_loop.do: PASS"
di as result "END_FIXTURE|reload_loop|rc=0"
