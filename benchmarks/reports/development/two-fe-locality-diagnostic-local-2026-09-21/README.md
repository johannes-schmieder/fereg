# Two-step graph locality diagnostic

Sixty descriptive measurements cover twenty captured N=245,760/K6 training
inputs: ten graph families, unit/moderate weights and three fixed sampling
seeds. Each measurement samples up to 512 left groups and records repeated
right-neighbor pairs, weighted concentration and component sizes. Input column
hashes are checked before calculation. The two fresh routing seeds are unused.

This diagnostic does not select or accept a solver rule. Ring and cycle inputs
have concentrated local adjacency, but weak-d3 inputs appear even more local
while the measured existing automatic route already wins there. Dense inputs
at this size can also have low effective-pair fractions. Component accounting
distinguishes disconnected dense blocks from a connected ring. Locality alone
therefore cannot justify switching to LSMR.

Collision-based effective-pair counts are noisy sample estimates and may
exceed the available pair count. Weighted effective counts describe only the
observed sample; they are not estimates of full-graph weighted support. Groups
with more than 64 distinct neighbors would be reported as skipped. No such
group is skipped in these inputs. No benchmark graph or tolerance is changed.

`graph-locality2.json` binds each measured input and the tracked diagnostic
source. The original unmatched-glob invocation failure is preserved separately
with its original and portable hashes. This is exploratory development evidence,
not routing acceptance or a performance claim.
