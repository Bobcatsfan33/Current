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

use current_ops::{Operator, StateBound};
use current_zset::{Canonical, EpochDeltas, Row, Schema, ZSetBatch};

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
    /// Table name → source node. Ordered, so fingerprints and errors are stable (I-2).
    sources: BTreeMap<String, NodeId>,
    sink: NodeId,
    epoch: Epoch,
    result: ResultStore,
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

    /// Add an input for `table`, emitting rows under `schema`.
    ///
    /// `schema` is the table's columns renamed to `alias.column`; it must have the same arity and
    /// types as the catalog's schema, which the caller has already established by binding.
    pub fn source(&mut self, table: impl Into<String>, schema: Schema) -> Result<NodeId> {
        let table = table.into();
        if self.sources.contains_key(&table) {
            return Err(CircuitError::DuplicateSource(table));
        }
        let id = NodeId(self.nodes.len());
        self.nodes.push(Node::Source {
            table: table.clone(),
            schema,
        });
        self.sources.insert(table, id);
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
        Ok(Circuit {
            nodes: self.nodes,
            sources: self.sources,
            sink,
            epoch: 0,
            result: ResultStore::new(schema),
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

    /// The maintained answer as of the latest sealed epoch.
    pub fn answer(&self) -> Result<Canonical> {
        self.result.canonical()
    }

    #[must_use]
    pub fn result_store(&self) -> &ResultStore {
        &self.result
    }

    /// The tables this circuit reads.
    pub fn source_tables(&self) -> impl Iterator<Item = &str> {
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

        for index in 0..self.nodes.len() {
            let produced = match self.nodes.get(index) {
                Some(Node::Source { table, schema }) => {
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
                        Some(Node::Operator { op, .. }) => op.step(&inputs)?,
                        _ => return Err(CircuitError::UnknownNode(index)),
                    }
                }
                None => return Err(CircuitError::UnknownNode(index)),
            };
            outputs.push(Some(produced));
        }

        // I-9, at the level C1 has: every operator declared what it would remember, so check it.
        // In C1 every declaration is `Stateless`, which makes this the executable form of §6 C1's
        // pitfall — a linear operator that started keeping something fails the run here rather
        // than passing it and quietly growing. C2 extends this to real bounds.
        self.check_state_declarations()?;

        let sink_output = outputs
            .get(self.sink.0)
            .ok_or(CircuitError::UnknownNode(self.sink.0))?
            .as_ref()
            .ok_or(CircuitError::UnknownNode(self.sink.0))?;
        self.result.absorb(sink_output)?;

        // The epoch advances only now, after everything above succeeded. A step that fails leaves
        // the circuit on the previous epoch rather than half-way into a new one (I-3).
        self.epoch += 1;
        Ok(self.epoch)
    }

    fn check_state_declarations(&self) -> Result<()> {
        for node in &self.nodes {
            let Node::Operator { op, .. } = node else {
                continue;
            };
            let declared = op.state_bound();
            let actual = op.state_size();
            let within = match declared {
                StateBound::Stateless => actual == 0,
                // Nothing declares these in C1; the accounting that checks them arrives with the
                // join in C2, which is the sprint that first has state to account for.
                StateBound::ProportionalToInputs { .. } | StateBound::Unbounded { .. } => true,
            };
            if !within {
                return Err(CircuitError::StateBoundViolated {
                    op: op.name(),
                    declared: declared.to_string(),
                    actual,
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
                Node::Source { table, schema } => {
                    out.push_str(&format!(
                        "node {index} source table={table} schema={schema}\n"
                    ));
                }
                Node::Operator { op, inputs } => {
                    let wiring: Vec<String> =
                        inputs.iter().map(|i| format!("node {}", i.0)).collect();
                    out.push_str(&format!(
                        "node {index} {} inputs=[{}] state_bound={} state_size={} schema={}\n",
                        op.name(),
                        wiring.join(", "),
                        op.state_bound(),
                        op.state_size(),
                        op.output_schema()
                    ));
                }
            }
        }
        out.push_str(&format!(
            "sink node {} · result store holds {} row(s)\n",
            self.sink.0,
            self.result.len()
        ));
        out.push_str(&self.result.canonical()?.render());
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
