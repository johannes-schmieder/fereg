# CMG hierarchy-guard post-change controls

This is focused development evidence, not qualification.

- The matrix contains 18 recorded runs: two datasets, three solver arms, and
  exactly three repetitions per arm. All 18 runs succeeded, certified, and
  matched on exported coefficients and RSS.
- On the one-large-plus-tiny graph, the hierarchy had an iterative terminal
  with `W = 60.01`. Auto used LSMR in all three repetitions with no fallback,
  warning, or MAP pilot. Median command time was 0.096 seconds, versus 0.113
  for explicit Schwarz-LSMR and 0.970 for explicit CMG.
- On the all-tiny graph, the hierarchy also had an iterative terminal but
  `W = 2.30`. Auto retained CMG in all three repetitions with no fallback,
  warning, or MAP pilot. Median command time was 0.046 seconds, equal to
  explicit CMG and below explicit Schwarz-LSMR's 0.095 seconds.
- Explicit `solver(cmg)` remained available on both graph shapes.

The development plugin SHA-256 was
`ff8592ad6d0b3ad9b00027e5de898ab08bceb6d87a1a4d73e8344d341afbc680`.
