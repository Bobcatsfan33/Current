# DECISIONS

Decision records beyond `ARCHITECTURE.md` §3, as they accumulate (§5, crate map).

Two kinds of entry live here:

- **D-records** (`D-10`, `D-11`, …) — decisions taken. Binding, like D-1…D-9 in §3.
- **Open questions** (`Q-1`, `Q-2`, …) — decisions *deliberately not yet taken*, each with the
  sprint by which it must be settled. An open question written down is engineering; an open
  question discovered later in a user's data is not.

A superseding note for anything in `ARCHITECTURE.md` goes here **before** the code changes.
Nothing in this file supersedes §3 or §4 today.

---

## D-records

### D-10 · Scalar types at rungs 1–3 are `Int64`, `Utf8`, `Boolean`; `Float64` is result-only

*Sprint: C0. Preserves: I-1. Recorded in `docs/SEMANTICS.md` S-3.*

No table column may be declared `Float64` and no scalar expression produces one. The sole source
of a `Float64` value is `AVG`.

**Why.** Floating-point addition is not associative. An incremental `SUM` maintains a running
total while the oracle recomputes from scratch in a different order; for `f64` the two answers
differ in the low bits. I-1 requires byte-for-byte equality, so a float `SUM` would place the
project's load-bearing invariant in permanent conflict with IEEE-754. `i64` addition is
associative and exact, so integers have no such conflict.

`AVG` is exempt because it is never accumulated as a float: an exact `i64` sum and an exact `i64`
count are maintained, and exactly one division is performed at emit time (S-31). Identical inputs
through an identical single operation give identical bits in both implementations.

**Cost, stated plainly.** Schweep cannot represent non-integer measures at all today. That is a
real limitation for real workloads and it is named in the README, not hidden. See Q-1.

### D-11 · Arithmetic errors are errors — not wraps, not saturation, not NULL

*Sprint: C0. Preserves: I-10. Recorded in `docs/SEMANTICS.md` S-20, S-21.*

`i64` overflow in `+`, `-`, `*`, in `SUM`, and division or modulo by zero all raise named errors
and abort evaluation of that query at that epoch. Wrapping is deterministic but wrong; saturating
is deterministic but a lie; coercing to `NULL` hides a bug inside a legitimate value. `SUM`
accumulates in `i128` so that a total which transits large partial values but lands in `i64` range
is correct — the property an incremental sum under retraction needs.

### D-12 · The oracle rejects malformed history rather than defining an answer for it

*Sprint: C0. Preserves: I-1, I-5. Recorded in `docs/SEMANTICS.md` S-5.*

A table's integral must have non-negative weights at every sealed epoch. Retracting a row that is
not present is a malformed history and raises `NegativeIntegral` naming table, row, and epoch.

**Why.** Defining an answer for "−2 copies of a row" would invent semantics nobody asked for, and
would let an ingest or generator bug travel silently through the engine and emerge as a plausible
number. Note the scope: this constrains *table integrals* only. Deltas and intermediate results
carry negative weights freely — that is I-5 and it is the point.

### D-13 · Nulls sort first, everywhere, with no modifier

*Sprint: C0. Preserves: I-2, D-7. Recorded in `docs/SEMANTICS.md` S-7.*

SQL leaves null ordering implementation-defined and offers `NULLS FIRST`/`NULLS LAST`. Schweep
fixes one rule — nulls before all non-null values — and provides no modifier, so there is nothing
for two implementations to disagree about. Strings order byte-wise over their UTF-8 encoding, with
no collation support in v1.

### D-14 · A neutral `schweep-plan` crate holds the logical plan, the binder, and the scalar expression library

*A deviation from the §5 crate map, recorded before the code moved.*

*Sprint: C1 (pre-work). Preserves: I-1, I-6. Supersedes: nothing in §4; **extends the crate map in
`ARCHITECTURE.md` §5**.*

**What changes.** A new crate `crates/schweep-plan/` is added to the workspace, holding three
things that were in `schweep-oracle`:

