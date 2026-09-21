# Two-way OLS: existing whole-action memory forecasts

This bounded local diagnostic exports the existing public `e(fereg_memory)`
matrix after timing in twenty-one controls: fourteen early-estimate ratio
exceedances and seven measured peak increases. One baseline/candidate pair
per cell gives 42 successful calls. The binaries, input semantics and timed
worker body are unchanged. The post-timing export patch, original worker,
controller identity and predeclared selection are retained.

All observed native peaks are within 5.9 percent of the existing whole-action
forecast: maximum observed/forecast ratio **1.05827**. All 42 reports use
schema 2 and `phase-capacity-calibrated-v2`. The earlier maximum ratio of
3.28144 used the narrower historical selected-memory estimate. That estimate
and every earlier observation remain unchanged in the prior control report.
No prediction was fitted to these new observations and no estimator or model
was changed.

The scientific bundles and all requested output vectors are bitwise identical
to the corresponding earlier calls. Each candidate-minus-baseline allocation
peak difference is exactly the earlier difference. Both low-memory arms rise
by 46 bytes relative to their earlier observations; this does not change their
paired difference. The seven prior increases remain: up to 5,111,856 bytes for
FW K=1, 319,536 bytes for the AW K=1 boundary and 72 bytes for protected one-FE
K=1 calls. This is not a memory-neutral change claim.

The public report still exposes strict proof-gap bits **125 or 381**. These
cover transport/compiler, construction, numerical frames, retained FE parents,
dependency containers and worker runtime; 381 additionally includes recovery.
Calibration status is separate from a complete allocation proof. This follow-up
supports the bounded development comparison but does not close the release
memory-proof gaps or constitute cross-platform qualification.

`memory-audit.json` retains all defined flags, every public matrix value,
method, forecast, observed peak, early ratio, previous observation, input hash
and scientific comparison. `plan.json` binds the exact earlier control evidence.
`instrumentation/` and `controller/` preserve the actual export change;
`measurements.json` and source identities bind all calls. The executed audit
and driver are included. Raw licensed logs and machine-specific paths are
excluded.
