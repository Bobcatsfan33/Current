//! The hand-built circuit API (`ARCHITECTURE.md` §6 C1) and the guarantees the step scheduler
//! makes.
//!
//! The differential harness proves the circuit *agrees with the oracle*. These tests prove the
//! things the harness cannot see because the oracle has no opinion about them: what the builder
//! refuses, what happens to the epoch counter when a step fails, and what a circuit holds.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use current_circuit::{CircuitBuilder, CircuitError};
use current_ops::{Filter, Project};
use current_plan::bind::Naming;
use current_plan::plan::{BinOp, Named};
use current_plan::Expr;
use current_zset::{DataType, EpochDeltas, Field, Row, Schema, Value};

fn input_schema() -> Schema {
    Schema::new(vec![
        Field::nullable("t.a", DataType::Int64),
        Field::nullable("t.b", DataType::Int64),
    ])
    .unwrap()
}

fn row(a: Option<i64>, b: Option<i64>) -> Row {
    Row::new(vec![
        a.map_or(Value::Null, Value::Int),
        b.map_or(Value::Null, Value::Int),
    ])
}

fn epoch(entries: Vec<(Row, i64)>) -> EpochDeltas {
    let mut d = EpochDeltas::new();
    d.extend("t", entries);
    d
}

/// A scan, a filter, and a projection, wired by hand and stepped over several epochs.
#[test]
fn a_hand_built_circuit_maintains_its_answer_from_deltas() {
    let mut builder = CircuitBuilder::new();
    let source = builder.source("t", input_schema()).unwrap();
    let filter = builder
        .add(
            Box::new(
                Filter::new(
                    input_schema(),
                    Naming::Qualified,
                    Expr::binary(BinOp::Gt, Expr::column("t.a"), Expr::int(0)),
                )
                .unwrap(),
            ),
            vec![source],
        )
        .unwrap();
    let project = builder
        .add(
            Box::new(
                Project::new(
                    input_schema(),
                    Naming::Qualified,
                    vec![Named::new("a", Expr::column("t.a"))],
                )
                .unwrap(),
            ),
            vec![filter],
        )
        .unwrap();
    let mut circuit = builder.build(project).unwrap();

    assert_eq!(circuit.epoch(), 0);
    assert!(circuit.answer().unwrap().is_empty());

    // Epoch 1: two rows pass the filter and project to the same output row, so they merge (S-25).
    circuit
        .step(&epoch(vec![
            (row(Some(1), Some(10)), 1),
            (row(Some(1), Some(20)), 1),
            (row(Some(-5), Some(30)), 1),
        ]))
        .unwrap();
    assert_eq!(circuit.epoch(), 1);
    assert_eq!(circuit.answer().unwrap().render(), "(a: Int64)\n(1) => 2\n");

    // Epoch 2: a retraction of one of them. The answer drops to weight 1 without recomputing.
    circuit
        .step(&epoch(vec![(row(Some(1), Some(10)), -1)]))
        .unwrap();
    assert_eq!(circuit.answer().unwrap().render(), "(a: Int64)\n(1) => 1\n");

    // Epoch 3: retract the other, and the row leaves entirely — no zero-weight tombstone.
    circuit
        .step(&epoch(vec![(row(Some(1), Some(20)), -1)]))
        .unwrap();
    assert!(circuit.answer().unwrap().is_empty());
    assert_eq!(circuit.result_store().len(), 0);
}

/// A row inserted and retracted within one epoch nets to nothing.
#[test]
fn same_epoch_churn_leaves_no_trace() {
    let mut builder = CircuitBuilder::new();
    let source = builder.source("t", input_schema()).unwrap();
    let mut circuit = builder.build(source).unwrap();

    circuit
        .step(&epoch(vec![
            (row(Some(1), Some(1)), 2),
            (row(Some(1), Some(1)), -2),
            (row(Some(2), Some(2)), 1),
        ]))
        .unwrap();
    assert_eq!(
        circuit.answer().unwrap().render(),
        "(t.a: Int64, t.b: Int64)\n(2, 2) => 1\n"
    );
}

/// An empty epoch still seals, and the answer does not move (S-6, I-3).
#[test]
fn an_empty_epoch_advances_the_clock_and_nothing_else() {
    let mut builder = CircuitBuilder::new();
    let source = builder.source("t", input_schema()).unwrap();
    let mut circuit = builder.build(source).unwrap();

    circuit
        .step(&epoch(vec![(row(Some(1), Some(1)), 1)]))
        .unwrap();
    let before = circuit.answer().unwrap();
    circuit.step(&EpochDeltas::new()).unwrap();
    assert_eq!(circuit.epoch(), 2);
    assert_eq!(circuit.answer().unwrap(), before);
}

/// A circuit only sees the tables it was wired to.
#[test]
fn deltas_for_a_table_this_circuit_does_not_read_are_ignored() {
    let mut builder = CircuitBuilder::new();
    let source = builder.source("t", input_schema()).unwrap();
    let mut circuit = builder.build(source).unwrap();

    let mut deltas = epoch(vec![(row(Some(1), Some(1)), 1)]);
    deltas.push("some_other_table", Row::new(vec![Value::Int(9)]), 1);
    circuit.step(&deltas).unwrap();

    assert_eq!(
        circuit.answer().unwrap().render(),
        "(t.a: Int64, t.b: Int64)\n(1, 1) => 1\n"
    );
}

