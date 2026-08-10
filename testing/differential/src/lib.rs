//! # current-differential — the oracle harness
//!
//! **The differential harness is the product's credibility** (`ARCHITECTURE.md` §7). Every
//! correctness claim Current will ever make routes through this crate: a seeded scenario is
//! generated, fed epoch by epoch to two implementations, and their answers are compared byte for
//! byte at every sealed epoch. That is invariant I-1, executed.
//!
//! ## What C0 proves, and what it does not
//!
//! In C0 the oracle is on **both** sides. That does not test the oracle — it tests the harness:
//! that scenarios generate reproducibly, that epochs seal in order, that answers are read at the
//! right epoch, that comparison actually detects a difference, and that a seed re-creates a run
//! exactly. The [`SaboteurEngine`] is the proof of the middle one: a deliberately wrong
//! implementation the harness must catch.
//!
//! There is no incremental engine yet, so **nothing here proves anything about incremental
//! evaluation**. From C1, one side becomes `current-circuit` and the same code starts earning
//! its keep.
//!
//! ## Layout
//!
//! - [`rng`] — the only randomness in the repository, seeded and value-stable (D-6, I-2).
//! - [`scenario`] — the generator: tables, a query, and epochs of deltas that always include
//!   retractions (§7).
//! - [`engine`] — [`EngineUnderTest`], the seam C1 attaches to.
//! - [`oracle_engine`] — the oracle as an implementation, plus the saboteur.
//! - [`harness`] — the comparison itself.
//!
//! ## Reproducing a failure
//!
//! Every failure prints its seed. `Scenario::generate(seed)` re-creates the run exactly:
//!
//! ```
//! use current_differential::{compare, OracleEngine, Scenario};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let scenario = Scenario::generate(42)?;
//! let report = compare::<OracleEngine, OracleEngine>(&scenario)
//!     .map_err(|d| d.to_string())?;
//! assert_eq!(report.seed, 42);
//! assert_eq!(report.comparisons, report.epochs + 1); // epoch 0 is compared too
//! # Ok(())
//! # }
//! ```

pub mod circuit_engine;
pub mod coverage;
pub mod engine;
pub mod harness;
pub mod oracle_engine;
pub mod rng;
pub mod scenario;

pub use circuit_engine::CircuitEngine;
pub use engine::EngineUnderTest;
pub use harness::{
    compare, sweep, sweep_matching, Divergence, DivergenceKind, Report, SweepReport,
};
pub use oracle_engine::{OracleEngine, SaboteurEngine};
pub use rng::Rng;
pub use scenario::{Family, Operation, Scenario};
