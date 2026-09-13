# Recovery admission boundary, prospective local diagnostic

Before any new performance comparisons, use the public zero-coordinate one-FE
saved-output fixture at N4096, four levels and one requested FE channel. It fits
only fixed effects, with ordinary noise; full-dummy OLS is the independent oracle.
No schema, rank or scientific tolerance is relaxed.

The initial execution model is N*(8*2+8+8+8+8+4)=212992 bytes, with zero finite
columns. The one-RHS initial CG bound is smaller than the minimum finishing
reserve. Recovery's retained/MAP reserve is 8*N*(1+4)+128*4+256+4096=168704.
The public test cap subtracts 512 MiB headroom. Hence exact admission is at
537252608 available bytes and one-byte-below is 537252607.

First verify the baseline uncapped scientific result and public selected model.
Then at the lower cap the old saved-Schur prototype should succeed and the
candidate must reject r(909) with the recovery-admission diagnostic before any
public output appears. Previous e(b), e(V), e(sample), variable identities and
original row order must remain. At the exact cap candidate must match the same
independent oracle. If the model or call path differs, preserve that failure and
localize it rather than silently replacing the fixture or cap.

This proves a named recovery admission boundary, not a whole-action allocator
limit or memory qualification. It does not measure performance. Initial solve
and other pre-existing memory-report gaps remain separate. No cluster work.
