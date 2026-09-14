# Generated predictor follow-up fixed before additional measurements

The first FW pair passes (1.452→1.366 seconds; 5.923%, 86 ms). Retain it and
its earlier separate baseline diagnostic. Candidate artifact e6978061 is fixed.

Complete three seeds (104729,8675309,20260819) for:
- Original FW N122880/K8 numeric plus 4×8 factors, t4, robust/exposure/missing.
- Unit N245760/K8 numeric plus 8×8 factors, t16, robust/missing (generated width72).
- PW N122880/K8 numeric plus 4×8 factors, t16, two clusters/offset/missing.

Fifteen single paired controls: generated weak PW cluster, bottleneck, cycle,
disconnected FW, unbalanced PW cluster; true three-FE FW; absorbed slope;
low FW cluster/exposure; saved/separated PW with original-row output; N4095
generated guard; small numeric K2; topology-backed factors plus rank deficiency;
generated widths7/8; and the existing tiny weighted bridge input with generated
factor controls appended. Controls use N8193–30720 except the explicit small
boundary and unchanged bridge N16384. Fixed seed31415927, warm seed+1.

All data are prepared before timing. Each call runs sequentially in a fresh
process after its own untimed warmup, with 120-second command cap. Arm order
rotates; no builds, tests, oracles or compression overlap measurements. Every
scientific field and non-time solver field must be exact because arithmetic is
unchanged. Saved values must also be exact. Primary repeated gain gate remains
both5%and10ms using the median paired change. Any protected increase above both
5% and10ms is retained and investigated. Report all phase/fallback/memory/RSS costs.

Run one separately instrumented candidate FW fit on the original data against
the earlier fixed baseline diagnostic. Compare scientific exports to that same
artifact's untraced result. Time the same initialization/accumulation/certificate
scopes; exclude diagnostics from command claims. Follow with targeted admission
and independent generation/weight checks if command gains survive controls.
