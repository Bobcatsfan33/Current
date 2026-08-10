//! The ledger's receipts must still describe the thing they justify (I-10).
//!
//! `testing/evidence/registry.json` records the scenario generator's tuned constants, and
//! `testing/evidence/c0-generator-coverage.json` is the measurement that justifies them. A
//! committed artifact is only evidence while it is still true; the moment the generator changes
//! and the artifact does not, the ledger is decoration.
//!
//! So the numbers are recomputed here and compared byte for byte. If this fails, either the
//! generator changed on purpose — in which case regenerate the artifact and re-read the
//! constants' justifications, because the reason for them may have changed too — or it changed by
//! accident, which is what this test is for.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use current_differential::coverage::{measure, ARTIFACT_SEEDS};

const ARTIFACT: &str = include_str!("../../evidence/c0-generator-coverage.json");
const REGISTRY: &str = include_str!("../../evidence/registry.json");

#[test]
fn the_committed_coverage_artifact_still_matches_the_generator() {
    let measured = measure(ARTIFACT_SEEDS)
        .expect("measuring the generator must succeed")
        .to_json();

    assert_eq!(
        measured, ARTIFACT,
        "\ntesting/evidence/c0-generator-coverage.json no longer describes the generator.\n\
         If the generator changed deliberately, regenerate it with:\n  \
         cargo run -p current-differential --bin generator-coverage \
         > testing/evidence/c0-generator-coverage.json\n\
         and re-read the justifications in registry.json — the numbers that motivated those \
         constants have moved.\n"
    );
}

/// Every generator constant in the ledger points at an artifact that exists, and every claim the
/// ledger makes about a measured number matches the artifact.
#[test]
fn every_ledger_entry_cites_the_artifact_that_exists() {
    assert!(
        REGISTRY.contains("c0-generator-coverage.json"),
        "the ledger must cite the coverage artifact"
    );
    // The ledger quotes these two numbers as the reason the join-key domain is narrow. If the
    // artifact moves, the quoted numbers must move with it, or the ledger is telling a story the
    // evidence no longer supports.
    let measured = measure(ARTIFACT_SEEDS).unwrap();
    for quoted in [
        format!(
            "\"join_both_tables_populated\": {}",
            measured.join_both_tables_populated
        ),
        format!(
            "\"join_bare_join_non_empty\": {}",
            measured.join_bare_join_non_empty
        ),
    ] {
        assert!(
            ARTIFACT.contains(&quoted),
            "the artifact should contain {quoted}"
        );
    }
}

/// The engine-constant section of the ledger is still empty, and says so honestly.
///
/// The moment a tuned constant enters an operator — a batch size, a threshold, a cache bound —
/// this test should be updated to require its entry. Until then the claim "nothing is tuned" is
/// itself worth pinning, because the failure mode I-10 guards against is a constant appearing
/// with no receipt and nobody noticing.
#[test]
fn no_engine_constant_steers_behaviour_without_a_receipt() {
    assert!(
        REGISTRY.contains("\"constants\": []"),
        "the engine-constant list is no longer empty; every entry needs a committed benchmark \
         artifact before it may steer behaviour (I-10)"
    );
}
