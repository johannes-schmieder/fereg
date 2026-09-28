# Portable canonical proposal product screen

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 21/21 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| profiled-small | 0.849 / 0.906 / 1.005 | 1.484 / 1.775 / 2.053 |
| small-low | 0.705 / 0.869 / 0.994 | 1.464 / 1.465 / 1.605 |
| small-serial | 0.636 / 0.649 / 0.959 | 1.112 / 1.521 / 1.973 |
| larger-wide | 0.726 / 0.754 / 0.767 | 1.086 / 1.150 / 1.179 |
| excluded-three-fe | 0.987 / 0.992 / 1.116 | 0.484 / 0.520 / 0.527 |
| excluded-slope | 1.028 / 1.080 / 1.187 | NR |
| excluded-ols | 0.995 / 1.000 / 1.016 | 1.538 / 1.542 / 1.588 |

profiled-small: selected native baseline / candidate teams = 4 / 4.

small-low: selected native baseline / candidate teams = 4 / 4.

small-serial: selected native baseline / candidate teams = 1 / 1.

larger-wide: selected native baseline / candidate teams = 8 / 8.

excluded-three-fe: selected native baseline / candidate teams = 8 / 8.

excluded-slope: selected native baseline / candidate teams = 4 / 4.

excluded-ols: selected native baseline / candidate teams = 8 / 8.

NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.

## Native thread scaling

| Lower → higher setting | Higher/lower min / median / max | Slower on every repetition |
|---|---:|---|

Independent-oracle failures: 0.

Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.

All **63 applications** and **21 native scientific pairs** pass on candidate
snapshot `4aa44429…` / plugin `d1156e6d…`, versus accepted baseline `8e2d5af2…`.
Eighteen default-stock scientific pairs pass; all three slope comparisons fail
the standard-error envelope and stay NR. The pinned unmodified stock library
is fixest 0.14.2. Robust PPML and IID OLS inference are included; no optional
saved observation outputs are requested. Native science gates include exact
sample, full covariance/objective, native rank/DoF and actual requested/effective
team counts. Licensed independent-oracle evidence is in the validation receipts;
no new independent large-design oracle is claimed by these rotations.

The four targeted two-FE PPML paired medians improve **9.4% / 13.1% / 35.1% /
24.6%**. All low, serial and larger-wide pairs improve; small fast has one 0.5%
loss. Serial is variable, ranging 0.636–0.959 of baseline. Marginal baseline /
candidate / stock medians (ms) are **430/394/222** fast, **381/331/223** low,
**613/432/295** serial and **943/715/617** larger K12. Only the wider case's
**1.150** paired stock ratio meets the 1.20 ceiling; small cases still take
**1.775 / 1.465 / 1.521** of stock. None meets the 0.90 aspiration.

The excluded three-FE control has C/B median 0.992, with one 11.6% loss; OLS is
1.000, spanning0.995–1.016. Its stock ratio remains1.542. The slope control
instead loses in **all three pairs**, median **1.080**, range1.028–1.187. Native
coefficients and SEs agree exactly there. Default stock differs by up to
5.618e-9 on SEs despite passing coefficients, full-covariance and deviance gates;
the individual registered envelopes must all pass. Successful process exits
and the attractive raw stock clock do not create a scientific timing rank.

No numeric storage is added. Observed median candidate-minus-baseline process
peak changes are +4.93/+0.54/+2.57/+1.52 MiB on the four targeted cases,
+0.18 MiB three-FE, -1.51 MiB slopes and approximately zero OLS. These sampled
process peaks do not establish a change to native allocation bounds.

Job **7763067** has clean accounting and both rotation/wrapper sentinels.
Whole-node isolation and equal requests/affinities apply throughout. No compile,
profile, archive or competing fit overlaps the screen. The candidate remains
unchanged for fresh confirmation and nine alternating slope-control pairs;
there is no acceptance yet. Do not fit a guard to the losing control or tune
thresholds from this screen.
