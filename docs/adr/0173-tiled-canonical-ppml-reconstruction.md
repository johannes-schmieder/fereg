# ADR 0173: bounded row reconstruction for canonical PPML products

Status: accepted for local development, 2026-09-24. Not qualification.

## Decision

Extend the existing low two-categorical-FE canonical product engine with bounded
row reconstruction when the full eliminated-group coefficient bank cannot fit.
Prefer retained centers when all overlapping banks fit; otherwise prefer the
existing full reconstruction after certification when it fits. Only then use
tiled reconstruction. These are storage lifetimes for the same grounded Schur
solve and independent original-input certificate, not different solvers.

The complete plan still fits within the existing two N-by-pool projection
reservations. Neither the outer memory model nor work-admission allowance grows.
A call refused by factor work, RHS/certificate reductions, numerical guards or
its complete memory plan keeps the previous certified route. The change applies
only at admitted low two-categorical-FE PPML canonical checkpoints. Ordinary
joint directions, fast mode, slopes, other FE counts and OLS retain their engines.

## Ownership and acceptance

After the independent original-input certificate, a tiled result owns immutable
normalized current weights, eliminated-group masses and solved retained-group
coefficients. It reconstructs the within response using one temporary
eliminated-group response bank. That bank ends before the row-tile bank exists.

Before each scratch write, enumerate eliminated groups represented by the next
rows and recompute only their coefficients from unchanged original columns,
weights, masses and retained coefficients. Use exactly the same pure helper and
arithmetic order as the original-input certificate. Encoding then reads this
prepared immutable bank in parallel. A group appearing in multiple tiles may
be recomputed; it must reproduce the same coefficient. No weighted state escapes
the current canonical call or survives a generation change.

The scratch writer accepts a bounded preparation state, runs preparation before
each encoded tile, and writes each cell once without reading old scratch.
Preparation, nonfinite-value or I/O failure poisons the private file. A poisoned
file refuses readers and further writes and is removed on drop. Nothing is
published until the existing full transaction accepts.

No acceptance tolerance or checking schedule changes. The unchanged original
FE residual moments, direct residual Gram and norms remain prerequisites.
Canonical rank, finite solve, inverse/span and reconstruction checks remain;
outer scores/objective, strict accepted-point verification, final current-weight
Hessian, VCE, weights, samples, separation and DoF are unchanged. Neither factor
residuals nor earlier weight frames substitute for these independent checks.

## Bounded storage

Let W include response, P be eliminated levels, Q retained levels, and N rows.
Set R=min(N,max(1,floor(8192/W))) and C=min(P,R). This fixes an approximately
64 KiB group-coefficient tile; it is an output working-set size, not a numerical
or benchmark admission threshold. The scratch writer may shorten R to its
already-owned encoding-buffer capacity. No second scratch file or N-by-K design
is introduced.

The tiled reconstruction payload, before unchanged binding/fixed overhead, is:

    max(8[2N + P + QW + 3W + W² + CW] + sizeof(usize)[P + C],
        8[2N + 2P + QW + 3W + W²])

The second term separately covers the temporary response coefficient bank,
including platforms where an index occupies fewer bytes than a double. The
ordinary setup, solve and independent certificate peaks are unchanged. Minimum
admission is the smaller of complete streamed/full and streamed/tiled peaks;
the preferred layout must still fit its own complete peak. Every allocation
uses the existing fallible actual-capacity ledger; only dropped owners earn
credits. The fixed descriptor allowance also covers the tile owner.

Tiling retains an extra N-vector of weights through writing, a P-vector of
masses, a P-index lookup and bounded group/value banks in exchange for eliminating
the full PW allocation. Repeated group visits can cost more arithmetic and
smaller writes; hence affordable full-bank paths remain preferred. This is not
a claim of smaller RSS or repaired physical constrained-memory behavior.

## Evidence

The [source-bound report](../../benchmarks/reports/development/ppml-tiled-output-local-2026-09-24/README.md)
records exact artifacts, distributions, failures, memory and controls. The new independent
nonzero-residual WLS construction tests 144 controls, changing weights, T1/T8,
exact tiled/full output equality, direct Gram agreement and the one-byte memory
boundary. Scratch preparation failure has its own poisoned-file test. The
existing scale, rank, component, frame and strict rollback checks remain.

Five fresh rotations reduce N22560/K142/T8 from 894 to 735 ms and
N44880/K146/T8 from 1512 to 1198 ms; strict stock takes 685 and 1290 ms.
Every native pair improves in those cases. The newly admitted serial
N17280/K138/T1 case regresses 1320 to 1364 ms in every pair; stock takes
2761 ms. Adoption accepts this measured 3.3% tradeoff for the 18–21% threaded
gains and smaller serial tracked peak. No serial exclusion is fitted to that
fresh case. Already-admitted wide performance changes 567 to 570 ms; other
excluded/control medians and all losses remain visible. This is not a universal
10% advantage and does not close the broad PPML goal.

All 192 development/fresh/oracle public states, 128 baseline pairs and
64 direct stock pairs pass. Six fits additionally pass the independent
explicit-design Poisson/Hessian/sandwich/means oracle. Five licensed fixtures
and 53 distinct core tests pass during development; the final source replays
all nine canonical tests and the prepared-tile failure test. All 24 OLS
applications and 12 output/science pairs pass, with medians within 1 ms.
Model and scratch sizes remain unchanged. Newly admitted median tracked peaks
change by +40,649 / -836,656 / -1,245,264 bytes. Physical memory-limit and RSS
qualification remain outside this local development milestone.