- the **logical plan IR** — `Query`, `Source`, `Expr`, `BinOp`, `AggFunc`, `GroupBy`, `Named`;
- the **binder** — scope rules, type checking, and the named refusals (S-10, S-12, S-19, S-27);
- the **scalar expression library** — three-valued evaluation (S-13…S-22).

`schweep-zset` additionally gains `EpochDeltas`, the per-table bundle of input deltas, replacing
two private copies (one in `schweep-oracle`, one in the differential harness).

**Why the crate map needed extending.** §5 lists ten crates and none of them is a home for a
logical plan that *both* the oracle and the engine can read. From C1 there are two implementations
of the query surface, and every option other than a neutral crate is worse:

- `schweep-circuit` depending on `schweep-oracle` inverts the relationship the oracle exists to
  have. The oracle is the arbiter (§5.1); an engine that imports it can inherit its bugs, and I-1
  stops being a comparison between two things.
- Duplicating the plan IR in the engine means two spellings of one query shape, which must then be
  kept in step by hand. I-6 ("SQL and the typed API compile to the same circuit plan") is a claim
  about there being *one* plan type; two would make it unprovable.
- Putting the plan in `schweep-zset` breaks that crate's cohesion. It is the data layer — "Z-set
  batches over Arrow; weight algebra; consolidation" — and a query IR is not data.

`EpochDeltas` goes to `schweep-zset` for the opposite reason: it *is* data, it is the delta
representation named in §1, and every crate already depends on that crate. `schweep-log` (§5.4) is
its eventual home for the *write path*, but that is C4 and the type is needed now.

**Why the binder moves with the plan.** The scoping rules (S-10 qualified before a GROUP BY,
S-27 unqualified after it) and the output-schema rules (S-11) decide what a query's answer schema
*is*. Schema equality is part of answer equality (S-8), so if the engine derived schemas by its
own second implementation of those rules, the two could disagree and every disagreement would
surface as a spurious I-1 failure. One binder, one answer schema.

**Why the scalar library moves with it, and what that costs.** §6 C5 already specifies the end
state: "scalar expression library … **implemented once, shared by oracle and engine** but *tested
differentially anyway* (shared code can still be called differently)." C1's filter and project
operators need expression evaluation now, so the choice is to share the library one sprint early
or to write a second one and delete it in C5.

The cost is real and is stated rather than glossed: **a bug inside shared code produces the same
wrong answer on both sides, and the differential harness cannot see it.** Three things bound that
risk, and none of them is "we were careful":

1. `schweep-plan` carries the S-rule unit tests that were in the oracle — the Kleene truth tables,
   checked arithmetic, CASE short-circuiting, null comparison. They pin the library against
   `docs/SEMANTICS.md` directly, not against another implementation.
2. What C1 is actually hunting is *incrementality* bugs — maintaining an answer from deltas versus
   recomputing it from scratch. That machinery is not shared, and the harness sees all of it.
3. §6 C5's parenthesis is the standing warning: shared code can still be *called* differently, and
   the harness does test that.

**Where this lands in C5.** `schweep-sql` becomes "sqlparser AST → a SQL-specific binder →
`schweep_plan::Query` → the incrementalizer". Both doors — SQL text and the typed API — produce
the same `schweep_plan::Query`, which is what I-6 needs in order to be checkable at all.

### D-15 · `StateBackend` keys are `Vec<Value>` ordered by S-7, not bytes

*Sprint: C2. Preserves: I-2, I-9. Realises `ARCHITECTURE.md` §5.5; the trait is frozen at C4 exit.*

§5.5 calls for "ordered KV with range scans, atomic multi-key write batches, and named snapshots".
The obvious reading of "KV" is `Vec<u8>` → `Vec<u8>`. Schweep's `StateBackend` instead uses
`Vec<Value>` keys ordered by the total order on values (S-7), with `i64` values.

**Why.** An order-preserving byte encoding of a row is a real piece of engineering — sign-aware
integer encoding, length-prefixed strings, null ordering — and getting it subtly wrong produces a
backend whose scans return the right rows in the wrong order. That is a *storage* problem, and D-5
and §2 both say storage is the boring part that lives behind the trait: "`schweep-log` and
`schweep-state` must sit behind traits rather than being called concretely from operators." Putting
the encoding in the *interface* would push a storage concern into every operator, and would make
C2's join correctness depend on a serialiser written the same week.

