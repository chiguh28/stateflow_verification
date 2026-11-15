use crate::engine::{EvalContext, evaluate_expression, to_truth};
use crate::{
    CellStatus, DiagnosticSeverity, EdgeKind, EventDef, MachineEvaluation, MachineId, Project,
    ReviewDecision, ReviewRuleId, ReviewStatus, Scenario, StateId, StepResult, TruthValue,
    TypedValue, ValueDomain, ValueReference,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const MAX_REVIEW_STEPS: u128 = 100_000;
const MAX_REVIEW_PAGE_SIZE: usize = 200;
const MAX_REVIEW_WINDOW_CELLS: usize = 50_000;
const MAX_REVIEW_EXPORT_CANDIDATES: usize = 100_000;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ReviewFilters {
    pub event_id: Option<String>,
    pub event_ids: Option<Vec<String>>,
    pub machine_id: Option<String>,
    pub rule_code: Option<String>,
    pub decision: Option<ReviewStatus>,
    pub query: Option<String>,
    pub event_start: Option<usize>,
    pub event_count: Option<usize>,
    pub combination_start: Option<String>,
    pub combination_count: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ReviewCandidate {
    pub fingerprint: String,
    pub evidence_hash: String,
    pub rule_id: ReviewRuleId,
    pub rule_code: String,
    pub event_id: String,
    pub event_name: String,
    pub combination_id: String,
    pub initial_states: BTreeMap<MachineId, StateId>,
    pub machine_id: MachineId,
    pub current_state: Option<StateId>,
    pub next_state: Option<StateId>,
    pub source_rule_id: Option<String>,
    pub reason: String,
    pub decision: Option<ReviewDecision>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReviewPage {
    pub revision: u64,
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
    pub candidates: Vec<ReviewCandidate>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ReviewCellKey {
    pub event_id: String,
    pub combination_id: String,
    pub machine_id: MachineId,
}

struct CandidateCollector<'a> {
    project: &'a Project,
    filters: &'a ReviewFilters,
    offset: usize,
    limit: usize,
    total: usize,
    candidates: Vec<ReviewCandidate>,
    deduplicate_cells: bool,
    seen_cells: HashSet<(String, String, MachineId)>,
}

impl CandidateCollector<'_> {
    fn add(&mut self, mut candidate: ReviewCandidate) {
        if !matches_filters(&candidate, self.filters) {
            return;
        }
        let mut decision = self
            .project
            .review_decisions
            .iter()
            .find(|decision| decision.fingerprint == candidate.fingerprint)
            .cloned();
        if let Some(decision) = &mut decision {
            decision.needs_recheck = decision.evidence_hash != candidate.evidence_hash;
        }
        if self.filters.decision.as_ref().is_some_and(|filter_status| {
            decision
                .as_ref()
                .is_none_or(|decision| &decision.status != filter_status)
        }) {
            return;
        }
        if self.deduplicate_cells
            && !self.seen_cells.insert((
                candidate.event_id.clone(),
                candidate.combination_id.clone(),
                candidate.machine_id.clone(),
            ))
        {
            return;
        }
        candidate.decision = decision;
        if self.total >= self.offset && self.candidates.len() < self.limit {
            self.candidates.push(candidate);
        }
        self.total += 1;
    }
}

pub fn get_review_page(
    project: &Project,
    filters: &ReviewFilters,
    offset: usize,
    limit: usize,
    revision: u64,
) -> Result<ReviewPage, String> {
    get_review_page_with_options(
        project,
        filters,
        offset,
        limit,
        revision,
        MAX_REVIEW_PAGE_SIZE,
        false,
    )
}

pub fn get_review_window(
    project: &Project,
    event_ids: Vec<String>,
    combination_start: u128,
    combination_count: usize,
    revision: u64,
) -> Result<Vec<ReviewCellKey>, String> {
    let cell_count = (event_ids.len() as u128)
        .saturating_mul(combination_count as u128)
        .saturating_mul(project.machines.len() as u128);
    if cell_count > MAX_REVIEW_WINDOW_CELLS as u128 {
        return Err(format!(
            "REVIEW_WINDOW_LIMIT: 1回のレビュー強調取得は{MAX_REVIEW_WINDOW_CELLS}セルまでです。"
        ));
    }
    let filters = ReviewFilters {
        event_ids: Some(event_ids),
        combination_start: Some(combination_start.to_string()),
        combination_count: Some(combination_count),
        ..ReviewFilters::default()
    };
    let page = get_review_page_with_options(
        project,
        &filters,
        0,
        MAX_REVIEW_WINDOW_CELLS,
        revision,
        MAX_REVIEW_WINDOW_CELLS,
        true,
    )?;
    let unique_keys: BTreeSet<_> = page
        .candidates
        .into_iter()
        .map(|candidate| ReviewCellKey {
            event_id: candidate.event_id,
            combination_id: candidate.combination_id,
            machine_id: candidate.machine_id,
        })
        .collect();
    Ok(unique_keys.into_iter().collect())
}

pub fn get_review_export_candidates(
    project: &Project,
    filters: &ReviewFilters,
    revision: u64,
) -> Result<Vec<ReviewCandidate>, String> {
    let page = get_review_page_with_options(
        project,
        filters,
        0,
        MAX_REVIEW_EXPORT_CANDIDATES,
        revision,
        MAX_REVIEW_EXPORT_CANDIDATES,
        false,
    )?;
    if page.total > page.candidates.len() {
        return Err(format!(
            "REVIEW_EXPORT_LIMIT: 1回のCSV出力は{MAX_REVIEW_EXPORT_CANDIDATES}件までです。フィルターを絞ってください。"
        ));
    }
    Ok(page.candidates)
}

fn get_review_page_with_options(
    project: &Project,
    filters: &ReviewFilters,
    offset: usize,
    limit: usize,
    revision: u64,
    max_page_size: usize,
    deduplicate_cells: bool,
) -> Result<ReviewPage, String> {
    if revision != project.revision {
        return Err("STALE_REVISION: requested review revision is no longer current".to_owned());
    }
    let errors: Vec<_> = crate::validate_project(project)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .collect();
    if !errors.is_empty() {
        return Err(format!(
            "INVALID_MODEL: {}",
            errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    let event_list: Vec<&EventDef> = project
        .events
        .iter()
        .filter(|event| {
            event.enabled
                && project.review_settings.enabled_events.contains(&event.id)
                && filters
                    .event_id
                    .as_ref()
                    .is_none_or(|filter| filter == &event.id.0)
                && filters
                    .event_ids
                    .as_ref()
                    .is_none_or(|event_ids| event_ids.contains(&event.id.0))
        })
        .collect();
    let event_start = filters.event_start.unwrap_or(0).min(event_list.len());
    let event_end = event_start
        .saturating_add(filters.event_count.unwrap_or(event_list.len()))
        .min(event_list.len());
    let selected_events = &event_list[event_start..event_end];
    let combination_total = crate::count_combinations(project)?;
    let combination_start = filters
        .combination_start
        .as_deref()
        .unwrap_or("0")
        .parse::<u128>()
        .map_err(|_| "組み合わせ開始位置が不正です。".to_owned())?
        .min(combination_total);
    let combination_end = combination_start
        .saturating_add(filters.combination_count.unwrap_or(usize::MAX) as u128)
        .min(combination_total);
    let combination_count = combination_end.saturating_sub(combination_start);
    let total_steps = combination_count.saturating_mul(selected_events.len() as u128);
    if total_steps > MAX_REVIEW_STEPS {
        return Err(format!(
            "REVIEW_SCAN_LIMIT: 候補抽出は最大{MAX_REVIEW_STEPS}件のイベント×状態組み合わせまでです。イベントまたは機能で絞り込んでください。"
        ));
    }

    let page_limit = limit.min(max_page_size);
    let mut collector = CandidateCollector {
        project,
        filters,
        offset,
        limit: page_limit,
        total: 0,
        candidates: Vec::with_capacity(page_limit),
        deduplicate_cells,
        seen_cells: HashSet::new(),
    };
    let order = crate::ordered_machines(project);
    for event in selected_events {
        for combination_index in combination_start..combination_end {
            let states = crate::combination_at(project, &order, combination_index)?;
            let step = crate::evaluate_step(project, event, &states)?;
            let combination_id = format!("COMBO-{:012}", combination_index + 1);
            collect_step_candidates(
                project,
                event,
                &combination_id,
                &states,
                &step,
                filters,
                &mut collector,
            );
        }
    }

    Ok(ReviewPage {
        revision: project.revision,
        offset,
        limit: page_limit,
        total: collector.total,
        candidates: collector.candidates,
    })
}

pub fn save_review_decision(
    project: &mut Project,
    fingerprint: &str,
    evidence_hash: &str,
    status: ReviewStatus,
    note: &str,
) -> Result<ReviewDecision, String> {
    if fingerprint.len() != 64
        || !fingerprint
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err("レビュー候補IDが不正です。".to_owned());
    }
    if evidence_hash.len() != 64
        || !evidence_hash
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err("レビュー根拠IDが不正です。".to_owned());
    }
    if note.chars().count() > 10_000 {
        return Err("レビューのメモは10,000文字以内で入力してください。".to_owned());
    }
    let updated_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| format!("日時を記録できません: {error}"))?;
    let decision = ReviewDecision {
        fingerprint: fingerprint.to_owned(),
        evidence_hash: evidence_hash.to_owned(),
        status,
        note: note.to_owned(),
        updated_at,
        needs_recheck: false,
    };
    if let Some(existing) = project
        .review_decisions
        .iter_mut()
        .find(|existing| existing.fingerprint == fingerprint)
    {
        *existing = decision.clone();
    } else {
        project.review_decisions.push(decision.clone());
    }
    project.revision = project.revision.saturating_add(1);
    Ok(decision)
}

fn collect_step_candidates(
    project: &Project,
    event: &EventDef,
    combination_id: &str,
    initial_states: &BTreeMap<MachineId, StateId>,
    step: &StepResult,
    filters: &ReviewFilters,
    collector: &mut CandidateCollector<'_>,
) {
    let candidate_context = CandidateContext {
        project,
        event,
        combination_id,
        initial_states,
        step,
    };
    let enabled_machine = |machine_id: &MachineId| {
        project
            .review_settings
            .enabled_machines
            .contains(machine_id)
    };
    let enabled_rule =
        |rule_id: &ReviewRuleId| project.review_settings.enabled_rules.contains(rule_id);

    if enabled_rule(&ReviewRuleId::UpstreamChangedDownstreamHeld) {
        for edge in project
            .graph_edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::CurrentState)
        {
            let Some(source) = step
                .evaluations
                .iter()
                .find(|result| result.machine_id == edge.source_machine)
            else {
                continue;
            };
            let Some(target) = step
                .evaluations
                .iter()
                .find(|result| result.machine_id == edge.target_machine)
            else {
                continue;
            };
            if source.status != CellStatus::Changed
                || !matches!(
                    target.status,
                    CellStatus::ExplicitHold | CellStatus::ImplicitHold
                )
                || !enabled_machine(&target.machine_id)
            {
                continue;
            }
            let reason = format!(
                "上流 `{}` が {} から {} に変化しましたが、CURRENT接続先 `{}` は状態を維持しました。条件成立状況を確認してください。",
                source.machine_id.0,
                source.previous_state.0,
                state_label(&source.next_state),
                target.machine_id.0,
            );
            collector.add(candidate(
                &candidate_context,
                ReviewRuleId::UpstreamChangedDownstreamHeld,
                target,
                None,
                reason,
            ));
        }
    }

    for evaluation in &step.evaluations {
        if !enabled_machine(&evaluation.machine_id)
            || filters
                .machine_id
                .as_ref()
                .is_some_and(|filter| filter != &evaluation.machine_id.0)
        {
            continue;
        }
        if enabled_rule(&ReviewRuleId::ImplicitHold)
            && evaluation.status == CellStatus::ImplicitHold
        {
            collector.add(candidate(
                &candidate_context,
                ReviewRuleId::ImplicitHold,
                evaluation,
                None,
                "一致する遷移ルールがなく状態を維持しました。仕様上の保持か確認できます。"
                    .to_owned(),
            ));
        }
        if enabled_rule(&ReviewRuleId::PreviousValueSuppressed) {
            for rule_evaluation in evaluation
                .evaluations
                .iter()
                .filter(|rule| rule.guard != TruthValue::True)
            {
                let Some(rule) = project
                    .transitions
                    .iter()
                    .find(|rule| rule.id == rule_evaluation.rule_id)
                else {
                    continue;
                };
                let expression = match &rule.guard {
                    Some(expression) => expression,
                    None => continue,
                };
                let Some(reason) = previous_value_suppression_reason(
                    project,
                    event,
                    initial_states,
                    step,
                    evaluation,
                    expression,
                ) else {
                    continue;
                };
                collector.add(candidate(
                    &candidate_context,
                    ReviewRuleId::PreviousValueSuppressed,
                    evaluation,
                    Some(rule.id.0.clone()),
                    reason,
                ));
            }
        }
    }

    if enabled_rule(&ReviewRuleId::AllMachinesHeld)
        && !step.evaluations.is_empty()
        && step.evaluations.iter().all(|evaluation| {
            enabled_machine(&evaluation.machine_id)
                && matches!(
                    evaluation.status,
                    CellStatus::ExplicitHold | CellStatus::ImplicitHold
                )
        })
    {
        let all = MachineEvaluation {
            machine_id: MachineId("ALL".to_owned()),
            previous_state: StateId("MULTIPLE".to_owned()),
            next_state: Some(StateId("MULTIPLE".to_owned())),
            status: CellStatus::ImplicitHold,
            selected_rule_id: None,
            evaluations: Vec::new(),
            reason: "すべての機能が状態を維持しました。".to_owned(),
        };
        collector.add(candidate(
            &candidate_context,
            ReviewRuleId::AllMachinesHeld,
            &all,
            None,
            "入力イベントに対して全機能が状態を維持しました。これは不具合確定ではなく確認候補です。".to_owned(),
        ));
    }
}

fn previous_value_suppression_reason(
    project: &Project,
    event: &EventDef,
    initial_states: &BTreeMap<MachineId, StateId>,
    step: &StepResult,
    target: &MachineEvaluation,
    expression: &crate::GuardExpr,
) -> Option<String> {
    let target_index = step
        .evaluations
        .iter()
        .position(|evaluation| evaluation.machine_id == target.machine_id)?;
    let mut current_states: HashMap<MachineId, Option<StateId>> = initial_states
        .iter()
        .map(|(machine, state)| (machine.clone(), Some(state.clone())))
        .collect();
    for evaluation in step.evaluations.iter().take(target_index) {
        current_states.insert(evaluation.machine_id.clone(), evaluation.next_state.clone());
    }
    let before_states: HashMap<MachineId, StateId> = initial_states
        .iter()
        .map(|(machine, state)| (machine.clone(), state.clone()))
        .collect();
    let (inputs_before, inputs_after) = input_values(project, event);
    let mut references = Vec::new();
    collect_previous_references(expression, &mut references);
    if references.is_empty() {
        return None;
    }
    let mut suppressed_reference = None;
    for reference in references {
        let values = alternative_previous_values(project, &reference, expression);
        for value in values {
            let mut scenario = project.scenario.clone();
            let previous_label = match &reference {
                ValueReference::PreviousState(machine) => {
                    scenario
                        .previous_states
                        .insert(machine.clone(), value.clone());
                    format!("PREV.STATE.{}", machine.0)
                }
                ValueReference::PreviousOutput(output) => {
                    scenario
                        .previous_outputs
                        .insert(output.clone(), value.clone());
                    format!("PREV.OUTPUT.{}", output.0)
                }
                _ => continue,
            };
            let context = EvalContext {
                current_states: &current_states,
                before_states: &before_states,
                scenario: &scenario,
                inputs_before: &inputs_before,
                inputs_after: &inputs_after,
                event,
            };
            if to_truth(&evaluate_expression(expression, &context, &mut Vec::new()))
                == TruthValue::True
            {
                let current_value = current_previous_value(&project.scenario, &reference);
                suppressed_reference = Some(format!(
                    "`{previous_label}` の現在値 {} ではルール条件が成立しません。ほかの条件を保ったまま {} に変更するとこのルールが成立するため、前回値が連動を抑止しています。",
                    current_value
                        .map(format_typed_value)
                        .unwrap_or_else(|| "UNKNOWN".to_owned()),
                    format_typed_value(&value),
                ));
                break;
            }
        }
        if suppressed_reference.is_some() {
            break;
        }
    }
    suppressed_reference
}

fn alternative_previous_values(
    project: &Project,
    reference: &ValueReference,
    expression: &crate::GuardExpr,
) -> Vec<TypedValue> {
    match reference {
        ValueReference::PreviousState(machine_id) => project
            .states
            .iter()
            .filter(|state| state.machine_id == *machine_id)
            .map(|state| TypedValue::Enum(state.id.0.clone()))
            .collect(),
        ValueReference::PreviousOutput(output_id) => {
            let Some(output) = project
                .outputs
                .iter()
                .find(|output| output.id == *output_id)
            else {
                return Vec::new();
            };
            values_in_domain(&output.domain, expression)
        }
        _ => Vec::new(),
    }
}

fn values_in_domain(domain: &ValueDomain, expression: &crate::GuardExpr) -> Vec<TypedValue> {
    match domain {
        ValueDomain::Boolean => vec![TypedValue::Boolean(false), TypedValue::Boolean(true)],
        ValueDomain::Enum(values) => values.iter().cloned().map(TypedValue::Enum).collect(),
        ValueDomain::Number { minimum, maximum } => {
            let mut numbers = Vec::new();
            if let Some(minimum) = minimum {
                numbers.push(*minimum);
            }
            if let Some(maximum) = maximum {
                numbers.push(*maximum);
            }
            collect_numeric_literals(expression, &mut numbers);
            numbers.sort_by(f64::total_cmp);
            numbers.dedup_by(|left, right| left == right);
            let mut candidates = numbers.clone();
            for number in numbers {
                let step = number.abs().max(1.0);
                let lower = number - step;
                let upper = number + step;
                if minimum.is_none_or(|minimum| lower >= minimum) {
                    candidates.push(lower);
                }
                if maximum.is_none_or(|maximum| upper <= maximum) {
                    candidates.push(upper);
                }
            }
            candidates.sort_by(f64::total_cmp);
            candidates.dedup_by(|left, right| left == right);
            candidates
                .into_iter()
                .filter(|value| value.is_finite())
                .map(TypedValue::Number)
                .collect()
        }
        ValueDomain::String => Vec::new(),
    }
}

fn collect_numeric_literals(expression: &crate::GuardExpr, values: &mut Vec<f64>) {
    match expression {
        crate::GuardExpr::Literal(TypedValue::Number(value)) => values.push(*value),
        crate::GuardExpr::Compare { left, right, .. } => {
            collect_numeric_literals(left, values);
            collect_numeric_literals(right, values);
        }
        crate::GuardExpr::Not(value) => collect_numeric_literals(value, values),
        crate::GuardExpr::And(values_expr) | crate::GuardExpr::Or(values_expr) => {
            for value in values_expr {
                collect_numeric_literals(value, values);
            }
        }
        _ => {}
    }
}

fn collect_previous_references(
    expression: &crate::GuardExpr,
    references: &mut Vec<ValueReference>,
) {
    match expression {
        crate::GuardExpr::Reference(reference @ ValueReference::PreviousState(_))
        | crate::GuardExpr::Reference(reference @ ValueReference::PreviousOutput(_)) => {
            references.push(reference.clone())
        }
        crate::GuardExpr::Compare { left, right, .. } => {
            collect_previous_references(left, references);
            collect_previous_references(right, references);
        }
        crate::GuardExpr::Not(value) => collect_previous_references(value, references),
        crate::GuardExpr::And(values) | crate::GuardExpr::Or(values) => {
            for value in values {
                collect_previous_references(value, references);
            }
        }
        _ => {}
    }
}

fn input_values(
    project: &Project,
    event: &EventDef,
) -> (
    HashMap<crate::InputId, TypedValue>,
    HashMap<crate::InputId, TypedValue>,
) {
    let mut before: HashMap<_, _> = project
        .scenario
        .fixed_inputs
        .iter()
        .map(|(id, value)| (id.clone(), value.clone()))
        .collect();
    for input in &project.inputs {
        before
            .entry(input.id.clone())
            .or_insert_with(|| input.default_value.clone());
    }
    let mut after = before.clone();
    before.insert(event.input_id.clone(), event.from_value.clone());
    after.insert(event.input_id.clone(), event.to_value.clone());
    (before, after)
}

fn current_previous_value<'a>(
    scenario: &'a Scenario,
    reference: &ValueReference,
) -> Option<&'a TypedValue> {
    match reference {
        ValueReference::PreviousState(machine) => scenario.previous_states.get(machine),
        ValueReference::PreviousOutput(output) => scenario.previous_outputs.get(output),
        _ => None,
    }
}

