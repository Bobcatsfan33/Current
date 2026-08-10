//! The circuit: DAG wiring, epochs, and the step scheduler (`ARCHITECTURE.md` §5, C1).
//!
//! > **Circuit** — the compiled form of a query: a directed acyclic graph of operators through
//! > which deltas flow. One step of a circuit consumes one epoch's input deltas and produces one
//! > epoch's output deltas.
//!
//! ## The step, in one paragraph
//!
//! Sealing an epoch hands the circuit that epoch's input deltas. Each source node turns its
//! table's entries into a Z-set batch; each operator node is stepped once, in order, consuming
//! the outputs of the nodes it was wired to; the sink's output delta is added into the result
//! store. The epoch counter advances only when all of that has succeeded, so a reader never sees
//! a partial epoch (I-3).
//!
//! ## Why the schedule is trivially deterministic
//!
//! Nodes are evaluated in index order, and the builder only lets a node take input from a node
//! that already exists. Index order is therefore a topological order, and it is the *same*
//! topological order on every run and in every process — not merely *a* valid one. Nothing here
//! consults a hash map, a thread, or a clock (I-2, D-6).
//!
//! This is the single-threaded scheduler §6 C1 asks for. When it grows a work queue, the ordering
//! guarantee has to be restated and re-proven; it is written down here so that whoever does that
//! knows what they are on the hook for.

use std::collections::BTreeMap;

use current_ops::{error_schema, Operator, StateBound};
use current_zset::{Canonical, EpochDeltas, Row, Schema, Value, ZSetBatch};

use crate::error::{CircuitError, Result};
use crate::result_store::ResultStore;

/// Epochs are dense integers starting at 1 (S-6). Epoch 0 means "nothing has been sealed".
pub type Epoch = u64;

/// A handle to a node in a circuit under construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(usize);

impl NodeId {
    #[must_use]
    pub fn index(self) -> usize {
        self.0
    }
}

/// A node id can be built from a raw index.
///
/// That sounds like a hole — anyone can name a node that does not exist — and it is not, because
/// [`CircuitBuilder::add`] validates every id it is handed: an index at or beyond the node being
/// added is refused as [`CircuitError::NodeOutOfOrder`], and one past the end is refused as
/// [`CircuitError::UnknownNode`] at build time. The validation is what makes the constructor
/// safe, so the constructor is public: a planner that computes wiring before building it (C5)
/// needs to name nodes, and the defensive checks need to be testable.
impl From<usize> for NodeId {
    fn from(index: usize) -> NodeId {
        NodeId(index)
    }
}

#[derive(Debug)]
enum Node {
    /// An input: one table's deltas, presented under the query's column names.
    Source {
        /// The table this node reads. Names the *catalog* entry.
        table: String,
        /// The alias this node reads it under. Sources are keyed by alias, not table, so one table
        /// can feed two nodes — which is what a self-join is (S-26, and the oracle already supports
        /// it). Keying by table would have refused `FROM t a JOIN t b` as a duplicate source.
        alias: String,
        /// The schema the node emits — the table's columns under their `alias.column` names
        /// (S-10, S-23). Rows are positional, so this is a pure rename of the table's schema.
        schema: Schema,
    },
    Operator {
        op: Box<dyn Operator>,
        inputs: Vec<NodeId>,
    },
}

/// A compiled query: operators, wiring, an epoch counter, and one result store.
#[derive(Debug)]
pub struct Circuit {
    nodes: Vec<Node>,
    /// Alias → source node. Ordered, so fingerprints and errors are stable (I-2).
    sources: BTreeMap<String, NodeId>,
    sink: NodeId,
    epoch: Epoch,
    result: ResultStore,
    /// Entries each node has ever emitted, indexed by node.
    ///
    /// This is the I-9 accounting ledger. An operator's state budget is the number of entries ever
    /// handed to it — see [`Circuit::check_state_declarations`] for why that is the right bound and
    /// what it does and does not catch.
    emitted_entries: Vec<usize>,
    /// The maintained integral of every operator's error stream (S-22b).
    ///
    /// A Z-set of messages, kept exactly like the answer: a row that raises contributes its error at
    /// the row's weight, and retracting the row retracts the error. The query has an answer iff this
    /// is empty, and the reported error is its least message — which is simply the first row of its
    /// canonical form (S-22c).
    error_store: ResultStore,
}

/// Builds a circuit in dependency order.
///
/// There is no SQL here and there is not meant to be: §6 C1 asks for "a hand-built (no SQL yet)
/// circuit API". The incrementalizer that compiles a plan into this shape is C5's job.
#[derive(Debug, Default)]
pub struct CircuitBuilder {
    nodes: Vec<Node>,
    sources: BTreeMap<String, NodeId>,
}

