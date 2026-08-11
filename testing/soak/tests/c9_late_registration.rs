//! **A late registration answers what the oracle answers** (§6 C9).
//!
//! Half of C9's memo-ceiling claim is correctness and half is memory, and they need different sizes: an
//! oracle over the whole history is cheap at thirty epochs and impossible at three hundred megabytes. This
//! file is the correctness half — a log of a few dozen epochs, an oracle recomputing over the same history,
//! and three answers compared: the late registration, a registration present from epoch 0, and the oracle.
//!
//! **It is a separate test binary from `c9_memo_ceiling.rs` on purpose.** That file measures the resident
//! memory of its own process, and resident memory is a property of the *process* — so a sibling test in the
//! same binary inflates it whether it runs concurrently or before. It did: the two started life in one file
//! and the ceiling gate failed in the full-workspace run at 123.9 MB against a 96 MiB budget, having peaked
//! at 54.6 MB when run alone. One RSS-measuring test per binary, and this is the other half of that split.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::io::{BufWriter, Write};

use schweep_log::{Epochs, Record};
use schweep_memo::{Admission, Memo, Sharing};
use schweep_plan::bind::Catalog;
use schweep_state::RedbFactory;
use schweep_zset::{DataType, EpochDeltas, Field, Row, Schema, Value};

/// Rows per table per epoch, and the padding width — together these set how fast the log grows.
const ROWS_PER_EPOCH: i64 = 750;
const PADDING: usize = 480;

/// Only ids below this reach the answer, so the *answer* stays small however large the input grows.
const ANSWER_KEYS: i64 = 100;

fn catalog() -> Catalog {
    let table = || {
        Schema::new(vec![
            Field::new("id", DataType::Int64, false),
            Field::new("pad", DataType::Utf8, false),
        ])
        .unwrap()
    };
    Catalog::from([("a".to_owned(), table()), ("b".to_owned(), table())])
}

fn row(id: i64) -> Row {
    Row::new(vec![
        Value::Int(id),
        Value::Str(format!("{:width$}", id, width = PADDING)),
    ])
}

/// The same shape C8 settled on: a join with near-unique keys behind a selective filter. Large state,
/// large input, small answer.
fn sql() -> String {
    format!("SELECT a.id AS id FROM a JOIN b ON a.id = b.id WHERE a.id < {ANSWER_KEYS}")
}

/// Write a segment holding at least `target` bytes **and** at least `min_epochs` epochs. Returns the
/// number of epochs sealed and the number of ids inserted.
///
/// **Written frame by frame rather than through [`schweep_log::Log`], and that is the point of the whole
/// file.** A `Log` keeps every sealed batch resident, so filling a gigabyte of history through it costs a
/// gigabyte of memory — the fixture would OOM under the very ceiling the gate applies, and the measurement
/// would be of the fixture rather than of the catch-up. These are the log's own frames, written by the
/// log's own encoder, so the reader below verifies exactly what the log would have written.
///
/// Two size conditions rather than one because the two phases want different things: the ceiling phase
/// wants bytes, the correctness phase wants a *history* — enough epochs that a late registration is
/// genuinely catching up over many of them rather than over one big one.
fn write_segment(
    path: &std::path::Path,
    target: u64,
    min_epochs: u64,
    retract: bool,
) -> (u64, i64) {
    let file = std::fs::File::create(path).unwrap();
    let mut out = BufWriter::with_capacity(64 * 1024, file);
    let mut epochs = 0u64;
    let mut ids = 0i64;
    let mut written = 0u64;
    loop {
        for table in ["a", "b"] {
            let mut entries = Vec::with_capacity(ROWS_PER_EPOCH as usize);
            for index in 0..ROWS_PER_EPOCH {
                entries.push((row(ids + index), 1i64));
            }
            // Retractions from day one, in the input the catch-up will stream (I-5). A catch-up that only
            // ever saw insertions would not test the path a real history takes.
            if retract && ids > ANSWER_KEYS + ROWS_PER_EPOCH {
                for index in 0..(ROWS_PER_EPOCH / 10) {
                    entries.push((row(ids - ROWS_PER_EPOCH + ANSWER_KEYS + index), -1));
                }
            }
            let frame = schweep_log::record::frame(
                &Record::Append {
                    source_id: "filler".to_owned(),
                    dedup_token: format!("{table}{epochs}"),
                    table: table.to_owned(),
                    entries,
                }
                .encode(),
            );
            written += frame.len() as u64;
            out.write_all(&frame).unwrap();
        }
        epochs += 1;
        let seal = schweep_log::record::frame(&Record::SealEpoch { epoch: epochs }.encode());
        written += seal.len() as u64;
        out.write_all(&seal).unwrap();
        ids += ROWS_PER_EPOCH;
        if written >= target && epochs >= min_epochs {
            out.flush().unwrap();
            out.into_inner().unwrap().sync_all().unwrap();
            return (epochs, ids);
        }
        assert!(
            epochs < 20_000,
            "the segment is not growing toward {target} bytes; it stalled at {written}"
        );
    }
}

