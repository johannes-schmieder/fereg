*! StFixest 0.1.0 Phase 0-1 20aug2026
program define _stfixest_load, rclass
    version 18.0
    syntax , ACTION(string) [THREADS(integer 0)]

    if c(stata_version) < 18 {
        di as error "StFixest requires Stata 18 or newer"
        exit 9
    }

    local os `"`c(os)'"'
    local machine_type `"`c(machine_type)'"'
    if `"`os'"' == "Windows" {
        local plugin_name "stfixest_windows.plugin"
    }
    else if `"`os'"' == "MacOSX" {
        local plugin_name "stfixest_macos.plugin"
    }
    else if `"`os'"' == "Unix" & strmatch(`"`machine_type'"', "Mac*") {
        local plugin_name "stfixest_macos.plugin"
    }
    else if `"`os'"' == "Unix" {
        local plugin_name "stfixest_linux.plugin"
    }
    else {
        di as error "StFixest does not recognize operating system/machine type: `os' / `machine_type'"
        exit 9
    }

    capture quietly findfile `plugin_name'
    if _rc {
        di as error "StFixest native plugin not found: `plugin_name'"
        di as error "Build the target-native plugin and place it on the ado-path."
        exit 601
    }
    local plugin_file `"`r(fn)'"'

    capture program __stfixest_plugin, plugin using(`"`plugin_file'"')
    local load_rc = _rc
    if `load_rc' != 0 & `load_rc' != 110 {
        exit `load_rc'
    }

    if `"`action'"' == "version" {
        plugin call __stfixest_plugin, version
    }
    else if `"`action'"' == "doctor" {
        plugin call __stfixest_plugin, doctor ///
            `"ado=0.1.0-phase01"' ///
            `"protocol_major=1"' ///
            `"protocol_minor=0"' ///
            `"stata=`c(stata_version)'"' ///
            `"threads=`threads'"' ///
            `"plugin_file=`plugin_file'"' ///
            `"dependency_audit=build-time-required"'
    }
    else if `"`action'"' == "selftest" {
        plugin call __stfixest_plugin, selftest ///
            `"ado=0.1.0-phase01"' ///
            `"protocol_major=1"' ///
            `"protocol_minor=0"' ///
            `"stata=`c(stata_version)'"' ///
            `"threads=`threads'"'
    }
    else {
        di as error "unknown StFixest plugin action: `action'"
        exit 198
    }

    return local handle "__stfixest_plugin"
    return local plugin_name "`plugin_name'"
    return local plugin_file `"`plugin_file'"'
end
