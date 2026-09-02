# Direct-terminal auto guard: post-change replay

This is focused development evidence, not qualification.

- Datasets: 54; recorded runs: 486; exactly three repetitions per solver cell.
- Explicit winner counts: {'cg': 35, 'cmg': 19}.
- Locked expected routes: {'cg': 15, 'cmg': 39}; observed auto routes: {'cg': 15, 'cmg': 39}.
- Route mismatches: 0; ineligible auto cells: 0; auto fallback cells: 0.
- Harmful CG selections where explicit CMG was more than 5% faster: 1.
- Auto regret: median 1.021; maximum 3.294; above 1.25: 18; above 2.0: 5.
- Aggregate current-CMG / observed-auto command time: 1.339x.
- Plugin SHA-256: `debb95dcaf648aa11c439deb2861971aeebb34fd707ff5608c6f371b39f7c458`.

The one apparent harmful explicit-CG comparison was the no-control
`dtf_p3_c685_s2` topology, whose eight-thread CG timings ranged from 0.098 to
0.299 seconds in this replay. The bounded paired recheck under
`postchange-recheck/` repeated both one- and eight-thread cells with three new
recorded repetitions. CG won both, all auto runs certified without fallback,
and maximum observed auto regret was 1.055. This was treated as timing
dispersion, not as a basis for another routing threshold.
