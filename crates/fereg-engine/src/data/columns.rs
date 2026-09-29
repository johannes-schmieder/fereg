//! Compile authoritative finite-column metadata without reading observations.
use super::{ColumnRecipe, DenseMatrix, ModelMatrixBuilder, Prepared};
use crate::{
    error::{copied_string, reserved, ErrorCode, FeregError, Result},
    spec::Estimator,
    wls::Role,
    ComponentKind, ExpandedTerm, LevelState, MetadataFlag, ModelSpec, RawTermKind, TimeSeriesState,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxStatus {
    Active,
    Base,
    ExplicitlyOmitted,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnSlot {
    pub name: String,
    pub syntax: SyntaxStatus,
    pub generated: Option<usize>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledColumns {
    pub slots: Vec<ColumnSlot>,
    pub recipes: Vec<ColumnRecipe>,
    pub roles: Vec<Role>,
    pub constant_slot: Option<usize>,
    pub structural_intercept: bool,
    pub implicit_constant: bool,
}
impl CompiledColumns {
    pub fn new(spec: &ModelSpec, estimator: Estimator) -> Result<Self> {
        spec.validate()?;
        if estimator == Estimator::Ppml && !spec.include_intercept {
            return Err(unsupported("PPML requires a reported structural constant"));
        }
        let implicit_constant = spec.absorbed_terms.is_empty();
        let absorbed_intercept =
            implicit_constant || spec.absorbed_terms.iter().any(|t| t.intercept);
        let structural_intercept = estimator == Estimator::Ppml || absorbed_intercept;
        let report = structural_intercept && spec.include_intercept;
        let capacity = spec
            .model
            .terms
            .len()
            .checked_add(usize::from(report))
            .ok_or_else(|| invalid("finite slot count overflows"))?;
        let mut result = Self {
            slots: reserved(capacity)?,
            recipes: reserved(capacity)?,
            roles: reserved(capacity)?,
            constant_slot: None,
            structural_intercept,
            implicit_constant,
        };
        for term in &spec.model.terms {
            let recipe = compile_term(term)?;
            // Component flags describe stripes. Only the whole-column flag
            // decides whether Stata intended a numerical regressor.
            let syntax = if !term.whole_column_omitted {
                SyntaxStatus::Active
            } else if term.components.iter().any(|c| c.base == MetadataFlag::True) {
                SyntaxStatus::Base
            } else {
                SyntaxStatus::ExplicitlyOmitted
            };
            let generated = if syntax == SyntaxStatus::Active {
                let position = result.recipes.len();
                result.recipes.push(recipe);
                result.roles.push(Role::Ordinary);
                Some(position)
            } else {
                None
            };
            result.slots.push(ColumnSlot {
                name: copied_string(&term.canonical_token)?,
                syntax,
                generated,
            });
        }
        if report {
            result.constant_slot = Some(result.slots.len());
            result.slots.push(ColumnSlot {
                name: copied_string("_cons")?,
                syntax: SyntaxStatus::Active,
                generated: Some(result.recipes.len()),
            });
            result.recipes.push(ColumnRecipe::Intercept);
            result.roles.push(if absorbed_intercept {
                Role::AbsorbedIntercept
            } else {
                Role::Ordinary
            });
        }
        Ok(result)
    }
    pub fn generate(&self, sample: &Prepared) -> Result<DenseMatrix> {
        sample.validate_alignment()?;
        ModelMatrixBuilder::with_nrows(sample.n_active(), &sample.regressors, &[])?
            .generate(&self.recipes)
    }
}
fn compile_term(term: &ExpandedTerm) -> Result<ColumnRecipe> {
    match (term.raw_kind,term.components.as_slice()) {
        (RawTermKind::Variable,[c]) if c.kind==ComponentKind::Continuous && c.level_state==LevelState::Absent
            && c.base==MetadataFlag::False && matches!(c.time_series_state,TimeSeriesState::None|TimeSeriesState::Materialized)
            && matches!(c.raw_operator.as_str(),""|"c"|"co")=>Ok(ColumnRecipe::Continuous{source:c.source.get() as usize-1}),
        (RawTermKind::Factor,[c]) if c.kind==ComponentKind::Categorical && c.level_state==LevelState::Present
            && matches!(c.time_series_state,TimeSeriesState::None|TimeSeriesState::Materialized)=>indicators(term),
        (RawTermKind::Interaction|RawTermKind::Product,components) if components.len()>=2
            && components.iter().all(|c|c.kind==ComponentKind::Categorical && c.level_state==LevelState::Present
                && c.time_series_state==TimeSeriesState::None)=>indicators(term),
        _=>Err(unsupported("finite terms support continuous variables, factor levels and all-categorical interactions only")),
    }
}
fn indicators(term: &ExpandedTerm) -> Result<ColumnRecipe> {
    let mut sources = reserved(term.components.len())?;
    let mut levels = reserved(term.components.len())?;
    for c in &term.components {
        if !c.level_value.is_finite() || c.level_value < 0. || c.level_value.fract() != 0. {
            return Err(FeregError::new(
                ErrorCode::InvalidData,
                "factor metadata level is not a nonnegative integer",
            ));
        }
        sources.push(c.source.get() as usize - 1);
        levels.push(c.level_value);
    }
    Ok(ColumnRecipe::NumericIndicators { sources, levels })
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn unsupported(message: &str) -> FeregError {
    FeregError::new(ErrorCode::Unsupported, message)
}
