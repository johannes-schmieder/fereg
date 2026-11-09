//! The fixed-effect route row shared by the OLS solver shadow and the PPML
//! route shadow: the public reporting vocabulary of `solver()` as numeric
//! codes, which the ado decodes into `e(fe_*)` strings.
//!
//! Columns (zero-based): 0 reduction (0 none, 1 full, 2 eliminate); 1 method
//! (0 none, 1 direct, 2 map, 3 cg, 4 lsmr, 5 derived); 2 MAP acceleration
//! (0 none, 1 cg, 2 irons-tuck, 3 squarem); 3 preconditioner (0 none,
//! 1 jacobi, 2 block, 3 schwarz, 4 cmg); 4..9 the local solves: with block,
//! the block-Jacobi local solve of each design effect (0 none or eliminated,
//! 1 dense, 2 cmg, 3 jacobi); with schwarz, the subdomains counted by local
//! solve (trivial, dense, skyline, approximate; all zero when unreported);
//! 9 escalations; 10 iterations; 11 certified fallback (0 or 1).

use fereg_core::error::{invalid_argument as invalid, Result};
use fereg_core::fe::{
    FeAcceleration, FeLocalSolve, FeMethod, FePreconditioner, FeReduction, FeRoute,
    FeSchwarzSolves, FE_ROUTE_MAX_EFFECTS,
};

pub(crate) const ROUTE_COLUMNS: usize = 12;
const LOCALS: usize = 4;
const ESCALATIONS: usize = LOCALS + FE_ROUTE_MAX_EFFECTS;

/// One projection's route as reported. `single_intercept` refines a plain
/// MAP projection of a one-channel intercept design (or the constant alone)
/// to the exact direct projection it is, unless MAP was requested by name.
pub(crate) struct RouteRow {
    pub(crate) route: FeRoute,
    pub(crate) iterations: usize,
    pub(crate) fallback: bool,
    pub(crate) single_intercept: bool,
    pub(crate) map_requested: bool,
}

pub(crate) fn write_route(output: &mut [f64], row: RouteRow) -> Result<()> {
    if output.len() < ROUTE_COLUMNS {
        return Err(invalid("fixed-effect route row is too short"));
    }
    let mut route = row.route;
    if route.method == FeMethod::Map
        && route.acceleration == FeAcceleration::None
        && row.single_intercept
        && !row.map_requested
    {
        route.method = FeMethod::Direct;
    }
    output[0] = match route.reduction {
        FeReduction::None => 0.0,
        FeReduction::Full => 1.0,
        FeReduction::Eliminate => 2.0,
    };
    output[1] = match route.method {
        FeMethod::None => 0.0,
        FeMethod::Direct => 1.0,
        FeMethod::Map => 2.0,
        FeMethod::Cg => 3.0,
        FeMethod::Lsmr => 4.0,
        FeMethod::Derived => 5.0,
    };
    output[2] = match route.acceleration {
        FeAcceleration::None => 0.0,
        FeAcceleration::Cg => 1.0,
        FeAcceleration::IronsTuck => 2.0,
        FeAcceleration::Squarem => 3.0,
    };
    output[LOCALS..ESCALATIONS].fill(0.0);
    output[3] = match route.preconditioner {
        FePreconditioner::None => 0.0,
        FePreconditioner::Jacobi => 1.0,
        FePreconditioner::Block(locals) => {
            for (cell, local) in output[LOCALS..ESCALATIONS].iter_mut().zip(locals) {
                *cell = match local {
                    None => 0.0,
                    Some(FeLocalSolve::Dense) => 1.0,
                    Some(FeLocalSolve::Cmg) => 2.0,
                    Some(FeLocalSolve::Jacobi) => 3.0,
                };
            }
            2.0
        }
        FePreconditioner::Schwarz(solves) => {
            let FeSchwarzSolves {
                trivial,
                dense,
                skyline,
                approximate,
            } = solves;
            for (cell, count) in
                output[LOCALS..ESCALATIONS]
                    .iter_mut()
                    .zip([trivial, dense, skyline, approximate])
            {
                *cell = f64::from(count);
            }
            3.0
        }
        FePreconditioner::Cmg => 4.0,
    };
    output[ESCALATIONS] = f64::from(route.escalations);
    output[ESCALATIONS + 1] = exact(row.iterations)?;
    output[ESCALATIONS + 2] = f64::from(row.fallback);
    Ok(())
}

