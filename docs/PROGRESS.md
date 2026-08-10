# PROGRESS

Sprint-by-sprint status: **what is proven, and by which test**. A claim here without a named test
that proves it is a violation of I-10, so every row below points at something runnable.

| Sprint | Status |
| --- | --- |
| **C0** — the oracle, the harness, and the rules | **complete; exit gate green in CI** |
| **C1** — linear operators + the first real circuit | **complete; exit gate green in CI** |
| **C2** — join | **complete; exit gate green in CI** |
| **C3** — aggregates and distinct | **complete; exit gate green in CI** |
| C4 — durability | not started |
| C5 … C13 | not started |

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

**122 tests**, zero ignored, zero skipped, zero flaky. (The workspace total is now 152; the
extra 30 arrived with C1 and the refactor that preceded it.)

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

---

## C1 — linear operators and the first real circuit

**Objective (§6):** the smallest true incremental engine. There is now an engine: it maintains an
answer from deltas and never looks at the whole input, and it is checked against the oracle at
every sealed epoch.

### The exit gate

§6 C1 names two conditions. Both are met.

| Gate condition | Proven by | Result |
| --- | --- | --- |
| Differential harness green, engine-vs-oracle, over randomized filter/project scenarios **including retractions** | `engine_vs_oracle_over_a_thousand_filter_project_scenarios` | 1,118 rung-1 scenarios drawn from 4,400 seeds, 5,187 epochs, 6,305 answer comparisons, **0 divergences** |
| I-2 gate: two runs of the same scenario produce byte-identical **state and answers** | `i2_two_runs_of_a_scenario_produce_byte_identical_state_and_answers` | 400 scenarios, identical fingerprints and answers, including from a scenario regenerated from its seed |

The "including retractions" clause is measured on the population the gate actually ran, not on the
generator as a whole: of those 1,118 scenarios, **894 contain a retraction, 312 retract in epoch
one, and 863 use a weight above 1** (`the_gate_population_is_full_of_retractions`). A family filter
that quietly selected a corner without retractions would fail that test.

**152 tests across the workspace**, zero ignored, zero skipped, zero flaky (two consecutive full
runs, identical results).

### This is the first time I-1 has meant anything

C0's harness compared the oracle to itself, which tested the harness. C1 puts two genuinely
different implementations on the two sides:

- the **circuit** sees only what changed, pushes it through stateless operators, and folds the
  output delta into a maintained integral — reading the answer is a lookup;
- the **oracle** replays the entire log from epoch 1 and recomputes from scratch, every time.

They agree byte for byte at every sealed epoch over 6,305 comparisons.

**And the gate has teeth — checked, not assumed.** Two deliberate mutations were introduced and
the gate caught both before being reverted:

| Mutation | Caught |
| --- | --- |
| Filter admits rows whose predicate is `NULL` (the classic S-17 bug) | seed 11, epoch 1 |
| Result store overwrites instead of accumulating — an error only a multi-epoch history reveals | seed 21, epoch 1 |

Worth recording alongside that: under both mutations the **I-2 test still passed**. A deterministic
bug is still deterministic. I-2 proves reproducibility, never correctness; only I-1 does that, and
the two gates are not substitutes.

### What is proven, and by which test

**The operators** — `crates/current-ops/`

| Claim | Test |
| --- | --- |
| Filter keeps TRUE only, weights untouched (S-17, S-24) | the differential gate; `a_hand_built_circuit_maintains_its_answer_from_deltas` |
| Projection merges rows and sums weights (S-25) | `a_hand_built_circuit_maintains_its_answer_from_deltas` |
| A non-Boolean predicate is refused at construction, not at data time (S-17) | `a_non_boolean_predicate_is_refused_at_construction` |
| **Linear operators declare and hold no state** — §6 C1's pitfall, as an assertion | `linear_operators_declare_and_hold_no_state`, plus the runtime check in every step |
| Projection's output schema comes from the shared binder, so it cannot drift from the oracle's (S-11, D-14) | `Project::new` calls `current_plan::projection_schema`; the gate would show any drift as a schema mismatch |

**The circuit** — `crates/current-circuit/`