struct CandidateContext<'a> {
    project: &'a Project,
    event: &'a EventDef,
    combination_id: &'a str,
    initial_states: &'a BTreeMap<MachineId, StateId>,
    step: &'a StepResult,
}

fn candidate(
    context: &CandidateContext<'_>,
    rule_id: ReviewRuleId,
    evaluation: &MachineEvaluation,
    source_rule_id: Option<String>,
    reason: String,
) -> ReviewCandidate {
    let rule_code = review_rule_code(&rule_id).to_owned();
    let identity = serde_json::to_vec(&(
        &context.project.project_id,
        &context.event.id,
        context.initial_states,
        &evaluation.machine_id,
        &rule_id,
        &source_rule_id,
    ))
    .unwrap_or_default();
    let fingerprint = hash(&identity);
    let evidence = serde_json::to_vec(&(
        &context.step.initial_states,
        &context.step.next_states,
        evaluation,
        &context.project.graph_edges,
        &context.project.scenario,
        &source_rule_id,
    ))
    .unwrap_or_default();
    let evidence_hash = hash(&evidence);
    let decision = context
        .project
        .review_decisions
        .iter()
        .find(|decision| decision.fingerprint == fingerprint)
        .cloned()
        .map(|mut decision| {
            decision.needs_recheck = decision.evidence_hash != evidence_hash;
            decision
        });
    ReviewCandidate {
        fingerprint,
        evidence_hash,
        rule_id,
        rule_code,
        event_id: context.event.id.0.clone(),
        event_name: context.event.name.clone(),
        combination_id: context.combination_id.to_owned(),
        initial_states: context.initial_states.clone(),
        machine_id: evaluation.machine_id.clone(),
        current_state: Some(evaluation.previous_state.clone()),
        next_state: evaluation.next_state.clone(),
        source_rule_id,
        reason,
        decision,
    }
}

