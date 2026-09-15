# Singleton-heavy review scale control

The predeclared 65,536/131,072-row sizes both pass exact retained-mask checks.
Half the observations start as isolated singletons; another quarter disappear
through a recursive chain. The final quarter remains. Small 256/512-row
versions also match both the historical scanner and an independent simultaneous
fixed-point oracle.

The isolated debug test records 0.057022 and
0.112789 seconds, a 1.978x ratio for doubling.
This is bounded diagnostic scaling, not a timing assertion or release claim.
SCC job 7586371 has failed=0/exit=0, one explicit passing test, a verified
compiler-reported executable and the terminal sentinel. No estimation, public
singleton policy, weight semantics or numerical tolerance changed.