With domain-typed keys, `MemBackend` is a `BTreeMap<Vec<Value>, i64>` and its ordering is the one
`docs/SEMANTICS.md` already defines and tests. `RocksBackend` (C4) will need the byte encoding, and
that is exactly where it belongs — one implementation, tested against `MemBackend` as its oracle.

**What is deliberately absent.** Named snapshots. Checkpoints are C4's deliverable and the protocol
(§5.5: state flush → checkpoint record → log trim) is not designed yet; adding the methods now
would be guessing at their shape. §5.5 says the trait is frozen at C4 exit, so it is allowed to
grow until then, and this records what it is missing so C4 does not have to rediscover it.

**Cost.** `state_size` is reported in *entries*, not bytes, because a `Vec<Value>` has no single
byte size. That is enough for the I-9 accounting C2 needs — the declarations are about how many
rows an operator retains — and not enough for C8's `EXPLAIN STATE`, which wants real memory. C8
replaces it, and until then no claim is made that entries are bytes.

### D-16 · An evaluation error is a property of the contents; the live errors are a Z-set

*Sprint: C3 (decided first, before any operator code). Preserves: I-1, I-2, I-3, I-5. Closes Q-2.
Recorded in `docs/SEMANTICS.md` S-22, S-22a, S-22b, S-22c, S-22d.*

**The decision.** The answer at epoch N is either a Z-set or an error, determined by the *contents*
at epoch N. Data on which the query raises means the query has no answer while that data is present;
retract it and the answer returns.

**The alternative, and why it lost.** C1 found that the two implementations disagreed about an
error's lifetime: the oracle recomputes over the integral, so a bad row raises at every epoch until
it is retracted; the circuit sees each row once, so it raised once and then answered normally again.
The second behaviour is not merely different, it is **incompatible with I-3**. If the epoch that
raises is dropped, the next epoch's changes land on contents that never absorbed the dropped epoch,
and the answer becomes a mixture of epoch N−1 and epoch N+1 — exactly the partial-epoch view I-3
forbids. Under this decision the epoch seals normally and only the *answer* is an error.

It also lost on principle. The engine's claim is that an answer is a function of the current
contents; an error whose presence depended on which epoch delivered a row would make "does this
query have an answer" a property of the delivery schedule.

**The mechanism, and why it is small.** The live errors are maintained as an ordinary Z-set (S-22b):
a row that raises contributes its error message at the row's weight, and retracting the row retracts
the error by the same arithmetic. Nothing special-cases removing an error — that is I-5 applied to
errors — and "the answer comes back when the data leaves" needs no separate machinery. In the engine
the error stream is integrated into a result store exactly like the answer stream; in the oracle the
live errors are collected during the recomputation it already does.

**Two sub-decisions that make I-1 checkable.**

- *Erroring rows are dropped* (S-22a) rather than carried forward with a placeholder. A placeholder
  would have to be invented identically by two implementations that see the data differently.
- *The least message is reported* when several errors are live (S-22c). Which error to report is
  arbitrary; being deterministic about it is not, and a rule stated over messages alone is one both
  implementations can follow without agreeing on scan order, stage order, or which row they looked
  at first. Order-independent by construction rather than by care.

**Cost, stated.** Every fallible operator now carries a second output stream and the state to
maintain it, which is state that has to be declared and accounted under I-9 like any other. A query
in error reports one of possibly several problems, and which one can change as data moves. And the
`an_evaluation_error_aborts_the_step_without_advancing_the_epoch` test from C1 was **wrong under this
rule** and has been replaced: the epoch now seals and the answer is the error, which is the I-3-safe
behaviour the old test was quietly preventing.

**What it opened up.** Error-raising expressions now enter the scenario generator, so the gates check
that the two implementations agree about *which* epochs raise and *what they say* — not merely that
neither raised. The generator's ledger receipts were regenerated as part of the same change, because
the measured population moved.

