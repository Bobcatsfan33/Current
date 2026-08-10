//! # current-ops — the circuit operators
//!
//! Each operator consumes one epoch's input deltas and produces one epoch's output delta
//! (`ARCHITECTURE.md` §5.3). C1 builds the linear ones; join arrives in C2, aggregates and
//! distinct in C3.
//!
//! | Operator | Kind | State | Sprint |
//! | --- | --- | --- | --- |
//! | [`Filter`] | linear | none | C1 |
//! | [`Project`] | linear | none | C1 |
//! | join | bilinear | O(\|A\| + \|B\|) | C2 |
//! | aggregate, distinct | stateful | per group | C3 |
//!
//! ## The rule that governs every operator in this crate
//!
//! **Never special-case a negative weight** (I-5). A retraction flows through the same code path
//! as an insertion, and in C1 that is not a discipline anyone has to maintain: neither operator
//! reads a weight at all. Filter carries it through; project carries it through and lets
//! consolidation add it up.
//!
//! ## Declared state, checked state
//!
//! Every operator declares a [`StateBound`] (I-9) and reports its actual [`Operator::state_size`].
//! The circuit compares them after every step, so a linear operator that quietly started
//! remembering something fails the run rather than passing it slowly. In C1 both declarations are
//! [`StateBound::Stateless`] and both reports are zero — which is §6 C1's pitfall turned into an
//! assertion instead of a warning.

pub mod error;
pub mod linear;
pub mod operator;

pub use error::{OpError, Result};
pub use linear::{Filter, Project};
pub use operator::{Operator, StateBound};
