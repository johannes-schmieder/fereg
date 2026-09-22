# Serial recovery row cache

The frozen `rrc1` candidate computes up to four normalized slope values once
per observation during serial direct saved-effect recovery. Two independent
five-pair blocks show roughly 1.8–2.2% gains for PW robust full-output fits and
0.4–1.2% for clustered PW. AW is neutral to modestly faster; saved-effects-only
results are mixed. The change is retained as a small local refinement under
ADR 0098. It does not close the serial fixest gap.

| Serial saved-output case | Block 1 paired candidate/prior | Block 2 paired candidate/prior |
|---|---:|---:|
| AW K10 robust, full outputs | 0.9878 | 0.9959 |
| AW K10 two-way cluster, full outputs | 0.9865 | 0.9967 |
| PW K6, three slopes only, robust, full outputs | 0.9825 | 0.9784 |
| PW K6, three slopes only, two-way cluster, full outputs | 0.9960 | 0.9883 |
| PW K6, three slopes only, robust, saved effects | 1.0204 | 0.9898 |

[All timings](timings.md) retain the mixed seven-cell three-pair screen against
`rgc1` and stock fixest, including AW's initial robust regression and no-output/
threaded controls. Both native-only confirmations use the same frozen candidate.
N is 245,760 and the seed is 104729 throughout. The screen gives PW robust/full
medians of 226/223/149 ms for prior/candidate/fixest. Threaded PW robust/full
is 97/97/124 ms. These are focused development comparisons, not a new broad
performance acceptance matrix.

All 163 application calls pass with no detected overlap. All 71 native pairs
have exact six-bundle numerical exports, samples and requested saved arrays;
independent output reconstruction and normal-equation checks pass. Tracked native
peak allocation is equal in every pair. The fixed four-double row cache adds
no heap storage and does not change memory admission. The five release fixtures
and actual plugin ABI check pass. They cover bitwise moments at all bounded
widths, zero targets, extreme scales, shuffled membership, rank degeneracy,
fresh certificates, memory fallback and protected PPML allocation/atomicity.

Only ADR 0097's serial geometry-cached direct recovery uses the row cache.
Each compensated entry retains its multiplication and row order. All scales,
rank decisions, coefficient solves and fresh residual checks remain unchanged.
Threaded/uncached recovery and generic MAP/PPML use the prior moment loop.
There is no solver-selection change or new public interface.

The archive binds two complete captured sources, controllers, reference-library
hashes, test logs and scientific/isolation audits. `verify.py` verifies 4,515
stored entries and reconstructs both sources. The 154 omitted local output
arrays retain explicit hashes; binaries and raw licensed logs are excluded.
Measured sources include the six preceding graph/window drafts. Prior final
blocks remain evidence for their original sources. Linux confirmation and the
larger optimization milestone remain open; no push, installation or release
is performed.
