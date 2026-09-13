# Follow-up to preserved PW timing flags

The six prospective cost pairs and two required PW repeats are terminal. Exact
science/solver decisions hold, but PW changes +819 ms, -70 ms and +1961 ms.
Nearly all excess time is in the unchanged initial CG solve, before the modified
derived-output selector runs. System load rises during the sequence; a subsequent
read-only process snapshot observes an unrelated Rust test using a CPU. Neither
observation establishes the cause of the timing differences. Do not interrupt
other work, discard measurements or call this a proven external/noise effect.

Run exactly four further ordinary PW pairs, preserving the original data/request:
one accepted-binary A/A pair, one A/B pair with candidate first, one accepted-
binary A/A pair, then one A/B pair with baseline first. Use fresh processes and
the same warm-up/worker/RSS protocol. No overlapping build/test/profile/data
generation. Keep A/A separate from implementation comparisons, preserve all
phase/solver/native/scratch/science outputs and all flags. Do not add seeds or
keep repeating until favorable results occur. Assess whether the changed-code
cost is repeatable; unresolved differences remain a disclosed limitation.
