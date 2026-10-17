//! One bounded deterministic admission decision for the original cell operator.
//! Tariffs predict relative cost; they are not wall-time or convergence bounds.
use super::*;

fn work_sum(terms: &[(u128, u128)], fixed: u128) -> Option<u64> {
    let sum = terms.iter().try_fold(fixed, |sum, (count, cost)| {
        sum.checked_add(count.checked_mul(*cost)?)
    })?;
    u64::try_from(sum).ok()
}

pub(super) struct Prices {
    pub(super) n: u128,
    pub(super) p: u128,
    pub(super) q: u128,
    pub(super) r: u128,
    pub(super) lanes: u128,
    pub(super) weighted: bool,
}

impl Prices {
    fn build(&self, cells: usize) -> Option<u64> {
        // Exact successful Work::charge total of the unchanged original builder.
        work_sum(
            &[
                (self.n, 292),
                (cells as u128, 68),
                (self.p, 28),
                (self.q, 24),
            ],
            19_032,
        )
    }

    fn census(&self) -> Option<u64> {
        work_sum(&[(self.n, 16), (self.p, 16), (self.q, 4)], 256)
    }

    fn scan(&self) -> Option<u64> {
        work_sum(&[(self.q, 16 * self.r)], 64)
    }

    fn operator_costs(&self, cells: usize) -> Option<(u128, u64)> {
        let rhs_cost = self.r.checked_mul(10)?;
        let observation = self
            .n
            .checked_mul(rhs_cost.checked_add(if self.weighted { 8 } else { 4 })?)
            .map(|work| work / self.lanes.max(1))?;
        let cell = work_sum(
            &[
                (cells as u128, rhs_cost.checked_add(16)?),
                (self.p, self.r.checked_mul(4)?.checked_add(8)?),
                (self.q, self.r.checked_mul(6)?.checked_add(8)?),
            ],
            128,
        )?;
        Some((observation, cell))
    }

    pub(super) fn operator_pays(&self, cells: usize) -> bool {
        self.operator_costs(cells)
            .is_some_and(|(observation, cell)| observation > u128::from(cell))
    }

    fn pays(&self, cells: usize, remaining: usize, sunk: u64, restart: u64) -> bool {
        let Some(build) = self.build(cells) else {
            return false;
        };
        let Some((observation, cell)) = self.operator_costs(cells) else {
            return false;
        };
        observation
            .checked_sub(u128::from(cell))
            .and_then(|saving| saving.checked_mul(remaining as u128))
            .is_some_and(|saving| {
                saving > 0 && saving >= u128::from(build) + u128::from(sunk) + u128::from(restart)
            })
    }
}

fn forecast(previous: f64, current: f64, target: f64, cap: usize) -> Option<usize> {
    if !(previous.is_finite()
        && current.is_finite()
        && target.is_finite()
        && previous > current
        && current > target
        && target > 0.0)
    {
        return None;
    }
    let remaining = 4.0 * (current.ln() - target.ln()) / (previous.ln() - current.ln());
    (remaining.is_finite() && remaining >= 1.0)
        .then(|| (remaining.floor() as usize).min(cap))
        .filter(|&n| n > 0)
}

fn proxy(
    graph: &TwoFePcgWorkspace,
    workspace: &BorrowedMapWorkspace,
    rhs: usize,
    stride: usize,
) -> Result<f64> {
    let mut maximum = 0.0_f64;
    for (level, &weight) in graph.right_weights.iter().enumerate() {
        for column in 0..rhs {
            crate::interrupt::checkpoint()?;
            let mean = graph.residual[level * stride + column] / weight;
            let reference = workspace.reference_norms[column];
            let relative = if reference == 0.0 {
                if mean == 0.0 {
                    0.0
                } else {
                    f64::INFINITY
                }
            } else {
                (mean.abs() / reference) * weight.sqrt().max(1.0)
            };
            if !relative.is_finite() {
                return Ok(f64::INFINITY);
            }
            maximum = maximum.max(relative);
        }
    }
    Ok(maximum)
}

