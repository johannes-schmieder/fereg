use super::*;
use crate::error::reserved;
impl<'a> Context<'a, '_> {
    pub(super) fn start(
        &self,
        diagnostics: &mut Diagnostics,
        interrupt: &mut Interrupt<'_>,
    ) -> Result<Accepted<'a, 'a>> {
        let value = self.family.global_start()?;
        let mut finite = reserved(self.problem.raw.ncols())?;
        finite.resize(self.problem.raw.ncols(), 0.);
        let mut absorbed = DenseMatrix::zeros(self.design.layout().total, 1)?;
        if let Some(effect) = self
            .design
            .topology()
            .effects()
            .iter()
            .position(|e| e.intercept)
        {
            let layout = &self.design.layout().effects[effect];
            for level in 0..layout.levels {
                absorbed[(layout.offset + level, 0)] = value;
            }
        } else if let Some(column) =
            self.problem
                .roles
                .iter()
                .enumerate()
                .position(|(column, role)| {
                    *role == Role::Ordinary
                        && (0..self.problem.raw.nrows())
                            .all(|row| self.problem.raw[(row, column)] == 1.)
                })
        {
            finite[column] = value;
        } else {
            return Err(invalid("PPML requires an explicit structural constant"));
        }
        let mut eta = reserved(self.problem.raw.nrows())?;
        eta.extend(
            (0..self.problem.raw.nrows()).map(|row| self.offset.map_or(0., |v| v[row]) + value),
        );
        let global = Accepted::start(
            self.family,
            Point {
                finite,
                absorbed,
                eta,
            },
            self.team,
        )?;
        global.certify_state(
            self.problem.raw,
            self.design,
            self.controls.publication_tolerance(),
            self.team,
        )?;
        let proposal = match self.family.one_fe_start(self.design.topology(), self.team) {
            Ok(Some(coefficients)) => coefficients,
            Ok(None) => return Ok(global),
            Err(error) if error.code == ErrorCode::NonConvergence => return Ok(global),
            Err(error) => return Err(error),
        };
        let absorbed = DenseMatrix::from_row_major(self.design.layout().total, 1, proposal)?;
        let d = self.design.multiply(&absorbed, self.team)?;
        let mut eta = reserved(self.problem.raw.nrows())?;
        eta.extend(
            (0..self.problem.raw.nrows())
                .map(|row| self.offset.map_or(0., |v| v[row]) + d[(row, 0)]),
        );
        let mut finite = reserved(self.problem.raw.ncols())?;
        finite.resize(self.problem.raw.ncols(), 0.);
        let proposal = Point {
            finite,
            absorbed,
            eta,
        };
        match global.try_step_checked(&proposal, self.team, interrupt) {
            Ok(next) => {
                diagnostics.analytic_start = true;
                diagnostics.step_halvings += next.halvings();
                Ok(next)
            }
            Err(failure)
                if !failure.callback && failure.error.code == ErrorCode::NonConvergence =>
            {
                Ok(global)
            }
            Err(failure) => Err(failure.error),
        }
    }
}
