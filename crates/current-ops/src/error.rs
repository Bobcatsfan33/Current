//! Errors from operators.

use current_plan::PlanError;
use current_zset::{DataType, ZSetError};

pub type Result<T> = std::result::Result<T, OpError>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpError {
    /// A binding refusal or an evaluation error — the same semantics the oracle raises (D-14).
    #[error(transparent)]
    Plan(#[from] PlanError),

    #[error(transparent)]
    ZSet(#[from] ZSetError),

    #[error("operator {op} takes {expected} input(s) but was given {found}")]
    Arity {
        op: &'static str,
        expected: usize,
        found: usize,
    },

    #[error(
        "operator {op} was given a delta with schema {found}, but its input schema is {expected}"
    )]
    InputSchemaMismatch {
        op: &'static str,
        expected: String,
        found: String,
    },

    #[error("{op} requires a Boolean predicate but the expression has type {found} (S-17)")]
    PredicateNotBoolean { op: &'static str, found: DataType },
}
