use crate::{ErrorCode, FeregError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Method {
    FixedEffects = 0,
    Simplex = 1,
    IteratedRectifier = 2,
    Mu = 3,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Methods(u8);
impl Default for Methods {
    fn default() -> Self {
        Self(0b0111)
    }
}
impl Methods {
    #[must_use]
    pub const fn none() -> Self {
        Self(0)
    }
    #[must_use]
    pub const fn only(method: Method) -> Self {
        Self(1 << method as u8)
    }
    #[must_use]
    pub const fn contains(self, method: Method) -> bool {
        self.0 & (1 << method as u8) != 0
    }
    pub fn iter(self) -> impl Iterator<Item = Method> {
        [
            Method::FixedEffects,
            Method::Simplex,
            Method::IteratedRectifier,
            Method::Mu,
        ]
        .into_iter()
        .filter(move |&method| self.contains(method))
    }
    pub fn parse(value: &str) -> Result<Self> {
        let mut methods = Self::none();
        let mut tokens = value.split_whitespace().peekable();
        if tokens.peek().is_none() {
            return Ok(Self::default());
        }
        while let Some(token) = tokens.next() {
            if token.eq_ignore_ascii_case("none") {
                if methods.0 != 0 || tokens.peek().is_some() {
                    return Err(syntax());
                }
                return Ok(Self::none());
            }
            let method = if token.eq_ignore_ascii_case("fe") {
                Method::FixedEffects
            } else if token.eq_ignore_ascii_case("simplex") {
                Method::Simplex
            } else if token.eq_ignore_ascii_case("ir") {
                Method::IteratedRectifier
            } else if token.eq_ignore_ascii_case("mu") {
                Method::Mu
            } else {
                return Err(syntax());
            };
            if methods.contains(method) {
                return Err(syntax());
            }
            methods.0 |= 1 << method as u8;
        }
        Ok(methods)
    }
}
fn syntax() -> FeregError {
    FeregError::new(
        ErrorCode::Syntax,
        "separation requires nonrepeated fe simplex ir mu, or none alone",
    )
}
