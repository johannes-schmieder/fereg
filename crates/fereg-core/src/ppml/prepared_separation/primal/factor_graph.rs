//! Search a two-partition submodel using ordinary factor recipes. With a
//! constant, each main-effect factor with at most one unrepresented level
//! spans all functions of its level. Other regressors have coefficient zero.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Axis {
    Ordinary(usize),
    Absorbed(usize),
}

impl Axis {
    fn partition(
        self,
        model: &PreparedPpmlModelGeneration,
        limit: usize,
    ) -> Result<Option<(&[u32], usize)>> {
        let data = model.fit_view();
        match self {
            Self::Ordinary(source) => {
                let Some(axis) = data.categorical_sources().get(source) else {
                    return Ok(None);
                };
                let levels = axis.n_levels(data.fe_topology())?;
                if levels < 2 || levels > limit {
                    return Ok(None);
                }
                let mut emitted =
                    fallible_filled(levels, false, "unable to reserve factor levels")?;
                factor::mark_emitted(model, source, &mut emitted)?;
                if emitted.iter().filter(|&&present| !present).take(2).count() == 2 {
                    return Ok(None);
                }
                Ok(Some((axis.codes(data.fe_topology())?, levels)))
            }
            Self::Absorbed(effect) => {
                if !data.effect_specs().get(effect).is_some_and(|e| e.intercept) {
                    return Ok(None);
                }
                let source = data.fe_topology().effect_to_topology()[effect];
                let partition = crate::fe::ValidatedBorrowedInterceptSource::from_finalized_source(
                    &data.fe_topology,
                    source,
                )
                .ok_or_else(|| invalid_argument("unavailable factor graph intercept"))?
                .effect();
                Ok(Some((partition.codes(), partition.n_levels())))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::ppml::prepared_separation) struct FactorPair {
    axes: [Axis; 2],
    potentials: Vec<usize>,
}

impl FactorPair {
    /// Re-read finite recipes and original memberships, independently of the
    /// equality graph and its component ordering. The unrepresented base's
    /// potential is an intercept, with emitted indicators taking differences
    /// from it. Every integer score is exactly representable in binary64.
    pub(super) fn visit(
        &self,
        model: &PreparedPpmlModelGeneration,
        mut inspect: impl FnMut(usize, Ordering, f64) -> bool,
    ) -> Result<bool> {
        if !factor::has_constant(model) {
            return Ok(false);
        }
        let size = self.potentials.len();
        let Some((left, split)) = self.axes[0].partition(model, size)? else {
            return Ok(false);
        };
        let Some((right, levels)) = self.axes[1].partition(model, size)? else {
            return Ok(false);
        };
        if split.checked_add(levels) != Some(size)
            || size as u128 > 1u128 << 53
            || self.potentials.iter().any(|&p| p >= size)
        {
            return Ok(false);
        }
        for (row, (&a, &b)) in left.iter().zip(right).enumerate() {
            crate::interrupt::row_checkpoint(row)?;
            let x = self.potentials[a as usize];
            let y = self.potentials[split + b as usize];
            if !inspect(row, x.cmp(&y), x as f64 - y as f64) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

pub(super) fn find(
    model: &PreparedPpmlModelGeneration,
    admission: &mut SparseProofAdmission<'_>,
) -> Result<Option<FactorPair>> {
    let data = model.fit_view();
    let sources = data.categorical_sources().len();
    if sources == 0 || !factor::has_constant(model) {
        return Ok(None);
    }
    let n = model.outcome().len();
    // Discovering the anchor and then visiting the other axes reads each
    // ordinary recipe list at most twice. Masks touch levels, not observations.
    let columns = model.model().columns.len();
    let mut mask_work = 0usize;
    for (source, axis) in data.categorical_sources().iter().enumerate() {
        crate::interrupt::row_checkpoint(source)?;
        let levels = axis.n_levels(data.fe_topology())?;
        mask_work = mask_work.saturating_add(levels.saturating_mul(4));
    }
    let passes = sources.saturating_mul(2);
    if columns.saturating_mul(passes) > ProofBudget::sparse_work(columns, passes)
        || mask_work > ProofBudget::sparse_work(n.saturating_mul(2), passes)
    {
        return Ok(None);
    }
    let axes = sources
        .saturating_add(data.effect_specs().len())
        .saturating_sub(1);
    // A pair has at most two observed vertices per row. Debit each construction
    // at its actual size; a factor's encoded dummy width never multiplies N.
    let mut work = ProofBudget::sparse_work(n.saturating_mul(72), axes);
    let partition = |axis: Axis, admission: &mut SparseProofAdmission<'_>| -> Result<_> {
        if let Axis::Ordinary(source) = axis {
            let levels = data.categorical_sources()[source].n_levels(data.fe_topology())?;
            if !admission.admit(levels) {
                return Ok(None);
            }
        }
        axis.partition(model, admission.words())
    };
    let mut anchor = None;
    let mut largest = 0;
    for index in 0..sources {
        crate::interrupt::checkpoint()?;
        let axis = Axis::Ordinary(index);
        if let Some((codes, levels)) = partition(axis, admission)? {
            if levels > largest {
                largest = levels;
                anchor = Some((axis, codes));
            }
        }
    }
    let Some((anchor, left)) = anchor else {
        return Ok(None);
    };
    let others = (0..sources)
        .map(Axis::Ordinary)
        .chain((0..data.effect_specs().len()).map(Axis::Absorbed));
    for axis in others.filter(|&axis| axis != anchor) {
        crate::interrupt::checkpoint()?;
        let Some((right, levels)) = partition(axis, admission)? else {
            continue;
        };
        let vertices = largest.saturating_add(levels);
        let peak = graph::categorical_peak_bytes(n, n, vertices);
        let charge = n.saturating_add(vertices).saturating_mul(24);
        if charge > work
            || !usize::try_from(peak.div_ceil(8)).is_ok_and(|required| admission.admit(required))
        {
            continue;
        }
        work -= charge;
        if let Some(pair) = graph::pair_direction(
            [left, right],
            [largest, levels],
            model.outcome(),
            &mut crate::interrupt::checkpoint,
        )? {
            return Ok(Some(FactorPair {
                axes: [anchor, axis],
                potentials: pair.potentials,
            }));
        }
    }
    Ok(None)
}
