//! `current-circuit` wearing the [`EngineUnderTest`] costume — the engine side of I-1.
//!
//! This is the file C0 was built to make possible. With it, the differential harness compares an
//! *incremental* engine to a *recompute-from-scratch* one, at every sealed epoch, over the same
//! seeded scenarios.
//!
//! ## What it supports, and what it refuses by name
//!
//! Dialect rungs 1–3 plus `DISTINCT`: a scan or an INNER equi-join, an optional `WHERE`, an optional
//! `GROUP BY` with aggregates and `HAVING`, an optional projection, and an optional `DISTINCT`. That
//! is the whole surface `docs/SEMANTICS.md` defines, so from C3 there is nothing left for this
//! adapter to refuse — the refusals that remain live in the binder, which turns away anything outside
//! the dialect by name (S-12).

use current_circuit::{Circuit, CircuitBuilder, NodeId};
use current_ops::{Aggregate, Distinct, Filter, Join, Operator, Project};
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
        matches!(
            scenario.family,
            Family::FilterProject | Family::Join | Family::Aggregate | Family::JoinAggregate
        )
    }

    /// True if the scenario groups — the predicate the C3 gate sweeps on.
    #[must_use]
    pub fn claims_aggregate(scenario: &Scenario) -> bool {
        matches!(scenario.family, Family::Aggregate | Family::JoinAggregate)
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

    /// Take the circuit out, for a caller that needs to drive it directly.
    ///
    /// C4's durable runtime builds a circuit of the right shape through this adapter — reusing the
    /// binder and the wiring rather than duplicating them — and then owns it, because recovery has to
    /// restore state into it and step it from the log rather than from a scenario.
    #[must_use]
    pub fn into_circuit(self) -> Circuit {
        self.circuit
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

        // Bind once, through the shared binder, so the circuit's idea of the answer's schema is
        // the oracle's idea of it by construction rather than by coincidence (D-14, S-8).
        let bound = bind(query, &catalog).map_err(|e| e.to_string())?;

        let mut builder = CircuitBuilder::new();
        let (mut tip, source_schema) = build_source(&mut builder, &query.source, &catalog)?;

        // WHERE, over the source's qualified columns (S-10).
        if let Some(predicate) = &query.filter {
            let filter = Filter::new(source_schema.clone(), Naming::Qualified, predicate.clone())
                .map_err(|e| e.to_string())?;
            tip = builder
                .add(Box::new(filter), vec![tip])
                .map_err(|e| e.to_string())?;
        }

        // GROUP BY, then HAVING. Grouping erases the input schema (S-27), so everything after it is
        // named unqualified — and HAVING is just a filter over the group output (S-32), which is why
        // it needs no operator of its own.
        let scope_schema = match &query.group_by {
            None => source_schema,
            Some(group_by) => {
                let aggregate = Aggregate::new(
                    source_schema,
                    bound.grouped_schema.clone(),
                    group_by.keys.clone(),
                    group_by.aggregates.clone(),
                    Box::new(MemBackend::new()),
                )
                .map_err(|e| e.to_string())?;
                tip = builder
                    .add(Box::new(aggregate), vec![tip])
                    .map_err(|e| e.to_string())?;

                if let Some(having) = &group_by.having {
                    let filter = Filter::new(
                        bound.grouped_schema.clone(),
                        Naming::Unqualified,
                        having.clone(),
                    )
                    .map_err(|e| e.to_string())?;
                    tip = builder
                        .add(Box::new(filter), vec![tip])
                        .map_err(|e| e.to_string())?;
                }
                bound.grouped_schema.clone()
            }
        };

        let naming = if query.group_by.is_some() {
            Naming::Unqualified
        } else {
            Naming::Qualified
        };

        if let Some(items) = &query.project {
            let project =
                Project::new(scope_schema, naming, items.clone()).map_err(|e| e.to_string())?;
            tip = builder
                .add(Box::new(project), vec![tip])
                .map_err(|e| e.to_string())?;
        }

        // DISTINCT, last of all (S-34).
        if query.distinct {
            let distinct = Distinct::new(bound.output_schema.clone(), Box::new(MemBackend::new()));
            tip = builder
                .add(Box::new(distinct), vec![tip])
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
