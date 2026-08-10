//! `current-circuit` wearing the [`EngineUnderTest`] costume — the engine side of I-1.
//!
//! This is the file C0 was built to make possible. With it, the differential harness compares an
//! *incremental* engine to a *recompute-from-scratch* one, at every sealed epoch, over the same
//! seeded scenarios.
//!
//! ## What it supports, and what it refuses by name
//!
//! Dialect rungs 1 and 2: a scan or an INNER equi-join, with an optional `WHERE` and an optional
//! projection. `GROUP BY` is C3 and is refused with a message that says so. Refusing loudly
//! matters: an engine that quietly answered something else for a query it cannot run would show up
//! as a correctness failure whose cause is a missing feature, and the harness would be reporting a
//! bug that is not there.

use current_circuit::{Circuit, CircuitBuilder, NodeId};
use current_ops::{Filter, Join, Operator, Project};
use current_plan::bind::{bind, bind_source, Catalog, Naming};
use current_plan::plan::{Query, Source};
use current_state::MemBackend;
use current_zset::{Canonical, EpochDeltas, Schema};

use crate::engine::EngineUnderTest;
use crate::scenario::{Family, Scenario};

/// An [`EngineUnderTest`] backed by a real circuit.
#[derive(Debug)]
pub struct CircuitEngine {
    circuit: Circuit,
}

impl CircuitEngine {
    /// True if this engine claims the scenario's query — the predicate the gates filter on.
    ///
    /// Stated as a property of the *scenario family* rather than by attempting a build and seeing
    /// what happens: a sweep that discovered its own coverage by catching errors would silently
    /// shrink the day a build started failing for an unrelated reason.
    #[must_use]
    pub fn claims(scenario: &Scenario) -> bool {
        matches!(scenario.family, Family::FilterProject | Family::Join)
    }

    /// True if the scenario is a join — used by the C2 gate to sweep the rung-2 population.
    #[must_use]
    pub fn claims_join(scenario: &Scenario) -> bool {
        scenario.family == Family::Join
    }

    #[must_use]
    pub fn circuit(&self) -> &Circuit {
        &self.circuit
    }
}

/// Build the circuit for a source, returning its sink node and the schema it emits.
///
/// Recursive, so a join of joins works without the caller knowing the shape. Each scan becomes a
/// source node keyed by its **alias**, which is what makes a self-join representable: two nodes
/// over one table.
fn build_source(
    builder: &mut CircuitBuilder,
    source: &Source,
    catalog: &Catalog,
) -> Result<(NodeId, Schema), String> {
    match source {
        Source::Scan { table, alias } => {
            let schema = bind_source(source, catalog).map_err(|e| e.to_string())?;
            let id = builder
                .source(table.clone(), alias.clone(), schema.clone())
                .map_err(|e| e.to_string())?;
            Ok((id, schema))
        }
        Source::Join { left, right, on } => {
            let (left_id, left_schema) = build_source(builder, left, catalog)?;
            let (right_id, right_schema) = build_source(builder, right, catalog)?;

            // The plan names key columns by their qualified names; the operator wants positions in
            // each side's schema. Resolving here keeps the operator free of name lookup, and the
            // binder has already proved these names resolve and that their types match (S-19, S-26).
            let mut pairs = Vec::with_capacity(on.len());
            for (left_name, right_name) in on {
                let left_index = left_schema.index_of(left_name).ok_or_else(|| {
                    format!("join key {left_name:?} is not in the left schema {left_schema}")
                })?;
                let right_index = right_schema.index_of(right_name).ok_or_else(|| {
                    format!("join key {right_name:?} is not in the right schema {right_schema}")
                })?;
                pairs.push((left_index, right_index));
            }

            // One `MemBackend` per side (§6 C2). The operator only ever sees `StateBackend`, so C4
            // can hand it a `RocksBackend` without the join changing (§2, §5.5).
            let join = Join::new(
                left_schema,
                right_schema,
                pairs,
                Box::new(MemBackend::new()),
                Box::new(MemBackend::new()),
            )
            .map_err(|e| e.to_string())?;
            let schema = join.output_schema().clone();
            let id = builder
                .add(Box::new(join), vec![left_id, right_id])
                .map_err(|e| e.to_string())?;
            Ok((id, schema))
        }
    }
}

impl EngineUnderTest for CircuitEngine {
    fn name() -> &'static str {
        "circuit"
    }

    fn build(tables: &[(String, Schema)], query: &Query) -> Result<Self, String> {
        let catalog: Catalog = tables.iter().cloned().collect();

        if query.group_by.is_some() {
            return Err(
                "current-circuit runs dialect rungs 1 and 2 — a scan or an INNER equi-join, with \
                 an optional WHERE and projection. GROUP BY and the aggregates are the C3 \
                 operators."
                    .to_owned(),
            );
        }

        // Bind once, through the shared binder, so the circuit's idea of the answer's schema is
        // the oracle's idea of it by construction rather than by coincidence (D-14, S-8).
        let bound = bind(query, &catalog).map_err(|e| e.to_string())?;

        let mut builder = CircuitBuilder::new();
        let (mut tip, source_schema) = build_source(&mut builder, &query.source, &catalog)?;

        // Before a GROUP BY every column is written `alias.column` (S-10), and this circuit never
        // has a GROUP BY, so the naming is qualified throughout.
        if let Some(predicate) = &query.filter {
            let filter = Filter::new(source_schema.clone(), Naming::Qualified, predicate.clone())
                .map_err(|e| e.to_string())?;
            tip = builder
                .add(Box::new(filter), vec![tip])
                .map_err(|e| e.to_string())?;
        }

        if let Some(items) = &query.project {
            let project = Project::new(source_schema, Naming::Qualified, items.clone())
                .map_err(|e| e.to_string())?;
            tip = builder
                .add(Box::new(project), vec![tip])
                .map_err(|e| e.to_string())?;
        }

        let circuit = builder.build(tip).map_err(|e| e.to_string())?;

        // The circuit and the binder must agree about the answer's schema. They share the binder,
        // so this can only fail if the wiring above dropped a stage — which is exactly the kind
        // of mistake that would otherwise surface as a mysterious I-1 divergence.
        if circuit.output_schema() != &bound.output_schema {
            return Err(format!(
                "circuit output schema {} does not match the bound answer schema {}",
                circuit.output_schema(),
                bound.output_schema
            ));
        }

        Ok(CircuitEngine { circuit })
    }

    fn seal_epoch(&mut self, deltas: &EpochDeltas) -> Result<(), String> {
        self.circuit.step(deltas).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn answer(&self) -> Result<Canonical, String> {
        self.circuit.answer().map_err(|e| e.to_string())
    }

    fn state_fingerprint(&self) -> Result<String, String> {
        self.circuit.state_fingerprint().map_err(|e| e.to_string())
    }
}
