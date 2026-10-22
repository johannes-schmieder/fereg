//! Checked integer prices for the frozen two-FE remaining-cost formulation.
//!
//! The caller supplies admitted execution, observed work and calibrated support.
//! These empirical prices neither certify convergence nor limit construction.

const SCALE: u128 = 1_u128 << 32;

#[derive(Clone, Copy, Debug)]
pub(super) struct CheapStructure {
    pub n: usize,
    pub p: usize,
    pub q: usize,
    pub d: usize,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Execution {
    /// Original RHS width, including retired columns.
    pub r: usize,
    /// Allocated bank stride, including padding.
    pub s: usize,
    /// Actual admitted host workers; the caller supplies one without a team.
    pub workers: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Work {
    pub h: u128,
    pub d: u128,
}

impl Work {
    /// Averages compatible completed actions, including zero-step actions.
    pub fn from_history(
        width: usize,
        actions: u128,
        shared_steps: u128,
        live_steps: u128,
        original_width: u128,
    ) -> Result<Self, Unknown> {
        if width == 0
            || actions == 0
            || original_width != mul(actions, width as u128)?
            || live_steps < shared_steps
            || live_steps > mul(shared_steps, width as u128)?
        {
            return Err(Unknown::InvalidInput);
        }
        let work = Self {
            h: ceil_div(shared_steps, actions)?,
            d: ceil_div(mul(width as u128, live_steps)?, original_width)?,
        };
        work.validate(width)?;
        Ok(work)
    }

    fn validate(self, width: usize) -> Result<(), Unknown> {
        if width == 0 || self.d < self.h || self.d > mul(self.h, width as u128)? {
            return Err(Unknown::InvalidInput);
        }
        Ok(())
    }
}

/// Raw unsigned Q32.32 coefficients, already quantized and support-checked.
/// There is deliberately no default or purported calibrated constant set.
#[derive(Clone, Copy, Debug)]
pub(super) struct Coefficients {
    pub i_s: u64,
    pub i_p: u64,
    pub b_s: u64,
    pub b_p: u64,
    pub a_0: u64,
    pub r_0: u64,
    pub t_0: u64,
    pub t_1: u64,
    pub kappa_f: u64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct FutureGroup {
    pub width: usize,
    pub stride: usize,
    /// Scheduled actions of this width, not columns or padded bank capacity.
    pub actions: u128,
    /// None means startup-only negative credit (e.g. final or unmatched tail).
    /// This does not waive the caller's required calibrated suffix support.
    pub work: Option<Work>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Unknown {
    InvalidInput,
    Overflow,
    UnpricedFuture,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Prices {
    pub incumbent: u128,
    pub setup: u128,
    pub application: u128,
    pub restart: u128,
    pub startup: u128,
    pub continuation: u128,
    pub future_actions: u128,
    pub future_reserve: u128,
    pub future_gain: i128,
    pub net_gain: i128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Decision {
    Build(Prices),
    Defer(Prices),
}

struct Exposure {
    o: u128,
    v: u128,
    z: u128,
    setup_serial: u128,
    setup_parallel: u128,
}

impl CheapStructure {
    fn exposure(self, execution: Execution) -> Result<Exposure, Unknown> {
        if self.n == 0
            || self.p == 0
            || self.q == 0
            || self.d == 0
            || execution.r == 0
            || execution.s < execution.r
            || execution.workers == 0
        {
            return Err(Unknown::InvalidInput);
        }
        let (n, p, q, r, s) = (
            self.n as u128,
            self.p as u128,
            self.q as u128,
            execution.r as u128,
            execution.s as u128,
        );
        let o = add(mul(mul(10, n)?, add(r, 1)?)?, mul(mul(4, p)?, r)?)?;
        let v = add(
            add(
                add(
                    add(mul(mul(64, q)?, r)?, mul(add(p, mul(3, q)?)?, s)?)?,
                    mul(8, p)?,
                )?,
                mul(12, q)?,
            )?,
            mul(64, r)?,
        )?;
        let z = add(q, mul(4, n)?)?;
        Ok(Exposure {
            o,
            v,
            z,
            setup_serial: mul(z, add(1, bit_length(z))?)?,
            setup_parallel: mul(n, add(1, bit_length(self.d as u128))?)?,
        })
    }
}

impl Coefficients {
    fn iteration(self, exposure: &Exposure, workers: usize) -> Result<u128, Unknown> {
        add(
            down(self.i_s, exposure.v)?,
            down(self.i_p, exposure.o)? / workers as u128,
        )
    }

    fn continuation(self, j: u128, startup: u128, a: u128, work: Work) -> Result<u128, Unknown> {
        add(
            up(self.t_0, add(j, startup)?)?,
            up(self.t_1, add(mul(work.h, j)?, mul(a, work.d)?)?)?,
        )
    }

    /// Pure evaluation after the caller has established all required support.
    /// Empty future groups mean no scheduled suffix; zero multiplicities are invalid.
    pub fn evaluate(
        self,
        structure: CheapStructure,
        execution: Execution,
        work: Work,
        future: &[FutureGroup],
    ) -> Result<Decision, Unknown> {
        if work.h == 0 || future.len() > 3 {
            return Err(Unknown::InvalidInput);
        }
        work.validate(execution.r)?;
        let exposure = structure.exposure(execution)?;
        let j = self.iteration(&exposure, execution.workers)?;
        let incumbent = mul(work.h, j)?;
        let setup = add(
            up(self.b_s, exposure.setup_serial)?,
            ceil_div(
                up(self.b_p, exposure.setup_parallel)?,
                execution.workers as u128,
            )?,
        )?;
        let application = up(self.a_0, exposure.z)?;
        let restart = up(self.r_0, add(exposure.o, exposure.v)?)?;
        let startup = mul(execution.r as u128, application)?;
        let continuation = self.continuation(j, startup, application, work)?;
        let mut future_actions = 0_u128;
        let mut future_gain = 0_i128;
        for group in future {
            if group.actions == 0 || group.width == 0 || group.stride < group.width {
                return Err(Unknown::InvalidInput);
            }
            future_actions = add(future_actions, group.actions)?;
            let startup_f = mul(group.width as u128, application)?;
            let gain = if let Some(work_f) = group.work {
                work_f.validate(group.width)?;
                let exposure_f = structure.exposure(Execution {
                    r: group.width,
                    s: group.stride,
                    workers: execution.workers,
                })?;
                let j_f = self.iteration(&exposure_f, execution.workers)?;
                let c_f = self.continuation(j_f, startup_f, application, work_f)?;
                signed(mul(work_f.h, j_f)?)?
                    .checked_sub(signed(add(startup_f, c_f)?)?)
                    .ok_or(Unknown::Overflow)?
            } else {
                -signed(startup_f)?
            };
            let discounted = if gain > 0 { gain / 2 } else { gain };
            future_gain = future_gain
                .checked_add(
                    discounted
                        .checked_mul(signed(group.actions)?)
                        .ok_or(Unknown::Overflow)?,
                )
                .ok_or(Unknown::Overflow)?;
        }
        if future_actions > 0 && application == 0 {
            return Err(Unknown::UnpricedFuture);
        }
        let future_reserve = up(self.kappa_f, mul(application, future_actions)?)?;
        future_gain = future_gain
            .checked_sub(signed(future_reserve)?)
            .ok_or(Unknown::Overflow)?;
        let net_gain = signed(incumbent)?
            .checked_sub(signed(continuation)?)
            .and_then(|value| value.checked_add(future_gain))
            .ok_or(Unknown::Overflow)?;
        let upfront = add(add(setup, restart)?, startup)?;
        let prices = Prices {
            incumbent,
            setup,
            application,
            restart,
            startup,
            continuation,
            future_actions,
            future_reserve,
            future_gain,
            net_gain,
        };
        Ok(if strict_build(net_gain, upfront)? {
            Decision::Build(prices)
        } else {
            Decision::Defer(prices)
        })
    }
}

fn add(left: u128, right: u128) -> Result<u128, Unknown> {
    left.checked_add(right).ok_or(Unknown::Overflow)
}

fn mul(left: u128, right: u128) -> Result<u128, Unknown> {
    left.checked_mul(right).ok_or(Unknown::Overflow)
}

fn signed(value: u128) -> Result<i128, Unknown> {
    i128::try_from(value).map_err(|_| Unknown::Overflow)
}

fn ceil_div(value: u128, divisor: u128) -> Result<u128, Unknown> {
    if divisor == 0 {
        return Err(Unknown::InvalidInput);
    }
    add(value / divisor, u128::from(value % divisor != 0))
}

fn down(coefficient: u64, exposure: u128) -> Result<u128, Unknown> {
    Ok(mul(coefficient as u128, exposure)? / SCALE)
}

fn up(coefficient: u64, exposure: u128) -> Result<u128, Unknown> {
    ceil_div(mul(coefficient as u128, exposure)?, SCALE)
}

fn bit_length(value: u128) -> u128 {
    (u128::BITS - value.leading_zeros()) as u128
}

fn strict_build(net_gain: i128, upfront: u128) -> Result<bool, Unknown> {
    let benefit = net_gain.checked_mul(4).ok_or(Unknown::Overflow)?;
    let cost = signed(upfront)?.checked_mul(5).ok_or(Unknown::Overflow)?;
    Ok(benefit > cost)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: u64 = 1_u64 << 32;

    // Synthetic arithmetic witnesses only; these are not calibrated constants.
    fn fractions() -> Coefficients {
        Coefficients {
            i_s: ONE / 2,
            i_p: 3 * ONE / 2,
            b_s: ONE / 4,
            b_p: ONE / 2,
            a_0: ONE / 2,
            r_0: ONE / 8,
            t_0: ONE / 4,
            t_1: ONE / 8,
            kappa_f: ONE / 4,
        }
    }

    fn example() -> (CheapStructure, Execution, Work) {
        (
            CheapStructure {
                n: 2,
                p: 1,
                q: 1,
                d: 2,
            },
            Execution {
                r: 2,
                s: 3,
                workers: 2,
            },
            Work { h: 4, d: 5 },
        )
    }

    #[test]
    fn hand_calculated_fractional_prices_include_padding_and_worker_division() {
        let (structure, execution, work) = example();
        // O=68,V=288,Z=9,S=45,P=6. J=144+102/2=195.
        // B=ceil(45/4)+ceil(ceil(6/2)/2)=14; C=ceil(205/4)+ceil(805/8)=153.
        let expected = Prices {
            incumbent: 780,
            setup: 14,
            application: 5,
            restart: 45,
            startup: 10,
            continuation: 153,
            future_actions: 0,
            future_reserve: 0,
            future_gain: 0,
            net_gain: 627,
        };
        assert_eq!(
            fractions().evaluate(structure, execution, work, &[]),
            Ok(Decision::Build(expected))
        );
        let single = Execution {
            workers: 1,
            ..execution
        };
        let Decision::Build(prices) = fractions().evaluate(structure, single, work, &[]).unwrap()
        else {
            panic!("synthetic positive gain must build");
        };
        assert_eq!(
            (prices.incumbent, prices.setup, prices.application),
            (984, 15, 5)
        );
    }

    #[test]
    fn original_width_startup_makes_exact_equality_defer() {
        let structure = CheapStructure {
            n: 1,
            p: 1,
            q: 1,
            d: 1,
        };
        let execution = Execution {
            r: 2,
            s: 2,
            workers: 1,
        };
        // Only one live lane needs one step (H=D=1), but both original lanes start CMG.
        let work = Work { h: 1, d: 1 };
        let coefficients = Coefficients {
            i_s: 5 * ONE,
            i_p: 0,
            b_s: 0,
            b_p: 563 * ONE,
            a_0: ONE,
            r_0: 0,
            t_0: 0,
            t_1: 0,
            kappa_f: 0,
        };
        let Decision::Defer(prices) = coefficients
            .evaluate(structure, execution, work, &[])
            .unwrap()
        else {
            panic!("4*1420 == 5*(1126+10) must defer");
        };
        assert_eq!(
            (prices.incumbent, prices.setup, prices.startup),
            (1420, 1126, 10)
        );
        // Pricing startup with the live count would incorrectly build this same case.
        assert!(strict_build(1420, 1126 + 5).unwrap());
        // One extra padded slot changes traffic, not startup width.
        let Decision::Build(padded) = coefficients
            .evaluate(structure, Execution { s: 3, ..execution }, work, &[])
            .unwrap()
        else {
            panic!("extra traffic gives positive gain in this synthetic table");
        };
        assert_eq!((padded.incumbent, padded.startup), (1440, 10));
    }

    #[test]
    fn future_groups_keep_losses_and_count_actions_not_columns() {
        let (structure, execution, work) = example();
        let future = [FutureGroup {
            width: 3,
            stride: 5,
            actions: 50,
            work: None,
        }];
        let Decision::Defer(prices) = fractions()
            .evaluate(structure, execution, work, &future)
            .unwrap()
        else {
            panic!("known future losses must outweigh current gain");
        };
        // Fifty calls, each costing 3*5, plus ceil((1/4)*5*50)=63.
        assert_eq!(
            (
                prices.future_actions,
                prices.future_reserve,
                prices.future_gain
            ),
            (50, 63, -813)
        );
        assert_eq!(prices.net_gain, -186);
        // A positive-only future accumulator would incorrectly pass.
        assert!(strict_build(627 - 63, 14 + 45 + 10).unwrap());
    }

    #[test]
    fn positive_future_gain_halves_before_multiplicity_and_reserve() {
        let (structure, execution, work) = example();
        let future = [
            FutureGroup {
                width: 2,
                stride: 3,
                actions: 2,
                work: Some(work),
            },
            FutureGroup {
                width: 1,
                stride: 3,
                actions: 1,
                work: None,
            },
        ];
        let Decision::Build(prices) = fractions()
            .evaluate(structure, execution, work, &future)
            .unwrap()
        else {
            panic!("synthetic compatible history has positive gain");
        };
        // df=780-(10+153)=617. 2*floor(617/2)-5-ceil(15/4)=607.
        assert_eq!(
            (
                prices.future_actions,
                prices.future_reserve,
                prices.future_gain
            ),
            (3, 4, 607)
        );
    }

    #[test]
    fn history_includes_zero_actions_and_keeps_shared_and_live_averages_distinct() {
        // Width three: one action takes four shared/eight live steps; one takes none.
        assert_eq!(Work::from_history(3, 2, 4, 8, 6), Ok(Work { h: 2, d: 4 }));
        assert_eq!(Work::from_history(3, 2, 0, 0, 6), Ok(Work { h: 0, d: 0 }));
        assert_eq!(Work::from_history(3, 2, 5, 9, 6), Ok(Work { h: 3, d: 5 }));
        let (structure, execution, work) = example();
        let future = [FutureGroup {
            width: 2,
            stride: 3,
            actions: 1,
            work: Some(Work { h: 0, d: 0 }),
        }];
        let Decision::Build(prices) = fractions()
            .evaluate(structure, execution, work, &future)
            .unwrap()
        else {
            panic!("zero history is valid, with a negative future price");
        };
        // Zero history still charges startup ten, continuation intercept 52, reserve two.
        assert_eq!(prices.future_gain, -64);
    }

    #[test]
    fn history_rejects_incompatible_raw_totals_before_averaging() {
        // Rounded averages can conceal these malformed raw histories.
        for (width, actions, shared, live, originals) in [
            (3, 2, 4, 8, 5),
            (3, 2, 4, 3, 6),
            (3, 2, 4, 13, 6),
            (3, 0, 0, 0, 0),
            (0, 2, 0, 0, 0),
            (3, 2, 0, 1, 6),
        ] {
            assert_eq!(
                Work::from_history(width, actions, shared, live, originals),
                Err(Unknown::InvalidInput)
            );
        }
        assert_eq!(
            Work::from_history(2, u128::MAX, 0, 0, u128::MAX),
            Err(Unknown::Overflow)
        );
    }

    #[test]
    fn rounding_and_representability_boundaries_are_checked_without_bias_overflow() {
        for (raw, exposure, floor, ceiling) in [
            (0, 17, 0, 0),
            (ONE, 17, 17, 17),
            (ONE / 2, 3, 1, 2),
            (1, 1, 0, 1),
            (ONE - 1, 1, 0, 1),
        ] {
            assert_eq!(down(raw, exposure), Ok(floor));
            assert_eq!(up(raw, exposure), Ok(ceiling));
        }
        assert_eq!(ceil_div(u128::MAX, 2), Ok((u128::MAX / 2) + 1));
        assert_eq!(up(1, u128::MAX), Ok(1_u128 << 96));
        assert_eq!(up(2, u128::MAX), Err(Unknown::Overflow));
        assert_eq!(strict_build(i128::MAX / 4 + 1, 0), Err(Unknown::Overflow));
        assert_eq!(
            strict_build(0, i128::MAX as u128 / 5 + 1),
            Err(Unknown::Overflow)
        );
        assert_eq!(
            Work::from_history(2, 1, u128::MAX, u128::MAX, 2),
            Err(Unknown::Overflow)
        );
        let (structure, execution, _) = example();
        assert_eq!(
            fractions().evaluate(
                structure,
                execution,
                Work {
                    h: u128::MAX,
                    d: u128::MAX
                },
                &[]
            ),
            Err(Unknown::Overflow)
        );
    }

    #[test]
    fn invalid_dimensions_zero_current_work_and_unpriced_future_defer_as_unknown() {
        let (structure, execution, work) = example();
        for invalid in [
            Execution {
                workers: 0,
                ..execution
            },
            Execution { s: 1, ..execution },
        ] {
            assert_eq!(
                fractions().evaluate(structure, invalid, work, &[]),
                Err(Unknown::InvalidInput)
            );
        }
        assert_eq!(
            fractions().evaluate(structure, execution, Work { h: 0, d: 0 }, &[]),
            Err(Unknown::InvalidInput)
        );
        let group = FutureGroup {
            width: 1,
            stride: 1,
            actions: 1,
            work: None,
        };
        assert_eq!(
            fractions().evaluate(structure, execution, work, &[group; 4]),
            Err(Unknown::InvalidInput)
        );
        let unpriced = Coefficients {
            a_0: 0,
            ..fractions()
        };
        assert_eq!(
            unpriced.evaluate(structure, execution, work, &[group]),
            Err(Unknown::UnpricedFuture)
        );
        assert_eq!(
            Work::from_history(1, 0, 0, 0, 0),
            Err(Unknown::InvalidInput)
        );
    }
}
