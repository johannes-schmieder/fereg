use crate::local::{Elimination, Forest};
use cmg::{Aggregation, CmgOptions, CmgPreconditioner, CmgWorkspace, Components, Laplacian};
use std::sync::Arc;

pub struct Full {
    pre: CmgPreconditioner,
    workspace: CmgWorkspace,
}
impl Full {
    pub fn from_parts(pre: CmgPreconditioner, workspace: CmgWorkspace) -> Self {
        Self { pre, workspace }
    }
    pub fn new(graph: &Laplacian) -> Self {
        let pre = CmgPreconditioner::build(graph, CmgOptions::default()).unwrap();
        let workspace = pre.workspace();
        Self { pre, workspace }
    }
    pub fn levels(&self) -> usize {
        self.pre.hierarchy().levels().len()
    }
    pub fn apply(&mut self, b: &[f64], x: &mut [f64]) {
        self.pre
            .apply_compatible_into(b, x, &mut self.workspace)
            .unwrap();
    }
}

pub struct Shallow {
    fine: Arc<Laplacian>,
    pub coarse: Arc<Laplacian>,
    transfer: Aggregation,
    components: Components,
    tail: Option<Full>,
    residual: Vec<f64>,
    rhs: Vec<f64>,
    solution: Vec<f64>,
}
impl Shallow {
    pub fn new(fine: Arc<Laplacian>) -> Self {
        let grouping = cmg::build_forest_grouping(&fine, 0.125).unwrap();
        let transfer =
            Aggregation::new(grouping.labels().to_vec(), grouping.aggregate_count()).unwrap();
        let coarse = Arc::new(transfer.contract(&fine).unwrap());
        let components = Components::from_laplacian(&coarse);
        let residual = vec![0.; fine.vertex_count()];
        let rhs = vec![0.; coarse.vertex_count()];
        let solution = rhs.clone();
        Self {
            fine,
            coarse,
            transfer,
            components,
            tail: None,
            residual,
            rhs,
            solution,
        }
    }
    pub fn upgrade(&mut self) -> usize {
        let tail = Full::new(&self.coarse);
        let levels = tail.levels();
        self.tail = Some(tail);
        levels
    }
    pub fn apply(&mut self, b: &[f64], x: &mut [f64]) {
        diagonal(self.fine.diagonal(), b, x);
        self.fine.matvec_into(x, &mut self.residual).unwrap();
        for (r, &b) in self.residual.iter_mut().zip(b) {
            *r = b - *r;
        }
        self.transfer
            .restrict_into(&self.residual, &mut self.rhs)
            .unwrap();
        self.components.center_in_place(&mut self.rhs).unwrap();
        if let Some(tail) = &mut self.tail {
            tail.apply(&self.rhs, &mut self.solution);
        } else {
            diagonal(self.coarse.diagonal(), &self.rhs, &mut self.solution);
        }
        self.components.center_in_place(&mut self.solution).unwrap();
        self.transfer.prolong_add_into(&self.solution, x).unwrap();
        self.fine.matvec_into(x, &mut self.residual).unwrap();
        for (((x, &b), &ax), &d) in x
            .iter_mut()
            .zip(b)
            .zip(&self.residual)
            .zip(self.fine.diagonal())
        {
            if d > 0. {
                *x += 0.5 * (b - ax) / d;
            }
        }
    }
}

pub enum Preconditioner {
    Diagonal(Vec<f64>),
    Forest(Forest),
    Elimination(Elimination),
    Shallow(Box<Shallow>),
    Full(Box<Full>),
}
impl Preconditioner {
    pub fn apply(&mut self, b: &[f64], x: &mut [f64]) {
        match self {
            Self::Diagonal(d) => diagonal(d, b, x),
            Self::Forest(f) => f.apply(b, x),
            Self::Elimination(e) => e.apply(b, x),
            Self::Shallow(s) => s.apply(b, x),
            Self::Full(f) => f.apply(b, x),
        }
    }
}
pub fn diagonal(d: &[f64], b: &[f64], x: &mut [f64]) {
    for ((x, &b), &d) in x.iter_mut().zip(b).zip(d) {
        *x = if d > 0. { 0.5 * b / d } else { 0. };
    }
}
