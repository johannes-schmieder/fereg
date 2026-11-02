use super::canonical_retention_bound;

#[test]
fn retention_bound_is_one_sided_at_extremes_and_equality() {
    assert!(canonical_retention_bound(-4.0, 4.0, 4.0, 1_000_000, 1e-9));
    for scale in [f64::MIN_POSITIVE, 1e-200, 1.0, 1e150] {
        assert!(canonical_retention_bound(-scale, scale, scale, 1024, 1e-9));
    }
    let tiny = f64::from_bits(1);
    assert!(!canonical_retention_bound(0.0, tiny, tiny, 2, 0.0));
    assert!(!canonical_retention_bound(
        1.0,
        f64::from_bits(1.0f64.to_bits() + 1),
        1.0,
        2,
        1e-9
    ));
    assert!(!canonical_retention_bound(
        -f64::MAX,
        f64::MAX,
        f64::MAX,
        2,
        0.0
    ));
    assert!(!canonical_retention_bound(2.0, 2.0, 2.0, 1024, 0.0));
    assert!(!canonical_retention_bound(0.0, 0.0, 0.0, 1024, 0.0));
    assert!(!canonical_retention_bound(
        -1.0,
        1.0,
        1.0,
        (1usize << 53) + 1,
        0.0
    ));
    let down = |v: f64| f64::from_bits(v.to_bits() - 1);
    let ratio = down(0.5 * down(2.0)) / 32.0;
    let boundary = ratio * ratio;
    assert!(!canonical_retention_bound(-1.0, 1.0, 1.0, 1024, boundary));
    assert!(canonical_retention_bound(
        -1.0,
        1.0,
        1.0,
        1024,
        down(boundary)
    ));
}
