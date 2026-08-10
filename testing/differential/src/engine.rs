//! The seam the differential harness tests through.
//!
//! The harness knows nothing about how an answer is produced. It hands an implementation a
//! sequence of sealed epochs and asks for the answer after each one. In C0 the only
//! implementation is the oracle, on both sides of the comparison, which proves the harness rather
//! than the engine. From C1 the incremental engine implements the same trait and the comparison
//! starts meaning something (I-1).
//!
//! Nothing here mentions the oracle's types on purpose: [`EpochInput`] is the harness's own
//! representation, and each adapter converts it to whatever its engine wants. That is the seam
//! `current-circuit` will attach to in C1 without the harness changing.

use std::collections::BTreeMap;

use current_oracle::Query;
use current_zset::{Canonical, Row, Schema};

/// The input deltas of one epoch, per table.
///
/// Deliberately *not* consolidated: a delta may contain `(r, +1)` and `(r, -1)` for the same row,
/// which is the same-epoch retract-and-reinsert §7 requires the generator to produce. Merging
/// them at the door would hide the shape from every implementation downstream.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EpochInput {
    tables: BTreeMap<String, Vec<(Row, i64)>>,
}

impl EpochInput {
    #[must_use]
    pub fn new() -> EpochInput {
        EpochInput::default()
    }

    /// Append one entry. A negative weight is a retraction and takes no special path (I-5).
    pub fn push(&mut self, table: &str, row: Row, weight: i64) {
        self.tables
            .entry(table.to_owned())
            .or_default()
            .push((row, weight));
    }

    #[must_use]
    pub fn tables(&self) -> &BTreeMap<String, Vec<(Row, i64)>> {
        &self.tables
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tables.values().all(Vec::is_empty)
    }

    /// The number of entries across all tables, retractions included.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.tables.values().map(Vec::len).sum()
    }

    /// A deterministic rendering, used in scenario fingerprints and failure reports.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        for (table, entries) in &self.tables {
            for (row, weight) in entries {
                out.push_str(&format!("  {table}: {row} => {weight}\n"));
            }
        }
        if out.is_empty() {
            out.push_str("  (empty epoch)\n");
        }
        out
    }
}

/// Something that can be fed sealed epochs and asked for an answer.
///
/// Errors are `String` rather than a typed error because the harness compares implementations
/// that will not share an error type: what must agree is *whether* a query failed and *what it
/// said*, not which enum carried it. Comparing the message keeps error paths inside I-1 instead
/// of quietly outside it.
pub trait EngineUnderTest: Sized {
    /// Shown in failure reports, so that "these two disagreed" names which two.
    fn name() -> &'static str;

    /// Register the tables and the single standing query this run is about.
    fn build(tables: &[(String, Schema)], query: &Query) -> Result<Self, String>;

    /// Seal one epoch (S-6). After this returns, the answer must reflect this epoch and no part
    /// of any later one (I-3).
    fn seal_epoch(&mut self, input: &EpochInput) -> Result<(), String>;

    /// The answer as of the latest sealed epoch, in canonical form (S-8).
    fn answer(&self) -> Result<Canonical, String>;
}