| Claim | Test |
| --- | --- |
| A hand-built circuit maintains its answer from deltas across epochs, including retractions | `a_hand_built_circuit_maintains_its_answer_from_deltas` |
| A row inserted and retracted in one epoch leaves no trace | `same_epoch_churn_leaves_no_trace` |
| A drained row leaves no zero-weight tombstone | `a_row_retracted_to_zero_leaves_no_tombstone` |
| An empty epoch advances the clock and nothing else (S-6, I-3) | `an_empty_epoch_advances_the_clock_and_nothing_else` |
| A circuit ignores deltas for tables it does not read | `deltas_for_a_table_this_circuit_does_not_read_are_ignored` |
| Wiring out of dependency order is refused, which is what makes the schedule deterministic (I-2) | `the_builder_refuses_wiring_that_is_not_in_dependency_order` |
| Arity is checked at wiring time, not discovered at step time | `the_builder_refuses_an_operator_wired_to_the_wrong_number_of_inputs` |
| **A failed step advances nothing** — the epoch and the result store are exactly where they were (I-3) | `an_evaluation_error_aborts_the_step_without_advancing_the_epoch` |
| Result store: integral maintained by addition, order-independent, overflow refused not wrapped | seven tests in `result_store.rs` |
| The state fingerprint is stable and reports wiring, declarations, and store | `the_state_fingerprint_is_stable_and_reports_what_is_held` |

**The engine, against the oracle** — `testing/differential/tests/c1_engine_vs_oracle.rs`

| Claim | Test |
| --- | --- |
| **I-1** over 1,118 randomized rung-1 scenarios, every sealed epoch | `engine_vs_oracle_over_a_thousand_filter_project_scenarios` |
| **I-2** byte-identical state and answers across runs and across regeneration from seed | `i2_two_runs_of_a_scenario_produce_byte_identical_state_and_answers` |
| A one-shot query is the degenerate standing query (§0): the whole history as one epoch gives the same answer as epoch-by-epoch | `feeding_the_whole_history_as_one_epoch_gives_the_same_answer` |
| What the engine cannot run it refuses **by name**, naming the sprint that brings it | `the_engine_refuses_beyond_rung_one_and_names_the_sprint` |
| The harness can still fail against a real circuit, not only against the oracle | `the_gate_would_catch_a_wrong_circuit` (150 of 150) |

**I-9, at the level C1 has.** Every operator declares a `StateBound` and reports its actual state
size, and `Circuit::step` checks the declaration against the report after *every* step. In C1 every
declaration is `Stateless` and every report is zero, so the check is the executable form of §6 C1's
pitfall rather than a warning in a comment. Real bounds — and the accounting that checks them —
arrive with the join in C2, which is the first sprint with state to account for.

### What C1 does **not** prove

- **Nothing about join, aggregation, or distinct.** The engine refuses all three by name. Of the
  4,400 seeds swept, 3,282 were skipped as outside rung 1; that number is printed by the gate
  rather than hidden, because "1,000 scenarios passed" and "three quarters were not attempted" are
  the same sentence.
- **The hard part of incrementality is still ahead.** Filter and project are *linear*:
  `f(a + b) = f(a) + f(b)`, so the incremental form is a one-line consequence rather than a
  theorem. C1 proves the machinery — wiring, scheduling, epoch discipline, result stores, state
  accounting — before C2 introduces an operator where the equality has three terms and one of them
  is the one everybody forgets.
- **Errors are not settled, and the gate stays away from them.** C1 found that the oracle and the
  circuit disagree about an evaluation error's *lifetime*: the oracle recomputes over the integral
  so a bad row raises forever, while the circuit sees each row once so it raises once. Neither is
  wrong; nothing has decided what a standing query does with an error. Recorded under **Q-2** in
  `docs/DECISIONS.md`, and the gate asserts that zero scenarios raised, so it never silently
  depends on the undecided part.
- **Shared scalar code is not covered by I-1.** The oracle and the engine call the same expression
  evaluator (D-14), so a bug inside it produces the same wrong answer on both sides and the harness
  cannot see it. `current-plan`'s own unit tests pin that code to `docs/SEMANTICS.md` directly.
