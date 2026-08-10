//! Errors from building and stepping a circuit.

use current_ops::OpError;
use current_plan::PlanError;
use current_zset::ZSetError;

pub type Result<T> = std::result::Result<T, CircuitError>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CircuitError {
    #[error(transparent)]
    Op(#[from] OpError),

    #[error(transparent)]
    Plan(#[from] PlanError),

    #[error(transparent)]
    ZSet(#[from] ZSetError),

    #[error("no node with id {0}")]
    UnknownNode(usize),

    #[error(
        "node {node} takes input from node {input}, which is not earlier in the circuit; \
         a circuit is a DAG and is built in dependency order"
    )]
    NodeOutOfOrder { node: usize, input: usize },

    #[error("operator {op} declares arity {expected} but was wired to {found} input(s)")]
    WiringArity {
        op: &'static str,
        expected: usize,
        found: usize,
    },

    #[error("table {0:?} is declared as a source more than once")]
    DuplicateSource(String),

    #[error("this circuit has no source for table {0:?}")]
    UnknownSourceTable(String),

    #[error("a circuit needs at least one node")]
    EmptyCircuit,

    #[error(
        "operator {op} declares its state bound as {declared} but is holding {actual} \
         entries between steps — an operator exceeding its declaration is a bug, not a tuning \
         problem (I-9)"
    )]
    StateBoundViolated {
        op: &'static str,
        declared: String,
        actual: usize,
    },

    #[error("weight arithmetic overflowed i64 while {while_doing}")]
    WeightOverflow { while_doing: &'static str },
}
