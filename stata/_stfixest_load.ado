*! StFixest 0.1.0 Phase 0-1 20aug2026
program define _stfixest_load, rclass
    version 18.0

    if real(c(stata_version)) < 18 {
        di as error "StFixest requires Stata 18 or newer"
        exit 9
    }

    local os `"`c(os)'"'
    if `"`os'"' == "Windows" {
        local plugin_name "stfixest_windows.plugin"
    }
    else if `"`os'"' == "MacOSX" {
        local plugin_name "stfixest_macos.plugin"
    }
    else if `"`os'"' == "Unix" {
        local plugin_name "stfixest_linux.plugin"
    }
    else {
        di as error "StFixest does not recognize operating system: `os'"
        exit 9
    }

    capture quietly findfile `plugin_name'
    if _rc {
        di as error "StFixest native plugin not found: `plugin_name'"
        di as error "Build the target-native plugin and place it on the ado-path."
        exit 601
    }
    local plugin_file `"`r(fn)'"'

    capture program drop __stfixest_plugin
    program __stfixest_plugin, plugin using(`"`plugin_file'"')

    return local handle "__stfixest_plugin"
    return local plugin_name "`plugin_name'"
    return local plugin_file `"`plugin_file'"'
end
