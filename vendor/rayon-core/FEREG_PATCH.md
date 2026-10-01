# Pinned Rayon runtime patch

Unmodified registry source is identified file by file in FEREG_ORIGIN.json,
including its crates.io archive digest and upstream commit. One production
change initializes each macOS worker sleep mutex/condition variable during pool
construction. macOS Rust 1.85.1 otherwise allocates their pthread storage on first
use, potentially inside an already warmed PPML iteration window. Worker broadcast
does not guarantee every worker sleeps. Linux/Windows scheduling is unchanged.

No sleep, work-stealing, waking, numerical or public API policy changes. The
existing per-worker runtime allowance covers this state; initialization moves
allocation earlier rather than adding state. See ADR0244 and its source-bound
allocation trace. The original licenses and upstream tests are retained.

Heap-job execution also moves the payload out and frees its container before
invoking it. A scoped payload can signal completion before returning; retaining
the container until after invocation lets the host observe a completed job's
allocation after the scope joins. The independent scoped-call allocator
regression captures that race with a 16,384-byte inline payload (16,400 bytes
including references), checking successful and canceled calls at one/eight
workers. Payload execution, panic handling, scheduling policy and numerical work are
unchanged. No new allocation, runtime state or memory allowance is added.