impl CircuitBuilder {
    #[must_use]
    pub fn new() -> CircuitBuilder {
        CircuitBuilder::default()
    }

    /// Add an input reading `table` under `alias`, emitting rows under `schema`.
    ///
    /// `schema` is the table's columns renamed to `alias.column`; it must have the same arity and
    /// types as the catalog's schema, which the caller has already established by binding.
    ///
    /// Sources are keyed by **alias**, so one table may feed several nodes. That is what makes a
    /// self-join representable — `FROM t a JOIN t b` needs two source nodes over one table, and the
    /// oracle has supported it since C0.
    pub fn source(
        &mut self,
        table: impl Into<String>,
        alias: impl Into<String>,
        schema: Schema,
    ) -> Result<NodeId> {
        let table = table.into();
        let alias = alias.into();
        if self.sources.contains_key(&alias) {
            return Err(CircuitError::DuplicateSource(alias));
        }
        let id = NodeId(self.nodes.len());
        self.nodes.push(Node::Source {
            table,
            alias: alias.clone(),
            schema,
        });
        self.sources.insert(alias, id);
        Ok(id)
    }

    /// Add an operator, wired to inputs that already exist.
    ///
    /// Refusing a forward reference is what makes index order a topological order, which is what
    /// makes the schedule deterministic (I-2). It also makes a cycle unrepresentable rather than
    /// merely undetected.
    pub fn add(&mut self, op: Box<dyn Operator>, inputs: Vec<NodeId>) -> Result<NodeId> {
        let id = NodeId(self.nodes.len());
        if op.arity() != inputs.len() {
            return Err(CircuitError::WiringArity {
                op: op.name(),
                expected: op.arity(),
                found: inputs.len(),
            });
        }
        // A state declaration that does not describe the operator cannot be checked, so it is
        // rejected here rather than accepted and quietly ignored later (I-9).
        match op.state_bound() {
            StateBound::ProportionalToInputs {
                inputs: declared, ..
            } if declared.len() != op.arity() => {
                return Err(CircuitError::StateDeclarationArityMismatch {
                    op: op.name(),
                    declared: declared.len(),
                    arity: op.arity(),
                });
            }
            StateBound::Unbounded { reason } => {
                return Err(CircuitError::UnboundedStateNotAdmissible {
                    op: op.name(),
                    reason,
                });
            }
            StateBound::Stateless | StateBound::ProportionalToInputs { .. } => {}
        }
        for input in &inputs {
            if input.0 >= id.0 {
                return Err(CircuitError::NodeOutOfOrder {
                    node: id.0,
                    input: input.0,
                });
            }
        }
        self.nodes.push(Node::Operator { op, inputs });
        Ok(id)
    }

    /// Finish, naming the node whose output stream the result store maintains.
    pub fn build(self, sink: NodeId) -> Result<Circuit> {
        if self.nodes.is_empty() {
            return Err(CircuitError::EmptyCircuit);
        }
        let schema = node_schema(&self.nodes, sink)?.clone();
        let node_count = self.nodes.len();
        Ok(Circuit {
            nodes: self.nodes,
            sources: self.sources,
            sink,
            epoch: 0,
            result: ResultStore::new(schema),
            emitted_entries: vec![0; node_count],
            error_store: ResultStore::new(error_schema()?),
        })
    }
}

impl Circuit {
    /// The highest sealed epoch; 0 before anything is sealed.
    #[must_use]
    pub fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// The schema of the answer this circuit maintains.
    #[must_use]
    pub fn output_schema(&self) -> &Schema {
        self.result.schema()
    }

    /// The maintained answer as of the latest sealed epoch — or the live error, if there is one.
    ///
    /// The query has no answer while data that raises is present (S-22). The reported error is the
    /// least live message, which is the first row of the error store's canonical form because
    /// canonical order sorts by the message column (S-22c).
    pub fn answer(&self) -> Result<Canonical> {
        if !self.error_store.is_empty() {
            let live = self.error_store.canonical()?;
            let least = live
                .entries()
                .first()
                .and_then(|(row, _)| row.get(0))
                .and_then(|value| match value {
                    Value::Str(message) => Some(message.clone()),
                    _ => None,
                })
                .ok_or(CircuitError::CorruptErrorStore)?;
            return Err(CircuitError::LiveEvaluationError(least));
        }
        self.result.canonical()
    }

    /// The live errors as a Z-set, for tests and for state fingerprints.
    #[must_use]
    pub fn error_store(&self) -> &ResultStore {
        &self.error_store
    }

    #[must_use]
    pub fn result_store(&self) -> &ResultStore {
        &self.result
    }

