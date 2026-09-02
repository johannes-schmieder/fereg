# Direct-terminal auto guard: post-change replay

This is focused development evidence, not qualification.

- Datasets: 2; recorded runs: 18; exactly three repetitions per solver cell.
- Explicit winner counts: {'cg': 2}.
- Locked expected routes: {'cg': 2}; observed auto routes: {'cg': 2}.
- Route mismatches: 0; ineligible auto cells: 0; auto fallback cells: 0.
- Harmful CG selections where explicit CMG was more than 5% faster: 0.
- Auto regret: median 1.044; maximum 1.055; above 1.25: 0; above 2.0: 0.
- Aggregate current-CMG / observed-auto command time: 1.300x.
- Plugin SHA-256: `debb95dcaf648aa11c439deb2861971aeebb34fd707ff5608c6f371b39f7c458`.
