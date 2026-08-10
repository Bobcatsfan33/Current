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

## Status: Sprint C5 complete (with the gaps named below)

Current is near the beginning. Sprints are numbered C0–C13 and a sprint is complete only when its
exit gate is green in CI. There are no dates.

**What exists today:** the whole query surface `docs/SEMANTICS.md` defines, reachable from **SQL
text**. A scan or an INNER equi-join, an optional `WHERE`, `GROUP BY` with
`SUM`/`COUNT`/`MIN`/`MAX`/`AVG` and `HAVING`, a projection, and `DISTINCT` — compiled to a circuit
that maintains its answer from deltas and never re-reads the input. Every answer is checked against a
from-scratch recomputation at every sealed epoch, over **all 4,400** randomized scenarios the generator
produces: 24,747 answer comparisons, zero divergences, and the scenarios are full of retractions,
weight multiplicities, same-epoch updates, and expressions that raise. The SQL door is checked the same
way over the 2,028 of those scenarios that have a SQL form, and I-6 asserts that both doors compile to
structurally identical plans with identical execution counters.

Anything SQL has that this dialect does not is refused **by name**: 60 such constructs are in
`crates/current-sql/tests/dialect.rs`, each with the message that must name it.

**What does not exist yet:** no shared circuitry (C6), no server (C9). Nothing here is usable as a database today, and nothing here is
fast: operator state is a `BTreeMap` walked linearly per probe, an aggregate re-folds a changed
group's whole value multiset, and `current-oracle` is *deliberately* slow, because its job is to be
obviously correct, not quick.

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
crates/current-ops/      circuit operators: filter, project, equi-join, aggregate, distinct
crates/current-circuit/  the circuit: DAG wiring, epochs, step scheduler, result stores
crates/current-sql/      SQL -> binder -> logical plan -> the incrementalizer -> circuit plan
crates/current-state/    the StateBackend trait, MemBackend, and the order-preserving key codec
crates/current-log/      the input log: a directory of files, epoch sealing, exactly-once admission
testing/crash/           the crash harness: named seams, byte faults, recovery vs an uncrashed twin
testing/differential/    the oracle harness: seeded scenarios, engine vs oracle, every epoch
testing/evidence/        the ledger, and the artifacts its entries cite
docs/                    SEMANTICS.md, PROGRESS.md, DECISIONS.md
```

`current-plan` is not in `ARCHITECTURE.md` §5's crate map; it was added in C1 and the reason is
recorded as **D-14** in [`docs/DECISIONS.md`](docs/DECISIONS.md), before the code moved.

**Known limitations, before you find them:** the SQL door is narrower than the typed API in one
specific way — a query that both groups and projects cannot be written in SQL, because a group key's
output name comes from the select list (S-11, S-36) — and the gate counts how much of its population
that excludes rather than passing over it in silence. There is no non-integer
arithmetic at all: `Float64` is a result-only type produced solely by `AVG`, and fixed-point decimals
are open question **Q-1**. `MemBackend`'s prefix scan is a linear walk, which is the wrong complexity
for a join, and nothing has been benchmarked. **`RocksBackend` is not implemented** — D-5 calls for
it and C4 could not build it in the development environment; `MemBackend` plus checkpoint files is what
durability is proven over today (**D-18**, amended by **D-19** to redb). Nothing tests *power loss*: the crash harness is
in-process, which models losing unwritten state at a named instant but not kernel write reordering.

Crates named in §5 that do not appear above have not been written yet.

## License

Apache-2.0, permanently — see [LICENSE](LICENSE). The engine is open because a correctness claim
nobody can audit is worthless.
