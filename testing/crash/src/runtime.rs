//! A durable run of a scenario: log, circuit, checkpoints, and recovery.
//!
//! This is the object a crash lands in the middle of. It owns the three durable things —
//! `current-log`'s segment, the checkpoint directory, and the circuit whose state they protect — and
//! sequences them in the orderings `docs/DURABILITY.md` numbers.

use std::path::{Path, PathBuf};

use current_circuit::{checkpoint, Circuit};
use current_differential::{CircuitEngine, EngineUnderTest, Scenario};
use current_log::{Ack, FaultInjector, FaultPlan, Log, Seam, SyncPolicy};
use current_zset::{EpochDeltas, Schema};

use crate::scenario_fault::Fault;

/// What one run produced, for comparison against a twin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunOutcome {
    /// The state fingerprint after every sealed epoch, in order.
    pub fingerprints: Vec<String>,
    /// The answer, or the live error, after every sealed epoch.
    pub answers: Vec<String>,
    /// The log's own rendering — epochs, batches, tokens.
    pub log: String,
    /// The highest epoch the circuit reached.
    pub epoch: u64,
    /// Batches the log accepted, by token. Used for the I-4 check: exactly one epoch each.
    pub accepted_tokens: Vec<String>,
}

/// A durable run.
pub struct Durable {
    dir: PathBuf,
    tables: Vec<(String, Schema)>,
    query: current_plan::Query,
    log: Log,
    circuit: Circuit,
    sync: SyncPolicy,
}

impl std::fmt::Debug for Durable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Durable")
            .field("dir", &self.dir)
            .field("epoch", &self.circuit.epoch())
            .finish()
    }
}

/// How a run is configured.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Take a checkpoint every `checkpoint_every` epochs. 0 means never.
    pub checkpoint_every: u64,
    /// Whether the log and checkpoints call `fsync`.
    ///
    /// The 10,000-cycle gate uses `Deferred`, because an in-process crash cannot observe the
    /// difference and `Full` costs hours. See `current_log::SyncPolicy` for the argument, and for
    /// what it means the gate does *not* test.
    pub sync: SyncPolicy,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            checkpoint_every: 2,
            sync: SyncPolicy::Deferred,
        }
    }
}

impl Config {
    /// The production configuration: every write synced.
    #[must_use]
    pub fn durable() -> Config {
        Config {
            sync: SyncPolicy::Full,
            ..Config::default()
        }
    }
}

fn log_dir(dir: &Path) -> PathBuf {
    dir.join("log")
}

fn ckpt_dir(dir: &Path) -> PathBuf {
    dir.join("ckpt")
}

impl Durable {
    /// Open, recovering whatever is on disk (`docs/DURABILITY.md` R1–R7).
    pub fn open(
        dir: impl AsRef<Path>,
        scenario: &Scenario,
        faults: &mut FaultInjector,
        _config: Config,
    ) -> Result<Durable, String> {
        let dir = dir.as_ref().to_path_buf();
        let catalog: std::collections::BTreeMap<String, Schema> =
            scenario.tables.iter().cloned().collect();

        // R5, R6 · open the log, discarding any torn tail and rebuilding the dedup index.
        let log =
            Log::open(log_dir(&dir), catalog, faults, _config.sync).map_err(|e| e.to_string())?;

        // Build a circuit of the right shape. Recovery restores state into it; it never has to guess
        // the shape, because the plan is the same plan.
        let engine =
            CircuitEngine::build(&scenario.tables, &scenario.query).map_err(|e| e.to_string())?;
        let mut circuit = engine.into_circuit();

        // R1, R2, R4 · choose and verify a checkpoint, deleting partials.
        if let Some(loaded) = checkpoint::load(ckpt_dir(&dir)).map_err(|e| e.to_string())? {
            circuit.restore(&loaded.state).map_err(|e| e.to_string())?;
        }

        if faults.reached(Seam::RecoveryAfterCheckpointBeforeReplay) {
            return Err(format!(
                "injected fault at seam {}",
                Seam::RecoveryAfterCheckpointBeforeReplay.name()
            ));
        }

        let mut durable = Durable {
            dir,
            tables: scenario.tables.clone(),
            query: scenario.query.clone(),
            log,
            circuit,
            sync: _config.sync,
        };

        // R7 · replay the epochs after the checkpoint's epoch.
        durable.replay_suffix(faults)?;
        Ok(durable)
    }

