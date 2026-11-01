/*
 * Missing-classification witness for the numeric readers. The scalar reader
 * classifies with the SDK's SF_is_missing (Stata's ismissing callback); the
 * batch and parallel readers classify `value >= SV_missval` after the SDK's
 * vdata conversion. This plugin stores special doubles through the SDK and
 * prints, for every stored cell, both classifications and the bits returned
 * by the checked and unchecked accessors.
 *
 *   plugin call missing_witness <double var> <float var>, store
 *   plugin call missing_witness <double var> <float var>, check
 */
#include "stplugin.h"

#include <float.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

static ST_double special(ST_int observation)
{
    switch (observation) {
    case 1: return NAN;
    case 2: return -NAN;
    case 3: return INFINITY;
    case 4: return -INFINITY;
    case 5: return DBL_MAX;
    case 6: return -DBL_MAX;
    case 7: return SV_missval;
    case 8: return nextafter(SV_missval, 0.0);
    case 9: return nextafter(SV_missval, INFINITY);
    case 10: return FLT_MAX;
    case 11: return 0.0;
    default: return -0.0;
    }
}

static unsigned long long bits(ST_double value)
{
    unsigned long long result;
    memcpy(&result, &value, sizeof(result));
    return result;
}

STDLL stata_call(int argc, char *argv[])
{
    ST_int observation, variable, rc;
    ST_double unchecked, checked;
    char line[256];
    if (argc != 1 || SF_nvars() != 2 || SF_nobs() < 12) return 198;
    if (strcmp(argv[0], "store") == 0) {
        for (observation = 1; observation <= 12; ++observation) {
            for (variable = 1; variable <= 2; ++variable) {
                rc = SF_vstore(variable, observation, special(observation));
                snprintf(line, sizeof(line), "WITNESS_STORE|%d|%d|rc=%d\n",
                         (int) observation, (int) variable, (int) rc);
                SF_display(line);
            }
        }
        return 0;
    }
    if (strcmp(argv[0], "check") != 0) return 198;
    for (observation = 1; observation <= SF_nobs(); ++observation) {
        for (variable = 1; variable <= 2; ++variable) {
            rc = _stata_->vdata(variable, observation, &unchecked);
            if (rc == 0) rc = SF_vdata(variable, observation, &checked);
            if (rc != 0) return rc;
            snprintf(line, sizeof(line),
                     "W|%d|%d|%016llx|%016llx|%d|%d\n",
                     (int) observation, (int) variable, bits(unchecked), bits(checked),
                     SF_is_missing(unchecked) ? 1 : 0, unchecked >= SV_missval ? 1 : 0);
            SF_display(line);
        }
    }
    return 0;
}