- **No durability, no sharing, no SQL, no network.** C4, C6, C5, C9. Circuits are hand-built by
  design (§6 C1); the incrementalizer that compiles a plan into one is C5.
- **No performance claim.** The engine has never been benchmarked and no artifact exists. Both
  implementations are knowingly slow: operators materialise rows out of the columnar batch, and the
  oracle replays the whole log per question. `testing/evidence/registry.json`'s engine-constant list
  is still empty, and `no_engine_constant_steers_behaviour_without_a_receipt` fails if that changes
  without a receipt.

### The pre-C1 refactor

Before any engine code, three things from review (one commit, no behaviour change):

- **D-14 · `current-plan`.** The plan IR, the binder, and the scalar expression library left
  `current-oracle` for a neutral crate — recorded in `docs/DECISIONS.md` *before* the move, because
  it extends §5's crate map. From C1 there are two implementations of the query surface and neither
  may own the definition of what a query is.
- **One delta type.** `current-zset::EpochDeltas` replaced the harness's `EpochInput` and the
  oracle's private copy. This corrected a comment in `engine.rs` that asserted the opposite of what
  the file did, and a claim in C0's section of this document that repeated it. Both now say what is
  true, and say that they were wrong.
- **Ledger receipts (I-10).** The scenario generator's nine tuned constants are in
  `testing/evidence/registry.json` with the measured number that justifies each, backed by
  `c0-generator-coverage.json` — regenerable by a committed binary and checked by
  `the_committed_coverage_artifact_still_matches_the_generator`, so the receipt cannot go stale
  quietly.

### What C2 needs

C2 is *join* — the first bilinear operator, and §6 calls it the hardest correctness class in the
engine. What C1 leaves it:

- **The `Operator` trait already fits it.** `step(&[&ZSetBatch])` takes a slice, so a binary
  operator needs no trait change; `StateBound::ProportionalToInputs { inputs: ["left", "right"] }`
  is already the vocabulary for declaring O(|A| + |B|), and `Circuit::step` already calls the
  check. What C2 must add is the *accounting* — `check_state_declarations` currently accepts any
  actual size for a non-`Stateless` declaration, because nothing declares one yet. That is the one
  place in the C1 code that is deliberately unfinished, and it is named here rather than left to be
  discovered.
- **The wiring is already a DAG.** `CircuitBuilder::add` takes a vector of inputs and validates
  arity and ordering, so a two-input node needs no builder change.
- **The scenarios exist.** `Family::Join` and `Family::JoinAggregate` are already generated —
  3,282 of the 4,400 seeds C1 skipped are mostly these. C2's gate widens `CircuitEngine::claims`
  to include `Family::Join` and the same sweep starts exercising it.
- **The delta-delta term needs its own scenario.** §6 C2's pitfall is that `ΔA⋈ΔB` is the term
  every implementer forgets, and the gate must have a scenario that fails if it is missing — both
  sides inserting matching rows in the *same* epoch. The generator produces multi-table epochs
  today, but nothing yet *isolates* that case or asserts it occurred. Writing that scenario family
  first, before the operator, is C2's first task.
- **Join weights multiply, and the oracle already says so.** `s26_join_multiplies_weights` and
  `s26_a_null_join_key_never_matches_even_another_null` pin the semantics C2's operator must match.

Per the sprint protocol in `CLAUDE.md`, **C2 does not begin in the session that finished C1.**

---

## C2 — join

**Objective (§6):** the first bilinear operator — "the hardest correctness class in the engine".

### The exit gate

