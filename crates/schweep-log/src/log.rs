//! `schweep-log` v1: a directory of files (`ARCHITECTURE.md` §5.4; `docs/DURABILITY.md` §1, §2, §4).
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

use schweep_zset::{Row, Schema};

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
    /// The token this batch was acknowledged under (I-4).
    ///
    /// Carried because compaction *rewrites* the retained records rather than copying bytes, and a
    /// rewritten record must carry the token the original did — otherwise reopening a compacted log
    /// would rebuild a dedup index full of invented tokens, and the real ones would be known only from
    /// the snapshot's ledger. That would still refuse a replay, by luck, while the index drifted from
    /// the records that produced it. A batch that knows its own token cannot drift.
    pub dedup_token: String,
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
    /// Batches per sealed epoch, in epoch order. Epoch `n` is at index `n - retained_from - 1`.
    sealed: Vec<Vec<Batch>>,
    /// The live segment file.
    segment: PathBuf,
    /// The last epoch whose records compaction discarded; 0 before any compaction.
    ///
    /// Epochs at or below this are gone from the log and live only in the snapshot. `sealed` holds
    /// epochs `retained_from + 1 ..= sealed_epoch()`, which is why every index arithmetic in this file
    /// goes through [`Log::epoch`] rather than subtracting one by hand.
    retained_from: Epoch,
    /// The live snapshot directory, if a compaction has published one.
    snapshot: Option<PathBuf>,
}

/// Where the log's authority lives: the segment, the snapshot, and the epoch they meet at.
///
/// Written by compaction's P7 as a single file, so that moving authority from one consistent pair to
/// another is one rename (`docs/DURABILITY.md` §4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pointer {
    pub segment: String,
    pub snapshot: Option<String>,
    pub retained_from: Epoch,
}

impl Pointer {
    /// The pointer's on-disk form: readable text, with a CRC over the body.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let body = format!(
            "schweep-log pointer v1\nsegment={}\nsnapshot={}\nretained_from={}\n",
            self.segment,
            self.snapshot.as_deref().unwrap_or("-"),
            self.retained_from
        );
        format!("{body}crc={:08x}\n", crate::record::crc32(body.as_bytes())).into_bytes()
    }

    pub fn decode(bytes: &[u8]) -> Result<Pointer> {
        let text =
            std::str::from_utf8(bytes).map_err(|_| LogError::Corrupt("pointer is not UTF-8"))?;
        let (body, crc_line) = text
            .rsplit_once("crc=")
            .ok_or(LogError::Corrupt("pointer has no crc"))?;
        let expected = u32::from_str_radix(crc_line.trim(), 16)
            .map_err(|_| LogError::Corrupt("pointer crc is not hex"))?;
        if crate::record::crc32(body.as_bytes()) != expected {
            return Err(LogError::Corrupt("pointer failed its CRC"));
        }
        let mut segment = None;
        let mut snapshot = None;
        let mut retained_from = 0u64;
        for line in body.lines() {
            match line.split_once('=') {
                Some(("segment", value)) => segment = Some(value.to_owned()),
                Some(("snapshot", "-")) => snapshot = None,
                Some(("snapshot", value)) => snapshot = Some(value.to_owned()),
                Some(("retained_from", value)) => {
                    retained_from = value
                        .parse()
                        .map_err(|_| LogError::Corrupt("pointer epoch is not a number"))?;
                }
                _ => {}
            }
        }
        Ok(Pointer {
            segment: segment.ok_or(LogError::Corrupt("pointer names no segment"))?,
            snapshot,
            retained_from,
        })
    }
}

/// The segment a log with no pointer uses.
const DEFAULT_SEGMENT: &str = "segment-00000001.log";
/// The pointer compaction's P7 swaps.
const POINTER: &str = "LOG";
/// The dedup ledger inside a snapshot directory (P2, R7).
pub const DEDUP_LEDGER: &str = "DEDUP";