    /// The aliases this circuit reads under. Two aliases may name one table (a self-join).
    pub fn source_aliases(&self) -> impl Iterator<Item = &str> {
        self.sources.keys().map(String::as_str)
    }

    /// Seal one epoch: push its deltas through the circuit and fold the sink's output delta into
    /// the result store (S-6, I-3).
    ///
    /// Deltas for tables this circuit does not read are ignored — a circuit only sees the inputs
    /// it was wired to, and a scenario may well change tables no standing query touches. A delta
    /// for a table that exists nowhere is a different matter and is caught when the source is
    /// built, not here.
    pub fn step(&mut self, deltas: &EpochDeltas) -> Result<Epoch> {
        let mut outputs: Vec<Option<ZSetBatch>> = Vec::with_capacity(self.nodes.len());
        let mut errors: Vec<ZSetBatch> = Vec::new();

        for index in 0..self.nodes.len() {
            let produced = match self.nodes.get(index) {
                Some(Node::Source { table, schema, .. }) => {
                    source_batch(schema, deltas.entries_for(table))?
                }
                Some(Node::Operator { .. }) => {
                    // Collect the inputs first so the borrow of `outputs` ends before the
                    // operator is borrowed mutably.
                    let input_ids = match self.nodes.get(index) {
                        Some(Node::Operator { inputs, .. }) => inputs.clone(),
                        _ => return Err(CircuitError::UnknownNode(index)),
                    };
                    let mut inputs: Vec<&ZSetBatch> = Vec::with_capacity(input_ids.len());
                    for id in &input_ids {
                        let slot = outputs
                            .get(id.0)
                            .ok_or(CircuitError::UnknownNode(id.0))?
                            .as_ref()
                            .ok_or(CircuitError::UnknownNode(id.0))?;
                        inputs.push(slot);
                    }
                    match self.nodes.get_mut(index) {
                        Some(Node::Operator { op, .. }) => {
                            let out = op.step(&inputs)?;
                            errors.push(out.errors);
                            out.data
                        }
                        _ => return Err(CircuitError::UnknownNode(index)),
                    }
                }
                None => return Err(CircuitError::UnknownNode(index)),
            };
            let emitted = produced.len();
            if let Some(slot) = self.emitted_entries.get_mut(index) {
                *slot = slot.saturating_add(emitted);
            }
            outputs.push(Some(produced));
        }

        // I-9: every operator declared what it would remember, so check it against what it holds.
        self.check_state_declarations()?;

        let sink_output = outputs
            .get(self.sink.0)
            .ok_or(CircuitError::UnknownNode(self.sink.0))?
            .as_ref()
            .ok_or(CircuitError::UnknownNode(self.sink.0))?;
        self.result.absorb(sink_output)?;

        // Every operator's error delta is folded into one live-error set (S-22b). Absorbed after
        // the answer so that a step which fails outright leaves neither store touched.
        for delta in &errors {
            self.error_store.absorb(delta)?;
        }

        // The epoch advances only now, after everything above succeeded. A step that fails leaves
        // the circuit on the previous epoch rather than half-way into a new one (I-3).
        self.epoch += 1;
        Ok(self.epoch)
    }

    /// Account every operator's actual state against its declaration (I-9).
    ///
    /// > Every stateful operator declares its state bound as a function of its input (e.g., join
    /// > state is O(|A| + |B|)); the runtime accounts actual state against declarations, and an
    /// > operator exceeding its declaration is a bug, not a tuning problem.
    ///
    /// **The budget.** For `ProportionalToInputs`, the bound is the number of entries ever handed
    /// to the operator on those inputs, times the factor the operator declared. That is a sound
    /// upper bound on "O(|A| + |B|)" as an operator can actually satisfy it: an index over a side's
    /// integral holds one entry per *distinct* row, and distinct rows can never outnumber the
    /// entries that delivered them. The factor covers operators that keep several entries per row
    /// for a stated reason — an aggregate keeps a value multiset per aggregate slot.
    ///
    /// **What it catches.** Anything whose state grows faster than its input. A join that stored
    /// the cross product would hold |A|·|B| entries against a budget of |A|+|B| and fail as soon as
    /// either side passes two rows. So would an operator that re-stored its whole input every
    /// epoch, or one that kept a tombstone per row it had ever seen.
    ///
    /// **What it does not catch.** A constant-factor overshoot — state of 2(|A|+|B|), say, from
    /// keeping a second copy of an index — sits inside the budget whenever retractions and
    /// multiplicities mean entries outnumber distinct rows. Tightening that needs the real
    /// per-operator input integrals, which is `EXPLAIN STATE` in C8; the honest position here is
    /// that this catches the wrong *complexity*, not every wasted byte.
    /// The entries an operator is allowed to hold, given its declaration and what it has been fed.
    ///
    /// One function, used by both the check and the state fingerprint, so the number a reader sees
    /// is the number the runtime enforced. They were computed separately once, and the fingerprint
    /// quietly reported a budget without the declared factor — a discrepancy that made the printed
    /// accounting wrong while the check was right.
    fn state_budget(
        &self,
        declared: StateBound,
        inputs: &[NodeId],
        op: &'static str,
    ) -> Result<usize> {
        match declared {
            StateBound::Stateless => Ok(0),
            StateBound::ProportionalToInputs { factor, .. } => {
                let mut total = 0usize;
                for input in inputs {
                    let emitted = self
                        .emitted_entries
                        .get(input.0)
                        .copied()
                        .ok_or(CircuitError::UnknownNode(input.0))?;
                    total = total.saturating_add(emitted);
                }
                Ok(total.saturating_mul(factor))
            }
            // Refused at wiring time (`CircuitBuilder::add`), so a circuit holding one cannot be
            // built. Reported rather than silently allowed, in case that ever changes.
            StateBound::Unbounded { reason } => {
                Err(CircuitError::UnboundedStateNotAdmissible { op, reason })
            }
        }
    }