fn matches_filters(candidate: &ReviewCandidate, filters: &ReviewFilters) -> bool {
    if filters
        .rule_code
        .as_ref()
        .is_some_and(|filter| filter != &candidate.rule_code)
    {
        return false;
    }
    if filters
        .event_id
        .as_ref()
        .is_some_and(|filter| filter != &candidate.event_id)
    {
        return false;
    }
    if filters
        .machine_id
        .as_ref()
        .is_some_and(|filter| filter != &candidate.machine_id.0)
    {
        return false;
    }
    filters.query.as_ref().is_none_or(|query| {
        let query = query.to_lowercase();
        candidate.event_name.to_lowercase().contains(&query)
            || candidate.machine_id.0.to_lowercase().contains(&query)
            || candidate.reason.to_lowercase().contains(&query)
            || candidate.rule_code.to_lowercase().contains(&query)
            || candidate
                .source_rule_id
                .as_ref()
                .is_some_and(|rule_id| rule_id.to_lowercase().contains(&query))
    })
}

fn review_rule_code(rule_id: &ReviewRuleId) -> &'static str {
    match rule_id {
        ReviewRuleId::UpstreamChangedDownstreamHeld => "R01",
        ReviewRuleId::PreviousValueSuppressed => "R02",
        ReviewRuleId::OrderMissedUpdate => "R03",
        ReviewRuleId::ImplicitHold => "R04",
        ReviewRuleId::AllMachinesHeld => "R05",
    }
}

