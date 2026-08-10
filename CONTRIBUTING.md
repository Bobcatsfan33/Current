# Contributing to Current

Current is an incremental-first query engine. Its credibility rests on one thing: every answer it
gives is checked, in CI, against a naive reference implementation that recomputes from scratch.
Contributions are welcome on exactly those terms.

**Before anything else, read [`ARCHITECTURE.md`](ARCHITECTURE.md).** It is the architecture of
record and it governs this repository. Then read [`docs/SEMANTICS.md`](docs/SEMANTICS.md) before
touching any code that decides what a query means, and [`CLAUDE.md`](CLAUDE.md) for the working
rules in their day-to-day form.

Licence: Apache-2.0, permanently (D-9). By contributing you agree your contribution is licensed
under it.

---

## The invariants (ARCHITECTURE.md §4, verbatim)

Every ticket names the invariant it preserves; a change that violates one is rejected however
good it is otherwise. Each law states the failure it prevents.

- **I-1 · The oracle law.** At every sealed epoch, for every registered query, the incremental
  answer must equal the oracle's full recomputation, byte for byte, including order (D-7).
  *Prevents:* the incremental engine drifting from SQL semantics in ways no unit test would
  catch. This is the load-bearing invariant of the whole project.
- **I-2 · Determinism.** The state and every answer at epoch N is a pure function of the log
  prefix up to N. No wall clock, no randomness, no dependence on thread scheduling or map
  iteration order anywhere downstream of the ingest boundary. *Prevents:* unverifiable recovery,
  diverging replicas, flaky tests, and answers that change when nothing changed.
- **I-3 · Epoch atomicity.** A reader sees the world as of a sealed epoch — never a partial
  epoch, never a mixture of two. *Prevents:* the classic streaming-system bug where a dashboard
  shows a join of "orders as of now" against "customers as of slightly earlier."
- **I-4 · Exactly-once ingest.** An acknowledged input batch is applied in exactly one epoch,
  survives crashes, and is never applied twice. Replays are detected and suppressed at the log.
  *Prevents:* silent double-counting — the incremental engine's equivalent of data corruption.
- **I-5 · Retraction symmetry.** Negative weights flow through every operator by the same code
  path as positive ones. No operator may special-case deletion. *Prevents:* the "inserts work,
  deletes drift" class of bug that kills incremental systems; also, this symmetry is the
  property MutinyDB's taint-as-retraction will stand on.
- **I-6 · Same door.** SQL and the typed API compile to the same circuit plan and run the same
  code; gate tests assert identical results *and* identical execution counters through both.
  *Prevents:* two dialects of behavior wearing one name.
- **I-7 · Crash equals replay.** Recovery = load last checkpoint + replay log suffix, and the
  recovered state is byte-identical to a process that never crashed (provable because of I-2).
  Crash-injection tests kill the process at randomized boundaries and assert exactly this.
  *Prevents:* recovery paths that "usually work."
- **I-8 · Memo transparency.** Whether a subplan is shared or private may change counters and
  cost, never a result byte. Every memo gate runs the same query shared and unshared and asserts
  identical answers. *Prevents:* cross-query contamination — the catastrophic failure mode of
  shared computation.
- **I-9 · No unbounded state without a declaration.** Every stateful operator declares its state
  bound as a function of its input (e.g., join state is O(|A| + |B|)); the runtime accounts
  actual state against declarations, and an operator exceeding its declaration is a bug, not a
  tuning problem. Unbounded-by-nature constructs (e.g., aggregation over an unbounded key space)
  must be admitted explicitly at query registration. *Prevents:* the slow memory death that
  takes down standing-query systems in month three.
- **I-10 · Honesty.** No performance number without a committed reproducible benchmark artifact;
  every tuned constant in the ledger with its receipt; every known weakness in the README before
  a user finds it; zero-flake test policy — a flaky test is a bug in the test or a bug in the
  engine, and both block merge. *Prevents:* the credibility debt that a database, of all
  products, cannot carry.

---

## What a pull request must say

Every PR names three things, in its description:

1. **The invariant it preserves** (one of I-1…I-10).
2. **The scenario family that covers it** — which generator family in `testing/differential`
   exercises this code, or the new family you added.
3. **The gate that proves it** — the named CI job and test that would fail if you broke it.

A PR that cannot name all three is not ready.

## What CI enforces

| Job | What it runs | Why |
| --- | --- | --- |
| `fmt` | `cargo fmt --all --check` | one style, no diff noise |
| `clippy` | `cargo clippy --all-targets --all-features -- -D warnings` | warnings are errors |
| `test` | `cargo test --workspace --all-features` | correctness, including the differential gate |
| `no-network` | build + test with the network removed (`--offline`, locked deps) | the build must be reproducible and must not fetch at test time |

All four must be green. Locally green is not done; CI green is done.

## The rules that get PRs rejected

- `unwrap`, `expect`, or a deliberate panic in library code. Return a `Result`. (Tests may panic.)
- `unsafe`. Forbidden before sprint C10 — every crate carries `#![forbid(unsafe_code)]`.
- A wall clock or unseeded randomness anywhere in engine or oracle code. All nondeterminism
  enters at the ingest boundary and nowhere else (D-6, I-2).
- Dependence on hash-map iteration order. Use ordered containers or sort before emitting (I-2).
- Special-casing a negative weight inside an operator. Retractions take the same path as
  insertions (I-5).
- Weakening the scenario generator so an implementation passes. The generator defines the bar,
  and the bar includes retractions from epoch one.
- A performance number without a committed benchmark artifact, or a behaviour-steering constant
  without an entry in `testing/evidence/registry.json` (I-10).
- A skipped, ignored, or TODO-ed test. Zero-flake policy: a flaky test blocks merge exactly like
  a failing one.

## The order of operations when semantics are in question

Always, without exception:

1. **`docs/SEMANTICS.md`** — decide it, write it down, say why.
2. **`current-oracle`** — implement it the slow, obvious way.
3. **The engine** — implement it the fast way, and let the differential harness prove they agree.

When the differential harness disagrees with you, the harness is right until proven otherwise,
and "proven otherwise" means the oracle had a bug — which you fix first.

## Running the gate locally

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

The differential gate (1,000 randomized scenarios) runs as part of `cargo test`. Every scenario
is reproducible from its seed: a failure prints the seed, and re-running that seed reproduces the
failure byte-for-byte. If it does not, that is itself a bug — report it as one.

---

*We would rather you read the tests than the marketing.*
