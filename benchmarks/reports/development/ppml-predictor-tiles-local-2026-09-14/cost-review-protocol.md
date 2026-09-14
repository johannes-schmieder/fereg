# Low FW cost review

The original low FW protected pair costs 142 ms/15.3%, with exact science,
modeled/native peaks and solver decisions. The unchanged finite/strict scopes
increase 74/64 ms; that observation is not explained by the small predictor
change. Preserve the pair. Run two same-input rechecks in reversed/forward order,
without different seeds or input transformations. Include these in the cost
review separately from the original primary three-seed aggregates. If cost
persists, diagnose before adoption or retain the established low-mode route.
Trace both low FW and weighted primaries with distinct diagnostic artifacts;
count predictor calls/rows and compare exact outputs to the original captures.