/// The builder refuses a forward reference, which is what makes index order a topological order
/// and the schedule deterministic (I-2).
#[test]
fn the_builder_refuses_wiring_that_is_not_in_dependency_order() {
    let mut builder = CircuitBuilder::new();
    let source = builder.source("t", input_schema()).unwrap();
    let filter = Filter::new(
        input_schema(),
        Naming::Qualified,
        Expr::is_not_null(Expr::column("t.a")),
    )
    .unwrap();
    // Node 1 cannot take input from node 7, which does not exist yet and never will.
    let err = builder
        .add(Box::new(filter), vec![current_circuit::NodeId::from(7)])
        .unwrap_err();
    assert!(
        matches!(err, CircuitError::NodeOutOfOrder { .. }),
        "expected NodeOutOfOrder, got {err}"
    );
    let _ = source;
}

/// Arity is checked at wiring time, not discovered at step time.
#[test]
fn the_builder_refuses_an_operator_wired_to_the_wrong_number_of_inputs() {
    let mut builder = CircuitBuilder::new();
    let source = builder.source("t", input_schema()).unwrap();
    let filter = Filter::new(
        input_schema(),
        Naming::Qualified,
        Expr::is_not_null(Expr::column("t.a")),
    )
    .unwrap();
    let err = builder
        .add(Box::new(filter), vec![source, source])
        .unwrap_err();
    assert!(
        matches!(
            err,
            CircuitError::WiringArity {
                op: "filter",
                expected: 1,
                found: 2
            }
        ),
        "expected WiringArity, got {err}"
    );
}

#[test]
fn the_builder_refuses_a_duplicate_source_and_an_empty_circuit() {
    let mut builder = CircuitBuilder::new();
    builder.source("t", input_schema()).unwrap();
    assert!(matches!(
        builder.source("t", input_schema()).unwrap_err(),
        CircuitError::DuplicateSource(_)
    ));

    let empty = CircuitBuilder::new();
    assert!(matches!(
        empty.build(current_circuit::NodeId::from(0)).unwrap_err(),
        CircuitError::EmptyCircuit
    ));
}

/// A predicate that is not Boolean is refused when the operator is built, not when data arrives
/// (S-17). A badly-typed circuit never gets the chance to answer anything.
#[test]
fn a_non_boolean_predicate_is_refused_at_construction() {
    let err = Filter::new(input_schema(), Naming::Qualified, Expr::column("t.a")).unwrap_err();
    assert!(
        err.to_string().contains("Boolean"),
        "the refusal must name the requirement: {err}"
    );
}

/// **An evaluation error aborts the step and leaves the epoch where it was** (S-22, I-3).
///
/// This also documents a real difference between the two implementations, recorded as part of
/// Q-2 in `docs/DECISIONS.md`: the oracle recomputes over the whole integral, so a bad row makes
/// it raise at *every* epoch from then on; the circuit sees each row once, so it raises at the
/// epoch that carried the row and not afterwards. Neither is wrong — nothing has decided what a
/// standing query should do with an error yet — and the C1 gate deliberately runs on scenarios
/// that cannot raise, so the gate never depends on the undecided part.
#[test]
fn an_evaluation_error_aborts_the_step_without_advancing_the_epoch() {
    let mut builder = CircuitBuilder::new();
    let source = builder.source("t", input_schema()).unwrap();
    let project = builder
        .add(
            Box::new(
                Project::new(
                    input_schema(),
                    Naming::Qualified,
                    vec![Named::new(
                        "q",
                        Expr::binary(BinOp::Div, Expr::column("t.a"), Expr::column("t.b")),
                    )],
                )
                .unwrap(),
            ),
            vec![source],
        )
        .unwrap();
    let mut circuit = builder.build(project).unwrap();

    circuit
        .step(&epoch(vec![(row(Some(6), Some(2)), 1)]))
        .unwrap();
    assert_eq!(circuit.epoch(), 1);
    let good = circuit.answer().unwrap();

    let err = circuit
        .step(&epoch(vec![(row(Some(1), Some(0)), 1)]))
        .unwrap_err();
    assert!(
        err.to_string().contains("division by zero"),
        "expected a division-by-zero, got {err}"
    );
    assert_eq!(
        circuit.epoch(),
        1,
        "a failed step must not advance the epoch (I-3)"
    );
    assert_eq!(
        circuit.answer().unwrap(),
        good,
        "a failed step must not half-apply its epoch"
    );
}

/// The state fingerprint reports the wiring, the declarations, and the store — and reports the
/// same bytes for the same history (I-2).
#[test]
fn the_state_fingerprint_is_stable_and_reports_what_is_held() {
    let build_and_run = || {
        let mut builder = CircuitBuilder::new();
        let source = builder.source("t", input_schema()).unwrap();
        let filter = builder
            .add(
                Box::new(
                    Filter::new(
                        input_schema(),
                        Naming::Qualified,
                        Expr::is_not_null(Expr::column("t.a")),
                    )
                    .unwrap(),
                ),
                vec![source],
            )
            .unwrap();
        let mut circuit = builder.build(filter).unwrap();
        circuit
            .step(&epoch(vec![
                (row(Some(1), Some(1)), 1),
                (row(None, Some(2)), 1),
            ]))
            .unwrap();
        circuit.state_fingerprint().unwrap()
    };

    let a = build_and_run();
    let b = build_and_run();
    assert_eq!(a, b, "the same history must fingerprint identically");

    assert!(a.contains("circuit @ epoch 1"));
    assert!(a.contains("node 0 source table=t"));
    assert!(
        a.contains("state_bound=stateless") && a.contains("state_size=0"),
        "linear operators must report no state:\n{a}"
    );
    assert!(
        a.contains("result store holds 1 row(s)"),
        "the null row was filtered out, leaving one:\n{a}"
    );
}
