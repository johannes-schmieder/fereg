# Checkpoint candidate ownership and cost audit

Only already admitted checkpoint-LSMR sessions enter the candidate. Eligibility
also requires exactly two categorical effects with existing Schur graph storage,
at least 16384 rows, 16..64 actual RHSs, and more than eight allowed iterations.
Ordinary LSMR joint/fused, separation, low-memory, one-FE, slope and three-FE
selection is unchanged. Narrow final residual-only checkpoints remain LSMR.

The selected PPML categorical accelerator model already owns both level-weight
vectors, two left-level and four right-level RHS banks and five scalar banks.
The probe resets and uses three lanes of that same graph, including on changing
weights. It does not allocate another graph or observation-sized probe state.
OLS calls the same probe helper with a newly allocated stride-three graph; its
weight normalization, selected columns, observation arithmetic and guard remain.

The candidate adds one temporary row-major 8*N*R double payload. Existing
checkpoint admission is `lsmr_schwarz_bytes + 8*N*R + 8*N`, beyond the selected
canonical session and ordinary specialized engine. The Schur candidate replaces
LSMR's transient banks. For the eligible width<=64, the LSMR model admits the
full RHS width and at least two full observation output banks. The caller's
original matrix remains in the separate 8*N*R term. Checked size admission and
fallible allocation occur before copying; tests reject payload-minus-one and
accept the exact bound, and inspect representative N/R/thread estimator shapes.
No second residualized design is retained across frames.

The copy uses the existing fallible matrix-copy helper, including zero-fill and
copy cost. Probe plus copy are timed as setup; Schur setup/recurrence/reconstruction/
certificate are in its solve scope. Rejected Schur candidates are dropped before
LSMR setup, and original-input LSMR/tighter retry/MAP continuation remains intact.
The sum is in complete-command time; a private trace labels bounded probe budget,
full Schur iterations, outcome, setup and solve. The existing Schur phase trace
can separate recurrence and independent certificate work. Small memory-estimator
metadata arithmetic is not separately timed, and physical memory traffic is not
measured.

The public four-row PPML report retains its current identities. Certified Schur
uses the established MAP accelerator identity, while ordinary auto remains joint
LSMR. A rejected Schur attempt contributes an additional checkpoint retry and
warning; costs aggregate on the final engine row just as existing LSMR/MAP retry
costs do. Pilot iterations are the eight-step probe budget, not exact early-exit
operator counts. This does not close P0's full physical attempt/build accounting.