impl AutoSchurRoute {
    fn cell_census(
        &mut self,
        design: &BorrowedInterceptFeDesign<'_>,
        graph: &TwoFePcgWorkspace,
    ) -> Result<Option<usize>> {
        let q = graph.right_weights.len();
        let bytes = q as u128 * std::mem::size_of::<usize>() as u128 + 128;
        if self
            .request
            .budget
            .is_some_and(|budget| !budget.fits(bytes))
        {
            return Ok(None);
        }
        let mut marks = Vec::new();
        marks
            .try_reserve_exact(q)
            .map_err(|_| allocation("deferred cell census allocation failed"))?;
        let actual = marks.capacity() as u128 * std::mem::size_of::<usize>() as u128 + 128;
        if actual > bytes {
            return Err(allocation(
                "deferred cell census capacity exceeds admission",
            ));
        }
        self.peak = self.peak.max(SolverWorkspacePeak::from_bytes(actual)?);
        for _ in 0..q {
            crate::interrupt::checkpoint()?;
            marks.push(usize::MAX);
        }
        let left = &design.effects()[graph.left_effect];
        let codes = design.effects()[graph.right_effect].codes();
        let mut cells = 0usize;
        for level in 0..left.n_levels() {
            crate::interrupt::checkpoint()?;
            for &row in left.level_index().checked_members(level)? {
                crate::interrupt::checkpoint()?;
                let firm = *codes
                    .get(row)
                    .ok_or_else(|| invalid_data("cell census membership exceeds rows"))?
                    as usize;
                let mark = marks
                    .get_mut(firm)
                    .ok_or_else(|| invalid_data("cell census firm exceeds levels"))?;
                if *mark != level {
                    *mark = level;
                    cells = cells
                        .checked_add(1)
                        .ok_or_else(|| invalid_data("cell census count overflows"))?;
                }
            }
        }
        // The marker bank is dropped here; it never overlaps the builder banks.
        Ok(Some(cells))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn deferred_cells_checkpoint(
        &mut self,
        design: &BorrowedInterceptFeDesign<'_>,
        weights: ValidatedWeights<'_>,
        graph: &mut TwoFePcgWorkspace,
        workspace: &BorrowedMapWorkspace,
        rhs: usize,
        stride: usize,
        iterations: usize,
        limit: usize,
        tolerance: f64,
        previous: &mut Option<f64>,
        setup_seconds: &mut f64,
        solve_started: &mut Instant,
    ) -> Result<bool> {
        if self.preset.kernel() != Some(super::super::routing::RoutingAblation::CellsDeferred)
            || self.setup.attempted
            || graph.kernel.is_some()
            || !matches!(iterations, 8 | 16)
            || iterations >= limit
        {
            return Ok(false);
        }
        let started = Instant::now();
        let prices = Prices {
            n: design.nobs() as u128,
            p: graph.left_weights.len() as u128,
            q: graph.right_weights.len() as u128,
            r: rhs as u128,
            lanes: [graph.left_weights.len(), graph.right_weights.len()]
                .into_iter()
                .map(|levels| {
                    crate::parallel::StagePlan::current(design.nobs() as u128 * rhs as u128, levels)
                        .lanes
                })
                .max()
                .unwrap_or(1) as u128,
            weighted: weights.values.is_some(),
        };
        let proxy8 = previous.filter(|value| value.is_finite()).unwrap_or(0.0);
        let mut proxy_now = 0.0;
        let mut reason = "progress";
        let mut predicted = 0;
        let mut cell_count = 0;
        let installed = (|| -> Result<bool> {
            if iterations == 16 {
                self.setup.attempted = true;
            }
            let Some(scan) = prices
                .scan()
                .filter(|&work| work <= self.setup.builder_allowance())
            else {
                self.setup.attempted = true;
                reason = "scan_budget";
                return Ok(false);
            };
            self.setup.charge(scan)?;
            let current = proxy(graph, workspace, rhs, stride)?;
            if current.is_finite() {
                proxy_now = current;
            }
            if iterations == 8 {
                *previous = Some(current);
                return Ok(false);
            }
            reason = "forecast";
            let Some(remaining) =
                previous.and_then(|old| forecast(old, current, tolerance, limit - iterations))
            else {
                return Ok(false);
            };
            predicted = remaining;
            reason = "structural_payback";
            let Some(census) = prices.census() else {
                return Ok(false);
            };
            let Some(sunk) = self.setup.used.checked_add(census) else {
                return Ok(false);
            };
            if census > self.setup.builder_allowance()
                || !prices.pays(
                    graph.left_weights.len().max(graph.right_weights.len()),
                    remaining,
                    sunk,
                    self.setup.restart_reserve,
                )
            {
                return Ok(false);
            }
            self.setup.charge(census)?;
            reason = "census_memory";
            let Some(cells) = self.cell_census(design, graph)? else {
                return Ok(false);
            };
            cell_count = cells;
            reason = "exact_payback";
            if !prices.pays(
                cells,
                remaining,
                self.setup.used,
                self.setup.restart_reserve,
            ) {
                return Ok(false);
            }
            reason = "build_budget";
            if prices
                .build(cells)
                .is_none_or(|work| work > self.setup.builder_allowance())
            {
                return Ok(false);
            }
            let built = super::super::routing_kernel::build(
                design,
                graph.left_effect,
                weights
                    .values
                    .map_or(BorrowedMapWeights::Unit, BorrowedMapWeights::Provided),
                weights.raw_scale,
                super::super::routing_kernel::KernelLimits {
                    work_budget: self.setup.builder_allowance(),
                    retained_live_bytes: 0,
                    memory: self.request.budget,
                },
            )?;
            self.setup.charge(built.work_used)?;
            self.peak = self.peak.max(built.peak);
            reason = "build_refusal";
            if let Some(kernel) = built.kernel {
                self.setup.charge(self.setup.restart_reserve)?;
                graph.kernel = Some(kernel);
                graph.use_cell_operator = true;
                reason = "installed";
                return Ok(true);
            }
            Ok(false)
        })();
        let elapsed = started.elapsed();
        *setup_seconds += elapsed.as_secs_f64();
        *solve_started += elapsed;
        if crate::diagnostics::solver_trace_enabled() {
            crate::diagnostics::emit_solver_trace(&format!(
                "FEREG_SOLVER_TRACE|phase=auto_cells_deferred|iterations={iterations}|forecast={predicted}|proxy8={proxy8:.17e}|proxy_now={proxy_now:.17e}|target={tolerance:.17e}|cells={cell_count}|setup_work={}|setup_limit={}|installed={}|reason={reason}",
                self.setup.used, self.setup.limit, u8::from(installed.as_ref().is_ok_and(|&value| value))));
        }
        installed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immediate_price_is_strict_checked_and_lane_aware() {
        let price = Prices {
            n: 9,
            p: 0,
            q: 0,
            r: 1,
            lanes: 1,
            weighted: false,
        };
        assert!(!price.operator_pays(0)); // 126 < fixed 128.
        assert!(Prices { n: 10, ..price }.operator_pays(0));
        assert!(!Prices {
            n: 10,
            lanes: 2,
            ..price
        }
        .operator_pays(0));
        assert!(!Prices {
            n: 64,
            r: 0,
            lanes: 2,
            ..price
        }
        .operator_pays(0)); // equality.
        assert!(!Prices {
            n: u128::MAX,
            ..price
        }
        .operator_pays(1));
        assert!(!Prices {
            r: u128::MAX,
            ..price
        }
        .operator_pays(1));
    }

    #[test]
    fn forecast_is_bounded_and_rejects_invalid_progress() {
        assert_eq!(forecast(1e-2, 1e-4, 1e-8, 100), Some(8));
        assert_eq!(forecast(1e-2, 1e-4, 1e-8, 3), Some(3));
        for (a, b, t) in [
            (1., 1., 1e-8),
            (1., 2., 1e-8),
            (1., 1e-9, 1e-8),
            (f64::INFINITY, 1., 1e-8),
            (1., f64::NAN, 1e-8),
            (1., 0., 1e-8),
            (1., 0.1, 0.),
        ] {
            assert_eq!(forecast(a, b, t, 100), None);
        }
        assert_eq!(forecast(1., 0.1, 1e-8, 0), None);
        assert!(forecast(1e300, 1e200, 1e-200, 100).is_some());
    }

    #[test]
    fn payback_requires_compression_and_counts_all_costs() {
        let price = Prices {
            n: 100_000,
            p: 1_000,
            q: 500,
            r: 4,
            lanes: 1,
            weighted: true,
        };
        assert!(!price.pays(100_000, 1000, 0, 0));
        assert!(price.pays(5_000, 20, 1_700_000, 20_000_000));
        assert!(!price.pays(5_000, 1, 1_700_000, 20_000_000));
        assert!(!price.pays(5_000, 20, u64::MAX, 20_000_000));
        assert!(Prices {
            n: u128::MAX,
            ..price
        }
        .build(1)
        .is_none());
    }
    #[test]
    fn census_matches_independent_pairs_and_builder_charge_is_exact() {
        const CHILD: &str = "FEREG_DEFERRED_CENSUS_BREAK_CHILD";
        let child = std::env::var_os(CHILD).is_some();
        if !child {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .env(CHILD, "1")
                .args(["fe::borrowed_map::deferred_cells::tests::census_matches_independent_pairs_and_builder_charge_is_exact", "--exact", "--test-threads=1"])
                .status().unwrap();
            assert!(status.success());
        }
        use crate::fe::{LevelIndex, ValidatedBorrowedInterceptSource};
        for repeats in [1, 3, 19] {
            let mut codes = [Vec::new(), Vec::new()];
            for &(a, b) in &[(0, 0), (0, 1), (1, 1), (2, 2), (3, 2), (3, 3)] {
                for _ in 0..repeats {
                    codes[0].push(a);
                    codes[1].push(b);
                }
            }
            let indexes = [
                LevelIndex::from_codes(&codes[0], 4).unwrap(),
                LevelIndex::from_codes(&codes[1], 4).unwrap(),
            ];
            let sources = [
                ValidatedBorrowedInterceptSource::new(&codes[0], 4, &indexes[0]).unwrap(),
                ValidatedBorrowedInterceptSource::new(&codes[1], 4, &indexes[1]).unwrap(),
            ];
            let design = BorrowedInterceptFeDesign::new(
                codes[0].len(),
                sources.iter().map(|s| s.effect()).collect(),
            )
            .unwrap();
            let mut owner = ReusableCategoricalMapAbsorber::new(design, 3, 1).unwrap();
            owner.request_auto_routing(
                super::super::super::RoutingPreset::ablation("cells_deferred").unwrap(),
                None,
                0,
            );
            let route = owner.auto_route.as_mut().unwrap();
            let graph = owner.two_fe_pcg.as_ref().unwrap();
            let count = route.cell_census(&owner.design, graph).unwrap().unwrap();
            let independent: std::collections::BTreeSet<_> =
                codes[0].iter().zip(&codes[1]).collect();
            assert_eq!(count, independent.len());
            let prices = Prices {
                n: codes[0].len() as u128,
                p: 4,
                q: 4,
                r: 3,
                lanes: 1,
                weighted: true,
            };
            let expected = prices.build(count).unwrap();
            let weights: Vec<_> = (0..codes[0].len()).map(|i| 1. + (i % 3) as f64).collect();
            for weights in [
                BorrowedMapWeights::Unit,
                BorrowedMapWeights::Provided(&weights),
            ] {
                for budget in [expected, expected - 1] {
                    let built = super::super::super::routing_kernel::build(
                        &owner.design,
                        graph.left_effect,
                        weights,
                        1.,
                        super::super::super::routing_kernel::KernelLimits {
                            work_budget: budget,
                            retained_live_bytes: 0,
                            memory: None,
                        },
                    )
                    .unwrap();
                    assert!(built.work_used <= budget);
                    assert_eq!(built.kernel.is_some(), budget == expected);
                    if built.kernel.is_some() {
                        assert_eq!(built.work_used, expected);
                    }
                }
            }
            if child {
                crate::interrupt::request_cancellation();
                let error = route.cell_census(&owner.design, graph).unwrap_err();
                assert_eq!(error.code, ErrorCode::Interrupted);
                let error = super::super::super::routing_kernel::build(
                    &owner.design,
                    graph.left_effect,
                    BorrowedMapWeights::Unit,
                    1.,
                    super::super::super::routing_kernel::KernelLimits {
                        work_budget: expected,
                        retained_live_bytes: 0,
                        memory: None,
                    },
                )
                .err()
                .expect("original builder propagates Break");
                assert_eq!(error.code, ErrorCode::Interrupted);
                crate::interrupt::reset_cancellation();
            }
            route.request.budget = Some(FeSolverMemoryBudget {
                execution_bytes: 0,
                usable_bytes: 0,
            });
            assert!(route.cell_census(&owner.design, graph).unwrap().is_none());
        }
    }
}
