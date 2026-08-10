//! `current-circuit` wearing the [`EngineUnderTest`] costume — the C1 side of I-1.
//!
//! This is the file C0 was built to make possible. With it, the differential harness stops
//! comparing the oracle to itself and starts comparing an *incremental* engine to a
//! *recompute-from-scratch* one, at every sealed epoch, over the same seeded scenarios.
//!
//! ## What it supports, and what it refuses by name
//!
//! Dialect rung 1 only: a scan, an optional `WHERE`, an optional projection. A join is C2 and a
//! `GROUP BY` is C3, and both are refused here with a message that says which sprint brings them.
//! Refusing loudly matters: an engine that quietly answered something else for a query it cannot
//! run would show up as a correctness failure whose cause is a missing feature, and the harness
//! would be reporting a bug that is not there.

use current_circuit::{Circuit, CircuitBuilder};
use current_ops::{Filter, Project};
use current_plan::bind::{bind, Catalog, Naming};
use current_plan::plan::{Query, Source};
use current_zset::{Canonical, EpochDeltas, Schema};

use crate::engine::EngineUnderTest;
use crate::scenario::{Family, Scenario};

/// An [`EngineUnderTest`] backed by a real circuit.
#[derive(Debug)]
pub struct CircuitEngine {
    circuit: Circuit,
}

impl CircuitEngine {
    /// True if this engine claims the scenario's query — the predicate the C1 sweep filters on.
    ///
    /// Stated as a property of the *scenario family* rather than by attempting a build and seeing
    /// what happens: a sweep that discovered its own coverage by catching errors would silently
    /// shrink the day a build started failing for an unrelated reason.
    #[must_use]
    pub fn claims(scenario: &Scenario) -> bool {
        scenario.family == Family::FilterProject
    }

    #[must_use]
    pub fn circuit(&self) -> &Circuit {
        &self.circuit
    }
}

impl EngineUnderTest for CircuitEngine {
    fn name() -> &'static str {
        "circuit"
    }

    fn build(tables: &[(String, Schema)], query: &Query) -> Result<Self, String> {
        let catalog: Catalog = tables.iter().cloned().collect();

        let Source::Scan { table, .. } = &query.source else {
            return Err(
                "current-circuit v0 runs dialect rung 1 only — a scan with an optional WHERE and \
                 projection. An INNER JOIN is the C2 operator."
                    .to_owned(),
            );
        };
        if query.group_by.is_some() {
            return Err(
                "current-circuit v0 runs dialect rung 1 only — a scan with an optional WHERE and \
                 projection. GROUP BY and the aggregates are the C3 operators."
                    .to_owned(),
            );
        }

        // Bind once, through the shared binder, so the circuit's idea of the answer's schema is
        // the oracle's idea of it by construction rather than by coincidence (D-14, S-8).
        let bound = bind(query, &catalog).map_err(|e| e.to_string())?;

        let mut builder = CircuitBuilder::new();
        let mut tip = builder
            .source(table.clone(), bound.input_schema.clone())
            .map_err(|e| e.to_string())?;

        // Before a GROUP BY every column is written `alias.column` (S-10), and this circuit never
        // has a GROUP BY, so the naming is qualified throughout.
        if let Some(predicate) = &query.filter {
            let filter = Filter::new(
                bound.input_schema.clone(),
                Naming::Qualified,
                predicate.clone(),
            )
            .map_err(|e| e.to_string())?;
            tip = builder
                .add(Box::new(filter), vec![tip])
                .map_err(|e| e.to_string())?;
        }

        if let Some(items) = &query.project {
            let project =
                Project::new(bound.input_schema.clone(), Naming::Qualified, items.clone())
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