    fn replay_suffix(&mut self, faults: &mut FaultInjector) -> Result<(), String> {
        let sealed = self.log.sealed_epoch();
        let mut at = self.circuit.epoch();
        while at < sealed {
            at += 1;
            let batches = self.log.epoch(at).map_err(|e| e.to_string())?;
            let mut deltas = EpochDeltas::new();
            for batch in batches {
                deltas.extend(batch.table.clone(), batch.entries.iter().cloned());
            }
            // The same step the live path takes. Replay is not a special mode; that is what makes
            // "crash equals replay" a statement about one code path rather than two.
            self.circuit.step(&deltas).map_err(|e| e.to_string())?;
        }
        let _ = faults;
        Ok(())
    }

    /// Feed one epoch: append its batches, seal, step, and maybe checkpoint.
    pub fn apply_epoch(
        &mut self,
        index: usize,
        deltas: &EpochDeltas,
        faults: &mut FaultInjector,
        config: Config,
    ) -> Result<(), String> {
        // §1 · append each table's entries as one batch, with a token derived from the epoch so a
        // retry after a crash offers the same token and is dropped as a replay (I-4).
        for (table, entries) in deltas.tables() {
            let token = format!("epoch-{index}-{table}");
            let _ack = self
                .log
                .append("src", table, entries.clone(), &token, faults)
                .map_err(|e| e.to_string())?;
        }

        // §2 · seal, then step. The seal record is the commit point (S1–S2); the step is a
        // deterministic function of it (S3–S4).
        self.log.seal_epoch(faults).map_err(|e| e.to_string())?;

        if faults.reached(Seam::SealAfterFsyncBeforeStep) {
            return Err(format!(
                "injected fault at seam {}",
                Seam::SealAfterFsyncBeforeStep.name()
            ));
        }

        self.circuit.step(deltas).map_err(|e| e.to_string())?;

        if faults.reached(Seam::SealAfterStepBeforeCounter) {
            return Err(format!(
                "injected fault at seam {}",
                Seam::SealAfterStepBeforeCounter.name()
            ));
        }

        // §3 · checkpoint, on the configured interval.
        if config.checkpoint_every > 0 && self.circuit.epoch() % config.checkpoint_every == 0 {
            checkpoint::take(ckpt_dir(&self.dir), &self.circuit, faults, self.sync)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// The outcome so far, for comparison.
    pub fn outcome(&self) -> Result<RunOutcome, String> {
        Ok(RunOutcome {
            fingerprints: vec![self
                .circuit
                .state_fingerprint()
                .map_err(|e| e.to_string())?],
            answers: vec![match self.circuit.answer() {
                Ok(answer) => answer.render(),
                Err(e) => format!("ERROR: {e}"),
            }],
            log: self.log.render(),
            epoch: self.circuit.epoch(),
            accepted_tokens: Vec::new(),
        })
    }

    #[must_use]
    pub fn epoch(&self) -> u64 {
        self.circuit.epoch()
    }

    #[must_use]
    pub fn log_epoch(&self) -> u64 {
        self.log.sealed_epoch()
    }

    #[must_use]
    pub fn checkpoint_root(&self) -> PathBuf {
        ckpt_dir(&self.dir)
    }

    #[must_use]
    pub fn tables(&self) -> &[(String, Schema)] {
        &self.tables
    }

    #[must_use]
    pub fn query(&self) -> &current_plan::Query {
        &self.query
    }

    #[must_use]
    pub fn known_tokens(&self) -> usize {
        self.log.known_tokens()
    }
}

/// Run a scenario cleanly to the end, returning the outcome after every epoch.
pub fn run_clean(
    dir: impl AsRef<Path>,
    scenario: &Scenario,
    config: Config,
) -> Result<RunOutcome, String> {
    let mut faults = FaultInjector::inert();
    let mut durable = Durable::open(&dir, scenario, &mut faults, config)?;
    let mut fingerprints = Vec::new();
    let mut answers = Vec::new();
    for (index, deltas) in scenario.epochs.iter().enumerate() {
        durable.apply_epoch(index, deltas, &mut faults, config)?;
        let out = durable.outcome()?;
        fingerprints.extend(out.fingerprints);
        answers.extend(out.answers);
    }
    Ok(RunOutcome {
        fingerprints,
        answers,
        log: durable.log.render(),
        epoch: durable.epoch(),
        accepted_tokens: Vec::new(),
    })
}

/// Run a scenario with a fault, then recover and finish. Returns the recovered outcome and which
/// fault actually fired.
pub fn run_with_fault(
    dir: impl AsRef<Path>,
    scenario: &Scenario,
    fault: Fault,
    config: Config,
) -> Result<(RunOutcome, Option<&'static str>), String> {
    let dir = dir.as_ref().to_path_buf();
    let mut injector = match fault {
        Fault::Seam(plan) => FaultInjector::planned(plan),
        _ => FaultInjector::inert(),
    };

    // Phase 1: run until the fault fires or the scenario ends. Every error is treated as a crash:
    // the objects are dropped and nothing in memory survives.
    //
    // Epochs already sealed on disk are skipped. Without that, a run over a recovered directory
    // re-seals every epoch and steps it again — the batches are dropped as replays by the log, but
    // the circuit would still be stepped twice and every weight would double. The idempotency test
    // found exactly that.
    let mut crashed_at: Option<usize> = None;
    {
        match Durable::open(&dir, scenario, &mut injector, config) {
            Ok(mut durable) => {
                for (index, deltas) in scenario.epochs.iter().enumerate() {
                    if (index as u64) < durable.log_epoch() {
                        continue;
                    }
                    if durable
                        .apply_epoch(index, deltas, &mut injector, config)
                        .is_err()
                    {
                        crashed_at = Some(index);
                        break;
                    }
                }
            }
            Err(_) => crashed_at = Some(0),
        }
        // `durable` is dropped here: every in-memory object is gone, which is the information loss a
        // process death causes.
    }

    // A byte-boundary fault is applied to what is on disk, after the run.
    if let Fault::Bytes {
        epoch_index,
        offset,
        truncate,
    } = fault
    {
        let epochs = checkpoint::published_epochs(ckpt_dir(&dir)).map_err(|e| e.to_string())?;
        if let Some(epoch) = epochs.get(epoch_index % epochs.len().max(1)) {
            let _ = checkpoint::corrupt_for_test(ckpt_dir(&dir), *epoch, offset, truncate);
        }
    }

    // Phase 2: recover, still carrying the fault plan. A recovery seam can only fire here — the log
    // is empty when phase 1 opens, so nothing is replayed then — and that is what makes "crash
    // during recovery" reachable at all. A failure here is another crash, and is expected.
    let _ = Durable::open(&dir, scenario, &mut injector, config);

    // What actually fired. Captured *before* the final injector shadows it — the first version of
    // this function returned the final, inert injector's `fired()`, so every cycle reported "no
    // fault" and the gate's fault-count assertion caught it on the first run. That assertion exists
    // for exactly this.
    let fired = injector.fired().map(Seam::name);

    // Phase 3: recover for real and finish the scenario from wherever the log left off.
    let mut injector = FaultInjector::inert();
    let mut durable = Durable::open(&dir, scenario, &mut injector, config)?;
    let mut fingerprints = Vec::new();
    let mut answers = Vec::new();

    // Everything the log already sealed is replayed by `open`; re-offer every epoch so that a batch
    // whose ack was lost is retried with the same token and dropped as a replay (I-4, A3).
    for (index, deltas) in scenario.epochs.iter().enumerate() {
        if (index as u64) < durable.log_epoch() {
            continue;
        }
        durable.apply_epoch(index, deltas, &mut injector, config)?;
    }
    // Re-offer the already-sealed epochs' batches too, to prove the dedup path drops them.
    for (index, deltas) in scenario.epochs.iter().enumerate() {
        for (table, entries) in deltas.tables() {
            let token = format!("epoch-{index}-{table}");
            let ack = durable
                .log
                .append("src", table, entries.clone(), &token, &mut injector)
                .map_err(|e| e.to_string())?;
            if ack != Ack::DroppedAsReplay {
                return Err(format!(
                    "re-offering token {token:?} was not dropped as a replay; exactly-once ingest \
                     is broken (I-4)"
                ));
            }
        }
    }

    let out = durable.outcome()?;
    fingerprints.extend(out.fingerprints);
    answers.extend(out.answers);
    let _ = crashed_at;
    Ok((
        RunOutcome {
            fingerprints,
            answers,
            log: out.log,
            epoch: durable.epoch(),
            accepted_tokens: Vec::new(),
        },
        fired,
    ))
}

/// The seam a plan would fire at, for reporting.
#[must_use]
pub fn planned_seam(plan: FaultPlan) -> &'static str {
    plan.seam.name()
}
