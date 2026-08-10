//! The `Operator` trait and the state declarations I-9 requires (`ARCHITECTURE.md` §5.3).

use std::fmt;

use current_zset::{Schema, ZSetBatch};

use crate::error::{OpError, Result};

/// What an operator promises to remember between steps (I-9).
///
/// The declaration is the *contract*; [`Operator::state_size`] reports what is actually held, and
/// the runtime checks one against the other. An operator that exceeds its declaration is a bug,
/// not a tuning problem — which is only enforceable because the declaration exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateBound {
    /// Nothing is remembered between steps.
    ///
    /// This is where the linear operators live — filter, map, project — and §6 C1 says they stay
    /// here: "resist adding any state to linear operators; if a linear operator seems to need
    /// state, the design is wrong." A linear operator's output delta is the operator applied to
    /// the input delta, and computing that needs no memory of any earlier epoch.
    Stateless,

    /// State proportional to the accumulated size of the named inputs.
    ///
    /// The join declares `["left", "right"]` in C2 — it keeps both sides' integrals indexed by
    /// key, so its state is O(|A| + |B|).
    ProportionalToInputs { inputs: &'static [&'static str] },

    /// Unbounded by nature. Must be admitted explicitly at query registration (I-9); aggregation
    /// over an unbounded key space is the example §4 gives. Nothing declares this in C1.
    Unbounded { reason: &'static str },
}

impl fmt::Display for StateBound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateBound::Stateless => f.write_str("stateless"),
            StateBound::ProportionalToInputs { inputs } => {
                write!(f, "proportional to {}", inputs.join(" + "))
            }
            StateBound::Unbounded { reason } => write!(f, "unbounded ({reason})"),
        }
    }
}

/// A node in a circuit: consumes one epoch's input deltas, produces one epoch's output delta.
///
/// **Nothing in any implementation may inspect the sign of a weight** (I-5). A retraction takes
/// the same path as an insertion. If you find yourself writing `if weight < 0` here — outside
/// MIN/MAX multiset bookkeeping or the sign logic in `distinct`, neither of which exists yet —
/// you are re-deriving a bug.
pub trait Operator: fmt::Debug + Send {
    /// A short, stable name, used in state fingerprints and failure reports.
    fn name(&self) -> &'static str;

    /// How many input deltas [`Operator::step`] expects, in order.
    fn arity(&self) -> usize;

    /// The schema of the deltas this operator emits.
    fn output_schema(&self) -> &Schema;

    /// What this operator promises to remember between steps (I-9).
    fn state_bound(&self) -> StateBound;

    /// How many entries are actually retained between steps, right now.
    ///
    /// Entries rather than bytes: at C1 there is no backend to ask for a byte count, and the unit
    /// that matters for the declarations above is "how many rows am I holding". C8 replaces this
    /// with real accounting when `EXPLAIN STATE` arrives.
    fn state_size(&self) -> usize;

    /// Consume one epoch's input deltas and produce this epoch's output delta.
    ///
    /// `inputs` has exactly [`Operator::arity`] elements. An operator that is handed the wrong
    /// number returns [`OpError::Arity`] rather than assuming.
    fn step(&mut self, inputs: &[&ZSetBatch]) -> Result<ZSetBatch>;

    /// A deterministic rendering of whatever this operator remembers, for state fingerprints.
    ///
    /// The default is empty, which is the honest answer for a stateless operator. A stateful one
    /// overrides it, and must render in a fixed order (I-2) — the I-2 gate compares these strings,
    /// so an operator that rendered its state in hash order would make two identical runs look
    /// different.
    fn render_state(&self) -> Result<String> {
        Ok(String::new())
    }
}

/// Fetch the single input of a unary operator, or report the arity mismatch.
pub(crate) fn unary<'a>(op: &'static str, inputs: &[&'a ZSetBatch]) -> Result<&'a ZSetBatch> {
    match inputs {
        [only] => Ok(only),
        _ => Err(OpError::Arity {
            op,
            expected: 1,
            found: inputs.len(),
        }),
    }
}