| Gate condition (§6 C2) | Proven by | Result |
| --- | --- | --- |
| Differential harness green over join scenarios | `engine_vs_oracle_over_randomized_join_scenarios` | 1,090 join scenarios from 4,400 seeds · 5,161 epochs · **6,251 answer comparisons · 0 divergences** |
| multi-key batches | `a_multi_key_batch_joins_only_the_matching_keys` | joins only the matching keys |
| retractions of joined rows | `retracting_a_joined_row_retracts_the_output`, `retracting_one_side_retracts_the_joined_rows` | both joined rows retract from one retraction |
| updates (retract+insert same epoch) | `a_same_epoch_update_moves_the_joined_row`, `a_same_epoch_update_on_both_sides` | |
| weight multiplicities > 1 | `weights_multiply`, `both_sides_inserting_together_with_multiplicities` | 3 × 2 = 6 |
| **the delta-delta term, with a scenario that isolates it** | `the_delta_delta_term_is_the_whole_answer_when_both_sides_insert_together`, `both_sides_inserting_a_matching_row_in_one_epoch` | see below |
| state-bound declarations (I-9) **and the runtime accounting that checks them** | five tests in `circuit.rs::accounting`, plus `the_joins_state_is_accounted_against_its_declaration` | see below |

**196 tests across the workspace**, zero ignored, zero skipped, zero flaky (two consecutive full
runs, identical results).

### The three-term rule, and the term everybody forgets

`ΔOut = ΔA ⋈ B + A ⋈ ΔB + ΔA ⋈ ΔB`, written literally as three probes in `Join::step`, with the
derivation in the module docs. `A` and `B` are the integrals **as they were before this epoch**, and
the code integrates only after all three probes — probing updated indexes would count this epoch's
rows twice.

Each term is isolated by its own test, so a failure names the term rather than a seed:

| Term | Isolating test | How it is isolated |
| --- | --- | --- |
| `ΔA ⋈ B` | `the_left_delta_probes_the_right_integral` | right side arrives in an earlier epoch |
| `A ⋈ ΔB` | `the_right_delta_probes_the_left_integral` | mirror image |
| `ΔA ⋈ ΔB` | `the_delta_delta_term_is_the_whole_answer_when_both_sides_insert_together` | one epoch, both indexes empty, so terms 1 and 2 probe nothing |
| order of operations | `probing_happens_before_integrating_so_nothing_is_counted_twice` | both sides already populated, both gain a row; must emit 3 new pairs, not 6 |

**And the gate has teeth — checked, not assumed.** Two deliberate mutations, both reverted:

