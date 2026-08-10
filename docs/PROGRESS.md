# PROGRESS

Sprint-by-sprint status: **what is proven, and by which test**. A claim here without a named test
that proves it is a violation of I-10, so every row below points at something runnable.

| Sprint | Status |
| --- | --- |
| **C0** — the oracle, the harness, and the rules | **complete; exit gate green in CI** |
| **C1** — linear operators + the first real circuit | **complete; exit gate green in CI** |
| **C2** — join | **complete; exit gate green in CI** |
| **C3** — aggregates and distinct | **complete; exit gate green in CI** |
| **C4** — durability | **exit gate green in CI; `RocksBackend` NOT delivered — see below** |
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

---

## C4 — durability

**Objective (§6):** survive death. `docs/DURABILITY.md` was written **first**, numbering every step of
the ack, seal, checkpoint and recovery sequences and naming the instant between each pair; the crash
harness lands on those instants.

**Read the honest summary first:** the exit gate is green and `RocksBackend` is **not delivered**. The
reasons are in **D-18** and repeated below. Everything else on §6 C4's list is done.

### The exit gate

| Gate condition (§6 C4) | Proven by | Result |
| --- | --- | --- |
| ≥10,000 randomized crash-and-recover cycles | `ten_thousand_crash_and_recover_cycles` | **10,000 cycles** · 5,767 seam faults fired · 1,832 byte-boundary faults · 604 clean runs · **18 of 18 named seams fired** |
| Every recovery byte-identical to the never-crashed twin (I-7) | same test | state fingerprints **and** answers **and** the log's rendering, all compared |
| Every acked batch appears exactly once (I-4) | `a_replayed_token_is_acknowledged_and_dropped`, plus the gate re-offering every token after recovery | a re-offered token that is not dropped fails the cycle |
| A torn checkpoint is detected and the previous one used | `a_torn_checkpoint_is_detected_and_the_previous_one_is_used` | 150 scenarios, byte-corrupted checkpoints |
| Recovery is idempotent | `recovery_is_idempotent_under_a_crash_during_recovery` | crash *during* recovery, twice, then recover: same state |
| `StateBackend` frozen at exit | **D-18** | frozen **provisionally**, with its compatibility promise — final when a second backend validates it, no later than C8 entry (**D-19**) |

**256 tests across the workspace**, zero ignored, zero skipped, zero flaky (two consecutive full
runs, identical). The crash gate runs in ~42 s.

### The gate's own assertions caught two harness bugs — before any engine bug

This is the C3 mutation lesson, applied to crashes, and it paid immediately.

1. **`0 of 10,000 cycles fired a seam fault.`** The first run of the gate reported that and failed on
   the fault-count assertion. The cause was in the harness: `run_with_fault` returned the *recovery*
   injector's `fired()`, which is always inert, so every cycle reported "no fault". Without that
   assertion the gate would have passed, green, having injected nothing.
2. **`seam RecoveryMidReplay was planned but never fired.`** The seam-coverage assertion then caught
   that recovery seams were unreachable, because the recovery phase used an inert injector. Fixing it
   needed a third phase — crash, crash-during-recovery, then recover for real.
3. Fixing (2) surfaced a third bug the idempotency test caught: a run over an already-recovered
   directory re-sealed every epoch and stepped the circuit twice, doubling every weight. Phase 1 now
   resumes from the log rather than replaying from zero.

Three real bugs, all in the test apparatus, all found by assertions about the apparatus rather than
about the engine.

### Teeth: the two canonical mutations

Both applied with a marker grepped before the run — the C3 discipline — and reverted.

| Mutation | Caught by |
| --- | --- |
| **(a) Acknowledge before the batch is durable** | all 3 crash tests fail; the recovered log differs from the twin's (I-4) |
| **(b) Skip the torn-checkpoint detection** | `a_torn_checkpoint_is_detected_and_the_previous_one_is_used` and the 10,000-cycle gate, at seed 2 |

