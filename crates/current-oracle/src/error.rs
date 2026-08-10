//! Errors from the oracle.
//!
//! Two kinds live here and the distinction matters:
//!
//! - **Refusals** — the query is outside the dialect, or does not bind. Every one names the
//!   construct it refused (S-12). A refusal is a statement about the *query*.
//! - **Evaluation errors** — overflow, division by zero (S-20, S-21, S-22). A statement about
//!   the *data*, raised deterministically, aborting the query for that epoch.
//!
//! Plus one that is neither: [`OracleError::NegativeIntegral`] reports a malformed *history*
//! (S-5, D-12) — a retraction of something that was never there.

use current_zset::{DataType, ZSetError};

pub type Result<T> = std::result::Result<T, OracleError>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OracleError {
    #[error(transparent)]
    ZSet(#[from] ZSetError),

    // ---- catalog and history -------------------------------------------------------------
    #[error("no table named {0:?}")]
    UnknownTable(String),

    #[error("table {0:?} is declared more than once")]
    DuplicateTable(String),

    #[error(
        "table {table:?} would hold row {row} at weight {weight} after epoch {epoch}: \
         a table's contents may never go negative (S-5). This is a malformed history — \
         something retracted a row that was not there."
    )]
    NegativeIntegral {
        table: String,
        row: String,
        weight: i64,
        epoch: u64,
    },

    #[error("epoch {requested} requested but only {sealed} epochs have been sealed")]
    EpochOutOfRange { requested: u64, sealed: u64 },

    // ---- binding: refusals, each naming its construct (S-12) -----------------------------
    #[error("{0} is not in the v1 dialect at rungs 1-3")]
    NotInDialect(&'static str),

    #[error("alias {0:?} is used more than once in one query")]
    DuplicateAlias(String),

    #[error(
        "column reference {0:?} is unqualified; before a GROUP BY every column is written \
         as alias.column (S-10)"
    )]
    UnqualifiedColumn(String),

    #[error(
        "column reference {0:?} is qualified, but after a GROUP BY the only columns are the \
         declared output names, referenced unqualified (S-10, S-27)"
    )]
    QualifiedColumnAfterGroupBy(String),

    #[error("no column named {name:?} in scope {scope}")]
    UnknownColumn { name: String, scope: String },

    #[error("output name {0:?} is declared more than once")]
    DuplicateOutputName(String),

    #[error(
        "untyped NULL literal: write a null with its type, so that binding never has to infer \
         one (S-19)"
    )]
    UntypedNullLiteral,

    #[error("operator {op} does not accept operands of type {left} and {right} (S-19)")]
    TypeMismatch {
        op: &'static str,
        left: DataType,
        right: DataType,
    },

    #[error("operator {op} does not accept an operand of type {found} (S-19)")]
    UnaryTypeMismatch { op: &'static str, found: DataType },

    #[error("{context} requires a Boolean expression but found {found} (S-17)")]
    ExpectedBoolean {
        context: &'static str,
        found: DataType,
    },

    #[error("CASE branches must all have one type: found {expected} and {found} (S-18)")]
    CaseBranchTypeMismatch { expected: DataType, found: DataType },

    #[error("CASE has no WHEN branches")]
    EmptyCase,

    #[error("a GROUP BY with no keys is refused; grand-total aggregation is undecided (S-33)")]
    EmptyGroupKeys,

    #[error("a join with no key pairs is a cross join, which is not supported at rung 2 (S-26)")]
    CrossJoinNotSupported,

    // S-32's `AggregateInHaving` refusal has no variant here on purpose: the typed API cannot
    // express an aggregate inside a HAVING, because `Expr` has no aggregate variant. The illegal
    // query fails to type-check in Rust, which is stronger than refusing it at bind time. The
    // refusal becomes real work in C5, when SQL text reaches the binder.
    #[error("aggregate {func} does not accept an argument of type {ty} (S-30)")]
    AggregateTypeUnsupported { func: &'static str, ty: DataType },

    // ---- evaluation errors (S-20, S-21, S-22) ---------------------------------------------
    #[error("arithmetic overflow in {op} (S-20)")]
    ArithmeticOverflow { op: &'static str },

    #[error("division by zero in {op} (S-21)")]
    DivisionByZero { op: &'static str },

    #[error("{func} overflowed the Int64 range (S-30)")]
    AggregateOverflow { func: &'static str },

    #[error("join produced a weight outside the Int64 range")]
    JoinWeightOverflow,

    /// A negative weight where the oracle's own reasoning says one cannot occur. This is an
    /// assertion against an oracle bug, not a user-facing condition: table integrals are
    /// non-negative (S-5), filter preserves weights, and join multiplies non-negatives.
    #[error("internal: negative weight {weight} reached {stage}, which should be impossible")]
    NegativeIntermediate { stage: &'static str, weight: i64 },
}
