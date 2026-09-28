# Schwarz topology: remaining admission overestimate

This source audit reviews the queued imbalance-aware scratch candidate without
changing it, the accepted estimator or either deployment. It identifies a larger
remaining opportunity and the ownership/API boundary needed to pursue it. It is
not runtime attribution, an implemented route or a measured memory reduction.

## The large degree-two graph has one subdomain

For the frozen unweighted `strong_d2` generator, let F=N/80 be the number of firms.
There are 40F workers, each observed twice. In layer zero, worker j connects firms
j and (j+1) mod F. These F workers form a cycle joining every firm. Every other
worker connects two firms already in that component. Therefore the full bipartite
graph is connected for every registered seed; no row or FE level is removed in
this generated nonmissing, nonsingleton OLS case.

The pinned `within` categorical builder creates one channel pair and splits it by
bipartite components, so this design has exactly one Schwarz subdomain. The
executor invokes each subdomain once per apply. Its one subdomain can borrow only
one buffer from that apply's `WorkerBufferStack`. Nested Rayon work may enter a
different RHS apply, but that call owns a separately charged top-level object.
It cannot create a second subdomain invocation in this one-subdomain apply.
The returned buffer is reused on later calls. Both accumulation strategies fit
the existing parallel-reduction payload bound.

Four small connected graph checks and four deliberately disconnected controls
pass in `check.py`; they supplement the analytical cycle argument. No large
design, regression, factorization or benchmark was run. `source-bindings.json`
binds the generator, current fereg callers/model and ten pinned dependency files.

For N3,932,160/K10/T4, holding the existing four cached objects, eleven concurrent
RHS applies and descriptor charges fixed gives:

| Scratch subset | Banks per apply | Four cached + eleven live applies |
|---|---:|---:|
| Current generic bound | 205 | 316.435 GiB |
| Queued imbalance-aware candidate | 205 | 140.764 GiB |
| One-subdomain bound for this graph | 1 | 0.687 GiB |

These are **scalar upper bounds on one storage subset**, not full admission or
actual RSS. The topology-specific result explains why tightening only local
vector dimensions can leave a large overestimate. It does not establish which
route a timed-out fit took; job 7767269 must still provide that runtime evidence.
Do not route by benchmark name, seed or this generator-specific proof.

## General implementation boundary

The generic `schwarz-precond::SchwarzPreconditioner` exposes `subdomains()` and
each entry's `scratch_size()`. However, fereg owns `within::Solver`, whose public
preconditioner wraps those entries in a private `Variant::Additive(FeSchwarz)`.
The public handle exposes operator dimensions/configuration and build duration,
but no typed subdomain or scratch inventory. Its Debug display includes a count;
parsing that display would be unsuitable for production memory admission.

Fereg currently admits the complete solve before `within::Solver::new`, and the
shared construction estimate includes four potentially retained *future* apply
pools. The dependency constructor initializes its `BufferPool` empty; numeric
apply buffers are allocated on the first apply. Thus a general successor could
separate construction admission from certified post-build apply admission, using
a typed current-operator inventory. This requires reviewed metadata access or
equivalent independently computed topology, plus changes to the caller's staged
ownership; the existing public API alone is insufficient.

Any such implementation must admit all construction allocations first, bind
metadata to the actual weighted operator, and admit all live recurrences/RHS
applies, retained pools and postprocessing before the first apply. On refusal,
release the constructed operator before fallback and charge the failed attempt's
peak. Rebuilt PPML geometry must invalidate its prior inventory. Signed covers,
disconnected components, failures, concurrent RHSs, exact budget boundaries and
existing scientific certificates remain controls. A connected whole multi-FE
graph does not establish connectedness of every pair subdomain.

Finish the already queued candidate and controlled-budget diagnostic first. If
the broader inventory confirms material constrained-memory or scaling losses,
this evidence favors topology-aware staged admission over more scalar threshold
tuning. No additional candidate, job, dependency patch or production change was
introduced by this audit. The accepted 259-file estimator snapshot is unchanged.
