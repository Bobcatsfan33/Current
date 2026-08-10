# DURABILITY — the exact orderings, and every point a crash may land

**This document is written before the code.** §6 C4's pitfall says so in as many words:

> fsync discipline — write down the exact ordering (state flush → checkpoint record → log trim) in a
> doc comment before implementing, and have the crash harness kill between each pair.

Durability bugs are not found by reading code, because the code looks right at every line. They are
found by naming the instants between the lines and then landing on each one deliberately. So this
document numbers the instants. **The crash harness enumerates the kill points defined here**; a seam
that is not in this document is a seam nothing tests.

Rules referenced as `S-n` are in `docs/SEMANTICS.md`; invariants as `I-n` are in `ARCHITECTURE.md`
§4.

---

## 0 · What durability has to deliver

Two invariants, and everything below exists to serve them.

- **I-4 · Exactly-once ingest.** An acknowledged input batch is applied in exactly one epoch,
  survives crashes, and is never applied twice. Replays are detected and suppressed at the log.
- **I-7 · Crash equals replay.** Recovery = load last checkpoint + replay log suffix, and the
  recovered state is byte-identical to a process that never crashed — *provable* because of I-2.

The word **provable** is the load-bearing one. Because everything downstream of a sealed epoch is a
deterministic function of the log (I-2, D-6), "byte-identical to a twin that never crashed" is a
comparison a test can actually make: run a scenario twice, crash one of them, compare state
fingerprints and answers. Without determinism this would be untestable and the invariant would be a
hope.

## 1 · The ack sequence

What happens when a source appends a batch. **Nothing is acknowledged before it is durable**, and
the durable record is what a later replay is checked against.

| Step | Action | Durable after? |
| --- | --- | --- |
| **A1** | Validate the batch against the table's schema (S-2). A malformed batch is refused and *nothing is written*. | no |
| **A2** | Look the `dedup_token` up in the in-memory dedup index, which was rebuilt from the log at open. | no |
| **A3** | If the token is known **and the content hash matches** → acknowledge and drop. Idempotent by construction; no write. | no |
| **A4** | If the token is known and the content hash **differs** → refuse loudly (`TokenReused`). Never silently rewritten. | no |
| **A5** | Append the record — length, CRC, then payload — to the open segment file. | not yet |
| **A6** | `fsync` the segment file. | **yes, here** |
| **A7** | Insert the token into the in-memory dedup index. | yes |
| **A8** | Return the ack to the caller. | yes |

**Why A4 refuses rather than overwriting.** A reused token with different content is not a replay,
it is a *bug in the caller* — two different batches claiming the same identity. Accepting either one
silently would make "exactly once" a statement about counting rather than about identity, and the
wrong batch would be the one that survived. §5.4 says "refused loudly" and that is what it means.

**Why the fsync is at A6 and not later.** The ack at A8 is a promise that the batch survives a crash.
A promise made before the data is on disk is the classic acknowledged-then-lost bug, and it is
invisible in any test that does not actually crash — which is why it is the first of C4's two
canonical mutations (§6 below).

### Kill points in the ack sequence

| Kill point | Lands between | What recovery must show |
| --- | --- | --- |
| `AckBeforeValidate` | before A1 | the batch never existed; the caller has no ack, so it may retry with the same token |
| `AckBeforeAppend` | A4 → A5 | as above: no record, no ack |
| `AckAfterAppendBeforeFsync` | A5 → A6 | the record may or may not be on disk. **Either is correct** — no ack was given. If a partial record is on disk it is a torn tail and is discarded (§4) |
| `AckAfterFsyncBeforeIndex` | A6 → A7 | the record **is** durable and the caller got no ack. On replay the token is found in the log, so a retry with the same token is acknowledged-and-dropped (A3) and the batch is applied exactly once |
| `AckAfterFsyncBeforeAck` | A7 → A8 | same as above. This is the case that makes A3 load-bearing: without dedup, the caller's retry would double-apply |

`AckAfterFsyncBeforeIndex` and `AckAfterFsyncBeforeAck` are the two that matter. They are the states
in which the system knows something the caller does not, and the dedup index — rebuilt from the log,
never from memory — is what closes the gap.

## 2 · The seal sequence

