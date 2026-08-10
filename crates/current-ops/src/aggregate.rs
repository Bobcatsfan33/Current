//! `GROUP BY` with the five aggregates (`ARCHITECTURE.md` §5.3; `docs/SEMANTICS.md` S-27 … S-32).
//!
//! ## The shape of the problem
//!
//! An aggregate is neither linear nor bilinear. Its output for a group is a function of the whole
//! group, so a delta touching one row can change that group's row arbitrarily — and can make the
//! group appear or vanish. The incremental form is therefore not an algebraic identity like the
//! join's three terms; it is bookkeeping:
//!
//! 1. work out which groups this epoch touches;
//! 2. for each of them, compute what it emitted **before** the epoch;
//! 3. apply the epoch's changes to that group's state;
//! 4. compute what it emits **after**, and emit `−before + after`.
//!
//! Untouched groups emit nothing, which is what makes this O(change) rather than O(data).
//!
//! ## Why MIN/MAX decide the state layout
//!
//! §5.3 is explicit: "MIN/MAX must keep a per-group multiset, not a single value, or retractions
//! break them: removing the current MIN must reveal the second-smallest, which you can only do if
//! you kept it."
//!
//! So the state is, per group and per aggregate slot, an **ordered multiset of the argument's
//! values** — keyed `[slot, group key…, value]`, which makes a prefix scan return that group's
//! values *in value order* (D-15, S-7). `MIN` is then the first entry of the scan and `MAX` the
//! last, and retracting the current minimum reveals the next one because the next one was never
//! thrown away.
//!
//! The same multiset serves `SUM`, `COUNT` and `AVG` by folding it, and `COUNT(*)` is one extra
//! entry per group holding the group's total row weight. Folding a changed group's multiset is
//! O(distinct values in that group) rather than O(1); that is the honest cost of a layout chosen so
//! that MIN/MAX are correct under retraction, and it is a C10 concern, not a C3 one. No performance
//! claim is made for it.
//!
//! ## What this operator does *not* share with the oracle
//!
//! Its arithmetic. The scalar expression library is shared (D-14) because §6 C5 says so, but
//! aggregation is implemented twice on purpose: the cliffs in this sprint — MIN under retraction,
//! groups vanishing at zero, AVG's single division — are exactly what I-1 is for, and sharing the
//! code would have removed the signal.

use std::collections::BTreeMap;

use current_plan::eval::eval;
use current_plan::plan::{AggFunc, Expr, Named};
use current_plan::PlanError;
use current_state::{Key, StateBackend, WriteBatch};
use current_zset::{Row, Schema, Value, ZSetBatch};

use crate::error::{OpError, Result};
use crate::operator::{error_row, unary, Operator, StateBound, StepOutput};

/// The single input an aggregate reads, named for its state declaration (I-9).
pub const AGGREGATE_INPUTS: &[&str] = &["input"];

/// Key tag for a group's total row weight — what `COUNT(*)` counts.
const TAG_TOTAL: &str = "*";
/// Key tag for a slot's ordered value multiset.
const TAG_VALUES: &str = "v";

/// What a group emits: one row, nothing, or an error.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Emission {
    /// The group does not exist — its total weight is not positive (S-29).
    Absent,
    /// The group's output row, always at weight 1 (S-27).
    Row(Row),
    /// The group cannot be evaluated, so it emits no row and this error is live (S-22a).
    Failed(PlanError),
}

/// `GROUP BY` with aggregates (S-27).
#[derive(Debug)]
pub struct Aggregate {
    input_schema: Schema,
    output_schema: Schema,
    keys: Vec<Named<Expr>>,
    aggregates: Vec<Named<AggFunc>>,
    state: Box<dyn StateBackend>,
}

impl Aggregate {
    /// Build an aggregate. `output_schema` comes from the shared binder, so the engine's idea of the
    /// answer's schema is the oracle's by construction (D-14, S-8, S-27).
    pub fn new(
        input_schema: Schema,
        output_schema: Schema,
        keys: Vec<Named<Expr>>,
        aggregates: Vec<Named<AggFunc>>,
        state: Box<dyn StateBackend>,
    ) -> Result<Aggregate> {
        if keys.is_empty() {
            return Err(OpError::Plan(PlanError::EmptyGroupKeys));
        }
        Ok(Aggregate {
            input_schema,
            output_schema,
            keys,
            aggregates,
            state,
        })
    }

    fn total_key(&self, group: &[Value]) -> Key {
        let mut key = Vec::with_capacity(group.len() + 1);
        key.push(Value::Str(TAG_TOTAL.to_owned()));
        key.extend(group.iter().cloned());
        key
    }

    fn values_prefix(&self, slot: usize, group: &[Value]) -> Key {
        let mut key = Vec::with_capacity(group.len() + 2);
        key.push(Value::Str(TAG_VALUES.to_owned()));
        key.push(Value::Int(slot as i64));
        key.extend(group.iter().cloned());
        key
    }

    fn value_key(&self, slot: usize, group: &[Value], value: &Value) -> Key {
        let mut key = self.values_prefix(slot, group);
        key.push(value.clone());
        key
    }

