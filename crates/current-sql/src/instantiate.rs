//! Circuit plan → running circuit (`ARCHITECTURE.md` §5.6, §5.7).
//!
//! The plan says what to build; this builds it. Nothing here makes a decision: every operator's
//! shape, every schema, and every resolved column position was fixed by the incrementalizer, so this
//! module is a constructor and reads like one.
//!
//! Keeping it separate from the incrementalizer is what makes I-6's comparison cheap and what makes
//! C6's memo possible: two standing queries can be compared, and shared, without either of them
//! having allocated a single byte of operator state.
//!
//! **State backends are allocated here, one per stateful operator.** Today every one is a
//! `MemBackend`; C8 swaps in the durable backend behind the frozen `StateBackend` trait (D-18, D-19)
//! and nothing above this line changes — which is the entire point of the trait, and the reason this
//! file is the only place in the SQL crate that names a backend at all.

use current_circuit::{Circuit, CircuitBuilder, NodeId};
use current_ops::{Aggregate, Distinct, Filter, Join, Project};
use current_state::MemBackend;

use crate::circuit_plan::{CircuitNode, CircuitPlan};
use crate::error::{Result, SqlError};

/// Build a runnable circuit from a plan.
pub fn instantiate(plan: &CircuitPlan) -> Result<Circuit> {
    let mut builder = CircuitBuilder::new();
    let root = add(&mut builder, &plan.root)?;
    let circuit = builder.build(root).map_err(circuit_error)?;

    // The same wiring check the incrementalizer makes, at the other end of the pipe. Cheap, and it
    // fails loudly at build time rather than quietly at answer time.
    if circuit.output_schema() != &plan.output_schema {
        return Err(SqlError::PlanWiringMismatch {
            emitted: circuit.output_schema().to_string(),
            expected: plan.output_schema.to_string(),
        });
    }
    Ok(circuit)
}

fn add(builder: &mut CircuitBuilder, node: &CircuitNode) -> Result<NodeId> {
    match node {
        CircuitNode::Source {
            table,
            alias,
            schema,
        } => builder
            .source(table.clone(), alias.clone(), schema.clone())
            .map_err(circuit_error),

        CircuitNode::Filter {
            input,
            naming,
            predicate,
        } => {
            let child = add(builder, input)?;
            let filter = Filter::new(input.schema().clone(), *naming, predicate.clone())
                .map_err(ops_error)?;
            builder
                .add(Box::new(filter), vec![child])
                .map_err(circuit_error)
        }

        CircuitNode::Project {
            input,
            naming,
            items,
            schema: _,
        } => {
            let child = add(builder, input)?;
            let project =
                Project::new(input.schema().clone(), *naming, items.clone()).map_err(ops_error)?;
            builder
                .add(Box::new(project), vec![child])
                .map_err(circuit_error)
        }

        CircuitNode::Join {
            left,
            right,
            keys,
            schema: _,
        } => {
            let left_id = add(builder, left)?;
            let right_id = add(builder, right)?;
            // One backend per side (§6 C2): the join keeps an integral of each input, because that
            // is what rule 2 requires — see `incremental.rs`.
            let join = Join::new(
                left.schema().clone(),
                right.schema().clone(),
                keys.clone(),
                Box::new(MemBackend::new()),
                Box::new(MemBackend::new()),
            )
            .map_err(ops_error)?;
            builder
                .add(Box::new(join), vec![left_id, right_id])
                .map_err(circuit_error)
        }

        CircuitNode::Aggregate {
            input,
            keys,
            aggregates,
            schema,
        } => {
            let child = add(builder, input)?;
            let aggregate = Aggregate::new(
                input.schema().clone(),
                schema.clone(),
                keys.clone(),
                aggregates.clone(),
                Box::new(MemBackend::new()),
            )
            .map_err(ops_error)?;
            builder
                .add(Box::new(aggregate), vec![child])
                .map_err(circuit_error)
        }

        CircuitNode::Distinct { input } => {
            let child = add(builder, input)?;
            let distinct = Distinct::new(input.schema().clone(), Box::new(MemBackend::new()));
            builder
                .add(Box::new(distinct), vec![child])
                .map_err(circuit_error)
        }
    }
}

/// A circuit-construction failure is a wiring bug, and it is reported as one rather than dressed up
/// as a refusal: the query bound, so nothing the user wrote is at fault.
fn circuit_error(error: current_circuit::CircuitError) -> SqlError {
    SqlError::PlanWiringMismatch {
        emitted: error.to_string(),
        expected: "a circuit the plan describes".to_owned(),
    }
}

fn ops_error(error: current_ops::OpError) -> SqlError {
    SqlError::PlanWiringMismatch {
        emitted: error.to_string(),
        expected: "an operator the plan describes".to_owned(),
    }
}