| Mutation | Caught by |
| --- | --- |
| **Drop `ΔA ⋈ ΔB`** (§6 C2's named pitfall) | 2 operator tests + 10 handwritten differential scenarios + the randomized gate at **seed 2, epoch 6** — 12 failures |
| **Integrate before probing** (the double-count bug) | 5 operator tests + the gate at **seeds 90003, 90009 and seed 2, epoch 6** |

Under both mutations the I-2 gate and the state accounting still passed. That is the same lesson C1
recorded: a deterministic bug is still deterministic, and state accounting measures size, not
correctness. Only I-1 catches a wrong answer.

The delta-delta case is also **common in the randomized population, not just present in a
handwritten test**: of 1,090 join scenarios, 946 change both sides in one epoch and **790 insert
matching keys on both sides in one epoch** (`the_gate_population_contains_the_shapes_c2_names`). If
the generator ever drifted so both sides stopped moving together, term 3 would go barely exercised
and that test would say so.

### I-9: the placeholder is gone

C1 left `check_state_declarations` accepting any state size for a non-`Stateless` declaration,
because nothing declared one. It no longer does. Every variant now has a real check:

| Declaration | What the runtime does | Test |
| --- | --- | --- |
| `Stateless` | requires actual state of exactly 0 | `a_stateless_declaration_that_holds_anything_is_caught` |
| `ProportionalToInputs` | budgets actual state against the entries ever handed to the operator | `state_growing_faster_than_its_input_is_caught`, `state_proportional_to_its_input_is_accepted` |
| `Unbounded` | **refused at wiring time** — admission needs the registry, which is C6 | `an_unbounded_declaration_is_not_admissible_yet` |
| any, mismatched | a declaration naming a different number of inputs than the operator takes is refused at wiring time | `a_declaration_that_does_not_match_the_arity_is_refused` |

The join declares `ProportionalToInputs { inputs: ["left", "right"] }` and reports the entries held
across both indexes. The budget is the entries ever delivered on those inputs, which is a sound
upper bound on O(|A| + |B|): an index over a side's integral holds one entry per *distinct* row, and
distinct rows can never outnumber the entries that delivered them.

**What that catches and what it does not**, stated in the code and repeated here: it catches the
wrong *complexity* — a join storing the cross product holds |A|·|B| against a budget of |A|+|B| and
fails as soon as either side passes two rows. It does not catch a constant-factor overshoot, because
retractions and multiplicities mean entries usually outnumber distinct rows. Tightening that needs
real per-operator input integrals, which is `EXPLAIN STATE` in C8.

### Also in C2

- **`current-state`** (§5.5, §6 C2's "MemBackend"): the `StateBackend` trait and `MemBackend`. The
  join reaches its indexes only through the trait, so C4 can hand it a `RocksBackend` without the
  operator changing (§2). Keys are `Vec<Value>` ordered by S-7 rather than bytes — **D-15**, with
  the reasoning and the cost. Named snapshots are deliberately absent until C4 designs the
  checkpoint protocol, and that gap is labelled rather than guessed at.
- **`WriteBatch` has `add` and no `delete`.** Every change to operator state in this engine is the
  addition of a weight; a row leaves when its weight reaches zero. An interface with `delete` would
  invite an operator to treat a retraction as a deletion, which is the special case I-5 forbids.
- **Sources are keyed by alias, not table**, so a self-join is representable: `FROM t a JOIN t b`
  needs two source nodes over one table. The oracle has supported that since C0 and the circuit
  refused it until now (`a_table_joined_to_itself_agrees_with_the_oracle`).
- **C1's gate kept its own meaning.** `CircuitEngine::claims` widened to include joins, so C1's gate
  now filters on `Family::FilterProject` directly. Had it kept using `claims`, C1's numbers would
  have silently become C1-and-C2's, and neither sprint's section here would describe its own gate.

### What C2 does **not** prove

- **Nothing about aggregation or distinct.** `GROUP BY` is refused by name, pointing at C3. Of
  4,400 seeds, 3,310 were skipped as outside rung 2 — printed by the gate, not hidden.
- **Nothing about outer joins or cross joins.** A join with no key pairs is refused
  (`a_join_with_no_key_pairs_is_refused`); `LEFT JOIN` is rung 5.
- **Nothing about state that outgrows memory.** `MemBackend` is a `BTreeMap` and
  `scan_prefix` is a filtered walk — O(n) per probe, which is the wrong complexity for a join and is
  knowingly left that way. C2 is the correctness sprint; the ordered-range fix is C10's, when there
  is a benchmark to justify it. **No performance claim is made** and the engine-constant section of
  the ledger is still empty.
- **Nothing about durability.** The join's indexes are in memory and have no checkpoint. C4.
- **Errors are still fenced off, and the fence is now scheduled to come down.** Both gates assert
  that no scenario raised an evaluation error. That is sound only while no generated expression can
  raise. See below.

### Q-2 is open, and now scheduled

C1 found that the oracle and the circuit disagree about an evaluation error's *lifetime*: the oracle
recomputes over the integral so a bad row raises forever, the circuit sees each row once so it
raises once. C2 did not touch it, and both gates continue to assert `error_answers == 0`.

**It is decided at the start of C3, doc-first**, ahead of its C5 deadline — the full plan is in
`docs/DECISIONS.md` under Q-2. Briefly: the aggregates make the question harder (`SUM` overflows,
`AVG` divides, and an error inside an aggregate is an error about a *group*), so the rule is settled
in `docs/SEMANTICS.md` before any aggregate code exists, then the oracle, then the engine. After
that, **error-raising expressions enter the gate population**: the `error_answers == 0` assertions
are replaced by assertions that both sides agree about which epochs raise and what they say, and the
ledger's generator entries are regenerated because the population will have moved.

### What C3 needs

- **`GROUP BY` semantics are already decided and pinned.** S-27 through S-32, and 12 tests in
  `crates/current-oracle/tests/semantics.rs` — drained groups vanish, MIN reveals the
  second-smallest under retraction, `COUNT` of an all-null group is 0 while `SUM` is NULL, AVG lands
  exactly on the weighted quotient. The engine is held to those, and does not get to reinterpret
  them.
- **The state vocabulary fits.** An aggregate declares
  `ProportionalToInputs { inputs: ["input"] }` — one entry per group is bounded by the entries that
  created the groups — and the runtime already budgets it. `StateBound::Unbounded` exists for
  aggregation over an unbounded key space and is currently **refused**, which is correct until C6's
  registry can admit it; if C3 needs it sooner, that is a decision to record, not a check to remove.
- **MIN/MAX need a per-group multiset**, not a single value (§5.3, S-30). `MemBackend`'s ordered
  prefix scan is exactly the shape for it: key the state as `[group key…, value]` and the smallest
  live value is the first entry under the prefix.
- **The families exist.** `Family::Aggregate` and `Family::JoinAggregate` are already generated —
  most of the 3,310 seeds C2 skipped. Widening `CircuitEngine::claims` turns them on.
- **Q-2 first, before any of it.**

Per the sprint protocol in `CLAUDE.md`, **C3 does not begin in the session that finished C2.**

---

## C3 — aggregates, distinct, and the error rule

**Objective (§6):** complete the stateful core. The engine now implements the whole surface
`docs/SEMANTICS.md` defines, and the differential gate sweeps **every** scenario the generator
produces.

C3 ran in three parts, in this order: settle **Q-2** doc-first; prove float rendering lossless
*before* the first float could flow; then build the aggregates.

### Part 1 — Q-2, closed by D-16

C1 found the oracle and the circuit disagreeing about an evaluation error's lifetime. C3 opened by
deciding it in `docs/SEMANTICS.md` before touching operator code (S-22, S-22a…S-22d), recording the
reasoning as **D-16**, then implementing oracle-first and engine-second.

**The rule.** The answer at epoch N is either a Z-set or an error, determined by the *contents* at
epoch N. Data that raises means the query has no answer while it is present; retract it and the
answer returns.

**Why the alternative lost.** An error as a property of the *change* is not merely different — it is
incompatible with **I-3**. Dropping the epoch that raised means the next epoch lands on contents
that never absorbed it, leaving the answer a mixture of epoch N−1 and N+1. The epoch now seals and
only the *answer* is an error.

**The mechanism is a Z-set**, which is why it is small: a row that raises contributes its message at
the row's weight, so retracting the row retracts the error by the same arithmetic (I-5 applied to
errors). The engine integrates the error stream into a result store exactly like the answer stream,
and "the least live message" (S-22c) is the first row of its canonical form.

| Claim | Test |
| --- | --- |
| An error lasts exactly while the offending data is present, and no longer | `s22_an_error_lasts_while_the_offending_data_is_present_and_no_longer` |
| With several live errors the least message is reported | `s22c_the_least_live_error_message_is_reported` |
| For an aggregate the unit is the group | `s22a_a_group_whose_aggregate_overflows_makes_the_answer_an_error` |
| Batching the history differently changes neither answer nor error | `s22d_batching_does_not_change_the_answer_or_the_error` |
| The epoch **seals** and the answer is the error; retraction restores everything the erroring epochs carried | `an_evaluation_error_seals_its_epoch_and_lasts_while_the_row_does` |

**A C1 test was wrong and was replaced, not deleted.**
`an_evaluation_error_aborts_the_step_without_advancing_the_epoch` asserted the I-3-violating
behaviour. Its replacement asserts the opposite and says why.

**The gate population moved, deliberately.** Raising expressions now enter the generator — division
by a column (2/3 of divisions) and `i64::MAX` literals (1/12), so two *kinds* of error can be live
at once, which is what exercises S-22c. The `error_answers == 0` fences are replaced by
`error_answers > 0`: the sweep passing already means both sides agreed at every comparison, error
text included, and the assertion now says the population is not vacuous. Every quoted number in
`testing/evidence/registry.json` was regenerated and two new generator constants recorded — with the
honest note that the raising rate is set by how often arithmetic appears at all, not by the knob
(moving the column-divisor rate from 1/3 to 5/6 shifted the count only from 14 to 15 scenarios), so
specific error behaviours are pinned by handwritten scenarios instead.

### Part 2 — float rendering, proven lossless before the first float

`AVG` is the only source of a `Float64` (S-3), and its exemption from the no-floats rule rests
entirely on both implementations doing one identical division and producing identical bits (D-10,
S-31). That is worth nothing if the *comparison* throws bits away — and the harness compares
**rendered strings**.

| Claim | Test |
| --- | --- |
| Distinct bit patterns never render identically | `distinct_bit_patterns_never_render_identically` |
| Rendering round-trips to the same bits | `rendering_round_trips_to_the_same_bits` |
| `-0.0` and `0.0` render apart, as S-7 orders them apart | `negative_zero_renders_differently_from_positive_zero` |
| 200,000 seeded arbitrary bit patterns, injective and round-tripping | `a_large_sweep_of_bit_patterns_renders_losslessly` |
| The property AVG's exemption rests on | `avgs_arithmetic_is_bit_stable_through_rendering` |

Every check goes through a real `ZSetBatch` canonical form, not through `format!` directly, so it
proves the path an answer actually takes. `NaN` is the one value where rendering is legitimately not
injective; it cannot arise (S-31) and is skipped for a stated reason.

### Part 3 — the exit gate

| Gate condition (§6 C3) | Proven by | Result |
| --- | --- | --- |
| Differential green over aggregate scenarios **heavy on retractions** | `engine_vs_oracle_over_randomized_aggregate_scenarios` | 2,192 aggregate scenarios · 10,154 epochs · **12,346 comparisons · 0 divergences**, 85 of them a shared live error |
| Retract the current MIN, second-smallest surfaces | `retracting_the_current_min_reveals_the_second_smallest` | and `a_multiplicity_must_be_drained_before_the_min_moves` |
| Drain a group to zero, the row **vanishes** (not zeroes) | `a_group_drained_to_zero_vanishes_leaving_no_phantom_row` | plus vanish-and-return, and churn-within-one-epoch |
| AVG over retractions lands exactly on the oracle's value | `avg_over_retractions_lands_exactly_on_the_oracles_value` | |

Of the 2,192 gate scenarios, **1,801 contain a retraction**, 1,739 use a weight above 1, 522 use
`DISTINCT`, and 943 of the 1,084 join-aggregate scenarios change both join sides in one epoch — so
C2's delta-delta coverage assertion extends to the new families rather than lapsing.

**224 tests across the workspace**, zero ignored, zero skipped, zero flaky (two consecutive full
runs, identical).

### The cliffs, each with its own isolating test

| Cliff | Test |
| --- | --- |
| MIN/MAX keep an ordered multiset (S-30, §5.3) | `retracting_the_current_min_reveals_the_second_smallest`, `min_and_max_work_on_strings_and_use_the_total_order` |
| A drained group vanishes (S-29) | `a_group_drained_to_zero_vanishes_leaving_no_phantom_row`, `a_group_can_vanish_and_return`, `a_group_created_and_drained_in_one_epoch_never_appears` |
| SUM transits `i128`, lands in `i64`, or raises | `a_sum_that_transits_out_of_range_and_returns_is_correct`, `a_sum_that_does_not_fit_raises_and_the_error_clears_when_the_data_leaves` |
| AVG is one division of two exact integers (S-31) | `avg_over_retractions_lands_exactly_on_the_oracles_value` |
| Grouping uses not-distinct while `ON` uses `=` — **in one query** | `grouping_groups_nulls_together_while_a_join_key_never_matches_a_null` |
| `COUNT(x)` is 0 where `SUM` is NULL (S-30's asymmetry) | `avg_of_an_all_null_group_is_null` |
| HAVING filters groups both ways as they change (S-32) | `having_filters_groups_and_a_null_predicate_rejects` |
| DISTINCT collapses weights and tracks presence incrementally (S-34) | `distinct_collapses_weights_and_tracks_presence_incrementally` |

**The state layout is chosen by MIN/MAX.** Per group and per aggregate slot, an *ordered multiset* of
the argument's values, keyed `[slot, group key…, value]` so a prefix scan returns them in value order
(D-15, S-7). MIN is the first entry, MAX the last, and retracting the current minimum reveals the
next because the next was never thrown away. The same multiset serves SUM, COUNT and AVG by folding
it — O(distinct values in the changed group), which is the honest cost of a layout chosen for
correctness under retraction, and a C10 concern. **No performance claim is made.**

**Aggregation is deliberately *not* shared with the oracle.** The scalar expression library is shared
(D-14) because §6 C5 says so; aggregation is implemented twice, because the cliffs above are exactly
what I-1 is for and sharing the code would have removed the signal.

### The gate's teeth, and a lesson about proving them

Two canonical mutations, both reverted:

| Mutation | Caught by |
| --- | --- |
| **MIN/MAX never forget a retracted value** (the single-value bug in effect) | 8 tests, randomized gate at **seed 4, epoch 3** |
| **A drained group emits a phantom `(key, 0)` row** (§6 C3's named pitfall) | 14 tests, including the whole-population sweep |

**A first attempt at the MIN/MAX mutation silently failed to apply** — `rustfmt` had collapsed the
target expression onto one line, so the patch matched nothing and the suite passed. A mutation that
does not land proves the opposite of what it appears to. Both mutations are now applied with a marker
that is grepped for before the run, and that check is the reason the first attempt was caught rather
than believed.

### I-9: no new placeholder

| Operator | Declares | Why that factor |
| --- | --- | --- |
| `Aggregate` | `1 + aggregates` × input | one entry per group for the total, plus one per (slot, distinct value) |
| `Distinct` | 1 × input | one entry per distinct input row |

`ProportionalToInputs` gained a **declared constant factor**, because a four-aggregate operator
legitimately keeps more entries than it received rows and the C2 check would have failed it. The
factor must be *justified* — a reader should be able to count the entries it claims — not raised
until the check passes; a wrong *complexity* still fails whatever the constant.

One real inconsistency was found and fixed while writing this: the state fingerprint computed its
budget **without** the factor while the checker applied it, so the printed accounting disagreed with
the enforced one. Both now come from one function (`Circuit::state_budget`).

### What C3 does **not** prove

- **Nothing about durability.** Aggregate and distinct state is in memory with no checkpoint. C4.
- **Nothing about SQL.** Circuits are still hand-built; the incrementalizer is C5. `DISTINCT` arrived
  in C3 because §6 C3's build list names it, ahead of its rung — recorded as **D-17**, and the rest
  of rung 4 (`UNION ALL`, `ORDER BY`/`LIMIT`) is not implemented.
- **Grand-total aggregation is still refused** (`EmptyGroupKeys`, S-33) and **Q-3 is still open**,
  now the only open question that C5 must settle.
- **No performance claim.** `MemBackend`'s prefix scan is a linear walk and the aggregate folds a
  changed group's whole multiset. The engine-constant section of the ledger is still empty and
  `no_engine_constant_steers_behaviour_without_a_receipt` fails if that changes without a receipt.
- **A bug in shared code is still invisible to I-1.** The scalar library and the binder are shared
  (D-14); `current-plan`'s own tests pin them to `docs/SEMANTICS.md` directly.

### What C4 needs

- **The seam is already in place.** Operator state lives behind `StateBackend` (§5.5, D-15), so
  `RocksBackend` slots in without touching an operator. C4's job on the trait is to add the **named
  snapshots** D-15 deliberately left out, once the checkpoint protocol is designed.
- **What must be checkpointed is enumerable.** Three operators hold state — join (two indexes),
  aggregate (one backend), distinct (one backend) — plus the circuit's result store, its live-error
  store, and `emitted_entries`, which is I-9 accounting and is state too. A recovery that restored
  the stores but not the counter would pass every answer test and then mis-account.
- **I-2 is already the shape I-7 needs.** `state_fingerprint` renders every operator's state and both
  stores deterministically, and the I-2 gates compare it across runs. Comparing a recovered circuit
  to its uncrashed twin is the same comparison with a crash in the middle.
- **fsync ordering must be written down before it is implemented** (§6 C4's pitfall): state flush →
  checkpoint record → log trim, in a doc comment, with the crash harness killing between each pair.
- **`EpochDeltas` should move to `current-log`.** It sits in `current-zset` because C4 had not
  happened yet (D-14); C4 is when the write path arrives and it can go where §5.4 puts it.

Per the sprint protocol in `CLAUDE.md`, **C4 does not begin in the session that finished C3.**
