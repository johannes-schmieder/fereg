//! The explicit-dummy OLS oracle's own self-tests live in
//! `crates/fereg-oracle/tests/ols.rs`; this file keeps the plugin boundary check.
use fereg_testkit::run_action;

#[test]
fn public_ols_dispatch_requires_the_live_stata_spi_boundary() {
    let capture = run_action(&["estimate-ols"]);
    assert_eq!(capture.return_code, 498);
    assert!(capture.error.contains("requires a live Stata SPI session"));
}
