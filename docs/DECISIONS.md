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

### Q-2 · What an evaluation error does to a *standing* query

*Raised: C0 (S-22). Must be settled by: C5, when queries first arrive through the SQL door; a
registry exists from C6.*

At rungs 1–3 in C0 an evaluation error aborts the query for that epoch and is reported, which is a
complete answer for a one-shot recomputation. It is not a complete answer for a standing query:
does the query stay registered and retry at the next epoch, get quarantined with its last good
answer, or get deregistered? Each choice has different implications for I-3 (what a reader sees)
and for the memo (I-8) when the failing subplan is shared. Nothing in C0 needs the answer, so none
is invented.

### Q-3 · Grand-total aggregation over an empty input

*Raised: C0 (S-33). Must be settled by: C5, before the binder accepts `SELECT COUNT(*) FROM t`.*

Aggregation with no group keys is refused at rungs 1–3 (`EmptyGroupKeys`). The edge case that must
be decided first: over an *empty* input, standard SQL returns exactly one row (`COUNT(*) = 0`),
whereas Current's rule that a group exists only if its total weight is positive (S-29) would
produce no row at all. Both are defensible. The tension is that the SQL answer requires the
"empty group" to be conjured from nothing, which an incremental engine must maintain as a special
initial state — a real implementation cost that should be paid knowingly, if it is paid.