### D-17 · `DISTINCT` arrives in C3, ahead of its rung, because §6 puts it there

*Sprint: C3. Preserves: I-1. Recorded in `docs/SEMANTICS.md` S-34.*

`ARCHITECTURE.md` §5.6 places `DISTINCT` on **rung 4** of the dialect ladder, alongside `UNION ALL`
and `ORDER BY`/`LIMIT`. §6 C3's build list names it directly: "GROUP BY with SUM/COUNT/AVG … ;
DISTINCT; HAVING as post-aggregate filter."

Both statements are in the architecture of record and they do not quite agree, so this records which
governs what. **§5.6's ladder describes the order the *dialect* grows in; §6 describes what each
*sprint* builds.** Where a sprint's build list names a construct, the sprint list wins for sprint
content — it is the more specific instruction, and §6 is the section a sprint is executed from.
`DISTINCT` is therefore implemented in C3 and the rest of rung 4 is not.

**Why it fits here rather than being awkward.** `DISTINCT` is the third kind of stateful operator, and
C3 is the sprint about stateful operators. It also needs exactly the machinery the aggregate needs:
an integral of its input, kept behind `StateBackend`, with a declared bound (I-9). Building it
alongside the aggregates costs one operator; building it in a later sprint would mean revisiting the
same ground.

**What changes in the plan.** `Query` gains a `distinct: bool`, applied after the projection. That is
a widening of the plan IR both doors will share (D-14), so it is recorded rather than slipped in.

### D-18 · `StateBackend` is frozen — **FINAL as of C8** — and `RocksBackend` is not delivered

*Sprint: C4; freeze made final in C8. Preserves: I-9, D-5. Freezes the trait §5.5 says to freeze at
C4's exit.*

> **The freeze is now FINAL.** It was recorded provisional at C4's exit and again at C5's, on the
> condition that it became final when a second backend validated it, no later than C8 entry. **C8's
> `RedbBackend` (D-19) implements this trait unchanged** — not one method added, removed, or widened —
> so the condition is met and the provisional clause is discharged. What the second implementation
> found is recorded at the end of this entry, because a freeze validated in silence teaches nothing.



**The freeze.** `StateBackend` is now: `write(&WriteBatch)`, `scan_prefix`, `get`, `len`, `iter_all`,
`snapshot`, `restore`. C4 added the last two — the "named snapshots" D-15 deliberately left out until
the checkpoint protocol was designed — and adds nothing further.

**The compatibility promise.** From here, a new backend can be written against this trait without any
operator changing, and the trait will not gain a required method. If a later sprint needs one it
arrives with a default implementation, or as a separate trait an operator may opt into. Concretely,
`RocksBackend` and any custom LSM (D-5) can be added by implementing seven methods.

