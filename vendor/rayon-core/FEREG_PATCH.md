# Pinned Rayon runtime patch

Unmodified registry source is identified file by file in FEREG_ORIGIN.json,
including its crates.io archive digest and upstream commit. The only production
change initializes each macOS worker sleep mutex/condition variable during pool
construction. macOS Rust 1.85.1 otherwise allocates their pthread storage on first
use, potentially inside an already warmed PPML iteration window. Worker broadcast
does not guarantee every worker sleeps. Linux/Windows scheduling is unchanged.

No sleep, work-stealing, waking, numerical or public API policy changes. The
existing per-worker runtime allowance covers this state; initialization moves
allocation earlier rather than adding state. See ADR0244 and its source-bound
allocation trace. The original licenses and upstream tests are retained.
