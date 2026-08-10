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

## Status: Sprint C2 complete

Current is near the beginning. Sprints are numbered C0–C13 and a sprint is complete only when its
exit gate is green in CI. There are no dates.

**What exists today:** the correctness machinery (C0), the incremental engine's machinery (C1),
and the first bilinear operator (C2). A query over one or two tables — a scan or an INNER
equi-join, with a `WHERE` and a projection — compiles to a circuit that maintains its answer from
deltas and never re-reads the input. Every answer is checked against a from-scratch recomputation
at every sealed epoch, over ~1,100 rung-1 and ~1,100 rung-2 randomized scenarios full of
retractions, weight multiplicities, and same-epoch updates.

**What does not exist yet:** no aggregation (C3), no durability (C4), no SQL (C5) — circuits are
hand-built — no shared circuitry (C6), no server (C9). Nothing here is usable as a database today,
and nothing here is fast: operator state is a `BTreeMap` walked linearly per probe, operators
materialise rows out of the columnar batch, and `current-oracle` is *deliberately* slow, because
its job is to be obviously correct, not quick.

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
crates/current-plan/     the logical plan, the binder, the scalar expression library
crates/current-oracle/   the naive reference engine — the spec, and the arbiter of disputes
crates/current-ops/      circuit operators: filter, project, and the equi-join
crates/current-circuit/  the circuit: DAG wiring, epochs, step scheduler, result stores
crates/current-state/    the StateBackend trait and MemBackend: operator state behind an interface
testing/differential/    the oracle harness: seeded scenarios, engine vs oracle, every epoch
testing/evidence/        the ledger, and the artifacts its entries cite
docs/                    SEMANTICS.md, PROGRESS.md, DECISIONS.md
```

`current-plan` is not in `ARCHITECTURE.md` §5's crate map; it was added in C1 and the reason is
recorded as **D-14** in [`docs/DECISIONS.md`](docs/DECISIONS.md), before the code moved.

**Known limitations, before you find them:** an evaluation error means something different to the
incremental engine than to the oracle, so both differential gates currently run only on scenarios
that cannot raise — open question **Q-2**, scheduled to be decided at the start of C3.
`MemBackend`'s prefix scan is a linear walk, which is the wrong complexity for a join.

Crates named in §5 that do not appear above have not been written yet.

## License

Apache-2.0, permanently — see [LICENSE](LICENSE). The engine is open because a correctness claim
nobody can audit is worthless.
