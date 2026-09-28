# ADR 0242: Reject general OLS host-processor scoping

Status: accepted development rejection, 2026-09-28.

## Decision

Do not generalize ADR0241's PPML host-processor scope to public OLS. Restore
`fereg.ado`, the Break controller and OLS Break fixture exactly to 4ddf7aaa; preserve
the rejected patch and added candidate-only fixture in the development report.
The accepted PPML scope remains unchanged. No categorical-only or benchmark-
specific guard is adopted from this experiment.

## Evidence

[The source-bound report](../../benchmarks/reports/development/ols-public-host-scc-2026-09-28/README.md)
binds candidate 72448897 and baseline 14c753ed to identical native artifacts.
Mac and SCC tests pass host/default/explicit-thread restoration, quiet/noisy,
transaction rollback, genuine Ctrl-C, saved-output and independent weighted
three-FE/slopes oracles, with unchanged PPML controls. All 222 timed applications
and 74 native pairs pass; all 74 native b/V pairs are byte-identical. Recorded
iterations agree and no native memory plan or numerical buffer changes.

Wide and low-memory OLS and multiple categorical FE cases improve, including
fresh 25.6% wide and 20.0% low gains. But fresh slopes lose 7.0%, then 15.1% in one
separately registered six-pair unchanged-source confirmation, with a 50% worst
pair. Wide's confirmation gain 31.9% cannot cancel that regression. Known slopes
also show mixed signs. The general candidate fails the adoption decision.
Neither pooling blocks nor adding favorable-case guards resolves its cause.

Five three-FE stock pairs remain unranked: coefficients and RSS pass but the
reported residual DoF differ. Their stated DoF factors explain the covariance
gaps to<8e-15; this is a readonly policy diagnostic, not changed eligibility,
a full-rank oracle or repair of earlier PPML discrepancies. Future comparison
work must establish matched inference rules before timing.

## Consequences

The restored production package remains exactly 14c753ed (accepted 4ddf7aaa), with
its PPML benefit and existing OLS losses. No rejected-candidate timing is a claim
about retained OLS. Runtime telemetry places the slope reversal inside native
computation; setting and display overhead alone do not explain it. Investigate
caller/team handoffs and native scheduling with unchanged scientific work before
another host-scope candidate. Do not remove guards, reduce certificates or change
native thread meaning on the strength of this rejected experiment. All evidence
is bounded development evidence; static allocation drift and qualification gaps
remain open.