Two things the freeze deliberately does *not* promise: that `snapshot` bytes are stable across
versions (they are a checkpoint format, and C7's compaction will revisit it), and that entries are a
proxy for memory — `len` counts entries, not bytes, and C8's `EXPLAIN STATE` is what will measure
real memory.

**`RocksBackend` is not delivered, and the freeze is weaker for it.** §6 C4's build list names it and
D-5 mandated it — **D-19 has since amended D-5 to redb**, with these blockers as the trigger.

> **Correction, made in C5 pre-work.** This record originally gave *two* independent blockers, the
> first of which was wrong:
>
> > 1. `librocksdb-sys` runs `bindgen`, which needs `libclang`. The Command Line Tools ship
> >    `libclang.dylib` but the build script is *linked* against `@rpath/libclang.dylib`, so neither
> >    `LIBCLANG_PATH` nor `DYLD_FALLBACK_LIBRARY_PATH` resolves it; the `bindgen-static` feature needs
> >    `libclang.a`, which is not shipped at all.
>
> **`libclang` was never the blocker.** The C4 probe passed `--no-default-features`, which disabled
> `librocksdb-sys`'s `bindgen-runtime` feature; without it `clang-sys` links `libclang` at load time
> instead of `dlopen`-ing it, which is why `LIBCLANG_PATH` had no effect. With
> `features = ["bindgen-runtime"]` and `LIBCLANG_PATH` pointing at the Command Line Tools library,
> **`bindgen` ran fine** — no `brew`, no LLVM install. The diagnosis was mine and it was wrong; it is
> corrected here rather than quietly deleted, because a wrong reason recorded as fact is worse than no
> reason.
>
> The **real** blocker is the one below, and it is sufficient on its own.

1. A `librocksdb-sys` debug build produces over 2.1 GiB of object files and exhausted the machine's
   free disk while archiving them (`No space left on device`). That is a permanent cost for every
   contributor and every CI runner, and it is why D-19 amends D-5.

What that costs, stated rather than glossed: **the freeze is validated by one implementation, not
two.** The whole point of freezing a trait at C4 is that a second backend can then slot in without
touching operators, and that claim is now argued from the interface rather than demonstrated by a
second implementation. The order-preserving byte codec `RocksBackend` will need
(`schweep_state::codec`) *was* built and is tested — byte order equals value order over a seeded
sweep — so the piece most likely to be got wrong is done. But `RocksBackend` itself is outstanding
work, it is named in `docs/PROGRESS.md` as such, and C4's gate does not depend on it: every I-4 and
I-7 claim is proven over `MemBackend` plus real checkpoint files on a real filesystem.

**What the second implementation found (C8).** The paragraph above worried that the freeze was argued
rather than demonstrated. `RedbBackend` demonstrated it, and the findings are worth the record:

- **One method caused friction, and only one.** `len` returns `usize`, not `Result<usize>`; redb cannot
  count a table without a transaction, and a transaction can fail. So the backend maintains the count
  itself, updated inside the write transaction that changes the entries. That is arguably what the
  signature was always asking for — a count you can read without asking the disk — and it cost eight
  lines.
- **Two methods mapped *better* than to `MemBackend`.** `write`'s atomicity and `scan_prefix`'s
  ordering are native to redb: a write transaction gives the first, and the order-preserving codec
  (D-15) makes the second a B-tree range rather than a filtered walk. The codec built in C4 for a
  backend that never arrived turned out to be the piece that made this one straightforward.
- **`snapshot() -> Vec<u8>` is the freeze's real cost, and it is not redb's fault.** A checkpoint
  materialises every entry in memory. So C8 can spill state larger than RAM but **cannot checkpoint
  it** — the spill and the checkpoint have different limits, and the difference is a consequence of
  this signature. It is named in `docs/PROGRESS.md` rather than worked around, because working around
  it means unfreezing the trait, and that is a decision for whoever needs it with a gate to prove it.
- **The trait accounts in entries, and C8's `EXPLAIN STATE` shows why that matters.** The note above
  said entries are not a proxy for memory and that C8 would "measure real memory". It measures what it
  can: entries exactly, bytes as a **floor** plus an independent count. A byte *ceiling* per entry is
  not expressible, because key width is unbounded — which is a fact about the data, not about the
  interface, and one this trait's shape makes visible instead of hiding.

### D-19 · The operator-state backend is **redb**, amending D-5

*Sprint: C5 (pre-work). Amends **D-5** in `ARCHITECTURE.md` §3. Preserves: I-9, D-1.
Implementation is C8-entry work; this record is the decision only.*

D-5 named "embedded LSM (RocksDB via `rust-rocksdb`)" as the first `StateBackend` implementation.
**That is amended: the first non-memory backend will be [`redb`](https://crates.io/crates/redb), a
pure-Rust embedded B-tree store.**

**The trigger.** Two sprints tried to build `rust-rocksdb` and could not, for reasons that are about
the toolchain rather than about Schweep:

- `librocksdb-sys` compiles RocksDB's C++ from source. A debug build produced over 2.1 GiB of object
  files and exhausted the development machine's free disk, which is the failure that actually stopped
  it (`No space left on device`, while archiving).
- It also requires `bindgen`, hence `libclang` — a second toolchain dependency, and one whose failure
  mode is obscure (see the D-18 correction below).

Neither is a Schweep problem, and both are permanent costs paid by every contributor and every CI
runner. D-5 itself says the backend "is an optimization with a known interface, not a research
problem" — so a dependency that makes the *build* a research problem is the wrong trade.

**Why redb, specifically.**

1. **Pure Rust removes both blockers permanently.** No C++ toolchain, no `bindgen`, no multi-GB build.
   That is the whole point of the switch, and it is worth more than architectural kinship with the
   original choice.
2. **Maturity behind a frozen trait beats architectural kinship.** The trait boundary (§5.5, frozen in
   D-18) is what D-5 says matters; which store sits behind it is replaceable. Given that, a mature,
   format-stable store is worth more than an LSM that happens to match RocksDB's shape. A project whose
   value is auditability should not stake its storage on a young on-disk format.
3. **A B-tree matches our access pattern.** `StateBackend` traffic is dominated by *prefix scans*: the
   join probes an index by key, and the aggregate reads `MIN` as the first entry of a scan and `MAX` as
   the last (S-30, §5.3). B-trees scan a range without merging; an LSM must merge across levels to
   answer the same question. The access pattern argues for a B-tree independently of the toolchain.
4. **Single-writer ACID transactions match §8**: "No multi-writer: one log, one writer, one epoch clock
   in v1." redb's transaction model is exactly that shape, so the checkpoint protocol
   (`docs/DURABILITY.md` §3) needs nothing bolted on.

**fjall, considered and rejected.** [`fjall`](https://crates.io/crates/fjall) is a pure-Rust LSM, and it
is the *closer* architectural match to what D-5 originally wanted — it would have removed the toolchain
blockers while keeping the LSM. It lost on two counts. It is younger, with a less settled on-disk
format, which is the wrong risk for a store holding irreplaceable state. And the advantage an LSM buys
— write amplification under heavy small writes — is not our bottleneck: our reads are scans, and
reason 3 above works against it.

**What is not decided here.** Nothing about tuning. D-5's successor inherits its rule that every tuned
constant needs a ledger entry with a receipt (I-10), and C8 is where block sizes, caches and durability
modes get measured. Today's decision is which crate, and why.

**Scope.** `RedbBackend` is **not implemented today**. It is C8-entry work, and D-18's freeze stays
**provisional** until it exists — because the freeze's whole claim is that a second backend can slot in
without an operator changing, and one implementation cannot demonstrate that.

### D-20 · Grand-total aggregation returns one row, always — closing Q-3

*Sprint: C5. Preserves: I-1. Recorded in `docs/SEMANTICS.md` S-33.*

`SELECT COUNT(*) FROM t` with no `GROUP BY` returns exactly one row, including over an empty input:
`COUNT` is `0` and the other four aggregates are `NULL`. It agrees with standard SQL.

**The tension it resolves.** S-29 says a group exists iff its total weight is positive — the rule that
makes a drained group *vanish* instead of emitting `(key, 0)`, and one of C3's two canonical mutations.
A grand total appears to contradict it. It does not, and the reason is worth stating precisely: S-29 is
about groups whose identity comes from the data. A key is a value some row supplied, so a group whose
rows have all left has nothing to name it. **A grand total has no key**, so its existence depends on
nothing. One group, always present.

**Why not stay uniform with S-29.** Returning no rows for `SELECT COUNT(*) FROM empty_table` is
defensible from the rule and was rejected. §8's "the dialect ladder is the dialect" licences leaving
constructs *out*; it does not licence giving a construct every user knows an answer they would call a
bug. And it cuts against the product's own claim: the answer to "how many rows" is `0`.

**What it costs.** The engine now needs a **defined initial state** — a non-empty answer before any
epoch is sealed. Every other answer starts empty and is accumulated from deltas, so this is genuinely
new: a circuit's result store is *primed* at build time by running the operator chain once with empty
inputs, without advancing the epoch. The aggregate records that it has emitted, so the priming pass
happens exactly once and survives a checkpoint like any other state (C4). One extra state entry, and
one extra step in circuit construction, in exchange for an answer that matches what the query means.

### D-21 · The project is renamed **Schweep**; "Current" is encumbered

*Recorded: 2026-08-11, between C8 and C9. Preserves: nothing technical — this is a naming decision, and
it touches no invariant. Trigger: MutinyDB's **MD-4** name sweep. MutinyDB records the resolution on its
own track in MD-4's addendum; this record is Schweep's, and the two are deliberately separate documents
because the projects are separate.*

**The trigger.** MD-4's sweep was run for MutinyDB and swept the sibling names with it. It found
"Current" **encumbered on three independent axes**, any one of which would have been enough:

| Axis | Finding |
| --- | --- |
| Trademark | Finco holds class-9 registrations **on the word itself** — the class that covers software |
| Category collision | Confluent's **Current** conference owns the term inside the data-infrastructure category, which is precisely this project's category |
| Namespace | the `current` crates.io name is taken |

A name that is trademarked in its own class, owned as an event brand by a large vendor in the same
category, and unavailable in the language's package registry is not a name with an obstacle. It is three
names' worth of obstacle wearing one word.

**The sweep history, so nobody repeats it.** Candidates considered and why each was set aside:

| Candidate | Disposition |
| --- | --- |
| **Weft** | Rejected. `WeaveMindAI/weft` is a 1,824-star Rust AI-orchestration language — same language, adjacent field, real users. The crate and the npm name are taken |
| Heddle, Artesian, Seiche, Millrace, Freshet, Weir, Oxbow | Occupied |
| **Thalweg** | **Viable, and passed over.** Recorded because "we never found another one" would be false: we did, and chose otherwise |
| **Schweep** | **Adopted** |

**Why Schweep.** It is clear on every axis the sweep checks:

- `schweep` is free on **crates.io**, **npm**, and **PyPI**;
- the **GitHub org** is free;
- there is **no software product and no trademark signal** — five zero-star hobby repositories, which
  makes it *unclaimed, not unheard-of*. A word with literally no prior use tends to be a word nobody
  can spell after hearing it once; a word with five abandoned repositories has been said aloud and
  claimed by nobody.

**`schweep.com` is registered by a third party.** Recorded here, plainly, so that nobody plans a launch
around acquiring it. It is not blocking — a package name, an org, and a clear trademark field are what a
library needs — but it is a fact about the name and it belongs in the record rather than in a surprise.

**What does not change.** The tagline **"every answer, current"** stays. It was always the adjective, and
with the project no longer named Current it reads as the adjective and nothing else — the sentence gets
*less* ambiguous, not more. Every invariant, every decision D-1…D-20, every semantic rule and every gate
is untouched: this record renames a product, not a design.

**What does change, mechanically.** The repository (`Current` → `schweep`, with GitHub redirecting the
old URLs), the crate names (`current-*` → `schweep-*`, `schweepd` → `schweepd`), and the product name in
prose across `ARCHITECTURE.md`, `CLAUDE.md`, `README.md`, and the `docs/`. Nothing is published, so this
is a grep, not a migration — and the absence of a migration is exactly why it happens now rather than
after v0.1.

---

## Open questions

### Q-1 · Non-integer arithmetic: fixed-point decimals

*Raised: C0 (D-10). Must be settled by: before any workload requiring non-integer measures — and
before v0.1 is described as generally useful. Not a C0 blocker.*

`Float64` is excluded from the type system for the reason in D-10, which leaves Schweep unable to
express prices, rates, or ratios. The honest answer is fixed-point decimal arithmetic
(`Decimal128` with a declared scale, as Arrow supports): exact, associative, and therefore
compatible with I-1. What must be decided: the scale/precision model, rounding on division, and
overflow behaviour. Until it is decided, the limitation stays in the README.

### Q-2 · What an evaluation error does to a *standing* query — **CLOSED in C3 by D-16**

*Raised: C0 (S-22). Confirmed as a real divergence in C1. **Decided doc-first at the start of C3**,
ahead of its C5 deadline. The answer is D-16: an error is a property of the contents, and the live
errors are a Z-set. What follows is the question as it stood, kept because the reasoning that led to
the decision is worth more than the decision alone.*

**Scheduled, not merely deferred.** C3 opens by settling this in `docs/SEMANTICS.md` — the doc
first, then the oracle, then the engine (§10) — before any aggregate code is written. Three reasons
it goes at the front of C3 rather than waiting for C5:

1. **The aggregates make it worse.** `SUM` overflows (S-30) and `AVG` divides. An error inside an
   aggregate is an error about a *group*, so the question stops being "what does the query answer"
   and becomes "does one poisoned group poison the answer" — a strictly harder question that is
   better answered before there is an implementation defending itself.
2. **The C2 gate is currently asserting the question away.** Both gates assert
   `report.error_answers == 0`, which is honest but is a fence, not a fix. It is only sound while no
   generated expression can raise, and C3's aggregates widen what can.
3. **It is a semantics decision, and semantics decisions go first.** Deciding it after the
   aggregates exist would mean fitting the rule to the code.

**What C3 must produce, in order:** a rule in `docs/SEMANTICS.md` saying whether an error is a fact
about the *state* (oracle-shaped: a poisoned row keeps raising until retracted — which an
incremental engine must then remember deliberately, and remembering is state that needs an I-9
declaration) or about the *change* (circuit-shaped: it raises once, as the row passes); then the
oracle; then the engine. **And then the gate population changes:** error-raising expressions enter
the scenario generator, the `error_answers == 0` assertions are replaced by assertions that both
sides agree about *which* epochs raise and what they say, and the ledger's generator entries are
regenerated because the population will have moved.

Until that is done, the fence stays and is labelled as one.

*Original statement of the question:*

At rungs 1–3 in C0 an evaluation error aborts the query for that epoch and is reported, which is a
complete answer for a one-shot recomputation. It is not a complete answer for a standing query:
does the query stay registered and retry at the next epoch, get quarantined with its last good
answer, or get deregistered? Each choice has different implications for I-3 (what a reader sees)
and for the memo (I-8) when the failing subplan is shared. Nothing in C0 needs the answer, so none
is invented.

**What C1 found, which sharpens the question.** With a real incremental engine next to the oracle,
the two disagree about an error's *lifetime*, and neither is wrong:

- The **oracle** recomputes over the whole integral at every epoch, so a row that makes an
  expression raise keeps making it raise — at that epoch and every epoch afterwards, until the row
  is retracted. The error is a property of the *state*.
- The **circuit** sees each row once, in the delta that carried it. It raises at that epoch and
  then, if no later delta touches the row, answers normally again. The error is a property of the
  *change*.

So "an evaluation error aborts the query for that epoch" is not one behaviour but two, and I-1
cannot hold across an erroring scenario until the question is settled. Settling it means choosing
what an error *is*: a fact about the data now in the table (oracle-shaped, which an incremental
engine would have to maintain deliberately — remembering poisoned rows is state, and state needs a
declaration under I-9), or a fact about a change that passed through (circuit-shaped, which makes
an answer's validity depend on when you asked).

C1 does not decide it, and keeps the gate away from it: the scenario generator emits division only
by non-zero literals over a value domain far too small to overflow, so rung-1 expressions cannot
raise, and the gate asserts that zero scenarios produced an error rather than trusting that
property to stay true. The behaviour that does exist is pinned by
`an_evaluation_error_aborts_the_step_without_advancing_the_epoch` in `schweep-circuit`: a failed
step leaves the epoch and the result store exactly where they were, so nothing is half-applied
(I-3), whatever the eventual policy turns out to be.

### Q-3 · Grand-total aggregation over an empty input — **CLOSED in C5 by D-20**

*Raised: C0 (S-33). Must be settled by: C5, before the binder accepts `SELECT COUNT(*) FROM t`.*

Aggregation with no group keys is refused at rungs 1–3 (`EmptyGroupKeys`). The edge case that must
be decided first: over an *empty* input, standard SQL returns exactly one row (`COUNT(*) = 0`),
whereas Schweep's rule that a group exists only if its total weight is positive (S-29) would
produce no row at all. Both are defensible. The tension is that the SQL answer requires the
"empty group" to be conjured from nothing, which an incremental engine must maintain as a special
initial state — a real implementation cost that should be paid knowingly, if it is paid.