Sealing an epoch makes its batches visible together (S-6, I-3).

| Step | Action | Durable after? |
| --- | --- | --- |
| **S1** | Append a `SealEpoch(n)` record to the segment. | not yet |
| **S2** | `fsync` the segment. | **yes, here** |
| **S3** | Step every resident circuit for epoch `n`. | in memory only |
| **S4** | Advance the in-memory sealed-epoch counter. | in memory only |

**The seal record is the commit point, not the circuit step.** A sealed epoch is a fact about the
*log*; the circuit's state for it is a deterministic function of that fact (I-2), so a crash after S2
and before S4 loses nothing — recovery replays epoch `n` from the log and arrives at the same state.
This is why the log is the source of truth and operator state is a cache of it.

### Kill points in the seal sequence

| Kill point | Lands between | What recovery must show |
| --- | --- | --- |
| `SealBeforeRecord` | before S1 | epoch `n` was never sealed; its batches are durable but not yet visible, and will be sealed by whatever the caller does next |
| `SealAfterRecordBeforeFsync` | S1 → S2 | torn tail: the seal record may be absent or partial. Absent or partial ⇒ not sealed (§4). Either outcome is consistent |
| `SealAfterFsyncBeforeStep` | S2 → S3 | epoch `n` **is** sealed. Recovery must replay it and reach the same state as a twin that stepped it normally |
| `SealAfterStepBeforeCounter` | S3 → S4 | same as above. The step is not durable and is redone; redoing it is safe *because* it is deterministic |

## 3 · The checkpoint sequence

The ordering §5.5 and §6 C4 name: **state flush → checkpoint record → log trim**.

| Step | Action | Durable after? |
| --- | --- | --- |
| **C1** | Serialise every operator's state and both circuit stores into a **new** checkpoint directory, `ckpt-<epoch>.partial`. | not yet |
| **C2** | `fsync` each state file, then the checkpoint directory. | the files, yes |
| **C3** | Write `MANIFEST` inside the checkpoint — the epoch number and a checksum over every state file — and `fsync` it. | the manifest, yes |
| **C4** | Atomically rename `ckpt-<epoch>.partial` → `ckpt-<epoch>`, then `fsync` the parent directory. | **the checkpoint exists, here** |
| **C5** | Update `CURRENT` to name `ckpt-<epoch>`, by write-to-temp + rename + `fsync` parent. | **the checkpoint is current, here** |
| **C6** | Trim log segments wholly before the checkpoint's epoch. | yes |
| **C7** | Delete superseded checkpoint directories. | yes |

**Publish-then-swap, never in-place.** A checkpoint becomes visible only at C4/C5, by rename. A
crash at any earlier point leaves a `.partial` directory that recovery ignores and deletes. This is
the same discipline C7 will need for compaction, and it is the reason a torn checkpoint cannot be
mistaken for a good one: a torn checkpoint is one that never got renamed.

**Why the trim is last, at C6.** The log is the source of truth. Trimming before the checkpoint is
current would create a window in which neither holds the history — the exact window in which a crash
loses committed data. The ordering is not a preference; reversing it is a data-loss bug.

**Why the manifest carries a checksum.** A renamed directory is atomic with respect to *its own
creation*, but the files inside it were written by an earlier step and could have been torn by a
crash between C1 and C2 on a filesystem that reorders. The checksum is what makes "torn checkpoint
detected" a fact rather than an assumption, and skipping it is the second of C4's canonical
mutations.

### Kill points in the checkpoint sequence

| Kill point | Lands between | What recovery must show |
| --- | --- | --- |
| `CheckpointBeforeStateFlush` | before C1 | no new checkpoint; the previous one plus the log suffix reconstructs the state |
| `CheckpointAfterStateFlushBeforeFsync` | C1 → C2 | a `.partial` directory with possibly-torn files. Ignored and deleted; previous checkpoint used |
| `CheckpointAfterFsyncBeforeManifest` | C2 → C3 | a `.partial` with good files but no manifest. Ignored — no manifest, no checkpoint |
| `CheckpointAfterManifestBeforePublish` | C3 → C4 | a complete `.partial` that was never renamed. **Still ignored**: publication is the commit point, and a checkpoint that was not published never happened |
| `CheckpointAfterPublishBeforeCurrent` | C4 → C5 | `ckpt-<n>` exists but `CURRENT` still names the older one. The older one is used and the log suffix covers the gap. Correct, and slower — which is the right trade |
| `CheckpointAfterCurrentBeforeTrim` | C5 → C6 | the new checkpoint is current and the log is longer than it needs to be. Replay of an already-checkpointed prefix must be **harmless**, which it is, because recovery replays only the suffix after the checkpoint's epoch |
| `CheckpointAfterTrimBeforeCleanup` | C6 → C7 | stale checkpoint directories left behind. Cleaned up on the next open; they are never selected because `CURRENT` names the live one |