    /// The ordered multiset of slot `slot`'s argument values for one group.
    ///
    /// Ordered by value, because the value is the last component of the key and the backend scans in
    /// key order (S-7). That ordering is the whole reason MIN/MAX work under retraction.
    fn multiset(&self, slot: usize, group: &[Value]) -> Result<Vec<(Value, i64)>> {
        let prefix = self.values_prefix(slot, group);
        let skip = prefix.len();
        let mut out = Vec::new();
        for (key, weight) in self.state.scan_prefix(&prefix)? {
            let value = key.get(skip).ok_or(OpError::CorruptAggregateState)?.clone();
            out.push((value, weight));
        }
        Ok(out)
    }

    /// What a group emits, given the state as it stands now.
    fn emission(&self, group: &[Value]) -> Result<Emission> {
        let total = self.state.get(&self.total_key(group))?.unwrap_or(0);
        // A group exists iff its total weight is positive; a drained group vanishes rather than
        // emitting a row of zeroes (S-29).
        if total <= 0 {
            return Ok(Emission::Absent);
        }

        let mut values: Vec<Value> = group.to_vec();
        for (slot, agg) in self.aggregates.iter().enumerate() {
            let computed = match &agg.value {
                AggFunc::CountStar => Ok(Value::Int(total)),
                other => self.fold(slot, group, other),
            };
            match computed {
                Ok(value) => values.push(value),
                Err(OpError::Plan(e)) if e.is_evaluation_error() => {
                    return Ok(Emission::Failed(e));
                }
                Err(other) => return Err(other),
            }
        }
        Ok(Emission::Row(Row::new(values)))
    }

    /// Compute one aggregate from its group's ordered multiset (S-30, S-31).
    fn fold(&self, slot: usize, group: &[Value], func: &AggFunc) -> Result<Value> {
        let name = func.name();
        let multiset = self.multiset(slot, group)?;

        match func {
            // Unreachable: handled by the caller from the group total.
            AggFunc::CountStar => Ok(Value::Int(
                self.state.get(&self.total_key(group))?.unwrap_or(0),
            )),

            // COUNT(x) counts entries whose value is not null, and returns 0 — never null — when
            // there are none (S-30). Nulls never enter the multiset, so this is its total weight.
            AggFunc::Count(_) => {
                let mut total: i64 = 0;
                for (_, weight) in &multiset {
                    total = total
                        .checked_add(*weight)
                        .ok_or(OpError::Plan(PlanError::AggregateOverflow { func: name }))?;
                }
                Ok(Value::Int(total))
            }

            AggFunc::Sum(_) => match sum_and_count(&multiset, name)? {
                None => Ok(Value::Null),
                Some((sum, _)) => Ok(Value::Int(sum)),
            },

            // One IEEE-754 division of two exact integers, at emit time, never accumulated
            // (S-31, D-10). This is why AVG may be a Float64 when a float SUM may not exist.
            AggFunc::Avg(_) => match sum_and_count(&multiset, name)? {
                None => Ok(Value::Null),
                Some((sum, count)) => Ok(Value::Float(sum as f64 / count as f64)),
            },

            // The first and last entries of an ordered multiset. Retracting the current MIN leaves
            // the next value in place, and it becomes the first entry — §5.3's requirement, met by
            // the layout rather than by extra code.
            AggFunc::Min(_) => Ok(multiset
                .first()
                .map_or(Value::Null, |(value, _)| value.clone())),
            AggFunc::Max(_) => Ok(multiset
                .last()
                .map_or(Value::Null, |(value, _)| value.clone())),
        }
    }
}

/// The exact weighted sum and count of a multiset, or `None` when it is empty (S-30).
///
/// `i128` accumulator, `i64` result: a sum that transits outside the `Int64` range and lands back
/// inside it is correct, which is exactly what an incremental sum under retraction needs (D-11).
fn sum_and_count(multiset: &[(Value, i64)], func: &'static str) -> Result<Option<(i64, i64)>> {
    let overflow = || OpError::Plan(PlanError::AggregateOverflow { func });
    let mut sum: i128 = 0;
    let mut count: i64 = 0;
    let mut any = false;

    for (value, weight) in multiset {
        let x = match value {
            Value::Int(x) => *x,
            // Binding proves SUM/AVG arguments are Int64 (S-30); reported rather than assumed.
            other => {
                return Err(OpError::Plan(PlanError::AggregateTypeUnsupported {
                    func,
                    ty: other.data_type().unwrap_or(current_zset::DataType::Int64),
                }))
            }
        };
        any = true;
        let term = i128::from(*weight)
            .checked_mul(i128::from(x))
            .ok_or_else(overflow)?;
        sum = sum.checked_add(term).ok_or_else(overflow)?;
        count = count.checked_add(*weight).ok_or_else(overflow)?;
    }

    if !any {
        return Ok(None);
    }
    Ok(Some((i64::try_from(sum).map_err(|_| overflow())?, count)))
}

