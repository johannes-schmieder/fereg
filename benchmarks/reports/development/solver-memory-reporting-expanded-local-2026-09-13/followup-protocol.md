# Prospective wide weak OLS follow-up

The first reporting comparison has three wide weak OLS command changes of
+49/+9/+51 ms; the third exceeds max(10 ms, 5%). Native differences are
+50.83/+7.37/+50.11 ms. Input/outer work does not explain the two larger losses.
All three have exact scientific exports and non-time solver decisions.

Before more timing, prescribe three identical accepted-binary A/A pairs
interleaved with three accepted/candidate A/B pairs on that same workload.
Use fresh seeds 15485863, 67867967 and 49979687, alternating A/A/A/B order and
arm order, fresh Stata processes and no optional profiling. Use the original
max(10 ms, 5%) review boundary and retain each observation separately. Require
matching artifact hashes and exact selected-model bytes for A/A.

This diagnoses whether the command losses persist; it neither changes the
original disposition nor proves their cause. It must not be used to call the
unfinished reporting patch adopted or to claim a speedup. Further regression
or material attribution uncertainty remains an open item.
