//! # current-sql — the SQL door, and the incrementalizer behind it
//!
//! ```text
//!   SQL text ──parse──► AST ──bind──► Query ──incrementalize──► CircuitPlan ──instantiate──► Circuit
//!                                      ▲
//!   typed API ──────────────────────────┘
//! ```
//!
//! The two doors meet at [`current_plan::Query`] and never diverge again. Everything downstream of
//! that junction — the plan, the hash, the circuit — is reached by one code path, which is what makes
//! I-6 ("the typed API and SQL compile to structurally identical plans") a property of the code
//! rather than a promise about it.
//!
//! - [`parse`] — text → AST, and a refusal by name for every clause SQL has that this dialect does
//!   not. A construct parsing is never a reason to support it (S-35).
//! - [`select`] — the binder: AST → `Query`. Names (S-11), grouping (S-27, S-33), projection (S-36).
//! - [`expr`] — SQL expressions → `Expr`, including the `CAST(NULL AS T)` rule (S-19) and the three
//!   refusals for an aggregate met where a scalar belongs (S-32).
//! - [`incremental`] — the incrementalizer. **Read this one first if you read only one**: it is the
//!   intellectual heart of the engine (§5.6) and it documents the DBSP rules rule by rule.
//! - [`circuit_plan`] — the plan type, its s-expression rendering, and its structural hash.
//! - [`instantiate`] — plan → running circuit, allocating one state backend per stateful operator.
//!
//! ## The whole surface
//!
//! ```no_run
//! # use current_sql::compile;
//! # use current_plan::bind::Catalog;
//! # fn main() -> Result<(), current_sql::SqlError> {
//! # let catalog = Catalog::new();
//! let plan = compile("SELECT t.a AS a, COUNT(*) AS n FROM t GROUP BY t.a", &catalog)?;
//! let circuit = current_sql::instantiate::instantiate(&plan)?;
//! # Ok(())
//! # }
//! ```

pub mod circuit_plan;
pub mod error;
pub mod expr;
pub mod incremental;
pub mod instantiate;
pub mod parse;
pub mod select;

pub use circuit_plan::{CircuitNode, CircuitPlan, Rule};
pub use error::{Result, SqlError};
pub use incremental::{incrementalize, incrementalize_typed};
pub use instantiate::instantiate;
pub use select::BoundQuery;

use current_plan::bind::Catalog;

/// SQL text → a bound query (S-9 … S-36). The whole front half of the pipeline.
pub fn bind_sql(sql: &str, catalog: &Catalog) -> Result<BoundQuery> {
    let parsed = parse::parse(sql)?;
    let statement = parse::select_of(&parsed)?;
    select::bind_select(&statement, catalog)
}

/// SQL text → a circuit plan. The whole pipeline, short of allocating state.
pub fn compile(sql: &str, catalog: &Catalog) -> Result<CircuitPlan> {
    let bound = bind_sql(sql, catalog)?;
    incrementalize(&bound, catalog)
}
