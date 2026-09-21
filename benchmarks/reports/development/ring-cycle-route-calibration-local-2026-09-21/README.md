# Ring/cycle whole-command route calibration, 2026-09-21

All 288 measured applications pass independent inference and requested-output
audits. This is calibration evidence, not an accepted routing rule or a final
benchmark against fixest. Every call uses the same `m5` native artifact,
plugin `a07732eea15a47a185903bf7403cd74f111a5eebbd5e558fbb371f02728bef56`.

The comparison covers ring and cycle graphs, N245760, K6/K50, unit/analytic
weights, robust/no outputs and two-way clustering/full outputs, at one, eight
and sixteen threads. Each of 48 cells runs three rotated automatic/explicit
LSMR pairs. Setup, rejected probes, fitting, inference and output recovery are
all inside the measured command. Solver reports distinguish Schur-CG work from
explicit symmetric CG. The two new validation seeds remain unused.

## Findings

The maximum paired automatic/fastest-valid ratio among these two tested routes
is 1.617 at sixteen threads, 1.889 at eight and 2.096 at one. These are measured
penalties against a scientifically valid alternative, not proof that LSMR is
globally fastest among all solver families.

| Case with full outputs | T16 auto/LSMR | T8 auto/LSMR | T1 auto/LSMR |
| --- | ---: | ---: | ---: |
| Unit ring, K6 | 1.244 | 1.243 | 1.203 |
| Analytic-weight ring, K6 | 0.895 | 1.131 | 1.240 |
| Unit ring, K50 | 1.297 | 1.281 | 2.096 |
| Analytic-weight cycle, K6 | 1.477 | 1.808 | 1.930 |
| Analytic-weight cycle, K50 | 1.617 | 1.889 | 1.985 |

Ratios in this table are medians of paired command-time ratios. The automatic
route also wins several robust-only T16/T8 cells. Weight kind, width, output
requirements and thread count therefore matter; a blanket LSMR switch is not
supported. The current CMG bulk passes and wide Schur fallback products remain
kernel/finishing opportunities to assess before freezing new routing guards.

One T16 cycle/AW/K6 robust automatic repetition has a possible foreign-process
overlap. Its application and science remain recorded, but its automatic timing
family is excluded from ranking. No overlap was detected in either the T8 or T1
blocks. All timings remain exploratory three-pair results.

## Evidence

`evidence.zip` preserves the three plans, attempts, requests, numerical exports,
generated workers, input and sample hashes, timings, overlap records and solver
reports. Large output vectors stay local with explicit hashes and byte counts.
`checks.zip` contains all 288 independent inference checks and references to
144 output audits, plus the expansion plan and completion records. The complete
output-audit receipts are in `evidence.zip`.
`sources.zip` preserves the native identity and overlays, controller sources,
and the exact auditor version used by each block. The later auditor adds paired
ratios while retaining the original ratio-of-medians field. Its filtered lookup
preserves handling of failed unselected families in broader comparisons.

The source and archive hash maps distinguish original bytes from portable path
substitutions. Licensed logs and binaries are excluded. Earlier family failures
and the first twelve-cell exploration remain in the preceding finishing report.
No old receipt was overwritten, and no package was installed, pushed or released.

Broader solver-family comparisons, new kernel screens, routing validation on
untouched seeds, protected matrices, final blocks and SCC/Linux confirmation
remain open. This report does not close the optimization milestone.
