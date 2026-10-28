//! Column bands for the wide raw-moment cache. Every band keeps each column's
//! serial row-order arithmetic (its maximum, scaled square sum and
//! compensated mean), so the band width only shares row loads and weight
//! reads; it is a resource choice, never a numerical one.
use super::*;

/// Fill the wide cache with the widest band that still gives every worker a
/// task. Narrower bands remain for teams with more workers than bands.
pub(super) fn fill_wide(
    raw: &DenseMatrix,
    weights: NormalizedWeights<'_>,
    weight_sum: f64,
    scales: &mut [f64],
    means: &mut [f64],
    norms: &mut [f64],
) -> Result<()> {
    let threads = rayon::current_num_threads();
    let fills_team = |band: usize| raw.ncols().div_ceil(band) >= threads;
    let fill = if fills_team(64) {
        fill_raw_moment_cache_blocked::<64>
    } else if fills_team(32) {
        fill_raw_moment_cache_blocked::<32>
    } else if fills_team(16) {
        fill_raw_moment_cache_blocked::<16>
    } else if fills_team(8) {
        fill_raw_moment_cache_blocked::<8>
    } else {
        fill_raw_moment_cache_blocked::<1>
    };
    fill(raw, weights, weight_sum, scales, means, norms)
}