/// Every epoch in the segment, as the deltas a catch-up consumes — one epoch resident at a time.
fn stream(segment: &std::path::Path) -> impl Iterator<Item = EpochDeltas> {
    Epochs::open(segment).unwrap().map(|sealed| {
        let sealed = sealed.unwrap();
        let mut deltas = EpochDeltas::new();
        for batch in sealed.batches {
            deltas.extend(batch.table, batch.entries);
        }
        deltas
    })
}

fn memo(dir: &std::path::Path) -> Memo {
    Memo::without_input_cache(catalog(), Sharing::On, Box::new(RedbFactory::new(dir))).unwrap()
}

/// **Phase 1 — correctness.** A late registration answers what the oracle answers, and what a
/// registration that was there all along answers.
#[test]
fn a_late_registration_answers_what_the_oracle_answers() {
    let root = std::env::temp_dir().join(format!("schweep-c9-late-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);

    // Small: a few dozen epochs, so a from-scratch oracle over the whole history is cheap.
    std::fs::create_dir_all(&root).unwrap();
    let segment = root.join("segment");
    let (epochs, _) = write_segment(&segment, 4 * 1024 * 1024, 30, true);
    assert!(
        epochs > 8,
        "the history must be long enough to be a history"
    );

    let plan = schweep_sql::compile(&sql(), &catalog()).unwrap();
    let bound = schweep_sql::bind_sql(&sql(), &catalog()).unwrap();

    // The oracle: full recomputation over the same history, epoch by epoch (I-1).
    let mut oracle = schweep_oracle::Oracle::new(catalog()).unwrap();
    // A registration present from epoch 0, stepped like the live path.
    let mut early_memo = memo(&root.join("early-state"));
    let early = early_memo.register(&plan, Admission::bounded());
    let early = match early {
        Ok(handle) => handle,
        // A memo without an input cache refuses `register`, by design: it has no history to catch up from.
        // At epoch 0 there is nothing to catch up, so the streaming door is the one to use.
        Err(_) => early_memo
            .register_from_chunks(&plan, Admission::bounded(), std::iter::empty())
            .unwrap(),
    };
    for deltas in stream(&segment) {
        oracle.seal_epoch(deltas.clone()).unwrap();
        early_memo.seal_epoch(&deltas).unwrap();
    }

    // The late registration: same history, streamed in as chunks, after the fact.
    let mut late_memo = memo(&root.join("late-state"));
    for deltas in stream(&segment) {
        late_memo.seal_epoch(&deltas).unwrap();
    }
    let late = late_memo
        .register_from_chunks(&plan, Admission::bounded(), stream(&segment))
        .unwrap();

    let oracle_answer = oracle.answer(&bound.query).unwrap().canonical().unwrap();
    let early_answer = early_memo.read(early).unwrap().1;
    let late_answer = late_memo.read(late).unwrap().1;

    assert_eq!(
        late_answer.render(),
        oracle_answer.render(),
        "a query registered at epoch {epochs} must answer as though it had been there since epoch 0 (I-1)"
    );
    assert_eq!(
        late_answer.render(),
        early_answer.render(),
        "and it must agree with the registration that was there all along"
    );
    assert_eq!(
        late_memo.epoch(),
        early_memo.epoch(),
        "catching up must not move the epoch: nothing was sealed by registering"
    );
    println!(
        "C9 late-registration correctness: {epochs} epochs · {} answer rows · oracle, early and late \
         registrations agree",
        late_answer.len()
    );

    let _ = std::fs::remove_dir_all(&root);
}
