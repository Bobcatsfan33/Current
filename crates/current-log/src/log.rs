//! `current-log` v1: a directory of files (`ARCHITECTURE.md` §5.4; `docs/DURABILITY.md` §1, §2, §4).
//!
//! > The write path and the only place time enters.
//!
//! The log is the **source of truth**. Operator state is a cache of it, which is why recovery can be
//! "load a checkpoint and replay the suffix" and why a crash after a seal record is durable loses
//! nothing: everything downstream of a sealed epoch is a deterministic function of the log (I-2, D-6).
//!
//! Every ordering in this file is the one `docs/DURABILITY.md` numbers, and every fault hook is a
//! seam that document names. Read them together.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use current_zset::{Row, Schema};

use crate::error::{LogError, Result};
use crate::fault::{FaultInjector, Seam};
use crate::record::{frame, read_framed, Record};

/// Epochs are dense integers starting at 1 (S-6).
pub type Epoch = u64;

/// Whether the log actually calls `fsync`.
///
/// **Why this is a choice and not a bug.** `fsync` is what makes an ack a promise against *power
/// loss*. The crash harness is in-process (`docs/DURABILITY.md` §5): it aborts at a named seam and
/// drops every in-memory object, then re-reads the file. The bytes are in the file either way,
/// because `write_all` already put them there and the page cache survives the simulated crash — so
/// `fsync` contributes **nothing** to what the 10,000-cycle gate measures, while costing a
/// millisecond or more per call on macOS and turning that gate into hours.
///
/// So the equivalence gate runs [`SyncPolicy::Deferred`] and says so, and the orderings — which are
/// what the seams test — are unchanged either way. [`SyncPolicy::Full`] is the default, is what
/// production uses, and is what the log's own durability tests use.
///
/// What this means honestly: **nothing here tests power loss.** Doing so needs a filesystem-level
/// fault injector or a VM that can be cut off mid-write, and that is named as remaining work in
/// `docs/PROGRESS.md` rather than implied by a green gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncPolicy {
    /// `fsync` every append and every seal. Production.
    Full,
    /// Skip `fsync`. For in-process crash simulation, where it changes nothing observable.
    Deferred,
}

/// The outcome of an append (`docs/DURABILITY.md` §1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ack {
    /// Durable, and this is the first time this token was seen.
    Appended,
    /// A replay: the same token with the same content. Acknowledged and dropped, so the batch is
    /// applied exactly once however many times it is offered (I-4, A3).
    DroppedAsReplay,
}

/// One batch as it will be replayed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Batch {
    /// Where the data came from. Carried from birth, and the hook taint-as-retraction and Loom's
    /// envelopes attach to later (§5.4, **[MutinyDB seam]**).
    pub source_id: String,
    pub table: String,
    pub entries: Vec<(Row, i64)>,
}

/// The append-only input log.
#[derive(Debug)]
pub struct Log {
    dir: PathBuf,
    sync: SyncPolicy,
    /// Table name → schema. Appends are validated against it (A1) before anything is written.
    catalog: BTreeMap<String, Schema>,
    /// `dedup_token` → content hash. Rebuilt from the log at open, never from memory (R6), which is
    /// what closes the window in which the log knows something the caller does not.
    dedup: BTreeMap<String, u64>,
    /// Batches appended but not yet sealed into an epoch.
    pending: Vec<Batch>,
    /// Batches per sealed epoch, in epoch order. Epoch `n` is at index `n - 1`.
    sealed: Vec<Vec<Batch>>,
    /// Byte offset of the first record after the retained prefix, used only for reporting.
    segment: PathBuf,
}

impl Log {
    /// Open, or create, a log in `dir`, recovering whatever is there (R5, R6).
    pub fn open(
        dir: impl AsRef<Path>,
        catalog: BTreeMap<String, Schema>,
        faults: &mut FaultInjector,
        sync: SyncPolicy,
    ) -> Result<Log> {
        let dir = dir.as_ref().to_path_buf();
        if dir.exists() && !dir.is_dir() {
            return Err(LogError::NotADirectory(dir.display().to_string()));
        }
        fs::create_dir_all(&dir)?;
        let segment = dir.join("segment-00000001.log");

        let mut log = Log {
            dir,
            sync,
            catalog,
            dedup: BTreeMap::new(),
            pending: Vec::new(),
            sealed: Vec::new(),
            segment,
        };
        log.replay_from_disk(faults)?;
        Ok(log)
    }