fn state_label(state: &Option<StateId>) -> String {
    state
        .as_ref()
        .map(|state| state.0.clone())
        .unwrap_or_else(|| "UNKNOWN".to_owned())
}

fn format_typed_value(value: &TypedValue) -> String {
    match value {
        TypedValue::Boolean(value) => {
            if *value {
                "ON".to_owned()
            } else {
                "OFF".to_owned()
            }
        }
        TypedValue::Enum(value) | TypedValue::String(value) => value.clone(),
        TypedValue::Number(value) => value.to_string(),
        TypedValue::Unknown => "UNKNOWN".to_owned(),
    }
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::{
        ReviewCellKey, ReviewFilters, get_review_page, get_review_window, save_review_decision,
    };
    use crate::{EdgeKind, ReviewStatus, import_excel_files};
    use std::path::PathBuf;

    fn imported_project() -> crate::Project {
        let workbook = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("golden-workbook.xlsx");
        let mut project = import_excel_files(&[workbook]).unwrap().project;
        project.graph_edges = vec![
            crate::GraphEdge {
                edge_id: "current-ac-link".to_owned(),
                source_machine: crate::MachineId("AC".to_owned()),
                target_machine: crate::MachineId("LINK".to_owned()),
                kind: EdgeKind::CurrentState,
                output_id: None,
                target_reference: None,
            },
            crate::GraphEdge {
                edge_id: "current-link-heater".to_owned(),
                source_machine: crate::MachineId("LINK".to_owned()),
                target_machine: crate::MachineId("HEATER".to_owned()),
                kind: EdgeKind::CurrentState,
                output_id: None,
                target_reference: None,
            },
        ];
        project
    }

    #[test]
    fn review_page_uses_enabled_rules_and_does_not_modify_the_model_results() {
        let mut project = imported_project();
        project.review_settings.enabled_rules = vec![crate::ReviewRuleId::ImplicitHold];
        let page =
            get_review_page(&project, &ReviewFilters::default(), 0, 20, project.revision).unwrap();
        assert!(
            page.candidates
                .iter()
                .all(|candidate| candidate.rule_code == "R04")
        );
        let unchanged_revision = project.revision;
        assert_eq!(project.revision, unchanged_revision);
    }

    #[test]
    fn review_window_returns_unique_keys_only_for_the_requested_cells() {
        let mut project = imported_project();
        project.review_settings.enabled_rules = vec![
            crate::ReviewRuleId::ImplicitHold,
            crate::ReviewRuleId::PreviousValueSuppressed,
        ];
        let event_ids = vec!["AUTO_SW_UP".to_owned(), "AUTO_SW_DOWN".to_owned()];
        let filters = ReviewFilters {
            event_ids: Some(event_ids.clone()),
            combination_start: Some("0".to_owned()),
            combination_count: Some(2),
            ..ReviewFilters::default()
        };
        let page = get_review_page(&project, &filters, 0, 200, project.revision).unwrap();
        let expected: std::collections::BTreeSet<_> = page
            .candidates
            .into_iter()
            .map(|candidate| ReviewCellKey {
                event_id: candidate.event_id,
                combination_id: candidate.combination_id,
                machine_id: candidate.machine_id,
            })
            .collect();
        let actual = get_review_window(&project, event_ids, 0, 2, project.revision).unwrap();
        assert_eq!(actual, expected.into_iter().collect::<Vec<_>>());
    }

    #[test]
    fn review_decisions_are_kept_and_marked_for_recheck_when_evidence_changes() {
        let mut project = imported_project();
        project.review_settings.enabled_rules = vec![crate::ReviewRuleId::ImplicitHold];
        let page =
            get_review_page(&project, &ReviewFilters::default(), 0, 1, project.revision).unwrap();
        let candidate = page
            .candidates
            .first()
            .expect("implicit hold should be a candidate")
            .clone();
        save_review_decision(
            &mut project,
            &candidate.fingerprint,
            &candidate.evidence_hash,
            ReviewStatus::ExpectedByDesign,
            "仕様通り",
        )
        .unwrap();
        project.graph_edges.push(crate::GraphEdge {
            edge_id: "flow-ac-heater".to_owned(),
            source_machine: crate::MachineId("AC".to_owned()),
            target_machine: crate::MachineId("HEATER".to_owned()),
            kind: EdgeKind::FlowOnly,
            output_id: None,
            target_reference: None,
        });
        project.revision += 1;
        let changed =
            get_review_page(&project, &ReviewFilters::default(), 0, 20, project.revision).unwrap();
        let same_identity = changed
            .candidates
            .iter()
            .find(|item| item.fingerprint == candidate.fingerprint)
            .unwrap();
        assert!(same_identity.decision.as_ref().unwrap().needs_recheck);
        assert_eq!(
            same_identity.decision.as_ref().unwrap().status,
            ReviewStatus::ExpectedByDesign
        );
    }
}
