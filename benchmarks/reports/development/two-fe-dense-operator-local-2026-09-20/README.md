# Counted Schur edges and multi-FE overlap

The `finaloverlap` prototype clears the previously failing shuffled narrow cell
in a five-pair screen: median paired native/default-fixest ratio 0.881620, five
wins, and command medians 290/328 ms. This is not final acceptance. The complete
primary matrix, both final blocks and all regression controls remain required.
All 79 completed calls in this archive pass their scientific comparisons.

## Separate incremental screens

These are distinct paired blocks on N=983,040, shuffled seed 31415927, sixteen
workers. Times are medians in milliseconds; ratios are medians of paired ratios.

| Change | K | Parent | Candidate | Stock fixest | Candidate/fixest |
|---|---:|---:|---:|---:|---:|
| Exact neighbor-code cache | 10 | 348 | 325 | 331 | 0.985119 |
| Exact neighbor-code cache | 50 | 853 | 776 | 1136 | 0.683099 |
| Narrow recurrence/certificate diagnostic | 10 | 347 | 329 | 340 | 0.964809 |
| Counted edges and narrow certificate | 10 | 324 | 310 | 323 | 0.960725 |
| Counted edges and narrow certificate | 50 | 752 | 729 | 1109 | 0.656447 |
| Parallel edge setup and outcome overlap | 10 | 309 | 302 | 322 | 0.924242 |
| Parallel edge setup and outcome overlap | 50 | 727 | 696 | 1102 | 0.629764 |
| Final projection/bread overlap (five pairs) | 10 | 293 | 290 | 328 | 0.881620 |

The recurrence/certificate diagnostic includes native phase instrumentation.
Its initial recurrence remains approximately 22–23 ms; the serial-recurrence
guard was removed. Extending the existing bounded certificate reduction to
narrow two-FE OLS cuts measured certification from roughly 15 to 10–12 ms.
Its admission still requires the existing partial-sum scratch to fit.

The first cache preserves original membership order and operator bits. Its six
native result comparisons are byte-exact and all six native allocation peaks
are unchanged. The successor combines repeated unit-weight observation pairs
into counted edges. It packs a u32 neighbor and u32 multiplicity into each
word of the existing group-statistics scratch, using bit-preserving f64 storage
without floating-point operations on those private words. No new owner is
allocated. The scratch is overwritten by the independent final certificate.
Insufficient scratch, supplied weights and lower worker counts keep the old
operator. Observation/sample ordering and the mathematical operator are intact;
aggregation changes its floating-point summation order.

A phase diagnostic reduces operator time from about 75 to 55 ms over 35
iterations, while serial counted-edge setup raises initialization from about
14 to 25 ms. The next prototype constructs disjoint group ranges on the existing
team. It also extends the independent outcome-statistics overlap to eligible
multi-FE fits. Ordinary topology validation remains borrowed and allocation-free;
categorical multi-FE DoF construction remains at its previous boundary.

After an accepted unit-weight initial Schur probe, the final prototype permits
the existing final-projection/bread/score scheduling. Raw-residual, bread, score
and projection owners were already jointly admitted. Original error precedence,
fresh projection/score certificates and output intent remain authoritative.

## Scientific checks and limits

The counted operator agrees with the independent observation-space operator
within 2e-12 on permuted, repeated-edge inputs and multiple RHS widths. Its full
projection matches independent strict MAP, and eight-/sixteen-worker results
are bit-identical in that fixture. An initial overly tight absolute comparison
against a default norm-scaled solve failed. The uncached solver has the same
1.10e-7 distance to strict MAP; counted versus uncached differs by only 1.42e-14.
The final test separately retains default-route agreement and tightens the
tested solve to 1e-12 for the unchanged 2e-9 absolute oracle bound. Production
tolerances are unchanged; the original failed log is preserved locally.

Seven outcome-overlap tests pass, including ordinary/categorical two-/three-FE
exact inference and deferred errors. The final-projection test covers narrow/
wide designs, packed/strided/absent score caches and conflicting failure paths.
Actual-plugin ABI checks and all three Stata fixtures pass for each timed
artifact. Artifact identities and per-call receipts are included; licensed raw
logs remain local. The new candidate is still being frozen for final validation.
No installed package, remote state or release qualification changed.
