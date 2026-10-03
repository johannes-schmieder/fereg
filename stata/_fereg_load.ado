*! fereg plugin loader 0.5.0 24aug2026
program define _fereg_load, rclass
    version 18.0
    syntax , ACTION(string) [THREADS(integer 0) ENGINE(string)]

    if c(stata_version) < 18 {
        di as error "fereg requires Stata 18 or newer"
        exit 9
    }

    local selected_engine = lower(strtrim(`"`engine'"'))
    if `"`selected_engine'"' == "" local selected_engine = lower(strtrim(`"$FEREG_ENGINE"'))
    if `"`selected_engine'"' == "" local selected_engine "next"
    if !inlist(`"`selected_engine'"', "next", "legacy") {
        di as error "engine() and FEREG_ENGINE must select next or legacy"
        exit 198
    }
    if `"`action'"' == "resolve" {
        return local engine "`selected_engine'"
        exit
    }
    local handle "__fereg_plugin"
    if `"`selected_engine'"' == "legacy" {
        local handle "__fereg_legacy_plugin"
        noisily di as txt "note: using frozen engine(legacy)"
    }

    local os `"`c(os)'"'
    local machine_type `"`c(machine_type)'"'
    if `"`os'"' == "Windows" {
        local plugin_name "fereg_windows.plugin"
    }
    else if `"`os'"' == "MacOSX" {
        local plugin_name "fereg_macos.plugin"
    }
    else if `"`os'"' == "Unix" & strmatch(`"`machine_type'"', "Mac*") {
        local plugin_name "fereg_macos.plugin"
    }
    else if `"`os'"' == "Unix" {
        local plugin_name "fereg_linux.plugin"
    }
    else {
        di as error "fereg does not recognize operating system/machine type: `os' / `machine_type'"
        exit 9
    }
    if `"`selected_engine'"' == "legacy" ///
        local plugin_name = subinstr("`plugin_name'", "fereg_", "fereg_legacy_", 1)

    capture quietly findfile `plugin_name'
    if _rc {
        di as error "fereg native plugin not found: `plugin_name'"
        di as error "Build the target-native plugin and place it on the ado-path."
        exit 601
    }
    local plugin_file `"`r(fn)'"'

    capture program `handle', plugin using(`"`plugin_file'"')
    local load_rc = _rc
    if `load_rc' != 0 & `load_rc' != 110 {
        exit `load_rc'
    }

    if `"`action'"' == "loadonly" {
        /* Developer/internal callers perform their own single substantive call. */
    }
    else if `"`action'"' == "version" {
        di as txt "fereg.engine=`selected_engine'"
        plugin call `handle', version
    }
    else if `"`action'"' == "doctor" {
        di as txt "fereg.engine=`selected_engine'"
        plugin call `handle', doctor ///
            `"ado=0.5.0"' ///
            `"protocol_major=1"' ///
            `"protocol_minor=0"' ///
            `"stata=`c(stata_version)'"' ///
            `"threads=`threads'"' ///
            `"plugin_file=`plugin_file'"' ///
            `"dependency_audit=build-time-required"'
    }
    else if `"`action'"' == "selftest" {
        di as txt "fereg.engine=`selected_engine'"
        plugin call `handle', selftest ///
            `"ado=0.5.0"' ///
            `"protocol_major=1"' ///
            `"protocol_minor=0"' ///
            `"stata=`c(stata_version)'"' ///
            `"threads=`threads'"'
    }
    else {
        di as error "unknown fereg plugin action: `action'"
        exit 198
    }

    return local engine "`selected_engine'"
    return local handle "`handle'"
    return local plugin_name "`plugin_name'"
    return local plugin_file `"`plugin_file'"'
end