    /// R5 and R6: scan the segment, stop at the torn tail, rebuild the dedup index.
    fn replay_from_disk(&mut self, faults: &mut FaultInjector) -> Result<()> {
        let bytes = match File::open(&self.segment) {
            Ok(mut file) => {
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)?;
                bytes
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e.into()),
        };

        let mut at = 0usize;
        let mut pending: Vec<Batch> = Vec::new();
        let mut records_replayed = 0u32;
        // R5: stop at the first record that fails its CRC or is short. Everything after it is
        // discarded — a torn tail is expected, not exceptional.
        while let Some((record, next)) = read_framed(&bytes, at)? {
            records_replayed += 1;
            if records_replayed % 4 == 0 && faults.reached(Seam::RecoveryMidReplay) {
                return Err(LogError::InjectedFault(Seam::RecoveryMidReplay.name()));
            }
            match record {
                Record::Append {
                    source_id,
                    dedup_token,
                    table,
                    entries,
                } => {
                    let replayed = Record::Append {
                        source_id: source_id.clone(),
                        dedup_token: dedup_token.clone(),
                        table: table.clone(),
                        entries: entries.clone(),
                    };
                    self.dedup.insert(dedup_token, replayed.content_hash());
                    pending.push(Batch {
                        source_id,
                        table,
                        entries,
                    });
                }
                Record::SealEpoch { .. } => {
                    self.sealed.push(std::mem::take(&mut pending));
                }
            }
            at = next;
        }
        // Appends after the last seal record are durable but not yet visible: they are pending, and
        // whatever seals next will include them (S-6).
        self.pending = pending;
        Ok(())
    }

    #[must_use]
    pub fn sealed_epoch(&self) -> Epoch {
        self.sealed.len() as Epoch
    }

    #[must_use]
    pub fn pending_batches(&self) -> &[Batch] {
        &self.pending
    }

    /// The batches of one sealed epoch, for replay.
    pub fn epoch(&self, epoch: Epoch) -> Result<&[Batch]> {
        if epoch == 0 || epoch > self.sealed_epoch() {
            return Err(LogError::EpochOutOfRange {
                requested: epoch,
                sealed: self.sealed_epoch(),
            });
        }
        self.sealed
            .get((epoch - 1) as usize)
            .map(Vec::as_slice)
            .ok_or(LogError::EpochOutOfRange {
                requested: epoch,
                sealed: self.sealed_epoch(),
            })
    }

    /// Append a batch (`docs/DURABILITY.md` §1, steps A1–A8).
    pub fn append(
        &mut self,
        source_id: &str,
        table: &str,
        entries: Vec<(Row, i64)>,
        dedup_token: &str,
        faults: &mut FaultInjector,
    ) -> Result<Ack> {
        if faults.reached(Seam::AckBeforeValidate) {
            return Err(LogError::InjectedFault(Seam::AckBeforeValidate.name()));
        }

        // A1 · validate. A malformed batch is refused and nothing is written.
        let schema = self
            .catalog
            .get(table)
            .ok_or_else(|| LogError::UnknownTable(table.to_owned()))?;
        for (row, _) in &entries {
            if row.len() != schema.len() {
                return Err(LogError::ZSet(current_zset::ZSetError::ArityMismatch {
                    expected: schema.len(),
                    found: row.len(),
                }));
            }
            for (index, value) in row.values().iter().enumerate() {
                schema.check_value(index, value)?;
            }
        }

        let record = Record::Append {
            source_id: source_id.to_owned(),
            dedup_token: dedup_token.to_owned(),
            table: table.to_owned(),
            entries: entries.clone(),
        };
        let hash = record.content_hash();

        // A2–A4 · dedup. Same token, same content is a replay; same token, different content is a
        // caller bug and is refused loudly (I-4).
        if let Some(known) = self.dedup.get(dedup_token) {
            if *known == hash {
                return Ok(Ack::DroppedAsReplay);
            }
            return Err(LogError::TokenReused {
                source_id: source_id.to_owned(),
                token: dedup_token.to_owned(),
            });
        }

        if faults.reached(Seam::AckBeforeAppend) {
            return Err(LogError::InjectedFault(Seam::AckBeforeAppend.name()));
        }

        // A5 · append.
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.segment)?;
        file.write_all(&frame(&record.encode()))?;

        if faults.reached(Seam::AckAfterAppendBeforeFsync) {
            // The record may or may not be on disk, and either is correct: no ack was given. If a
            // partial record landed it is a torn tail and R5 discards it.
            return Err(LogError::InjectedFault(
                Seam::AckAfterAppendBeforeFsync.name(),
            ));
        }

        // A6 · fsync. Nothing above this line is a promise; everything below it is.
        if self.sync == SyncPolicy::Full {
            file.sync_all()?;
        }

        if faults.reached(Seam::AckAfterFsyncBeforeIndex) {
            // Durable, and the caller has no ack. A retry with the same token will be dropped as a
            // replay after the index is rebuilt from the log — which is why A3 is load-bearing.
            return Err(LogError::InjectedFault(
                Seam::AckAfterFsyncBeforeIndex.name(),
            ));
        }

        // A7 · index.
        self.dedup.insert(dedup_token.to_owned(), hash);
        self.pending.push(Batch {
            source_id: source_id.to_owned(),
            table: table.to_owned(),
            entries,
        });

        if faults.reached(Seam::AckAfterFsyncBeforeAck) {
            return Err(LogError::InjectedFault(Seam::AckAfterFsyncBeforeAck.name()));
        }

        // A8 · ack.
        Ok(Ack::Appended)
    }

    /// Seal an epoch (`docs/DURABILITY.md` §2, steps S1–S2).
    ///
    /// The circuit step (S3) and the counter (S4) belong to the caller, which is why they are not
    /// here: the log's job ends when the seal record is durable, and that is the commit point.
    pub fn seal_epoch(&mut self, faults: &mut FaultInjector) -> Result<Epoch> {
        if faults.reached(Seam::SealBeforeRecord) {
            return Err(LogError::InjectedFault(Seam::SealBeforeRecord.name()));
        }
        let epoch = self.sealed_epoch() + 1;

        // S1 · record.
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.segment)?;
        file.write_all(&frame(&Record::SealEpoch { epoch }.encode()))?;

        if faults.reached(Seam::SealAfterRecordBeforeFsync) {
            return Err(LogError::InjectedFault(
                Seam::SealAfterRecordBeforeFsync.name(),
            ));
        }

        // S2 · fsync. The epoch is sealed here and nowhere else.
        if self.sync == SyncPolicy::Full {
            file.sync_all()?;
        }

        self.sealed.push(std::mem::take(&mut self.pending));
        Ok(epoch)
    }

    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.dir
    }

    #[must_use]
    pub fn segment_path(&self) -> &Path {
        &self.segment
    }

    /// Tokens the log knows about — for tests, and for reporting.
    #[must_use]
    pub fn known_tokens(&self) -> usize {
        self.dedup.len()
    }

    /// A deterministic rendering of the whole log, for crash comparisons (I-2, I-7).
    pub fn render(&self) -> String {
        let mut out = format!(
            "log @ epoch {} · {} pending · {} token(s)\n",
            self.sealed_epoch(),
            self.pending.len(),
            self.dedup.len()
        );
        for (index, batches) in self.sealed.iter().enumerate() {
            out.push_str(&format!("epoch {}\n", index + 1));
            for batch in batches {
                for (row, weight) in &batch.entries {
                    out.push_str(&format!(
                        "  {}/{}: {row} => {weight}\n",
                        batch.source_id, batch.table
                    ));
                }
            }
        }
        out
    }
}
