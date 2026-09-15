# Completed OLS commands and unresolved residual comparison

Source `06609452bab13516a84a48363014c6685482f7d4`, Linux plugin
`83d27773042bea0f6096d4f27eef21b1f08b4a449f8caf0dcd4ca72e723af550`.
The unchanged deterministic 5% worker sample has 4,553,489 inputs and 4,361,733
retained rows after 191,756 singleton drops. Both arms use four application
threads, two alternating fresh-process pairs on scc-ei3, native fast/auto,
conventional VCE and logrwage2. Private pinned package setup precedes both
complete-command timers. No raw observations or licensed logs are committed.

| Estimator | First command (s) | Second command (s) | Median (s) | Process peak RSS (KiB), first/second |
| --- | ---: | ---: | ---: | ---: |
| fereg | 293.338 | 301.054 | 297.196 | 4,312,712 / 4,312,580 |
| reghdfe | 1,315.637 | 1,259.514 | 1,287.5755 | 3,587,880 / 3,588,028 |

All four process/FIT_RC/STATA_RC checks and completion receipts pass, and the
manifest records all four commands complete. The following comparison fails;
job 7580504 therefore ends failed=0/exit=1 after 3,189 s, maxvmem 4.764G.
Its original failed summary is preserved. The descriptive median timing ratio
is 4.332; this is not an accepted performance or qualification claim.

A separate 12-second compute-node diagnostic (7580896, failed=0/exit=0,
853.215M maxvmem) reads the saved outputs without re-estimation. It uses the
unchanged numerical comparison helpers, thresholds and models. Both pairs have
exact raw rows, order, sample and DoF; coefficients and VCE pass, with maximum
absolute differences 9.9425e-13 and 5.9166e-19. The failing test is the saved
residual envelope: maximum absolute difference 6.3657e-5, maximum envelope ratio
63,254.6, and 699,592 rows outside the registered pointwise envelope in each
pair. The L2 difference is 2.5343e-4. Replayed RSS is 79,135.31456255841 native
and 79,135.31456262042 reference.

Independent finite/FE normal-equation checks pass for both. Native maximum
normalized finite/FE scores are 2.0011e-15 / 2.8683e-16, versus reference
2.3854e-11 / 2.8813e-10. Smaller native scores suggest reference stopping
accuracy as a possibility, but they do not prove prediction feasibility or
identify which residual is accurate. The independent model-space oracle was
not reached by the original comparison and is not replaced by this diagnostic.
The comparison remains unaccepted; no tolerance or adjudication is changed.
The special large-auto-LSMR residual adjudication is not applied to this
published CG route merely because it appears in the scientific contracts.

Native runtime input/native seconds are 0.729/289.525 and 0.749/297.205.
Both design and residual absorption report CG (code4), 221/254 iterations,
and fallback_used=true with one warning each. The retained stage reports
include setup and solve costs; separate failed-attempt cost is not identified.
All four processes report zero swaps; native scratch is zero.

An additional OLS memory-model gap is observed in both native runs: tracked
allocation delta 3,570,575,392 bytes versus selected model 2,841,380,024 bytes,
reported ratio 1.256634. Observed native allocation exceeds the model
by about 25.7%; this is separate from process RSS and the repaired PPML model.
The 40 GiB cap has headroom, so these runs do not demonstrate low-cap admission
safety. Preserve this as an unresolved modeling issue, not accepted memory
coverage.

All SCC jobs in this comparison are terminal. The full local ppmlhdfe reference
continues separately; no local build or numerical audit overlaps its timing.
The 15-minute monitor continues that reference. The PPML 5% model-space audit
failure in the sibling continuation report remains unresolved. No estimator
was rerun, no installed binary changed, and no stopped campaign was restarted.