fn read_pointer(dir: &Path) -> Result<Option<Pointer>> {
    match fs::read(dir.join(POINTER)) {
        Ok(bytes) => match Pointer::decode(&bytes) {
            Ok(pointer) => Ok(Some(pointer)),
            // A pointer that does not verify is treated as absent: the default segment is
            // authoritative, which is the same outcome as a crash before P7 (§4).
            Err(_) => Ok(None),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Write the pointer by write-to-temp + rename + fsync parent — P7, and the only commit point.
fn write_pointer(dir: &Path, pointer: &Pointer, sync: SyncPolicy) -> Result<()> {
    let temp = dir.join("LOG.partial");
    {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&temp)?;
        file.write_all(&pointer.encode())?;
        if sync == SyncPolicy::Full {
            file.sync_all()?;
        }
    }
    fs::rename(&temp, dir.join(POINTER))?;
    sync_dir(dir, sync)
}

fn sync_dir(dir: &Path, sync: SyncPolicy) -> Result<()> {
    if sync == SyncPolicy::Full {
        // A rename is only durable once the directory holding it is synced.
        File::open(dir)?.sync_all()?;
    }
    Ok(())
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

        // R5 · read `LOG`. A pointer that is absent, unreadable, or names artefacts that are not
        // there leaves the default segment authoritative — which is exactly what every kill point
        // before compaction's P7 must produce (§4).
        let pointer = read_pointer(&dir)?;
        let (segment, snapshot, retained_from) = match pointer {
            Some(pointer) => {
                let segment = dir.join(&pointer.segment);
                let snapshot = pointer.snapshot.as_ref().map(|name| dir.join(name));
                let usable = segment.exists()
                    && snapshot
                        .as_ref()
                        .is_none_or(|path| path.join("MANIFEST").exists());
                if usable {
                    (segment, snapshot, pointer.retained_from)
                } else {
                    (dir.join(DEFAULT_SEGMENT), None, 0)
                }
            }
            None => (dir.join(DEFAULT_SEGMENT), None, 0),
        };

        let mut log = Log {
            dir,
            sync,
            catalog,
            dedup: BTreeMap::new(),
            pending: Vec::new(),
            sealed: Vec::new(),
            segment,
            retained_from,
            snapshot: snapshot.clone(),
        };

        // R7 · seed the dedup index from the snapshot's ledger *before* scanning the segment. This is
        // what carries I-4 across a compaction: the tokens acknowledged in the discarded prefix are
        // remembered here and nowhere else.
        if let Some(snapshot) = &snapshot {
            let ledger = snapshot.join(DEDUP_LEDGER);
            match fs::read(&ledger) {
                Ok(bytes) => log.dedup = crate::dedup::decode(&bytes)?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // A published snapshot with no ledger cannot be trusted to keep I-4, so it is a
                    // corruption rather than an absence.
                    return Err(LogError::Corrupt("snapshot has no dedup ledger"));
                }
                Err(e) => return Err(e.into()),
            }
        }

        log.replay_from_disk(faults)?;
        Ok(log)
    }

    /// The epoch whose records the log no longer holds; 0 before any compaction.
    #[must_use]
    pub fn retained_from(&self) -> Epoch {
        self.retained_from
    }

    /// The live snapshot directory, if a compaction has published one.
    #[must_use]
    pub fn snapshot(&self) -> Option<&Path> {
        self.snapshot.as_deref()
    }

    /// The dedup ledger this log would write into a snapshot (compaction's P2).
    #[must_use]
    pub fn dedup_ledger(&self) -> Vec<u8> {
        crate::dedup::encode(&self.dedup)
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
                    self.dedup
                        .insert(dedup_token.clone(), replayed.content_hash());
                    pending.push(Batch {
                        source_id,
                        dedup_token,
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
        self.retained_from + self.sealed.len() as Epoch
    }

    #[must_use]
    pub fn pending_batches(&self) -> &[Batch] {
        &self.pending
    }

    /// The batches of one sealed epoch, for replay.
    ///
    /// An epoch at or below [`Log::retained_from`] is not an error the caller can recover from by
    /// retrying: those records are in the snapshot, and asking the log for them is asking the wrong
    /// artefact. `EpochCompacted` says so by name rather than reporting it as out of range.
    pub fn epoch(&self, epoch: Epoch) -> Result<&[Batch]> {
        if epoch != 0 && epoch <= self.retained_from {
            return Err(LogError::EpochCompacted {
                requested: epoch,
                retained_from: self.retained_from,
            });
        }
        if epoch == 0 || epoch > self.sealed_epoch() {
            return Err(LogError::EpochOutOfRange {
                requested: epoch,
                sealed: self.sealed_epoch(),
            });
        }
        let index = (epoch - self.retained_from - 1) as usize;
        self.sealed
            .get(index)
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
                return Err(LogError::ZSet(schweep_zset::ZSetError::ArityMismatch {
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
            dedup_token: dedup_token.to_owned(),
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

    /// Compaction's log side — **P6, P7, P8** of `docs/DURABILITY.md` §4.
    ///
    /// The snapshot has already been written and published by the caller (P2–P5); this writes the
    /// retained suffix to a new segment, swaps authority with one rename, and only then deletes the
    /// superseded segment.
    ///
    /// `anchor` is the epoch the snapshot covers. Records for epochs after it, and the appends not yet
    /// sealed into any epoch, are what the new segment holds.
    pub fn compact(
        &mut self,
        anchor: Epoch,
        snapshot: &Path,
        faults: &mut FaultInjector,
    ) -> Result<()> {
        if anchor <= self.retained_from {
            return Err(LogError::NothingToCompact {
                anchor,
                retained_from: self.retained_from,
            });
        }
        if anchor > self.sealed_epoch() {
            return Err(LogError::EpochOutOfRange {
                requested: anchor,
                sealed: self.sealed_epoch(),
            });
        }
        let snapshot_name = snapshot
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(LogError::Corrupt("snapshot path has no name"))?
            .to_owned();

        // P6 · write the retained suffix to a *new* segment. The old one is untouched and stays
        // authoritative until P7.
        let next = self.next_segment_name();
        let partial = self.dir.join(format!("{next}.partial"));
        let mut bytes = Vec::new();
        for epoch in (anchor + 1)..=self.sealed_epoch() {
            for batch in self.epoch(epoch)? {
                bytes.extend_from_slice(&frame(
                    &Record::Append {
                        source_id: batch.source_id.clone(),
                        dedup_token: batch.dedup_token.clone(),
                        table: batch.table.clone(),
                        entries: batch.entries.clone(),
                    }
                    .encode(),
                ));
            }
            bytes.extend_from_slice(&frame(&Record::SealEpoch { epoch }.encode()));
        }
        for batch in &self.pending {
            bytes.extend_from_slice(&frame(
                &Record::Append {
                    source_id: batch.source_id.clone(),
                    dedup_token: batch.dedup_token.clone(),
                    table: batch.table.clone(),
                    entries: batch.entries.clone(),
                }
                .encode(),
            ));
        }
        {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&partial)?;
            file.write_all(&bytes)?;
            if self.sync == SyncPolicy::Full {
                file.sync_all()?;
            }
        }
        fs::rename(&partial, self.dir.join(&next))?;
        sync_dir(&self.dir, self.sync)?;

        if faults.reached(Seam::CompactAfterSegmentBeforePointer) {
            // Both a whole log and a complete snapshot+suffix are on disk, and `LOG` still names the
            // old pair. The old log is authoritative; the new artefacts are orphans.
            return Err(LogError::InjectedFault(
                Seam::CompactAfterSegmentBeforePointer.name(),
            ));
        }

        // P7 · THE SWAP. One rename moves authority from one consistent pair to another.
        let pointer = Pointer {
            segment: next.clone(),
            snapshot: Some(snapshot_name),
            retained_from: anchor,
        };
        write_pointer(&self.dir, &pointer, self.sync)?;

        if faults.reached(Seam::CompactAfterPointerBeforeTrim) {
            // The swap happened. The superseded segment is still on disk and nothing reads it, because
            // `LOG` does not name it.
            return Err(LogError::InjectedFault(
                Seam::CompactAfterPointerBeforeTrim.name(),
            ));
        }

        // P8 · delete the superseded segment. Only now: before P7 it was the authoritative one.
        let superseded = std::mem::replace(&mut self.segment, self.dir.join(&next));
        if superseded != self.segment {
            let _ = fs::remove_file(&superseded);
        }

        // The in-memory view follows the on-disk one: the compacted epochs are gone from `sealed`,
        // and `retained_from` is what keeps `epoch(n)` honest about which epochs the log still has.
        let drop_count = (anchor - self.retained_from) as usize;
        self.sealed.drain(0..drop_count.min(self.sealed.len()));
        self.retained_from = anchor;
        self.snapshot = Some(snapshot.to_path_buf());
        Ok(())
    }

    /// The segment file a compaction would write next.
    fn next_segment_name(&self) -> String {
        let current = self
            .segment
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| stem.rsplit('-').next())
            .and_then(|digits| digits.parse::<u64>().ok())
            .unwrap_or(1);
        format!("segment-{:08}.log", current + 1)
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

    /// Every acknowledged token, in order.
    ///
    /// Named rather than counted, because I-4 is a statement about *which* batches were applied. A
    /// count agrees with itself while the identities drift.
    pub fn tokens(&self) -> impl Iterator<Item = &str> {
        self.dedup.keys().map(String::as_str)
    }

    /// A deterministic rendering of the whole log, for crash comparisons (I-2, I-7).
    pub fn render(&self) -> String {
        let mut out = format!(
            "log @ epoch {} · retained from {} · {} pending · {} token(s)\n",
            self.sealed_epoch(),
            self.retained_from,
            self.pending.len(),
            self.dedup.len()
        );
        for (index, batches) in self.sealed.iter().enumerate() {
            out.push_str(&format!(
                "epoch {}\n",
                self.retained_from + index as Epoch + 1
            ));
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
