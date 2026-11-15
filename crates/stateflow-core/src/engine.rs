use crate::{
    CompareOperator, EventDef, GraphNode, GuardExpr, MachineId, Project, RuleId, Scenario, StateId,
    TransitionRule, TypedValue, ValueReference,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

const MAX_MATRIX_WINDOW_CELLS: usize = 50_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TruthValue {
    True,
    False,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CellStatus {
    Changed,
    ExplicitHold,
    ImplicitHold,
    Indeterminate,
    Conflict,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ExpressionTrace {
    pub expression: String,
    pub value: String,
    pub truth: TruthValue,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuleEvaluation {
    pub rule_id: RuleId,
    pub priority: u32,
    pub guard: TruthValue,
    pub destination: StateId,
    pub expressions: Vec<ExpressionTrace>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MachineEvaluation {
    pub machine_id: MachineId,
    pub previous_state: StateId,
    pub next_state: Option<StateId>,
    pub status: CellStatus,
    pub selected_rule_id: Option<RuleId>,
    pub evaluations: Vec<RuleEvaluation>,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StepResult {
    pub event_id: String,
    pub initial_states: BTreeMap<MachineId, StateId>,
    pub next_states: BTreeMap<MachineId, Option<StateId>>,
    pub evaluations: Vec<MachineEvaluation>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MatrixMetadata {
    pub revision: u64,
    pub event_count: usize,
    pub machine_count: usize,
    pub combination_count: String,
    pub ordered_machines: Vec<MachineId>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MatrixCell {
    pub event_id: String,
    pub combination_id: String,
    pub machine_id: MachineId,
    pub current_state: StateId,
    pub next_state: Option<StateId>,
    pub status: CellStatus,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MatrixWindow {
    pub revision: u64,
    pub event_start: usize,
    pub combination_start: String,
    pub event_count: usize,
    pub combination_count: usize,
    pub cells: Vec<MatrixCell>,
}

#[derive(Clone, Debug)]
pub(crate) struct EvalContext<'a> {
    pub(crate) current_states: &'a HashMap<MachineId, Option<StateId>>,
    pub(crate) before_states: &'a HashMap<MachineId, StateId>,
    pub(crate) scenario: &'a Scenario,
    pub(crate) inputs_before: &'a HashMap<crate::InputId, TypedValue>,
    pub(crate) inputs_after: &'a HashMap<crate::InputId, TypedValue>,
    pub(crate) event: &'a EventDef,
}

pub fn ordered_machines(project: &Project) -> Vec<MachineId> {
    let mut nodes: Vec<&GraphNode> = project.graph_nodes.iter().collect();
    nodes.sort_by(|left, right| {
        left.x
            .total_cmp(&right.x)
            .then_with(|| left.machine_id.cmp(&right.machine_id))
    });
    nodes
        .into_iter()
        .map(|node| node.machine_id.clone())
        .collect()
}

pub fn count_combinations(project: &Project) -> Result<u128, String> {
    let order = ordered_machines(project);
    let states_by_machine: HashMap<_, usize> = project
        .machines
        .iter()
        .map(|machine| {
            let count = project
                .states
                .iter()
                .filter(|state| state.machine_id == machine.id)
                .count();
            (machine.id.clone(), count)
        })
        .collect();
    order.into_iter().try_fold(1_u128, |count, machine_id| {
        let state_count = states_by_machine.get(&machine_id).copied().unwrap_or(0);
        count
            .checked_mul(state_count as u128)
            .ok_or_else(|| "状態組み合わせ数がu128を超えました。".to_owned())
    })
}

pub fn get_matrix_metadata(project: &Project) -> Result<MatrixMetadata, String> {
    Ok(MatrixMetadata {
        revision: project.revision,
        event_count: project.events.iter().filter(|event| event.enabled).count(),
        machine_count: project.machines.len(),
        combination_count: count_combinations(project)?.to_string(),
        ordered_machines: ordered_machines(project),
    })
}

pub fn get_matrix_window(
    project: &Project,
    event_start: usize,
    event_count: usize,
    combination_start: u128,
    combination_count: usize,
    revision: u64,
) -> Result<MatrixWindow, String> {
    if revision != project.revision {
        return Err("STALE_REVISION: requested matrix revision is no longer current".to_owned());
    }
    let ordered = ordered_machines(project);
    let cell_count = event_count
        .checked_mul(combination_count)
        .and_then(|count| count.checked_mul(ordered.len()))
        .ok_or_else(|| "指定したマトリクス窓のサイズが大きすぎます。".to_owned())?;
    if cell_count > MAX_MATRIX_WINDOW_CELLS {
        return Err(format!(
            "1回に取得できるセル数は{MAX_MATRIX_WINDOW_CELLS}件までです。"
        ));
    }

    let available_events: Vec<&EventDef> = project
        .events
        .iter()
        .filter(|event| event.enabled)
        .collect();
    let combination_total = count_combinations(project)?;
    if combination_start > combination_total {
        return Err("組み合わせ開始位置が範囲外です。".to_owned());
    }
    let effective_event_count = event_count.min(available_events.len().saturating_sub(event_start));
    let effective_combination_count = combination_count.min(
        usize::try_from(combination_total.saturating_sub(combination_start)).unwrap_or(usize::MAX),
    );
    let mut cells =
        Vec::with_capacity(effective_event_count * effective_combination_count * ordered.len());

    for event in available_events
        .iter()
        .skip(event_start)
        .take(effective_event_count)
    {
        for combination_index in 0..effective_combination_count {
            let absolute_index = combination_start + combination_index as u128;
            let states = combination_at(project, &ordered, absolute_index)?;
            let step = evaluate_step(project, event, &states)?;
            let combination_id = format!("COMBO-{:012}", absolute_index + 1);
            for machine_id in &ordered {
                let initial_state = states
                    .get(machine_id)
                    .ok_or_else(|| format!("initial state missing for machine {}", machine_id.0))?;
                let machine_result = step
                    .evaluations
                    .iter()
                    .find(|result| result.machine_id == *machine_id)
                    .ok_or_else(|| format!("evaluation missing for machine {}", machine_id.0))?;
                cells.push(MatrixCell {
                    event_id: event.id.0.clone(),
                    combination_id: combination_id.clone(),
                    machine_id: machine_id.clone(),
                    current_state: initial_state.clone(),
                    next_state: machine_result.next_state.clone(),
                    status: machine_result.status,
                });
            }
        }
    }

    Ok(MatrixWindow {
        revision: project.revision,
        event_start,
        combination_start: combination_start.to_string(),
        event_count: effective_event_count,
        combination_count: effective_combination_count,
        cells,
    })
}

pub fn combination_at(
    project: &Project,
    ordered: &[MachineId],
    mut index: u128,
) -> Result<BTreeMap<MachineId, StateId>, String> {
    if index >= count_combinations(project)? {
        return Err("組み合わせインデックスが範囲外です。".to_owned());
    }
    let mut states_by_machine: HashMap<MachineId, Vec<StateId>> = HashMap::new();
    for state in &project.states {
        states_by_machine
            .entry(state.machine_id.clone())
            .or_default()
            .push(state.id.clone());
    }
    let mut result = BTreeMap::new();
    for machine_id in ordered.iter().rev() {
        let states = states_by_machine
            .get(machine_id)
            .ok_or_else(|| format!("機能 `{}` に状態がありません。", machine_id.0))?;
        let state_index = (index % states.len() as u128) as usize;
        index /= states.len() as u128;
        result.insert(machine_id.clone(), states[state_index].clone());
    }
    Ok(result)
}

pub fn evaluate_step(
    project: &Project,
    event: &EventDef,
    initial_states: &BTreeMap<MachineId, StateId>,
) -> Result<StepResult, String> {
    let machine_order = ordered_machines(project);
    let mut current_states: HashMap<MachineId, Option<StateId>> = initial_states
        .iter()
        .map(|(machine, state)| (machine.clone(), Some(state.clone())))
        .collect();
    let before_states: HashMap<MachineId, StateId> = initial_states
        .iter()
        .map(|(id, state)| (id.clone(), state.clone()))
        .collect();
    let mut inputs_before: HashMap<crate::InputId, TypedValue> = project
        .scenario
        .fixed_inputs
        .iter()
        .map(|(id, value)| (id.clone(), value.clone()))
        .collect();
    for input in &project.inputs {
        inputs_before
            .entry(input.id.clone())
            .or_insert_with(|| input.default_value.clone());
    }
    let mut inputs_after = inputs_before.clone();
    inputs_before.insert(event.input_id.clone(), event.from_value.clone());
    inputs_after.insert(event.input_id.clone(), event.to_value.clone());
    let mut evaluations = Vec::with_capacity(machine_order.len());

    for machine_id in machine_order {
        let state_before = current_states
            .get(&machine_id)
            .cloned()
            .flatten()
            .ok_or_else(|| format!("current state for {} is unknown", machine_id.0))?;
        let candidates: Vec<&TransitionRule> = project
            .transitions
            .iter()
            .filter(|rule| rule.machine_id == machine_id && rule.from_state == state_before)
            .collect();
        let context = EvalContext {
            current_states: &current_states,
            before_states: &before_states,
            scenario: &project.scenario,
            inputs_before: &inputs_before,
            inputs_after: &inputs_after,
            event,
        };
        let mut rule_evaluations = Vec::with_capacity(candidates.len());
        for rule in candidates {
            let expression = match &rule.guard {
                Some(expression) => expression.clone(),
                None => crate::parse_guard(&rule.guard_source)
                    .map_err(|error| format!("{}: {error}", rule.id.0))?,
            };
            let mut expression_trace = Vec::new();
            let result = evaluate_expression(&expression, &context, &mut expression_trace);
            let truth = to_truth(&result);
            rule_evaluations.push(RuleEvaluation {
                rule_id: rule.id.clone(),
                priority: rule.priority,
                guard: truth,
                destination: rule.to_state.clone(),
                expressions: expression_trace,
            });
        }
        rule_evaluations.sort_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| left.rule_id.cmp(&right.rule_id))
        });

        let mut true_indices: Vec<usize> = rule_evaluations
            .iter()
            .enumerate()
            .filter_map(|(index, evaluation)| {
                (evaluation.guard == TruthValue::True).then_some(index)
            })
            .collect();
        true_indices.sort_by(|left, right| {
            rule_evaluations[*left]
                .priority
                .cmp(&rule_evaluations[*right].priority)
                .then_with(|| {
                    rule_evaluations[*left]
                        .rule_id
                        .cmp(&rule_evaluations[*right].rule_id)
                })
        });
        let unknown_indices: Vec<usize> = rule_evaluations
            .iter()
            .enumerate()
            .filter_map(|(index, evaluation)| {
                (evaluation.guard == TruthValue::Unknown).then_some(index)
            })
            .collect();

        let evaluation = if let Some(first_index) = true_indices.first().copied() {
            let best_priority = rule_evaluations[first_index].priority;
            if unknown_indices
                .iter()
                .any(|index| rule_evaluations[*index].priority <= best_priority)
            {
                MachineEvaluation {
                    machine_id: machine_id.clone(),
                    previous_state: state_before.clone(),
                    next_state: None,
                    status: CellStatus::Indeterminate,
                    selected_rule_id: None,
                    evaluations: rule_evaluations,
                    reason: "同位または上位priorityの条件にUNKNOWNがあり、遷移先を一意に決められません。".to_owned(),
                }
            } else {
                let best: Vec<usize> = true_indices
                    .into_iter()
                    .filter(|index| rule_evaluations[*index].priority == best_priority)
                    .collect();
                let distinct_destinations: std::collections::HashSet<&StateId> = best
                    .iter()
                    .map(|index| &rule_evaluations[*index].destination)
                    .collect();
                if distinct_destinations.len() > 1 {
                    MachineEvaluation {
                        machine_id: machine_id.clone(),
                        previous_state: state_before.clone(),
                        next_state: None,
                        status: CellStatus::Conflict,
                        selected_rule_id: None,
                        evaluations: rule_evaluations,
                        reason: format!(
                            "priority {best_priority}で異なる遷移先が同時に成立しました。"
                        ),
                    }
                } else {
                    let selected = &rule_evaluations[best[0]];
                    let next_state = selected.destination.clone();
                    let selected_rule_id = selected.rule_id.clone();
                    let status = if next_state == state_before {
                        CellStatus::ExplicitHold
                    } else {
                        CellStatus::Changed
                    };
                    let reason = format!(
                        "{} が成立し、priority {} を採用しました。",
                        selected_rule_id.0, best_priority
                    );
                    MachineEvaluation {
                        machine_id: machine_id.clone(),
                        previous_state: state_before.clone(),
                        next_state: Some(next_state),
                        status,
                        selected_rule_id: Some(selected_rule_id),
                        evaluations: rule_evaluations,
                        reason,
                    }
                }
            }
        } else if !unknown_indices.is_empty() {
            MachineEvaluation {
                machine_id: machine_id.clone(),
                previous_state: state_before.clone(),
                next_state: None,
                status: CellStatus::Indeterminate,
                selected_rule_id: None,
                evaluations: rule_evaluations,
                reason: "条件がUNKNOWNのルールがあり、遷移または状態維持を判定できません。"
                    .to_owned(),
            }
        } else {
            MachineEvaluation {
                machine_id: machine_id.clone(),
                previous_state: state_before.clone(),
                next_state: Some(state_before.clone()),
                status: CellStatus::ImplicitHold,
                selected_rule_id: None,
                evaluations: rule_evaluations,
                reason: "該当するガードがなく、状態を維持しました。".to_owned(),
            }
        };

        current_states.insert(machine_id, evaluation.next_state.clone());
        evaluations.push(evaluation);
    }

    let next_states = current_states.into_iter().collect();
    Ok(StepResult {
        event_id: event.id.0.clone(),
        initial_states: initial_states.clone(),
        next_states,
        evaluations,
    })
}

pub(crate) fn evaluate_expression(
    expression: &GuardExpr,
    context: &EvalContext<'_>,
    trace: &mut Vec<ExpressionTrace>,
) -> TypedValue {
    let value = match expression {
        GuardExpr::Literal(value) => value.clone(),
        GuardExpr::Reference(reference) => evaluate_reference(reference, context),
        GuardExpr::Rise(input_id) => {
            let old = context
                .inputs_before
                .get(input_id)
                .unwrap_or(&TypedValue::Unknown);
            let new = context
                .inputs_after
                .get(input_id)
                .unwrap_or(&TypedValue::Unknown);
            TypedValue::Boolean(
                input_id == &context.event.input_id
                    && old == &TypedValue::Boolean(false)
                    && new == &TypedValue::Boolean(true),
            )
        }
        GuardExpr::Fall(input_id) => {
            let old = context
                .inputs_before
                .get(input_id)
                .unwrap_or(&TypedValue::Unknown);
            let new = context
                .inputs_after
                .get(input_id)
                .unwrap_or(&TypedValue::Unknown);
            TypedValue::Boolean(
                input_id == &context.event.input_id
                    && old == &TypedValue::Boolean(true)
                    && new == &TypedValue::Boolean(false),
            )
        }
        GuardExpr::Compare {
            operator,
            left,
            right,
        } => {
            let left = evaluate_expression(left, context, trace);
            let right = evaluate_expression(right, context, trace);
            compare_values(operator, &left, &right)
        }
        GuardExpr::Not(value) => match to_truth(&evaluate_expression(value, context, trace)) {
            TruthValue::True => TypedValue::Boolean(false),
            TruthValue::False => TypedValue::Boolean(true),
            TruthValue::Unknown => TypedValue::Unknown,
        },
        GuardExpr::And(values) => {
            let mut result = TruthValue::True;
            for value in values {
                result = and_truth(
                    result,
                    to_truth(&evaluate_expression(value, context, trace)),
                );
            }
            truth_to_value(result)
        }
        GuardExpr::Or(values) => {
            let mut result = TruthValue::False;
            for value in values {
                result = or_truth(
                    result,
                    to_truth(&evaluate_expression(value, context, trace)),
                );
            }
            truth_to_value(result)
        }
    };
    trace.push(ExpressionTrace {
        expression: format_expression(expression),
        value: format_value(&value),
        truth: to_truth(&value),
    });
    value
}

fn format_expression(expression: &GuardExpr) -> String {
    match expression {
        GuardExpr::Literal(TypedValue::Boolean(value)) => {
            if *value {
                "ON".to_owned()
            } else {
                "OFF".to_owned()
            }
        }
        GuardExpr::Literal(TypedValue::Enum(value) | TypedValue::String(value)) => value.clone(),
        GuardExpr::Literal(TypedValue::Number(value)) => value.to_string(),
        GuardExpr::Literal(TypedValue::Unknown) => "UNKNOWN".to_owned(),
        GuardExpr::Reference(reference) => match reference {
            ValueReference::Input(id) => format!("INPUT.{}", id.0),
            ValueReference::State(id) => format!("STATE.{}", id.0),
            ValueReference::BeforeState(id) => format!("BEFORE.STATE.{}", id.0),
            ValueReference::PreviousState(id) => format!("PREV.STATE.{}", id.0),
            ValueReference::PreviousOutput(id) => format!("PREV.OUTPUT.{}", id.0),
        },
        GuardExpr::Rise(id) => format!("RISE({})", id.0),
        GuardExpr::Fall(id) => format!("FALL({})", id.0),
        GuardExpr::Compare {
            operator,
            left,
            right,
        } => {
            let operator = match operator {
                CompareOperator::Equal => "==",
                CompareOperator::NotEqual => "!=",
                CompareOperator::Less => "<",
                CompareOperator::LessOrEqual => "<=",
                CompareOperator::Greater => ">",
                CompareOperator::GreaterOrEqual => ">=",
            };
            format!(
                "{} {operator} {}",
                format_expression(left),
                format_expression(right)
            )
        }
        GuardExpr::Not(value) => format!("NOT ({})", format_expression(value)),
        GuardExpr::And(values) => values
            .iter()
            .map(format_expression)
            .collect::<Vec<_>>()
            .join(" AND "),
        GuardExpr::Or(values) => values
            .iter()
            .map(format_expression)
            .collect::<Vec<_>>()
            .join(" OR "),
    }
}

fn evaluate_reference(reference: &ValueReference, context: &EvalContext<'_>) -> TypedValue {
    match reference {
        ValueReference::Input(input_id) => context
            .inputs_after
            .get(input_id)
            .cloned()
            .unwrap_or(TypedValue::Unknown),
        ValueReference::State(machine_id) => context
            .current_states
            .get(machine_id)
            .and_then(Option::as_ref)
            .map(|state| TypedValue::Enum(state.0.clone()))
            .unwrap_or(TypedValue::Unknown),
        ValueReference::BeforeState(machine_id) => context
            .before_states
            .get(machine_id)
            .map(|state| TypedValue::Enum(state.0.clone()))
            .unwrap_or(TypedValue::Unknown),
        ValueReference::PreviousState(machine_id) => context
            .scenario
            .previous_states
            .get(machine_id)
            .cloned()
            .unwrap_or(TypedValue::Unknown),
        ValueReference::PreviousOutput(output_id) => context
            .scenario
            .previous_outputs
            .get(output_id)
            .cloned()
            .unwrap_or(TypedValue::Unknown),
    }
}

fn compare_values(operator: &CompareOperator, left: &TypedValue, right: &TypedValue) -> TypedValue {
    if matches!(left, TypedValue::Unknown) || matches!(right, TypedValue::Unknown) {
        return TypedValue::Unknown;
    }
    let left = boolean_symbol_for_comparison(left, right);
    let right = boolean_symbol_for_comparison(right, &left);
    let result = match (operator, left, right) {
        (CompareOperator::Equal, left, right) => left == right,
        (CompareOperator::NotEqual, left, right) => left != right,
        (CompareOperator::Less, TypedValue::Number(left), TypedValue::Number(right)) => {
            left < right
        }
        (CompareOperator::LessOrEqual, TypedValue::Number(left), TypedValue::Number(right)) => {
            left <= right
        }
        (CompareOperator::Greater, TypedValue::Number(left), TypedValue::Number(right)) => {
            left > right
        }
        (CompareOperator::GreaterOrEqual, TypedValue::Number(left), TypedValue::Number(right)) => {
            left >= right
        }
        _ => return TypedValue::Unknown,
    };
    TypedValue::Boolean(result)
}

fn boolean_symbol_for_comparison(value: &TypedValue, other: &TypedValue) -> TypedValue {
    if matches!(other, TypedValue::Boolean(_)) {
        if let TypedValue::Enum(symbol) = value {
            return match symbol.as_str() {
                "ON" => TypedValue::Boolean(true),
                "OFF" => TypedValue::Boolean(false),
                _ => value.clone(),
            };
        }
    }
    value.clone()
}

pub(crate) fn to_truth(value: &TypedValue) -> TruthValue {
    match value {
        TypedValue::Boolean(true) => TruthValue::True,
        TypedValue::Boolean(false) => TruthValue::False,
        TypedValue::Unknown => TruthValue::Unknown,
        _ => TruthValue::Unknown,
    }
}

fn truth_to_value(truth: TruthValue) -> TypedValue {
    match truth {
        TruthValue::True => TypedValue::Boolean(true),
        TruthValue::False => TypedValue::Boolean(false),
        TruthValue::Unknown => TypedValue::Unknown,
    }
}

fn and_truth(left: TruthValue, right: TruthValue) -> TruthValue {
    match (left, right) {
        (TruthValue::False, _) | (_, TruthValue::False) => TruthValue::False,
        (TruthValue::True, TruthValue::True) => TruthValue::True,
        _ => TruthValue::Unknown,
    }
}

fn or_truth(left: TruthValue, right: TruthValue) -> TruthValue {
    match (left, right) {
        (TruthValue::True, _) | (_, TruthValue::True) => TruthValue::True,
        (TruthValue::False, TruthValue::False) => TruthValue::False,
        _ => TruthValue::Unknown,
    }
}

fn format_value(value: &TypedValue) -> String {
    match value {
        TypedValue::Boolean(value) => value.to_string().to_uppercase(),
        TypedValue::Enum(value) | TypedValue::String(value) => value.clone(),
        TypedValue::Number(value) => value.to_string(),
        TypedValue::Unknown => "UNKNOWN".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{TruthValue, and_truth, or_truth};

    #[test]
    fn three_valued_and_and_or_follow_kleene_logic() {
        assert_eq!(
            and_truth(TruthValue::False, TruthValue::Unknown),
            TruthValue::False
        );
        assert_eq!(
            and_truth(TruthValue::True, TruthValue::Unknown),
            TruthValue::Unknown
        );
        assert_eq!(
            or_truth(TruthValue::True, TruthValue::Unknown),
            TruthValue::True
        );
        assert_eq!(
            or_truth(TruthValue::False, TruthValue::Unknown),
            TruthValue::Unknown
        );
    }
}
