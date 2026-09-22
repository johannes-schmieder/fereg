# Serial weighted-slope norms: measured, not adopted

The captured sn1 experiment computes fresh max-normalized OLS column norms
during the existing ordered serial slope reconstruction. Raw-weight projection,
rank decisions and fresh QR certificates are unchanged. It does not rescale
an already computed raw norm. A separate bank is admitted against the existing
joint budget; insufficient memory, shuffled rows and nonserial execution keep
the previous norm scan.

The plugin builds and passes its actual ABI check and five focused release
tests: exact normalized norms across weight scales and slope layouts, the
one-byte memory boundary, actual worker-pool exclusion, the existing independent
slope oracle and ordered serial fusion. The nine-cell three-pair screen
completes 78 application calls with clean isolation. All 27 native pairs have
bit-identical scientific exports, samples and requested outputs. Tracked peak
increases are at most 416 bytes; several peaks and all excluded-control peaks
are unchanged.

The screen suggests small, mixed gains. Two independent five-pair native-only
confirmations cover AW K10, three-slope-only PW K6, AW K50 and a unit-weight K10
control, all serial. Their 80 calls have clean isolation and exact native
results and outputs. Paired sn1/owf1 ratios are:

| Case | First confirmation | Second confirmation |
|---|---:|---:|
| AW K10, robust and full outputs | 0.9849 | 0.9849 |
| PW K6, three slopes only, robust and full outputs | 1.0308 | 1.0044 |
| AW K50, robust and full outputs | 0.9701 | 0.9810 |
| Unit K10, iid without outputs, excluded control | 0.9583 | 1.0261 |

Six additional diagnostic calls preserve exact results. The AW K10 trace
reduces the norm-related phase from 17.40 to 10.77 ms while initial absorption
increases from 26.94 to 28.16 ms. The wide AW trace eliminates a 28.20 ms norm
scan but adds 14.27 ms to absorption. These single traces explain where work
moves; they are not headline timing evidence. Narrow weighted serial fits
still lose to fixest in the screen. Ordinary fixest is omitted from the FW
comparison because its weighted inference does not implement Stata frequency
weights.

The broad handoff is not adopted. Its small AW gains do not justify the mixed
whole-command result for the multiple-slope case. The active experiment flag
and instrumentation files are removed after their exact bytes are archived;
the existing native slope source and earlier graph drafts remain unchanged.
`source-disposition.json` binds that restoration to the archived source.

All 164 application calls, five focused tests, isolated source trees, controllers,
audit results and filtered phase traces are preserved. Large output arrays
remain local under hashes; raw licensed logs and binaries are excluded. Run
`verify.py` to check archive integrity and complete source reconstruction.
Detailed measurements are in [timings.md](timings.md). This is development
evidence, not a release, installation, Linux result or completion of the broader
optimization milestone.
