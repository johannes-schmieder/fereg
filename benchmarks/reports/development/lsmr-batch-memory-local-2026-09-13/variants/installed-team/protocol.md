# Installed-team follow-up (prospective)

The original guarded candidate is not adopted: six weak PPML checkpoint pairs include +27/+43/+31 ms command increases. Preserve all six and the earlier universal-window rejection.

Correct the batch-width source to the actual installed Rayon pool, not LsmrConfig::threads (PPML checkpoints retain its default). Restore the upstream free-function construction for narrow transient batches. Instrument existing checkpoint traces with actual RHS/window/team values. These changes preserve matrix arithmetic and certificates.

Compare the unchanged ac47b19 baseline (plugin 6c364b062dc505f478ad9079a8106db4e89c0aed4079735ad89d939f7d1a0497) against the source-frozen candidate. First use six rotated PPML pairs: each of seeds 104729, 15485863, 32452843 twice with reversed arm order. Same N30720/K10/four-thread weak PW/offset/cluster workload. All full science and non-time solver decisions must remain exact; only changed memory-estimate equality is omitted. Command regressions above max(10 ms,5%) require localization; do not discard bad pairs.

If these pass, verify a wide PPML checkpoint crossing the 4*team guard and repeat the affected wide OLS control. No builds or tests alongside timing; no cluster/qualification/installed-dist mutation.

The six narrow follow-up pairs pass with -2 to +9 ms changes. Before width timings, specify three new PPML N30720/K17/four-thread PW/offset/cluster pairs (seeds104729/15485863/32452843), one separate K10 explicit solver-trace pair, and three OLS N122880/K65/four-thread weak AW/robust pairs (seeds104729/8675309/20260819). All retain the same exact-science and timing envelope. The private measure copy only enables the existing solver trace and directs it to each run; prior trace=True enabled phase/Stata profiling, not this native solver trace. Keep the extra profiled narrow pair separate from the six repeated timing controls.