impl Operator for Aggregate {
    fn name(&self) -> &'static str {
        "aggregate"
    }

    fn arity(&self) -> usize {
        1
    }

    fn output_schema(&self) -> &Schema {
        &self.output_schema
    }

    /// One entry per group for the total, plus one per (slot, group, distinct value).
    ///
    /// The factor is `1 + aggregates` because that is the most entries one input row can create: it
    /// belongs to one group, and contributes at most one value to each slot's multiset. It is a count
    /// of entries, not a tuning knob (I-9).
    fn state_bound(&self) -> StateBound {
        StateBound::ProportionalToInputs {
            inputs: AGGREGATE_INPUTS,
            factor: 1 + self.aggregates.len(),
        }
    }

    fn state_size(&self) -> usize {
        self.state.len()
    }

    fn step(&mut self, inputs: &[&ZSetBatch]) -> Result<StepOutput> {
        let input = unary("aggregate", inputs)?;
        if input.schema() != &self.input_schema {
            return Err(OpError::InputSchemaMismatch {
                op: "aggregate",
                expected: self.input_schema.to_string(),
                found: input.schema().to_string(),
            });
        }

        let mut errors: Vec<(Row, i64)> = Vec::new();
        // Per touched group: the change to its total weight, and to each slot's multiset.
        // `BTreeMap` throughout, so the groups are processed in a fixed order (I-2).
        let mut total_delta: BTreeMap<Vec<Value>, i64> = BTreeMap::new();
        let mut value_delta: BTreeMap<(usize, Vec<Value>, Value), i64> = BTreeMap::new();

        for (row, weight) in input.entries()? {
            // Group keys first. A row whose key cannot be computed belongs to no group, so it is
            // dropped and the error recorded (S-22a).
            let mut group = Vec::with_capacity(self.keys.len());
            let mut key_raised = false;
            for key in &self.keys {
                match eval(&key.value, &row, &self.input_schema) {
                    Ok(value) => group.push(value),
                    Err(e) if e.is_evaluation_error() => {
                        errors.push((error_row(&e), weight));
                        key_raised = true;
                        break;
                    }
                    Err(e) => return Err(OpError::Plan(e)),
                }
            }
            if key_raised {
                continue;
            }

            *total_delta.entry(group.clone()).or_insert(0) += weight;

            for (slot, agg) in self.aggregates.iter().enumerate() {
                let Some(argument) = agg.value.argument() else {
                    // COUNT(*) has no argument and needs no multiset.
                    continue;
                };
                match eval(argument, &row, &self.input_schema) {
                    // Nulls never enter the multiset: every aggregate but COUNT(*) ignores them,
                    // and "no non-null values" is then simply an empty multiset (S-30).
                    Ok(Value::Null) => {}
                    Ok(value) => {
                        *value_delta.entry((slot, group.clone(), value)).or_insert(0) += weight;
                    }
                    Err(e) if e.is_evaluation_error() => {
                        errors.push((error_row(&e), weight));
                    }
                    Err(e) => return Err(OpError::Plan(e)),
                }
            }
        }

        // Every group this epoch touches, in a fixed order.
        let touched: Vec<Vec<Value>> = total_delta.keys().cloned().collect();

        // What each touched group emitted BEFORE the epoch.
        let mut before: Vec<Emission> = Vec::with_capacity(touched.len());
        for group in &touched {
            before.push(self.emission(group)?);
        }

        // Apply the epoch to the state, atomically.
        let mut batch = WriteBatch::new();
        for (group, delta) in &total_delta {
            batch.add(self.total_key(group), *delta);
        }
        for ((slot, group, value), delta) in &value_delta {
            batch.add(self.value_key(*slot, group, value), *delta);
        }
        self.state.write(&batch)?;

        // What each touched group emits AFTER, and therefore what changed.
        let mut out: Vec<(Row, i64)> = Vec::new();
        for (group, was) in touched.iter().zip(before) {
            let now = self.emission(group)?;
            if was == now {
                continue;
            }
            // Retract what the group used to say, then state what it says now. A group that
            // vanished emits only the retraction; one that appeared, only the insertion.
            match was {
                Emission::Absent => {}
                Emission::Row(row) => out.push((row, -1)),
                Emission::Failed(e) => errors.push((error_row(&e), -1)),
            }
            match now {
                Emission::Absent => {}
                Emission::Row(row) => out.push((row, 1)),
                Emission::Failed(e) => errors.push((error_row(&e), 1)),
            }
        }

        let batch = ZSetBatch::from_entries(self.output_schema.clone(), out)?;
        StepOutput::new(batch.consolidate()?, errors)
    }

    /// The state, in key order — deterministic, so two identical runs fingerprint identically (I-2).
    fn render_state(&self) -> Result<String> {
        let mut out = String::new();
        for (key, weight) in self.state.iter_all()? {
            let rendered: Vec<String> = key.iter().map(ToString::to_string).collect();
            out.push_str(&format!("    agg: [{}] => {weight}\n", rendered.join(", ")));
        }
        Ok(out)
    }
}
