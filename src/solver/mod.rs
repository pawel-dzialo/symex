use ::boolector::BVSolution;

mod bv;
mod solver;

pub use bv::BV;
pub use solver::Solver;

pub enum BinaryOperation {
    Add,
    Sub,
    Mul,
    UDiv,
    SDiv,
    URem,
    SRem,
    And,
    Or,
    Xor,
}

pub enum UnaryOperation {}

#[derive(Debug)]
pub enum Solutions {
    /// Could not find any solutions with the current constraints.
    None,

    /// Found these solutions, and no more.
    Exactly(Vec<BVSolution>),

    /// Non-exhaustive list of solutions, there exist more than this.
    AtLeast(Vec<BVSolution>),
}
