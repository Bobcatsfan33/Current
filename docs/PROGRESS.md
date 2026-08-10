# PROGRESS

Sprint-by-sprint status: **what is proven, and by which test**. A claim here without a named test
that proves it is a violation of I-10, so every row below points at something runnable.

| Sprint | Status |
| --- | --- |
| **C0** — the oracle, the harness, and the rules | **complete; exit gate green in CI** |
| C1 — linear operators + the first real circuit | not started |
| C2 … C13 | not started |

---

## C0 — the oracle, the harness, and the rules

**Objective (§6):** stand up the workspace with the correctness machinery *before any engine code
exists.* That is what happened: there is no engine in this repository, deliberately.

### The exit gate

§6 C0 names four conditions. All four are met.

| Gate condition | Proven by | Result |
| --- | --- | --- |
| CI green (fmt, clippy `-D warnings`, test, no-network) | `.github/workflows/ci.yml` | all green, plus the aggregate `ci` check — 5 jobs |
| Harness runs oracle-vs-oracle over 1,000 randomized scenarios | `oracle_vs_oracle_over_one_thousand_randomized_scenarios` | 1,000 scenarios, 4,668 epochs, 5,668 answer comparisons, 0 divergences |
| Property tests for Z-set algebra pass | `crates/current-zset/tests/properties.rs` | 13 property tests |
| A seeded scenario is reproducible byte-for-byte from its seed | `a_seed_reproduces_its_scenario_byte_for_byte`, `a_seed_reproduces_its_run_byte_for_byte` | byte-identical scenario *and* run |

**122 tests across 11 binaries**, zero ignored, zero skipped, zero flaky.

### What is proven, and by which test

**Z-set algebra** — `crates/current-zset/`

| Claim | Test |
| --- | --- |
| Addition is commutative (§5.2) | `addition_is_commutative`, and byte-identically after consolidate in `commutativity_is_byte_identical_after_consolidate` |
| Addition is associative (§5.2) | `addition_is_associative` |
| `consolidate` is idempotent (§5.2) | `consolidate_is_idempotent` |
| `negate ∘ negate = identity` (§5.2) | `double_negation_is_identity` |
| **I-5** as arithmetic: a retraction of everything cancels everything | `a_plus_negative_a_is_empty` |
| **I-2**: canonical form depends on the data, not on entry order | `canonical_form_is_invariant_under_permutation` |
| Canonical form is sorted, deduplicated, zero-free (S-8) | `canonical_form_is_sorted_deduplicated_and_zero_free` |
| Consolidation preserves total weight | `consolidate_preserves_total_weight` |
| The total order on values, nulls first (S-7) | `s7_null_sorts_before_every_non_null`, `s7_within_type_orders`, `ordering_is_total_and_antisymmetric_across_variants` |
| Weight overflow is refused, not wrapped (D-11) | `negate_refuses_i64_min_rather_than_saturating`, `consolidate_reports_weight_overflow_rather_than_wrapping` |
| Arrow round-trips entries exactly (D-2) | `arrow_round_trip_preserves_entries`, `from_arrow_agrees_with_from_entries` |

**Semantics of rungs 1–3** — `crates/current-oracle/tests/semantics.rs` (39 tests)

