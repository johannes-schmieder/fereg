The archived weak_d3 generator label was described as three FE in the repair
protocol/report. Inspection of the actual result cell shows absorb(g1 g2): the
shared pair helper selected only two FE. Preserve that useful two-FE control,
correct its label, and run exactly one additional pair at the original seed,
N/K/threads with explicit absorb(g1 g2 g3), identical in both arms. This finishes
the originally specified three-FE check without replacing an observation or
changing the repair. Do not run while the separate wider candidate builds/tests.
