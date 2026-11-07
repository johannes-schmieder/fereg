//! Test-only comparison helpers.

/// Assert two values print identically except that floating-point literals
/// may differ by `relative` times the larger magnitude (at least one). Used
/// where two provenances share every discrete outcome but sum their norms in
/// different fixed orders.
pub(crate) fn assert_debug_close<T: std::fmt::Debug>(actual: &T, expected: &T, relative: f64) {
    let (a, b) = (format!("{actual:?}"), format!("{expected:?}"));
    let split = |text: &str| -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        for ch in text.chars() {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '+' | '_') {
                current.push(ch);
            } else {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                tokens.push(ch.to_string());
            }
        }
        if !current.is_empty() {
            tokens.push(current);
        }
        tokens
    };
    let (ta, tb) = (split(&a), split(&b));
    assert_eq!(ta.len(), tb.len(), "structures differ:\n{a}\n{b}");
    for (x, y) in ta.iter().zip(&tb) {
        if x == y {
            continue;
        }
        match (x.parse::<f64>(), y.parse::<f64>()) {
            (Ok(p), Ok(q)) if x.contains(['.', 'e']) || y.contains(['.', 'e']) => {
                let scale = p.abs().max(q.abs()).max(1.0);
                assert!(
                    (p - q).abs() <= relative * scale,
                    "{x} != {y} beyond {relative:e}:\n{a}\n{b}"
                );
            }
            _ => panic!("token {x} != {y}:\n{a}\n{b}"),
        }
    }
}
