//! Optional parts of an installation: the row-operator plan and lane storage.

use super::*;

/// Dimensions that size one installation's apply storage.
#[derive(Clone, Copy)]
pub(super) struct ApplyShape {
    pub(super) vertices: usize,
    pub(super) components: usize,
    pub(super) explicit: bool,
}

impl ApplyShape {
    /// Batch workspace bytes, lane elements and the apply storage bound for
    /// `columns` lanes after the scalar workspace bytes.
    pub(super) fn bound(
        self,
        preconditioner: &CmgPreconditioner,
        work: &mut Work,
        scalar_bytes: usize,
        columns: usize,
    ) -> Attempt<(usize, usize, usize)> {
        // Explicit centering scratch already holds every lane.
        let lane_bytes = if !self.explicit && columns > 1 {
            bytes::<[f64; 2]>(
                self.components
                    .checked_mul(columns)
                    .ok_or_else(|| allocation("CMG Schur component batch dimensions overflow"))?,
            )?
        } else {
            0
        };
        let batch_bytes = if columns > 1 {
            work.cmg(|_, cancelled| {
                CmgBatchWorkspace::allocation_bytes_controlled(preconditioner, columns, cancelled)
            })?
        } else {
            0
        };
        let elements = self
            .vertices
            .checked_mul(columns)
            .ok_or_else(|| allocation("CMG Schur batch dimensions overflow"))?;
        let bound = scalar_bytes
            .checked_add(batch_bytes)
            .and_then(|total| total.checked_add(bytes::<[f64; 2]>(elements).ok()?))
            .and_then(|total| total.checked_add(std::mem::size_of::<CmgSchurPreconditioner>()))
            .and_then(|total| total.checked_add(lane_bytes))
            .ok_or_else(|| allocation("bounded CMG apply storage overflows"))?;
        Ok((batch_bytes, elements, bound))
    }
}

/// The row-operator plan of an explicit or automatic team installation. An
/// automatic installation keeps the scalar application when the optional
/// row operators would leave no room for its one-lane apply storage; an
/// explicit request refuses instead. Admitted plan storage joins `parents`.
pub(super) fn install_plan(
    preconditioner: &CmgPreconditioner,
    install: Install,
    shape: ApplyShape,
    parents: &mut u128,
    work: &mut Work,
    memory: &mut Admission,
) -> Attempt<Option<cmg::ParallelCmgPlan>> {
    if !install.planned() {
        return Ok(None);
    }
    work.step(128)?;
    let bound = work.cmg(|team, _| {
        team.map_or(Ok(0), |team| {
            cmg::ParallelCmgPlan::construction_allocation_bound(preconditioner, team)
        })
    })?;
    let room = shape.explicit || {
        let scalar_bytes = work
            .cmg(|_, cancelled| preconditioner.workspace_allocation_bytes_controlled(cancelled))?;
        let scalar_apply = shape.bound(preconditioner, work, scalar_bytes, 1)?.2;
        fits(memory, *parents + bound as u128, scalar_apply)
    };
    if !room {
        return Ok(None);
    }
    let admitted = admit(memory, work, *parents, bound)?;
    let plan = work.cmg(|team, cancelled| {
        team.map(|team| {
            cmg::ParallelCmgPlan::build_with(
                preconditioner,
                cmg::BuildContext {
                    team: Some(team),
                    cancel: Some(cancelled),
                    budget_bytes: Some(admitted),
                },
            )
        })
        .transpose()
    })?;
    *parents += bound as u128;
    Ok(plan)
}