**An honest note on mutation (a).** §6 C4 asks for "ack before the fsync completes". That exact bug is
**not observable to an in-process harness**: `write_all` has already put the bytes in the page cache,
which survives a simulated crash, so recovery finds the record either way. The observable form of the
same bug class was used instead — acknowledge before the record is written at all — and it is caught.
Detecting the literal fsync-ordering bug needs a filesystem-level fault injector or a VM that can be
cut off mid-write. That is named as remaining work, not implied by a green gate.

### What is simulated, and what is not

The 10,000 cycles use **in-process fault injection**: abort at a named seam, drop every in-memory
object, recover from disk. What that faithfully models is loss of everything not yet written, at a
named instant. What it does **not** model is kernel-level write reordering or power loss.

Consequently the gate runs with `SyncPolicy::Deferred` — `fsync` skipped — because `fsync` changes
nothing an in-process crash can observe while costing hours on macOS. `SyncPolicy::Full` is the
default, is what production uses, and is what the log's own durability tests use. **Nothing here tests
power loss**, and no count in this document should be read as if it did.

**There is no real-`kill -9` subprocess test.** It was planned as the check that the in-process model
is faithful, and it is not delivered.

**Where it lands: C9.** §6 C9's exit gate is precisely this test, under load and over the network:
"kill -9 under load at 1,000 random points — every ack honored on recovery, no duplicate epochs
delivered to subscribers". So the gap is not merely named, it is *scheduled*: C9 must kill a real
process, and when it does it becomes the check that C4's in-process model was faithful. If the two
ever disagree, C4's simulation is what is wrong, and C9 is where that shows up.

Until then, a **nightly job** runs the crash gate at `SyncPolicy::Full`
(`testing/crash/tests/nightly_full_sync.rs`, schedule-triggered). It observes nothing an in-process
crash could not observe with fsync deferred — it is not a power-loss test and is labelled as such in
its own module docs — and it exists so that every `sync_all` call in the log and the checkpoint
protocol is exercised in bulk rather than by a handful of unit tests. A path never run in bulk is a
path that quietly stops being reached.

### What is proven, and by which test

| Claim | Test |
| --- | --- |
| A replayed token is acknowledged and dropped (A3, I-4) | `a_replayed_token_is_acknowledged_and_dropped` |
| The same token with **different content** is refused loudly, never rewritten (A4, I-4) | `the_same_token_with_different_content_is_refused_loudly` |
| Dedup survives a reopen, because the index is rebuilt from the log (R6) | `dedup_survives_a_reopen` |
| A malformed batch is refused and writes nothing (A1) | `a_malformed_batch_is_refused_and_writes_nothing` |
| A torn tail is discarded; the prefix survives (R5) | `a_torn_tail_is_discarded_and_the_prefix_survives`, `a_truncated_frame_reads_as_a_torn_tail`, `a_flipped_byte_fails_the_crc` |
| `source_id` travels with every batch (§5.4, MutinyDB seam) | `the_source_id_survives_a_reopen` |
| Byte order equals value order (D-15) | `byte_order_equals_value_order`, `a_seeded_sweep_agrees_on_order`, `a_component_prefix_is_a_byte_prefix` |
| A snapshot restores to an identical backend, replacing not merging | `a_snapshot_restores_to_an_identical_backend` |
| Faults are deterministic by seed; every seam is selectable | `a_seed_chooses_the_same_fault_every_time`, `every_seam_is_selected_by_some_seed` |

### What C4 does **not** deliver

- **Any non-memory backend.** §6 C4 names `RocksBackend` and D-5 mandated it. **D-19 has since amended
  D-5 to `redb`**, a pure-Rust B-tree store, with the RocksDB build cost as the trigger — a debug
  `librocksdb-sys` build produces over 2.1 GiB of object files and exhausted the machine's disk. (The
  `libclang` half of C4's original diagnosis was **wrong** and is corrected in D-18: with
  `bindgen-runtime` enabled, bindgen ran fine.) `RedbBackend` is C8-entry work; the order-preserving
  byte codec it will need *was* built and tested here, so the riskiest part is done. The trait freeze is
  **provisional** until it exists.
