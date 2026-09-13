# Functional harness adjudication before paired cost controls

The original N30720 full comparison passed science and stopped on exact modeled
selected bytes: every modeled phase and fast/low/selected total differs by +576
bytes. Native peak is exact and admitted in both arms. The unchanged plugin
snapshots actual embedding live bytes before core entry (ppml_estimate.rs), and
memory.rs adds that dynamic input equally to every phase. This precedes the
changed result validator. The particular allocation is not traced. Record this
constant offset explicitly instead of treating a measured embedding input as a
pure-shape constant; all scientific exactness checks remain. Cost controls still
use the original exact modeled-byte comparator unless separately adjudicated.

The independent oracle originally asserted reported finite rank equals 257
identified slopes. The existing result contract explicitly adds its normalized
structural constant: finite rank 258, sandwich rank 257. Correct that harness
assertion and independently verify kept/status/zero covariance, nested FE counts
and the unchanged reported ranks. Preserve the initial script and failed log.
This does not change a scientific policy, model input, tolerance or output.
