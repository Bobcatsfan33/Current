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

**Cost, stated plainly.** Current cannot represent non-integer measures at all today. That is a
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

SQL leaves null ordering implementation-defined and offers `NULLS FIRST`/`NULLS LAST`. Current
fixes one rule — nulls before all non-null values — and provides no modifier, so there is nothing
for two implementations to disagree about. Strings order byte-wise over their UTF-8 encoding, with
no collation support in v1.

### D-14 · A neutral `current-plan` crate holds the logical plan, the binder, and the scalar expression library

*A deviation from the §5 crate map, recorded before the code moved.*

*Sprint: C1 (pre-work). Preserves: I-1, I-6. Supersedes: nothing in §4; **extends the crate map in
`ARCHITECTURE.md` §5**.*

**What changes.** A new crate `crates/current-plan/` is added to the workspace, holding three
things that were in `current-oracle`:

- the **logical plan IR** — `Query`, `Source`, `Expr`, `BinOp`, `AggFunc`, `GroupBy`, `Named`;
- the **binder** — scope rules, type checking, and the named refusals (S-10, S-12, S-19, S-27);
- the **scalar expression library** — three-valued evaluation (S-13…S-22).

`current-zset` additionally gains `EpochDeltas`, the per-table bundle of input deltas, replacing
two private copies (one in `current-oracle`, one in the differential harness).

**Why the crate map needed extending.** §5 lists ten crates and none of them is a home for a
logical plan that *both* the oracle and the engine can read. From C1 there are two implementations
of the query surface, and every option other than a neutral crate is worse:

- `current-circuit` depending on `current-oracle` inverts the relationship the oracle exists to
  have. The oracle is the arbiter (§5.1); an engine that imports it can inherit its bugs, and I-1
  stops being a comparison between two things.
- Duplicating the plan IR in the engine means two spellings of one query shape, which must then be
  kept in step by hand. I-6 ("SQL and the typed API compile to the same circuit plan") is a claim
  about there being *one* plan type; two would make it unprovable.
- Putting the plan in `current-zset` breaks that crate's cohesion. It is the data layer — "Z-set
  batches over Arrow; weight algebra; consolidation" — and a query IR is not data.

`EpochDeltas` goes to `current-zset` for the opposite reason: it *is* data, it is the delta
representation named in §1, and every crate already depends on that crate. `current-log` (§5.4) is
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

1. `current-plan` carries the S-rule unit tests that were in the oracle — the Kleene truth tables,
   checked arithmetic, CASE short-circuiting, null comparison. They pin the library against
   `docs/SEMANTICS.md` directly, not against another implementation.
2. What C1 is actually hunting is *incrementality* bugs — maintaining an answer from deltas versus
   recomputing it from scratch. That machinery is not shared, and the harness sees all of it.
3. §6 C5's parenthesis is the standing warning: shared code can still be *called* differently, and
   the harness does test that.

**Where this lands in C5.** `current-sql` becomes "sqlparser AST → a SQL-specific binder →
`current_plan::Query` → the incrementalizer". Both doors — SQL text and the typed API — produce
the same `current_plan::Query`, which is what I-6 needs in order to be checkable at all.

### D-15 · `StateBackend` keys are `Vec<Value>` ordered by S-7, not bytes

*Sprint: C2. Preserves: I-2, I-9. Realises `ARCHITECTURE.md` §5.5; the trait is frozen at C4 exit.*

§5.5 calls for "ordered KV with range scans, atomic multi-key write batches, and named snapshots".
The obvious reading of "KV" is `Vec<u8>` → `Vec<u8>`. Current's `StateBackend` instead uses
`Vec<Value>` keys ordered by the total order on values (S-7), with `i64` values.

**Why.** An order-preserving byte encoding of a row is a real piece of engineering — sign-aware
integer encoding, length-prefixed strings, null ordering — and getting it subtly wrong produces a
backend whose scans return the right rows in the wrong order. That is a *storage* problem, and D-5
and §2 both say storage is the boring part that lives behind the trait: "`current-log` and
`current-state` must sit behind traits rather than being called concretely from operators." Putting
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

---

## Open questions

### Q-1 · Non-integer arithmetic: fixed-point decimals

*Raised: C0 (D-10). Must be settled by: before any workload requiring non-integer measures — and
before v0.1 is described as generally useful. Not a C0 blocker.*

`Float64` is excluded from the type system for the reason in D-10, which leaves Current unable to
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
`an_evaluation_error_aborts_the_step_without_advancing_the_epoch` in `current-circuit`: a failed
step leaves the epoch and the result store exactly where they were, so nothing is half-applied
(I-3), whatever the eventual policy turns out to be.

### Q-3 · Grand-total aggregation over an empty input

*Raised: C0 (S-33). Must be settled by: C5, before the binder accepts `SELECT COUNT(*) FROM t`.*

Aggregation with no group keys is refused at rungs 1–3 (`EmptyGroupKeys`). The edge case that must
be decided first: over an *empty* input, standard SQL returns exactly one row (`COUNT(*) = 0`),
whereas Current's rule that a group exists only if its total weight is positive (S-29) would
produce no row at all. Both are defensible. The tension is that the SQL answer requires the
"empty group" to be conjured from nothing, which an incremental engine must maintain as a special
initial state — a real implementation cost that should be paid knowingly, if it is paid.