    fn check_state_declarations(&self) -> Result<()> {
        for (index, node) in self.nodes.iter().enumerate() {
            let Node::Operator { op, inputs } = node else {
                continue;
            };
            let declared = op.state_bound();
            let actual = op.state_size();

            let budget = self.state_budget(declared, inputs, op.name())?;

            if actual > budget {
                let _ = index;
                return Err(CircuitError::StateBoundViolated {
                    op: op.name(),
                    declared: declared.to_string(),
                    actual,
                    budget,
                });
            }
        }
        Ok(())
    }

    /// A deterministic rendering of everything this circuit holds.
    ///
    /// This is what the I-2 gate compares between two runs of one scenario. Answers alone are not
    /// enough: two runs can agree on every answer while holding different state, and that
    /// difference becomes a wrong answer later — or, from C4, a recovery that does not match its
    /// uncrashed twin (I-7).
    pub fn state_fingerprint(&self) -> Result<String> {
        let mut out = format!("circuit @ epoch {}\n", self.epoch);
        for (index, node) in self.nodes.iter().enumerate() {
            match node {
                Node::Source {
                    table,
                    alias,
                    schema,
                } => {
                    out.push_str(&format!(
                        "node {index} source table={table} alias={alias} emitted={} schema={schema}\n",
                        self.emitted_entries.get(index).copied().unwrap_or(0)
                    ));
                }
                Node::Operator { op, inputs } => {
                    let wiring: Vec<String> =
                        inputs.iter().map(|i| format!("node {}", i.0)).collect();
                    let budget = self.state_budget(op.state_bound(), inputs, op.name())?;
                    out.push_str(&format!(
                        "node {index} {} inputs=[{}] state_bound={} state_size={} budget={} emitted={} schema={}\n",
                        op.name(),
                        wiring.join(", "),
                        op.state_bound(),
                        op.state_size(),
                        budget,
                        self.emitted_entries.get(index).copied().unwrap_or(0),
                        op.output_schema()
                    ));
                    // An operator's own state, if it has any. This is what makes the I-2 gate a
                    // comparison of *state* and not only of answers: a join holding different
                    // indexes with the same answer must still register as different.
                    out.push_str(&op.render_state()?);
                }
            }
        }
        out.push_str(&format!(
            "sink node {} · result store holds {} row(s) · {} live error(s)\n",
            self.sink.0,
            self.result.len(),
            self.error_store.len()
        ));
        out.push_str(&self.result.canonical()?.render());
        if !self.error_store.is_empty() {
            out.push_str("live errors:\n");
            out.push_str(&self.error_store.canonical()?.render());
        }
        Ok(out)
    }
}

fn node_schema(nodes: &[Node], id: NodeId) -> Result<&Schema> {
    match nodes.get(id.0) {
        Some(Node::Source { schema, .. }) => Ok(schema),
        Some(Node::Operator { op, .. }) => Ok(op.output_schema()),
        None => Err(CircuitError::UnknownNode(id.0)),
    }
}

/// Turn one table's raw entries into the batch its source node emits.
///
/// The rows are the table's rows; the schema is the query's `alias.column` naming. Rows are
/// positional, so this is a rename and nothing else — no reordering, no coercion. Validation
/// against the schema happens inside `ZSetBatch::from_entries`, so a row of the wrong shape is
/// refused here rather than misread downstream.
fn source_batch(schema: &Schema, entries: &[(Row, i64)]) -> Result<ZSetBatch> {
    Ok(ZSetBatch::from_entries(schema.clone(), entries.to_vec())?)
}
