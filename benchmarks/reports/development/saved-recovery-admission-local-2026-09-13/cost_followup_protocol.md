# Bounded cost attribution follow-up

The original unit pair costs +36 ms/5.3%, the two prescribed seeds +333 ms/11.3%
and +24 ms/3.6%. All science/solver fields and native peaks are exact. The +333 ms
case rejects the Schur probe (contraction .01251536) on both arms; initial
absorption solve increases .3173 seconds before recovery-budget code executes.
The other two pairs accept Schur and their additional work is outside initial
absorption. This is unresolved cost evidence, not an excuse to discard results.

Run four interleaved fresh pairs on already captured requests: same accepted
prototype binary A/A on seed8675309, reversed A/B on original seed104729, A/A on
104729, and reversed A/B on8675309. Use the existing uninstrumented harness,
exact full science/solver checks, phase/native/model/RSS reporting and no
concurrent builds/tests. No new seeds, workload substitutions or new thresholds.
A/A demonstrates variation, not the cause of all A/B differences. Retain all
observations; stop after these four pairs and document any unresolved cost.