| Claim | Test |
| --- | --- |
| Join multiplies weights (S-26) | `s26_join_multiplies_weights` |
| A null join key never matches, not even another null (S-13, S-26) | `s26_a_null_join_key_never_matches_even_another_null` |
| Grouping puts all nulls in **one** group (S-28) — the other side of the same coin | `s28_grouping_puts_all_nulls_in_one_group` |
| A drained group vanishes; no phantom `(key, 0)` row (S-29) | `s29_a_group_drained_to_zero_rows_vanishes_rather_than_zeroing` |
| Retracting the current MIN reveals the second-smallest (S-30) | `s30_retracting_the_current_min_reveals_the_second_smallest` |
| A value retracted to weight zero stops being the MIN | `s30_a_value_retracted_to_weight_zero_is_no_longer_the_min` |
| `COUNT(x)` of an all-null group is 0; `SUM` is NULL (S-30) | `s30_an_all_null_group_counts_zero_and_sums_to_null` |
| AVG lands exactly on the weighted quotient under retraction (S-31) | `s31_avg_lands_exactly_on_the_weighted_quotient_under_retraction` |
| Weights are multiplicities in every aggregate (S-30) | `s30_weights_are_multiplicities`, `s30_count_star_counts_weights_and_count_x_skips_nulls` |
| Projection merges rows and sums weights (S-25) | `s25_projection_merges_rows_and_sums_their_weights` |
| Filter preserves weights exactly (S-24) | `s24_filter_preserves_weights_exactly` |
| `WHERE NOT p` is not the complement of `WHERE p` (S-17) | `s17_where_not_p_is_not_the_complement_of_where_p` |
| Kleene truth tables, including `F AND N = F` (S-15) | `s15_kleene_truth_tables` |
| `AND`/`OR` do not short-circuit (S-15) | `s15_and_does_not_short_circuit` |
| CASE takes the first TRUE branch and evaluates only that branch (S-18) | `s18_case_takes_the_first_true_branch_and_skips_null_conditions`, `s18_case_does_not_evaluate_the_branch_it_did_not_take` |
| Overflow and division by zero are errors, not wraps or nulls (S-20, S-21, D-11) | `s20_overflow_is_an_error_not_a_wrap`, `s21_division_and_modulo_by_zero_and_the_min_over_minus_one_case` |
| Retracting a row that was never there is a malformed history (S-5, D-12) | `s5_retracting_a_row_that_is_not_there_is_a_malformed_history`, `s5_retracting_more_copies_than_exist_is_a_malformed_history` |
| An empty epoch does not move the answer (S-6) | `s6_an_empty_epoch_does_not_move_the_answer` |
| Every refusal names its construct (S-12) | `s10_…_refused_as_unqualified`, `s19_there_are_no_implicit_conversions`, `s19_an_untyped_null_literal_is_refused_…`, `s33_a_group_by_with_no_keys_is_refused`, `s26_a_cross_join_is_refused_by_name`, `s3_a_float_column_cannot_be_declared_…` |
| Binding fails identically on an empty and a populated database (S-12) | `s12_binding_fails_the_same_way_on_an_empty_database` |
| **I-2**: two oracles fed the same log answer byte-identically | `i2_two_oracles_fed_the_same_log_give_byte_identical_answers` |

**The harness** — `testing/differential/`

| Claim | Test |
| --- | --- |
| 1,000 randomized scenarios, oracle vs oracle, compared at every sealed epoch | `oracle_vs_oracle_over_one_thousand_randomized_scenarios` |
| The harness **catches a wrong implementation** — it is not comparing nothing | `the_harness_catches_a_deliberately_wrong_implementation` (155 of 155 sabotaged runs caught) |
| A divergence report is actionable alone: seed, epoch, both answers, whole scenario | `a_divergence_report_contains_everything_needed_to_reproduce_it` |
| Divergence is reported at the **first** epoch answers part, not the last (I-3) | `divergence_is_reported_at_the_first_epoch_where_answers_part` |
| A seed reproduces its scenario and its run byte-for-byte (I-2) | `a_seed_reproduces_its_scenario_byte_for_byte`, `a_seed_reproduces_its_run_byte_for_byte` |
| Different seeds produce different scenarios | `different_seeds_produce_different_scenarios` |
| Entry order within an epoch changes no answer (S-6, I-2) | `shuffling_the_entries_within_an_epoch_does_not_change_any_answer` |
| The RNG stream is value-stable, so recorded seeds keep their meaning | `the_stream_is_value_stable_for_a_known_seed` |

**Generator coverage** — asserted, not assumed. §7 requires certain shapes always be produced;
the gate fails if any of them stops appearing.

| Required shape (§7) | Test |
| --- | --- |
| Retractions **in epoch one** (the §6 C0 pitfall) | `retractions_appear_in_the_first_epoch_of_some_scenarios` |
| Retractions common, not rare (> 300 of 1,000 scenarios) | same test |
| Weight multiplicities above 1 (> 150 of 500 scenarios) | `weights_above_one_are_common` |
| Same-epoch retract-and-insert of the same row | `Operation::ChurnSameEpoch` asserted in the gate |
| Update in place (retract + insert, one epoch) | `Operation::UpdateInPlace` asserted in the gate |
| Empty epochs, and empty inputs | asserted in the gate; `an_empty_epoch_never_changes_the_answer` |
| All four query families (rungs 1, 2, 3, and 2→3) | asserted in the gate |
| Scenarios that produce a **non-empty answer** ≥ 40% (measured: 53%) | `a_healthy_share_of_scenarios_produce_a_non_empty_answer` |