- **Power-loss testing**, and the literal ack-before-fsync mutation. Needs a filesystem fault injector
  or a VM.
- **A real-kill subprocess test.**
- **Log segment rotation and trimming.** The C6 trim step is a no-op in v1: one segment, and recovery
  replays only the suffix after the checkpoint's epoch, so trimming would save disk and change no
  behaviour. The seam exists and is exercised so the ordering is right; the work is C7's compaction.
- **No performance claim.** Nothing is benchmarked; the engine-constant ledger is still empty.

### What C5 needs

- **The plan type is already shared** (`current-plan`, D-14), which is what I-6 will be checked
  against: SQL text and the typed API must produce the same `current_plan::Query`.
- **Q-3 is the only open question left**, and C5 must settle it: grand-total aggregation over an empty
  input (S-33). Doc first.
- **The gate infrastructure is ready.** `sweep_matching` takes a predicate, so a SQL door adds a second
  `EngineUnderTest` rather than a second harness.
- **`EpochDeltas` can now move to `current-log`**, where §5.4 puts it. It has lived in `current-zset`
  since C1 only because the log did not exist.

Per the sprint protocol in `CLAUDE.md`, **C5 does not begin in the session that finished C4.**

---

## C5 — the SQL frontend and the incrementalizer

**Objective (§6):** the same-door moment — SQL in, circuits out.

Everything on §6 C5's list is delivered: `current-sql` (parser gate, binder, incrementalizer, plan
type, instantiator), the SQL fuzzer, the I-6 plan and counter gates, and both canonical mutations. Two
things are worth reading before the tables: **Q-3 is closed** (D-20), and **the SQL door is narrower
than the typed API** in one specific way that is counted rather than glossed.

### Pre-work carried into this sprint

- **D-19** amends D-5 to **redb**, with the RocksDB blockers as the trigger and fjall recorded as
  considered and rejected. The implementation stays C8-entry work; **D-18's freeze is provisional**
  until a second backend validates the trait.
- **D-18 corrected, visibly.** `libclang` was never the blocker — `--no-default-features` had disabled
  `bindgen-runtime`. Disk was. The wrong reason is quoted in D-18 above the correction rather than
  deleted, for the same reason C1's seam claim was corrected in the open.
- **A nightly `SyncPolicy::Full` crash job** (`.github/workflows/ci.yml`, `nightly-full-sync`, cron
  `17 3 * * *`, 400 cycles at `Config::durable()`). It observes nothing an in-process crash cannot
  observe with `fsync` deferred, and its own module docs say so; it exists so the `fsync` path cannot
  rot. Deliberately absent from `ci`'s `needs`: a scheduled job must not gate a push.
- **C4's kill -9 gap now forward-points to C9's gate**, which runs exactly that test under load.

### Part 1 — Q-3, closed by D-20

`SELECT COUNT(*) FROM t` over an empty input returns **one row**, not zero. Doc first (S-33 rewritten),
then the oracle, then the engine.

The oracle side is three lines: seed the keyless group with no members, and exempt it from S-29's
"a drained group vanishes" guard. The engine side is the interesting half, because a grand total is an
answer that must exist **before any epoch is sealed**, and every other answer starts empty. So
`CircuitBuilder::build` now *primes* the circuit by running an empty epoch through the same `run()`
path a real step takes — no second code path — and emission is made idempotent by a `primed` marker in
state. That marker costs one state entry, which is declared through a new **`constant` term** on
`StateBound::ProportionalToInputs`; I-9's vocabulary had no way to say "O(1) state" before, and adding
one number to the accounting is cheaper than exempting an operator from the accounting.

