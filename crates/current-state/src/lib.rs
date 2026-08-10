//! # current-state — operator state, behind an interface
//!
//! `ARCHITECTURE.md` §5.5. An ordered key-value store with prefix range scans and atomic
//! multi-key write batches. Operators reach their state through [`StateBackend`] and never through
//! a concrete container, which is the seam §2 insists on:
//!
//! > `current-log` and `current-state` must sit behind traits rather than being called concretely
//! > from operators.
//!
//! C2 uses it for the join's two indexes. C4 adds `RocksBackend` and the checkpoint protocol, and
//! **freezes the trait at its exit** — so until then this is allowed to grow, and what it is
//! missing is written down rather than left to be rediscovered (D-15).
//!
//! ## Keys are values, not bytes
//!
//! A key is a `Vec<Value>`, ordered by the total order on values (S-7); a stored value is an `i64`
//! weight. An order-preserving *byte* encoding is a storage concern and belongs inside a backend
//! that needs one, not in the interface every operator sees. `RocksBackend` will need it;
//! `MemBackend` is a `BTreeMap` and does not. The full argument is D-15.
//!
//! ## What is deliberately absent
//!
//! Named snapshots. The checkpoint protocol is C4's design and guessing at its shape now would be
//! worse than leaving a gap that is labelled.

pub mod backend;
pub mod error;
pub mod mem;

pub use backend::{Key, StateBackend, WriteBatch};
pub use error::{Result, StateError};
pub use mem::MemBackend;
