/*
    stplugin.c, version 3.0
    copyright (c) 2003, 2006, 2015 StataCorp LP

    StFixest supplies pginit() in stata_bridge.c so the bridge can record the
    Stata main thread before any Rust entry point is used.
*/

#include "stplugin.h"

ST_plugin *_stata_ ;