| Claim | Test |
| --- | --- |
| A grand total returns one row over an empty input, on the **oracle** side | `s33_a_grand_total_returns_one_row_even_over_an_empty_input` |
| The same, on the **engine** side, at epoch 0, through SQL text | `s33_the_grand_total_answers_before_any_epoch_is_sealed` |
| `HAVING COUNT(*) > 0` filters the grand total away, with no special case | `s33_having_can_filter_a_grand_total_away` |
| A GROUP BY that computes nothing is still refused | `s33_a_group_by_with_neither_keys_nor_aggregates_is_refused` |

### Part 2 — binder semantics, doc first

Three rules that C0 deferred to "the binder in C5" now say what they mean, and two new rules were
needed to say it: **S-35** (the SQL door translates and can only shrink) and **S-36** (a projection is
emitted only when the select list is not already the answer).

| Rule | Decision | Why |
| --- | --- | --- |
| **S-11** name derivation | `AS n`, or a bare column reference's own name. Nothing else. | Every derived name is a name nobody chose, and the schema is part of the answer (S-8) |
| **S-11** `SELECT *` | refused (`SelectStarNotSupported`) | The one refusal that exists *because* queries are standing: adding a column to a table must not change a running query's schema |
| **S-11** identifiers | verbatim, case-sensitive, quoted or not; keywords and function names fold | Dialects disagree about which way to fold, and folding in one door only would make the doors disagree about what a column is called |
| **S-19** untyped `NULL` | refused; write `CAST(NULL AS <type>)`, the only accepted cast | Inference would be a second analysis of the query, living in one door, that must agree with S-19's table |
| **S-32** `AggregateInHaving` | a real refusal now, with `AggregateInWhere`, `NestedAggregate` and `AggregateNotTopLevel` beside it | SQL text can write what the typed API cannot represent; each place gets its own name |

### Part 3 — `current-sql`, and where the documentation went

`crates/current-sql/src/incremental.rs` is the best-documented file in the repository, as §5.6 requires:
the three DBSP rules (linear, bilinear, stateful) each stated with the algebra, the reason it holds for
the operators it covers, and the trap it sets. Every plan node carries its rule as **data**
(`CircuitNode::rule`), so "this operator is linear" is a claim a test checks rather than a comment.

The pipeline is split at a seam that pays for itself three times: **incrementalize** (pure, hashable,
no state) then **instantiate** (allocates operators and one backend each). I-6 compares plans rather
than circuits; C6's memo will hash subtrees; and a failed comparison prints two s-expression trees
instead of two 64-bit numbers.

Two honest notes about that file:

- It performs **no general `δ`/`∫` rewrite**. Each logical operator has exactly one incremental
  implementation, already in `current-ops`, so the incrementalizer chooses it and records why. The
  file says this out loud, and says when a general rewriter would earn its keep (an open operator set,
  several forms per operator, nested time domains — none of which v1 has).
- It performs **no optimisation**. No pushdown, no reordering, no CSE. An optimiser today would change
  what I-6 compares and what the harness covers, for a benefit nobody has measured (I-10).

The old ad-hoc wiring in `testing/differential/src/circuit_engine.rs` is **deleted**: the typed door now
calls `incrementalize_typed`, the SQL door calls `compile`, and both end in `instantiate`. There is one
path from a query to a circuit, which is the only way I-6 can mean anything.

### The exit gate

