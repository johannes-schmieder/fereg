//! Reusable numerical engines borrow one immutable fit-lifetime FE owner.
use crate::fe::FeDesign;

/// The nonempty effect bank owns all codes, memberships and slope values.
/// Comparing its allocation distinguishes substituted designs without a second
/// per-effect descriptor bank. The session must keep the original owner live:
/// an address stamp itself does not enforce a Rust lifetime or prevent ABA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FeTopologyIdentity {
    effects_address: usize,
    effects: usize,
    observations: usize,
    coefficients: usize,
}

impl FeTopologyIdentity {
    pub(super) fn capture(design: &FeDesign) -> Self {
        Self {
            effects_address: design.effects().as_ptr() as usize,
            effects: design.effects().len(),
            observations: design.nobs(),
            coefficients: design.total_coefficients(),
        }
    }

    pub(super) fn matches(self, design: &FeDesign) -> bool {
        self == Self::capture(design)
    }
}
