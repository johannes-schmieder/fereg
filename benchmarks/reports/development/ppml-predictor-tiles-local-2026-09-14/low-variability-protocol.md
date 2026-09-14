# Fixed low-FW A/A and A/B variability review

The original untraced low-FW costs are +142, +171 and +3 ms. Separate predictor
diagnostics reproduce exact science and save about 2 ms in predictor construction;
the remaining finite/strict I/O work is unchanged in source. Its extra complete
cost remains unexplained. Do not discard those observations or change the input.

Run six A/A pairs (the accepted plugin in both arms) interleaved with six A/B
pairs (the original accepted/candidate plugins), on the original full and warm
data. Alternate arm order in each stratum; run all twelve pairs irrespective of
the intermediate result, unless a scientific or execution failure stops the run.
No profiling hooks, compilation, numerical oracle work or compression overlaps.

Report each pair, A/A and A/B medians/ranges and all nine original-plus-new A/B
pairs. The existing protected cost gate remains both 5% and 10 ms. If either the
new six-pair or all-nine A/B median breaches that gate, reject the standalone
prototype pending a demonstrated cause/fix. Otherwise evaluate remaining individual
flags against A/A variability and changed-phase evidence, without treating absence
of a median flag as proof of zero overhead or claiming low-FW speed improvement.
No seed-specific or memory-mode exclusion will be added to make the check pass.