/// Structural check of a route row before publication.
pub(crate) fn validate_route(row: &[f64]) -> Result<()> {
    let code = |value: f64, max: f64| {
        value.is_finite() && value.fract() == 0.0 && (0.0..=max).contains(&value)
    };
    let block = row.get(3) == Some(&2.0);
    let schwarz = row.get(3) == Some(&3.0);
    let local = |value: f64| {
        if schwarz {
            code(value, f64::from(u32::MAX))
        } else {
            code(value, 3.0) && (block || value == 0.0)
        }
    };
    if row.len() < ROUTE_COLUMNS
        || !code(row[0], 2.0)
        || !code(row[1], 5.0)
        || !code(row[2], 3.0)
        || !code(row[3], 4.0)
        || row[LOCALS..ESCALATIONS].iter().any(|&value| !local(value))
        || (schwarz && row[ESCALATIONS - 1] != 0.0)
        || (block && row[LOCALS..ESCALATIONS].iter().all(|&local| local == 0.0))
        || (row[2] != 0.0 && row[1] != 2.0)
        || !code(row[ESCALATIONS], f64::from(u32::MAX))
        || !code(row[ESCALATIONS + 1], 9_007_199_254_740_992.0)
        || !code(row[ESCALATIONS + 2], 1.0)
    {
        return Err(invalid("fixed-effect route row is invalid"));
    }
    Ok(())
}

fn exact(value: usize) -> Result<f64> {
    if value as u128 > 9_007_199_254_740_992 {
        Err(invalid(
            "fixed-effect route count is not exactly representable",
        ))
    } else {
        Ok(value as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(route: FeRoute) -> Vec<f64> {
        let mut output = vec![9.0; ROUTE_COLUMNS];
        write_route(
            &mut output,
            RouteRow {
                route,
                iterations: 7,
                fallback: false,
                single_intercept: false,
                map_requested: false,
            },
        )
        .expect("route row");
        validate_route(&output).expect("valid row");
        output
    }

    #[test]
    fn route_rows_encode_the_public_vocabulary() {
        let block = FePreconditioner::Block([
            None,
            Some(FeLocalSolve::Cmg),
            Some(FeLocalSolve::Dense),
            None,
            None,
        ]);
        let mut route = FeRoute::new(FeReduction::Eliminate, FeMethod::Cg, block);
        route.escalations = 2;
        assert_eq!(
            row(route),
            [2.0, 3.0, 0.0, 2.0, 0.0, 2.0, 1.0, 0.0, 0.0, 2.0, 7.0, 0.0]
        );
        let mut map = FeRoute::new(FeReduction::Full, FeMethod::Map, FePreconditioner::None);
        map.acceleration = FeAcceleration::Squarem;
        assert_eq!(&row(map)[..4], &[1.0, 2.0, 3.0, 0.0]);
        let lsmr = FeRoute::new(
            FeReduction::Full,
            FeMethod::Lsmr,
            FePreconditioner::Schwarz(FeSchwarzSolves::default()),
        );
        assert_eq!(
            &row(lsmr)[..9],
            &[1.0, 4.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );
        let mut schwarz = FeRoute::new(
            FeReduction::Full,
            FeMethod::Cg,
            FePreconditioner::Schwarz(FeSchwarzSolves {
                trivial: 1,
                dense: 4,
                skyline: 2,
                approximate: 3,
            }),
        );
        schwarz.escalations = 1;
        assert_eq!(
            row(schwarz),
            [1.0, 3.0, 0.0, 3.0, 1.0, 4.0, 2.0, 3.0, 0.0, 1.0, 7.0, 0.0]
        );
    }

    #[test]
    fn a_single_intercept_map_projection_is_direct_unless_map_was_requested() {
        let map = FeRoute::new(FeReduction::Full, FeMethod::Map, FePreconditioner::None);
        for (requested, method) in [(false, 1.0), (true, 2.0)] {
            let mut output = vec![0.0; ROUTE_COLUMNS];
            write_route(
                &mut output,
                RouteRow {
                    route: map,
                    iterations: 1,
                    fallback: false,
                    single_intercept: true,
                    map_requested: requested,
                },
            )
            .expect("route row");
            assert_eq!(output[1], method);
        }
    }

    #[test]
    fn malformed_route_rows_fail_closed() {
        let good = row(FeRoute::new(
            FeReduction::Full,
            FeMethod::Cg,
            FePreconditioner::Cmg,
        ));
        for (cell, value) in [
            (0, 3.0),
            (1, 6.0),
            (2, 1.0),
            (3, 0.5),
            (5, 1.0),
            (10, -1.0),
            (11, 2.0),
        ] {
            let mut bad = good.clone();
            bad[cell] = value;
            assert!(validate_route(&bad).is_err(), "cell {cell} = {value}");
        }
        let mut empty_block = good;
        empty_block[3] = 2.0;
        assert!(validate_route(&empty_block).is_err());
    }
}
