//! Thread-local fault plans for tests.
//!
//! A fault plan arms one failure on the current test thread. Outside
//! `cfg(test)` each plan is an inert constant with the same `get`, `set`,
//! `replace` and `with` surface, so shipped builds compile every fault branch
//! out while production call sites stay unchanged.

/// Declare fault plans: thread-local cells in tests, inert constants otherwise.
macro_rules! fault_plan {
    ($(static $name:ident: $ty:ty = $init:expr;)+) => {
        #[cfg(test)]
        std::thread_local! {
            $(static $name: std::cell::Cell<$ty> = const { std::cell::Cell::new($init) };)+
        }
        $(
            #[cfg(not(test))]
            static $name: crate::fault::Inert<$ty> = crate::fault::Inert($init);
        )+
    };
}
pub(crate) use fault_plan;

/// Shipped stand-in for a fault plan: always the unarmed value.
#[cfg(not(test))]
pub(crate) struct Inert<T>(pub(crate) T);

#[cfg(not(test))]
impl<T: Copy> Inert<T> {
    #[inline(always)]
    pub(crate) fn get(&'static self) -> T {
        self.0
    }

    #[inline(always)]
    pub(crate) fn set(&'static self, _value: T) {}

    #[inline(always)]
    pub(crate) fn replace(&'static self, _value: T) -> T {
        self.0
    }

    #[inline(always)]
    pub(crate) fn with<R>(&'static self, f: impl FnOnce(&std::cell::Cell<T>) -> R) -> R {
        f(&std::cell::Cell::new(self.0))
    }
}
