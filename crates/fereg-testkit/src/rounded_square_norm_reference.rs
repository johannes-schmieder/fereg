//! Independent twofold square-sum norm for bounded, already-rounded elements.
//! Unlike the production norm, this forms exact square-product expansions and
//! accumulates them without scale recurrences, row leaves, or norm summaries.

fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let sum = a + b;
    assert!(sum.is_finite());
    let b_virtual = sum - a;
    let error = (a - (sum - b_virtual)) + (b - b_virtual);
    (sum, error)
}

pub fn rounded_element_norm(values: impl Iterator<Item = f64>) -> f64 {
    let mut high = 0.0;
    let mut low = 0.0;
    for value in values {
        assert!(value.is_finite());
        let square = value * value;
        // This witness intentionally covers the bounded handoff fixture, not
        // extreme square underflow/overflow. Those have separate E1 witnesses.
        assert!(value == 0.0 || (square.is_normal() && square.is_finite()));
        let product_error = value.mul_add(value, -square);
        for term in [square, product_error] {
            let (sum, error) = two_sum(high, term);
            (high, low) = two_sum(sum, low + error);
        }
    }
    (high + low).sqrt()
}