| Gate condition (§6 C5) | Proven by | Result |
| --- | --- | --- |
| SQL fuzzer: hundreds of shapes, thousands of runs, green engine-vs-oracle | `the_sql_door_agrees_with_the_oracle_over_the_whole_renderable_population` | **2,028 scenarios**, 9,516 epochs, **11,544 answer comparisons**; all four families; every operation kind including retractions; 249 empty-input scenarios, 1,270 with an empty epoch, 122 error answers |
| I-6: both doors produce structurally identical plans (hash equality) | `i6_the_two_doors_compile_to_structurally_identical_plans` | **2,028 plan pairs**, compared as s-expressions *and* by FNV-1a hash *and* by answer schema |
| I-6: identical counters | `i6_the_two_doors_execute_identical_counters` | **470 scenarios** stepped through both doors, per-node counters compared after **every** epoch, 10,022 entries emitted |
| Every refusal names its construct | `every_construct_outside_the_dialect_is_refused_by_name` | **60 constructs**, each refused by a message containing its name; `the_dialect_itself_is_accepted` proves the refusals are not "everything" |
| Scalar expression library shared, tested differentially anyway | `current-plan` (D-14) + the fuzzer | unchanged from C3; the SQL door reaches the same `eval` |

**315 tests across the workspace**, zero ignored except the scheduled nightly, zero skipped, zero
flaky. The C5 gate runs in under a second.

### The population, and the part of it that has no SQL form

The fuzzer drives the SQL door by rendering the *existing* typed population back to SQL. That choice is
what makes I-6 checkable over thousands of shapes — there is a typed query to compare each SQL plan
against — and it puts the renderer under the I-6 assertion, so a renderer that writes SQL meaning
something else fails the gate with both trees printed.

Not every typed query has a SQL form, and the census is printed rather than implied:

| Reason | Count (of 4,400 seeds) |
| --- | --- |
| **renderable** | **2,028** |
| no projection and no GROUP BY — would need `SELECT *` | 1,110 |
| a projection over a GROUP BY | 1,099 |
| two group keys with one expression | 163 |

`every_scenario_either_has_a_sql_form_or_a_named_reason` asserts that the four numbers account for
every seed, and that the two large reasons still occur — so a change that made one unreachable is
noticed rather than celebrated as improved coverage.

**The middle row is a real difference in reach.** In SQL a group key's output name comes from the select
list (S-11), so a query that both groups *and* projects would need to name its keys twice and has one
select list to do it in. The typed API can express it; the SQL door cannot. That is recorded here, in
`NoSqlForm::ProjectionOverGroupBy`'s own documentation, and in S-33's note about `ColumnNotGrouped`.

### The gate's teeth

Both mutations applied with a marker **grepped before the run**, and reverted; `grep -rn MUTANT` over
`crates/` and `testing/` returns nothing.

| Mutation | Caught by |
| --- | --- |
| **(a) binder invents a name** — `SELECT t.n + 1` accepted, named by its own SQL text | `s11_names_come_from_as_or_from_a_bare_column_reference` **and** `every_construct_outside_the_dialect_is_refused_by_name` (two independent tests) |
| **(b) mis-incrementalized pipeline** — `DISTINCT` applied *before* the projection instead of last (S-34) | C1's gate at seed 0 epoch 4, **and** the C5 SQL-door gate at seed 0 epoch 4 |

Mutation (b) is the interesting one: it still type-checks, still emits the right output schema, and so
passes both wiring checks — the plan and the circuit both agree with the binder about the answer's
schema. Only the *answers* change. Nothing but a differential comparison against a recompute-from-
scratch oracle would have caught it, which is the argument for I-1 in one line.

### What is proven, and by which test

