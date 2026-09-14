# Prototype execution notes

The first test invocation used a nonexistent cargo launcher path and exited127
before compilation. The toolchain bin PATH invocation compiles successfully.
An attempted local process listing was sandbox-denied; it neither establishes
termination nor changes the running test handle.

The v1 prototype and source identity are preserved. Review found that the serial
Schur apply validates inactive operator lanes. A failed prior full-width attempt
can leave nonfinite inactive lanes; the reused probe now clears that bank before
working on its three selected columns. The new reuse test fills prior state with
NaNs instead of finite sentinels. Whitespace-only rustfmt changes are removed.

Reporting retains the existing four engine rows: successful Schur is the existing
MAP accelerator identity; rejected Schur plus subsequent LSMR costs aggregate on
the final engine row, as existing LSMR/MAP fallback costs already do. The existing
checkpoint retry counter adds the rejected candidate. Probe budget, actual full
attempt iterations, copy/probe setup, solve time and outcome have private traces.
This is not completion of P0's separate all-attempt construction/copy accounting.
