# Direct-terminal LSMR stopping diagnosis

This is bounded development evidence, not qualification.

Private environment-gated per-RHS traces compared modified LSMR's own
normwise stopping diagnostic with fereg's independent per-FE publication
certificate. The trace added no public Stata option or result.

On `dth_p4_c450_s2_k16_t1` with explicit Schwarz-LSMR at tolerance `1e-10`,
the 17-column initial batch generally stopped after 13 iterations. Two columns
had fereg relative-infinity certificate values `1.1080e-10` and `1.0910e-10`,
about 9--11 percent above the request. A later one-column solve stopped after
three iterations with the modified-LSMR normwise diagnostic `0.009193`, while
the actual fereg FE certificate was `1.49e-11` and passed. The two diagnostics
are therefore not interchangeable; the earlier `0.0076` observation did not
show gross FE nonconvergence.

At tolerance `1e-11`, the initial batch took 15 iterations and all fereg
per-column certificate values were approximately `1.5e-12` to `3.9e-12`. That
fixed this case. A subsequent ordinary five-RHS, one-thread calibration case,
`dtc_p3_c660_s2_k4_t1`, then exceeded two minutes under the same `1e-11`
configuration and was interrupted. The earlier `1e-12` probe had shown the
same qualitative failure at a looser stage of the investigation.

Conclusion: tolerance tightening can resolve marginal column-specific
certificate misses, but it moves the runtime cliff to another common cell.
Requested- or tighter-tolerance LSMR is not the deterministic destination for
the direct-terminal CMG cost guard. Direct CG is the reliable measured
destination; fallback remains a safety mechanism rather than a normal route.
