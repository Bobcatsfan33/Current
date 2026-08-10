//! # current-circuit — the smallest true incremental engine
//!
//! A [`Circuit`] is the compiled form of a query: a DAG of operators through which deltas flow.
//! One [`Circuit::step`] consumes one epoch's input deltas and produces one epoch's output delta,
//! which is folded into a [`ResultStore`] — the maintained integral of the query's output stream.
//!
//! ```text
//!   EpochDeltas ──► source ──► filter ──► project ──► ResultStore ──► answer
//!      (Δ)                    (linear, stateless)      (integral)      (lookup)
//! ```
//!
//! ## What makes this incremental
//!
//! The circuit never looks at the whole input. It sees only what changed, and it keeps the answer
//! up to date by adding each epoch's *output* delta into the result store. Reading the answer is a
//! lookup. The oracle, next to it in CI, does the opposite: it replays the entire log and
//! recomputes from scratch. I-1 is the claim that the two produce the same bytes at every sealed
//! epoch, and the differential harness is where that claim is checked.
//!
//! In C1 every operator is linear, so the equality is a consequence of `f(a + b) = f(a) + f(b)`
//! rather than a theorem. That is deliberate: it lets the *machinery* — wiring, scheduling, epoch
//! discipline, result stores, state accounting — be proven correct before C2 introduces an
//! operator where the incremental form is genuinely hard.
//!
//! ## What C1 does not have
//!
//! No SQL: circuits are hand-built (§6 C1), and the incrementalizer that compiles a plan into one
//! is C5. No durability: the result store is in memory, and C4 brings checkpoints. No sharing
//! between circuits: that is the memo, C6. No join and no aggregate: C2 and C3.

pub mod checkpoint;
pub mod circuit;
pub mod error;
pub mod result_store;

pub use checkpoint::{load as load_checkpoint, take as take_checkpoint};
pub use circuit::{Circuit, CircuitBuilder, Epoch, NodeId};
pub use error::{CircuitError, Result};
pub use result_store::ResultStore;
