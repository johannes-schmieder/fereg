# Serial LSMR window: isolated development experiment

This experiment bounds serial LSMR work to one RHS column at a time. Transport
and memory admission use the same enforced bound; final output banks remain
fully charged. The preconditioner, arithmetic within each RHS, tolerances and
fresh certificates are unchanged. This is captured-source evidence, not
production or shared/PPML routing acceptance.

All 26 application attempts pass. The four-cell, three-pair native screen has
no detected competing work. All twelve cross-artifact pairs have bit-identical
finite exports, samples and saved vectors. Eight independent inference checks,
twenty-four exact within-artifact repeats and twelve candidate output audits
pass. Five focused batch tests and 63 memory tests pass, including exact
upstream comparisons across widths 1/2/12/52/67 and preconditioners, and failure
without returning a partial batch.

| K50, clustered/full outputs | wr2 median | sw1 median | Paired sw1/wr2 |
|---|---:|---:|---:|
| Unit ring, T1 | 5.635 s | 5.648 s | 1.0011 |
| Unit degree-two, T1 | 3.623 s | 3.628 s | 1.0011 |
| AW degree-two, T1 | 3.828 s | 3.840 s | 1.0092 |
| Unit ring, unchanged T16 control | 1.275 s | 1.245 s | 0.9765 |

The serial tracked allocation peaks fall from 645,481,767 to 523,613,047 bytes
for the ring, from 705,032,279 to 569,466,535 for unit degree-two, and from
708,964,451 to 575,364,787 for AW degree-two. Threaded peaks vary despite the
unchanged threaded route and are retained without claiming an improvement.

The serial ring completes with both 1.5 GB and 1 GB usable memory, retaining
the default automatic preconditioner. Both calls pass independent inference
and output checks, and both tracked peaks are 523,613,047 bytes. Their traced
diagnostic times are 5.762/5.745 seconds; these are not paired timings. The
earlier wr2 timeout at 1.5 GB remains in the prior immutable report. The initial
memory-output auditor used a mismatched run-name glob and failed before finding
any records; its source and failure are preserved beside the corrected audit.

The result supports a memory improvement without an unconstrained speed gain.
An OLS-scoped implementation and cross-platform routing evidence remain open;
the generic experimental memory rule is not present in production source.

`wr2` is plugin
`2b956b420b141b2d5c1ddb119451f48e69fa50510ab77f7ff7ee52c202f23b03`;
`sw1` is
`96c4f0e302e1d4675fe8114aa68ec6988d01ad94f30de65ae5e98597c629eb40`.
Both build and pass the actual ABI fixture. The portable archives preserve
measurement identities, controllers, numerical exports, audits and complete
captured-source differences, including instrumented files beyond checkout
overlays. `verify.py` verifies 860 stored entries, 26 omitted local observation
arrays and both complete source reconstructions. Raw licensed logs and
binaries remain local. No installation, push or qualification occurred.