### What C0 does **not** prove

Stated plainly, because a progress document that only lists wins is marketing.

- **Nothing about incremental evaluation.** There is no engine: no operators, no circuit, no
  scheduler, no result stores. Every answer in this repository is produced by recomputing from
  scratch.
- **Nothing about the oracle's correctness against SQL.** The oracle *is* the spec (§5.1); the
  tests pin it to `docs/SEMANTICS.md`, and both could be wrong together about what a user expects.
  That risk is real and is what the dialect ladder and, later, real workloads reduce.
- **Oracle-vs-oracle does not test the oracle.** It tests the harness. Both sides run the same
  code, so agreement is guaranteed and only the machinery around it is under test. The
  `SaboteurEngine` is what stops that from being vacuous.
- **Nothing about durability, crash recovery, concurrency, or the network.** C4 and C9.
- **No performance claim of any kind**, and no benchmark artifact exists.
  `testing/evidence/registry.json` is empty because nothing is tuned. Both `current-zset` and
  `current-oracle` are knowingly slow: consolidation materialises rows out of the columnar batch,
  and the oracle replays the entire log prefix on every question, with a nested-loop join.
- **I-1 is not yet exercised in anger.** The oracle law needs two different implementations. It
  gets one in C1.
- **I-3, I-4, I-6, I-7, I-8, I-9 have no engine to hold to them yet.** I-2, I-5, and I-10 are
  exercised at the level C0 has.

### Decisions taken during C0

Recorded in `docs/DECISIONS.md`: **D-10** `Float64` is result-only (`AVG` is the sole source);
**D-11** arithmetic errors are errors, not wraps or nulls; **D-12** the oracle rejects malformed
history rather than defining an answer for it; **D-13** nulls sort first, everywhere, with no
modifier.

Deliberately *not* decided, each with the sprint that must settle it: **Q-1** fixed-point decimals
for non-integer arithmetic; **Q-2** what an evaluation error does to a standing query (by C5);
**Q-3** grand-total aggregation over an empty input (by C5).

Three semantics rules were added mid-sprint, when writing the oracle exposed questions the first
draft of `docs/SEMANTICS.md` had not answered: null literals carry a type (S-19), `AND`/`OR` do not
short-circuit (S-15), every output column is declared nullable (S-11). In each case the document
moved first and the code followed, which is the order §10 requires.

### What C1 needs

C1 is *linear operators + the first real circuit*. Everything it needs from C0 exists:

- **The seam.** `EngineUnderTest` in `testing/differential/src/engine.rs` is what `current-circuit`
  implements. Add an adapter, put it on one side of `compare`, and the 1,000-scenario gate becomes
  a real engine-vs-oracle gate — that is the C1 exit gate.

  *Correction, made in C1:* this section originally claimed "nothing in the harness mentions the
  oracle's types". That was false — the trait imported `current_oracle::Query`, and a comment in
  `engine.rs` asserted the opposite of what the file did. It cost C1 a preparatory refactor
  (D-14) rather than "one file". The claim is true now because the types moved to a neutral
  crate, not because the wording was softened.
- **The scenarios.** The generator already emits retractions, multiplicities, churn, updates, and
  empty epochs across four query families. C1's gate ("randomized filter/project scenarios
  including retractions") is a filter over `Family::FilterProject`, not new generator work.
- **The I-2 gate.** C1 must show that two runs of one scenario produce byte-identical state and
  answers. `a_seed_reproduces_its_run_byte_for_byte` is that test with the engine substituted in.
- **The spec.** `docs/SEMANTICS.md` S-23, S-24, S-25 define scan, filter, and projection, and
  `crates/current-oracle/tests/semantics.rs` pins them. C1's operators are held to those rules and
  do not get to reinterpret them.

One thing C1 will have to decide, flagged now rather than discovered later: `EpochInput` and the
oracle's `EpochDeltas` are two spellings of the same idea, and they exist separately only because
`current-log` does not arrive until C4. When the circuit lands, one of them should become the
shared type — most naturally in `current-zset`, since it is the delta representation and every
crate already depends on it.

Per the sprint protocol in `CLAUDE.md`, **C1 does not begin in the session that finished C0.**