| Claim | Test |
| --- | --- |
| Names come from `AS`, or from a bare column reference, or not at all (S-11) | `s11_names_come_from_as_or_from_a_bare_column_reference` |
| `SELECT *` is refused, with the standing-query reason in the message (S-11) | `s11_select_star_is_refused_because_a_standing_query_fixes_its_schema` |
| Identifiers are verbatim: `A` and `a` are two columns; quoting changes only legality (S-11) | `s11_identifiers_are_verbatim` |
| A null is written `CAST(NULL AS T)`, and that is the only cast (S-19) | `s19_a_null_is_written_with_its_type`, `a_cast_that_converts_is_refused` |
| A negative literal is a literal, including `i64::MIN` (S-19) | `negative_integer_literals_fold_into_the_literal` |
| A grouped query emits no projection when the select list is already the group output (S-36) | `s27_a_group_by_binds_to_keys_then_aggregates_with_no_projection` |
| Reordering or narrowing the select list emits one (S-36) | `s36_reordering_the_select_list_emits_a_projection`, `s27_a_key_absent_from_the_select_list_still_gets_a_name` |
| Two aliases for one key both read the key (S-36) | `s36_two_aliases_for_one_key_read_the_same_column` |
| An aggregate with no `GROUP BY` is the grand total (S-33) | `s33_an_aggregate_with_no_group_by_is_the_grand_total` |
| A column outside the grouping belongs to no group, and the workaround binds (S-33) | `s33_a_column_outside_the_grouping_is_refused` |
| Each misplaced aggregate has its own refusal (S-32) | `s32_each_misplaced_aggregate_has_its_own_refusal` |
| The plan has one node per stage, in pipeline order, with the right DBSP rule on each (§5.6) | `the_plan_has_one_node_per_stage_in_pipeline_order` |
| Naming switches at the GROUP BY: `WHERE` sees `t.n`, `HAVING` sees `n` (S-10, S-27) | `naming_switches_at_the_group_by` |
| Every clause of the sqlparser AST outside the dialect is refused by name | `every_clause_outside_the_dialect_is_refused_by_name`, `every_construct_outside_the_dialect_is_refused_by_name` |
| A plan's structural form and hash distinguish plans that differ anywhere | `the_structural_form_distinguishes_plans_that_differ`, `a_column_and_a_string_literal_render_differently` |

### What C5 does **not** prove

- **The differential SQL sweep is not an independent check of the answers.** Because the two doors
  compile to *identical* plans — which is exactly what I-6 asserts — a green SQL sweep follows from a
  green typed sweep. Its value is that compile-and-build succeeds across the population and that the
  identity holds at runtime, not that the answers were checked twice. The independent content is I-6
  itself, plus the hand-written binder tests, where SQL text sits on one side and the plan the rule
  says it means sits on the other.
- **The fuzzer's SQL is written by the same author as the binder.** `sql_render` is a renderer, not an
  independent SQL generator; a shared misconception about SQL would render and bind consistently and
  the gate would stay green. What guards against that is `crates/current-sql/tests/dialect.rs` — 60
  hand-written constructs — and `binder.rs`'s hand-written plans, not the fuzzer.
- **No grand total, and no `HAVING`, is reached by the fuzzer.** The generator always makes at least
  one group key and sets `having` only through the typed path; both shapes are covered by hand-written
  tests instead, including the I-6 pairs.
- **No `Float64` flows through the SQL door except from `AVG`.** There is no way to write a float
  literal (S-3), which is the point, but it means the SQL door's float handling is exactly AVG's.
- **No performance claim, again.** The parser, binder and incrementalizer are not benchmarked, and the
  engine-constant ledger is still empty. C8 owns that.
- **No memo, no sharing.** Two identical standing queries build two circuits. `CircuitPlan::nodes` and
  the structural hash exist for C6 to use; C6 has not happened.

### What C6 needs

- **The structural hash is ready**, and it is stable by construction: FNV-1a over a rendering, not
  `std::hash::Hash`, whose output is explicitly not stable across releases. A memo that shares
  sub-circuits needs subtree hashes, and `CircuitNode::structural_hash` is one call per node.
- **Counters are public** (`Circuit::counters`), which is what I-8's counter gate will assert on, and
  what I-6 already does.
- **The incrementalizer is the place sharing will attach**, and it now has one caller for both doors,
  so a memo lookup inserted there is inserted once.
- **`EpochDeltas` still has not moved to `current-log`**, where §5.4 puts it. Named in C4's list and
  still true.

Per the sprint protocol in `CLAUDE.md`, **C6 does not begin in the session that finished C5.**
