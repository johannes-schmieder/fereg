# Focused PPML two-way control recheck

The initial seed15485863 control rises .345 to .388 seconds (+43 ms, 12.5%), beyond max(10 ms,5%). Inference improves .009389 to .008254, while finite work rises .034769 to .059918 and strict/final work .090994 to .106859. Source and scientific outputs/solver decisions are exact. This does not establish the cause of the regression.

Run exactly three fresh seeds (32452843,49979687,67867967), rotated arm order, with public traces and unchanged specification. Preserve the initial control and report this tier separately. Investigate/reject if the control loss repeats beyond the same threshold. Do not infer acceptance from a faster inference kernel alone. No other experiment runs concurrently with these timings.
