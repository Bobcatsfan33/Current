# Current

[![CI](https://github.com/Bobcatsfan33/Current/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Bobcatsfan33/Current/actions/workflows/ci.yml)

**The incremental-first query engine.** Every major database of the last two decades —
ClickHouse, Snowflake, Elasticsearch, MongoDB, Postgres — shares one assumption: a query is a
one-shot program. You ask; the engine reads the data, computes the answer, returns it, and
forgets everything. Ask again after three rows changed and it recomputes everything from
scratch. The cost of a question is O(data), every time. Current inverts the assumption: a query
is a *standing computation*. The first time a query is asked, Current compiles it into a
dataflow circuit and runs the data through it once. From then on the circuit stays alive: every
batch of changes (a *delta*) flows through it, and the circuit updates its answer incrementally.
The cost of keeping an answer correct is O(change), and the cost of reading an answer is a
lookup. A one-shot query is just the degenerate case: a circuit fed one big delta (the whole
dataset) and then torn down — same machinery, one code path.

**The one-sentence pitch:** every answer, current.

Current is the compute plane of a future database called MutinyDB, but it is a **standalone
engine**: it has no dependency on any sibling system, and none may be added.

## Status: Sprint C0 in progress

Current is at the very beginning. Sprints are numbered C0–C13 and a sprint is complete only when
its exit gate is green in CI. There are no dates.

**What exists today:** the correctness machinery, deliberately built before any engine code —
the Z-set algebra (`current-zset`), the naive reference engine (`current-oracle`), and the
differential harness that will hold every future line of engine code to the oracle's answers.

**What does not exist yet:** the engine. There are no operators, no circuits, no scheduler, no
SQL frontend, no server, no durability, and no persistence. Nothing here is usable as a database
today, and nothing here is fast — `current-oracle` is *deliberately* slow, because its job is to
be obviously correct, not quick.

**Numbers we publish:** none. Per invariant I-10, no performance claim is made without a
committed, reproducible benchmark artifact, and no such artifact exists yet. When they exist
they will live in `testing/evidence/` and be linked from here, with the worst supported
configuration quoted alongside the best.

See [`docs/PROGRESS.md`](docs/PROGRESS.md) for exactly what is proven and by which test.

## Architecture of record

[`ARCHITECTURE.md`](ARCHITECTURE.md) is the architecture of record. It defines the glossary (§1),
the binding decisions D-1…D-9 (§3), the invariants I-1…I-10 (§4), the crate map (§5), the sprint
gates (§6), the testing strategy (§7), and the non-goals (§8). If code and that document
disagree, the document wins; a genuine error in it is corrected by a superseding note in
[`docs/DECISIONS.md`](docs/DECISIONS.md) first, never by quietly diverging code.

Contributors start with [`CONTRIBUTING.md`](CONTRIBUTING.md) and
[`docs/SEMANTICS.md`](docs/SEMANTICS.md).

## The theory is not ours

Current implements the **DBSP** model of incremental computation (Budiu, McSherry, Ryzhyk,
Tannen — VLDB 2023), in which every relational operator has an incremental form that consumes
deltas and emits deltas, and any composition of them is itself incremental. We did not discover
the theory. The work here is building a general-purpose, enterprise-grade, evidence-obsessed
engine on it — and proving every answer against a naive reference implementation that recomputes
from scratch, every time, in CI.

## Repository layout

```
crates/current-zset/     Z-set batches over Arrow; weight algebra; consolidation
crates/current-oracle/   the naive reference engine — the spec, and the arbiter of disputes
testing/differential/    the oracle harness: seeded scenarios, oracle vs engine, every epoch
testing/evidence/        registry.json — the tuned-constant ledger (empty; nothing is tuned yet)
docs/                    SEMANTICS.md, PROGRESS.md, DECISIONS.md
```

Crates named in §5 that do not appear above have not been written yet.

## License

Apache-2.0, permanently — see [LICENSE](LICENSE). The engine is open because a correctness claim
nobody can audit is worthless.
