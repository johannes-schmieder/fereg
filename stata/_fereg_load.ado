*! fereg plugin loader 0.5.2 07nov2026
program define _fereg_load, rclass
    version 15.0
    syntax , ACTION(string) [THREADS(integer 0) VERBOSE]

    if c(stata_version) < 15 {
        di as error "fereg requires Stata 15 or newer"
        exit 9
    }

    local handle "__fereg_plugin"

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

    capture quietly findfile `plugin_name'
    if _rc {
        di as error "fereg native plugin not found: `plugin_name'"
        di as error "Build the target-native plugin and place it on the ado-path."
        exit 601
    }
    _fereg_fullpath `"`r(fn)'"'
    local plugin_file `"`r(fn)'"'

    capture program `handle', plugin using(`"`plugin_file'"')
    local load_rc = _rc
    if `load_rc' != 0 & `load_rc' != 110 {
        exit `load_rc'
    }

    if `"`action'"' == "loadonly" {
        /* Developer/internal callers perform their own single substantive call. */
    }
    else if `"`action'"' == "selftest" {
        local native_verbose = (`"`verbose'"' != "")
        plugin call `handle', selftest ///
            `"ado=0.5.2"' ///
            `"protocol_major=1"' ///
            `"protocol_minor=7"' ///
            `"stata=`c(stata_version)'"' ///
            `"threads=`threads'"' ///
            `"plugin_file=`plugin_file'"' ///
            `"verbose=`native_verbose'"'
    }
    else {
        di as error "unknown fereg plugin action: `action'"
        exit 198
    }

    return local handle "`handle'"
    return local plugin_name "`plugin_name'"
    return local plugin_file `"`plugin_file'"'
end

program define _fereg_fullpath, rclass
    version 15.0
    local cur `"`c(pwd)'"'
    mata: st_local("base", pathbasename(st_local("1")))
    * Strip only the filename: pathgetparent() simplifies ~/../ before cd
    * can expand ~, which would change the meaning of that path.
    local dir = substr(`"`1'"', 1, strlen(`"`1'"') - strlen(`"`base'"'))
    if `"`dir'"' == "" local dir .
    * cd expands home/relative paths that program, plugin does not expand.
    * Preserve the caller's directory even on Break or path-construction error.
    nobreak {
        quietly cd `"`dir'"'
        capture mata: st_local("full", pathjoin(pwd(), st_local("base")))
        local path_rc = _rc
        quietly cd `"`cur'"'
    }
    if `path_rc' exit `path_rc'
    return local fn `"`full'"'
end
