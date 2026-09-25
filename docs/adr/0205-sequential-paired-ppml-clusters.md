# ADR 0205: bounded sequential paired PPML cluster scores

Status: accepted for local development, 2026-09-25.

## Decision

Scratch-backed PPML clustered inference may accumulate separate production and
certificate group scores in original row order. Use the existing immutable range
reader, retain compensation independently for both scores, and finish production
in lexicographic group order and the certificate in reverse group order. This
supersedes ADR 0135's exclusion of clustered PPML for this admitted route only.

Plan every subset before allocating or reading its design. Its complete workspace
must fit within the **two simultaneous N-word ordering arrays** displaced from
the prior radix-sort route: four group-score arrays, presence flags, one row,
three K-by-K arrays and owner descriptors. Count physical retained rows and the
mixed-radix product, including absent groups. Check arithmetic and actual score-bank
capacities; allocation/read/numerical failure remains an error before publication.
Large or overflowing products retain the prior ordered route. For positive score
rank, the admitted bank bound also implies that the displaced route admits radix
counts rather than its sparse-label single-order fallback. Rank-zero inference
retains its separate group-inventory route.

A design preference exposes expensive point access without tying policy to an
estimator option, thread count, dataset name or seed. Only the certified scratch
source opts in. Dense PPML, robust PPML and OLS retain their current reductions.
No N-by-K cache, persistent topology owner, parallel team or cross-frame authority
is added. The action-local bank is released on return; an immutable borrow keeps
the final certified within design unchanged throughout its scan.

## Scientific boundary

Production and certificate retain independently computed scalar multipliers,
separate compensated group sums, separate meats, opposite merge orders and final
bread/sandwich certification. The certificate never reads production sums or meat.
The old partial-block merge tree is replaced; bitwise equality to that tree is not
required. Literal integer group-score oracles, explicit-design Poisson/Hessian/
sandwich oracles and the existing independent weighted licensed fixtures govern
accuracy. No tolerance, sample, rank, weight, DoF, PSD, separation, outer score,
objective, reconstruction or exact final q*mu Hessian policy changes.

The initial one-order-array budget already improves the public command but leaves
costly intersection subsets on random reads. One bounded correction charges the
**entire** replacement workspace against the old two-array construction high-water
mark. It is retained after fresh validation; there is no further threshold tuning.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-cluster-scan-local-2026-09-25/README.md)
binds `pairedscan2` to `daa0ce4e` plus four source overlays. Five fresh affected
families improve 31.7–60.5%, winning all 25 candidate/baseline pairs. Median
inference drops from 548 to 40 ms in the largest fresh case. Several cases still
miss 0.90 times stock; the large unbalanced factor case remains 1100/720 ms.
Remaining projection and strict-final work, rather than another cluster threshold,
are the next substantial targets.

All 35 fresh direct-stock scientific comparisons pass. Twenty-six distinct Rust
tests, thirteen Python surface tests and three licensed fixtures pass. Focused OLS
exports are exact, but its narrow control loses one ms at the median; excluded PPML
and OLS distributions remain visible. No universal non-regression claim follows.

All fresh tracked peaks, modeled storage and scratch sizes match baseline. The
primary pilot retains tracked peak differences from -1.43 to +2.02 MB. The local
workspace dominance proof is not a repair of the existing static allocation
inventory drift or physical-cap qualification. No new warmed-allocation campaign,
Linux/SCC run, release or push is claimed.
