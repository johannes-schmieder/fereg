# PPML generation-zero FE bounds: local development evidence

Parent source is `c0c6a2c`, parent plugin `fdaf3e16`; candidate plugin is
`6181be78e7d3`. No full PPML fit or release qualification is claimed.

## Change and proof

PPML refines FE cardinality before generation-zero admission. Tuple runs give
a valid upper bound in arbitrary row order. When that bound is loose, sufficiently
large bounded-integer sources use exact disjoint value-window bitmap counts.
The bitmap is a fixed 256 KiB stack allocation, independent of N and level count;
there are at most 128 passes per source. The count caps at active N, and interaction
bounds use capped products. Unsupported or wider domains keep the run/N bound.
Classification/error precedence executes first; excluded rows can only widen the
bound. No row mask, group IDs, observation-sized allocation, SPI worker call,
post-admission route reselection, headroom change or scientific tolerance change.
Model-sized borrowed source views are fallible and temporary. Private memory
tracing records dimensions and itemized phase/capacity inventories outside the
planner allocator observation interval; it is off in normal commands.

The frozen envelope binds after selection and later topology construction.
The 1,024 group-oracle comparisons cover order, subset restrictions, interactions
and slopes. Integer tests independently count exact sets across value-window and
word boundaries, signed/zero values, bounded/wider domains, caps and interruption.
All 354 PPML/separation and 97 plugin tests pass, as do universal binary/ARM64 ABI
checks and 26 licensed native fits including weak weighted graphs, FW/PW, clusters,
singletons and fast/low/pool controls. These are focused development tests.

## Full input memory diagnostic

A deliberate one-byte cap rejects both probes before fit workspace allocation;
it does not measure an OOM. Original N is 91,045,290. Planner allocation event,
current and peak growth remain zero. Fixed stack use is separate from these heap
counters. Input ownership is charged once against availability sampled before
native input loading.

Tuple runs alone are ineffective on this input order: fast remains 184.57 GB.
Exact integer counts reduce fast to **72.69 GB**, low/pool4 to **55.93 GB**, and
low/pool1 to **50.28 GB**. The preceding adopted estimate was 184.58/88.41/68.74 GB.
Pool4 diagnostic command time rises 16.913→25.824 s, pool1 15.425→24.498 s.
These are single sequential diagnostics including input loading, not paired
full-fit timings. Actual fast RSS and the additional selected solver admission
remain separate checks. Neither a proportional RSS extrapolation nor a reduced
safety margin authorizes fast mode.

## Paired complete commands

Three alternating pairs per case, eight native threads, serial runs. Both arms
use identical input and outcome; only candidate memory preparation differs.
The auto case imposes 676,870,912 available bytes in both arms (140 MB usable
after unchanged headroom). Baseline admits low; candidate admits fast.

| Case | Parent median | Candidate median | Interpretation |
|---|---:|---:|---|
| German 65,503 retained, explicit low | 2.218 s | 2.231 s | +0.6%; preparation +4.21 ms |
| Same sample, constrained auto | 2.240 s | 1.381 s | −38.3%; fast admitted |
| Narrow 512-row low control | 0.088 s | 0.089 s | +1 ms |

Auto gains occur in projection, finite solve, strict work and inference;
preparation increases about 4.65 ms. Auto native peak rises 40.00→58.00 MB;
median process maximum RSS rises 103.22→120.39 MB; scratch falls 9.43 MB→zero.
Explicit-low native/scratch peaks are unchanged, while median process RSS rises
103.45→115.28 MB; this process RSS difference is disclosed and not attributed to
the 256 KiB stack alone. All solver fallback counts remain zero. Per-phase costs,
planned/observed memory and pair differences are in paired-results.json.
Samples/order/status/DoF/drop counts agree exactly; b/V and predicted products
pass registered envelopes. Candidate means also pass independent score and
fixed-coefficient model-space checks. The failed Python environment import and
successful existing scientific environment rerun are preserved locally.

## Remaining work

The consistency-loop repair is separate. Full fast/auto needs its complete
solver workspace admission and empirical certificates before any full-scale
performance conclusion. No installed binary replacement, cluster work or push.