## 4 · The recovery sequence

| Step | Action |
| --- | --- |
| **R1** | Read `CURRENT`. If it is missing or names a directory that is absent, fall back to the newest **published** checkpoint whose manifest verifies; if none, start from epoch 0. |
| **R2** | Verify the chosen checkpoint's manifest checksums. On mismatch, discard it and repeat R1 with the next-newest. |
| **R3** | Load operator state and both circuit stores from it; the circuit is now as of the checkpoint's epoch. |
| **R4** | Delete every `.partial` directory and every checkpoint not reachable from `CURRENT`. |
| **R5** | Scan log segments from the beginning of the retained log, verifying each record's CRC. **Stop at the first record that fails CRC or is short** — that is the torn tail, and everything after it is discarded. |
| **R6** | Rebuild the dedup index from every `Append` record in the retained log. |
| **R7** | Replay epochs **after** the checkpoint's epoch, sealing each one and stepping the circuit exactly as the live path does. |

**Torn tails are expected, not exceptional.** A crash between A5 and A6, or S1 and S2, leaves a
partial record. R5's rule — stop at the first bad record — is what makes that a non-event. It is
also why every record carries its length *and* a CRC: a length alone cannot distinguish a short
write from a valid record whose payload happens to look like a length.

**Recovery must be idempotent.** A crash *during* recovery must leave the next recovery able to
reach the same state. R1–R7 only read the log and the checkpoint, and write nothing except the R4
cleanup — which is itself idempotent, because deleting an already-deleted directory is a no-op. This
is a bug class that has bitten sibling systems, so the gate tests it explicitly rather than arguing
it from the code.

## 5 · What the crash harness does with this document

The named kill points above are **deterministic seams in the code**, not timers. Each is a call to a
fault hook that, when the seed's fault plan selects it, aborts the operation and discards every
in-memory object — the same information loss as process death, at a point that can be named and
reproduced.

The harness runs two kinds of fault, chosen by seed:

1. **Seam faults** — the named points above, at a chosen occurrence (the *k*th time that seam is
   reached), so that a crash on the third checkpoint is as reachable as one on the first.
2. **Byte-boundary faults** — truncate a log segment or a checkpoint file at a random byte offset, or
   flip a byte. These are the faults no seam enumeration can predict, and they are what exercises R2
   and R5.

**The harness asserts the fault count it injected.** A crash suite that injects no faults passes
trivially and proves nothing; C3 learned that lesson from a mutation that silently failed to apply,
and the same discipline applies here. Every cycle reports which fault fired, and the gate fails if
the total is zero or if any named seam was never reached.

**Everything is seeded (I-2, D-6).** Which seam, which occurrence, which byte offset, and the
scenario itself all come from one seed. There is no timing in the harness, no sleep, no wall clock,
and no thread scheduling — a crash test that is flaky is worse than no crash test, because it teaches
people to re-run.

### What is simulated, and what is not

The 10,000-cycle gate uses in-process fault injection: the fault hook aborts and the harness drops
all in-memory state before recovering from disk. **It is not 10,000 process kills**, and saying so
matters:

- what it faithfully models: loss of everything not yet written to disk, at a named instant;
- what it does not model: kernel-level reordering of writes that never reached the filesystem, and
  anything the OS does to a dying process that our own code cannot observe.

A separate, smaller test does use real `kill -9` on a child process, over the same scenarios and
asserting the same invariants. Its job is to check that the in-process model is faithful — if the
simulation were wrong, the real-kill test is where that shows up. The counts of each are reported
separately in `docs/PROGRESS.md` rather than added together.
