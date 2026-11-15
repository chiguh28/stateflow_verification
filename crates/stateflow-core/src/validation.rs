use crate::{
    CompareOperator, Diagnostic, DiagnosticSeverity, EdgeKind, GuardExpr, InputId, MachineId,
    Project, TransitionRule, TypedValue, ValueDomain, ValueReference, ValueType,
};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpressionType {
    Boolean,
    Enum,
    Number,
    String,
    Unknown,
}

pub fn validate_project(project: &Project) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let machine_by_id: HashMap<_, _> = project
        .machines
        .iter()
        .map(|machine| (machine.id.clone(), machine))
        .collect();
    let input_by_id: HashMap<_, _> = project
        .inputs
        .iter()
        .map(|input| (input.id.clone(), input))
        .collect();
    let output_by_id: HashMap<_, _> = project
        .outputs
        .iter()
        .map(|output| (output.id.clone(), output))
        .collect();
    let mut machine_ids = HashSet::new();
    let mut input_ids = HashSet::new();
    let mut output_ids = HashSet::new();
    let mut event_ids = HashSet::new();
    let mut rule_ids = HashSet::new();
    let mut state_ids: HashMap<MachineId, HashSet<String>> = HashMap::new();

    for machine in &project.machines {
        if !machine_ids.insert(machine.id.clone()) {
            diagnostics.push(error(
                "DUPLICATE_MACHINE",
                format!("機能ID `{}` が重複しています。", machine.id.0),
                None,
            ));
        }
        if !project
            .states
            .iter()
            .any(|state| state.machine_id == machine.id && state.id == machine.initial_state)
        {
            diagnostics.push(error(
                "INITIAL_STATE_MISSING",
                format!(
                    "機能 `{}` の初期状態 `{}` がStatesにありません。",
                    machine.id.0, machine.initial_state.0
                ),
                None,
            ));
        }
    }

    for state in &project.states {
        if !machine_by_id.contains_key(&state.machine_id) {
            diagnostics.push(error(
                "STATE_MACHINE_MISSING",
                format!(
                    "状態 `{}` の機能 `{}` がMachinesにありません。",
                    state.id.0, state.machine_id.0
                ),
                None,
            ));
        }
        if !state_ids
            .entry(state.machine_id.clone())
            .or_default()
            .insert(state.id.0.clone())
        {
            diagnostics.push(error(
                "DUPLICATE_STATE",
                format!(
                    "機能 `{}` の状態ID `{}` が重複しています。",
                    state.machine_id.0, state.id.0
                ),
                None,
            ));
        }
    }

    for input in &project.inputs {
        if !input_ids.insert(input.id.clone()) {
            diagnostics.push(error(
                "DUPLICATE_INPUT",
                format!("入力ID `{}` が重複しています。", input.id.0),
                None,
            ));
        }
        if let Some(message) =
            validate_typed_value(&input.default_value, &input.value_type, &input.domain)
        {
            diagnostics.push(error(
                "INPUT_DEFAULT_INVALID",
                format!("入力 `{}` のdefault_value: {message}", input.id.0),
                None,
            ));
        }
    }

    for output in &project.outputs {
        if !output_ids.insert(output.id.clone()) {
            diagnostics.push(error(
                "DUPLICATE_OUTPUT",
                format!("出力ID `{}` が重複しています。", output.id.0),
                None,
            ));
        }
        if !machine_by_id.contains_key(&output.owner_machine_id) {
            diagnostics.push(error(
                "OUTPUT_OWNER_MISSING",
                format!(
                    "出力 `{}` の所有機能 `{}` がありません。",
                    output.id.0, output.owner_machine_id.0
                ),
                None,
            ));
        }
        if let Some(value) = &output.default_previous_value {
            if let Some(message) = validate_typed_value(value, &output.value_type, &output.domain) {
                diagnostics.push(error(
                    "OUTPUT_DEFAULT_INVALID",
                    format!("出力 `{}` のdefault_previous_value: {message}", output.id.0),
                    None,
                ));
            }
        }
    }

    for event in &project.events {
        if !event_ids.insert(event.id.clone()) {
            diagnostics.push(error(
                "DUPLICATE_EVENT",
                format!("イベントID `{}` が重複しています。", event.id.0),
                event.source.clone(),
            ));
        }
        let Some(input) = input_by_id.get(&event.input_id) else {
            diagnostics.push(error(
                "EVENT_INPUT_MISSING",
                format!(
                    "イベント `{}` の入力 `{}` がありません。",
                    event.id.0, event.input_id.0
                ),
                event.source.clone(),
            ));
            continue;
        };
        for (field, value) in [
            ("from_value", &event.from_value),
            ("to_value", &event.to_value),
        ] {
            if let Some(message) = validate_typed_value(value, &input.value_type, &input.domain) {
                diagnostics.push(error(
                    "EVENT_VALUE_INVALID",
                    format!("イベント `{}` の{field}: {message}", event.id.0),
                    event.source.clone(),
                ));
            }
        }
        if event.from_value == event.to_value {
            diagnostics.push(error(
                "EVENT_NO_CHANGE",
                format!(
                    "イベント `{}` はfrom_valueとto_valueが同じです。",
                    event.id.0
                ),
                event.source.clone(),
            ));
        }
    }

    for rule in &project.transitions {
        if !rule_ids.insert(rule.id.clone()) {
            diagnostics.push(error(
                "DUPLICATE_RULE",
                format!("ルールID `{}` が重複しています。", rule.id.0),
                rule.source.clone(),
            ));
        }
        if !machine_by_id.contains_key(&rule.machine_id) {
            diagnostics.push(rule_error(
                rule,
                "RULE_MACHINE_MISSING",
                format!("機能 `{}` がMachinesにありません。", rule.machine_id.0),
            ));
        }
        if !state_ids
            .get(&rule.machine_id)
            .is_some_and(|states| states.contains(&rule.from_state.0))
        {
            diagnostics.push(rule_error(
                rule,
                "RULE_FROM_STATE_MISSING",
                format!(
                    "開始状態 `{}` が機能 `{}` にありません。",
                    rule.from_state.0, rule.machine_id.0
                ),
            ));
        }
        if !state_ids
            .get(&rule.machine_id)
            .is_some_and(|states| states.contains(&rule.to_state.0))
        {
            diagnostics.push(rule_error(
                rule,
                "RULE_TO_STATE_MISSING",
                format!(
                    "遷移先 `{}` が機能 `{}` にありません。",
                    rule.to_state.0, rule.machine_id.0
                ),
            ));
        }

        let expression = match crate::parse_guard(&rule.guard_source) {
            Ok(parsed) => {
                if rule.guard.as_ref().is_some_and(|cached| cached != &parsed) {
                    diagnostics.push(rule_error(
                        rule,
                        "GUARD_AST_MISMATCH",
                        "保存されたガードASTがguard文字列と一致しません。".to_owned(),
                    ));
                }
                Some(parsed)
            }
            Err(parse_error) => {
                diagnostics.push(rule_error(
                    rule,
                    "GUARD_PARSE_ERROR",
                    format!("ガード式を解析できません: {parse_error}"),
                ));
                None
            }
        };
        if let Some(expression) = expression {
            match infer_type(
                &expression,
                project,
                &input_by_id,
                &output_by_id,
                &machine_by_id,
            ) {
                Ok(ExpressionType::Boolean | ExpressionType::Unknown) => {}
                Ok(_) => diagnostics.push(rule_error(
                    rule,
                    "GUARD_NOT_BOOLEAN",
                    "ガード式全体はBOOLである必要があります。".to_owned(),
                )),
                Err(message) => diagnostics.push(rule_error(rule, "GUARD_TYPE_ERROR", message)),
            }
            if let Err(message) =
                validate_enum_literals(&expression, project, &input_by_id, &output_by_id)
            {
                diagnostics.push(rule_error(rule, "GUARD_ENUM_VALUE_INVALID", message));
            }
            let mut edge_context = RequiredEdgeContext {
                project,
                machines: &machine_by_id,
                outputs: &output_by_id,
                diagnostics: &mut diagnostics,
            };
            validate_required_edges(&expression, rule, &mut edge_context);
        }
    }

    validate_scenario(
        project,
        &input_by_id,
        &output_by_id,
        &machine_by_id,
        &mut diagnostics,
    );
    validate_graph(project, &machine_by_id, &mut diagnostics);
    diagnostics
}

fn validate_graph(
    project: &Project,
    machines: &HashMap<MachineId, &crate::Machine>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut nodes = HashSet::new();
    let mut x_positions = HashSet::new();
    for node in &project.graph_nodes {
        if !machines.contains_key(&node.machine_id) {
            diagnostics.push(error(
                "GRAPH_NODE_MACHINE_MISSING",
                format!("グラフノードの機能 `{}` がありません。", node.machine_id.0),
                None,
            ));
        }
        if !nodes.insert(node.machine_id.clone()) {
            diagnostics.push(error(
                "GRAPH_NODE_DUPLICATE",
                format!(
                    "機能 `{}` のグラフノードが重複しています。",
                    node.machine_id.0
                ),
                None,
            ));
        }
        if !node.x.is_finite() || !node.y.is_finite() {
            diagnostics.push(error(
                "GRAPH_POSITION_INVALID",
                format!("機能 `{}` のノード位置が不正です。", node.machine_id.0),
                None,
            ));
        } else if !x_positions.insert(node.x.to_bits()) {
            diagnostics.push(error(
                "GRAPH_ORDER_AMBIGUOUS",
                format!(
                    "機能 `{}` と同じx位置のノードがあります。実行順序を一意にしてください。",
                    node.machine_id.0
                ),
                None,
            ));
        }
    }
    for machine in &project.machines {
        if !nodes.contains(&machine.id) {
            diagnostics.push(error(
                "GRAPH_NODE_MISSING",
                format!(
                    "機能 `{}` のノードがありません。グラフには全機能を追加してください。",
                    machine.id.0
                ),
                None,
            ));
        }
    }

    let node_by_machine: HashMap<_, _> = project
        .graph_nodes
        .iter()
        .map(|node| (node.machine_id.clone(), node))
        .collect();
    let mut edge_ids = HashSet::new();
    let mut exact_edges = HashSet::new();
    for edge in &project.graph_edges {
        if !edge_ids.insert(edge.edge_id.as_str()) {
            diagnostics.push(error(
                "GRAPH_EDGE_ID_DUPLICATE",
                format!("接続ID `{}` が重複しています。", edge.edge_id),
                None,
            ));
        }
        if !machines.contains_key(&edge.source_machine)
            || !machines.contains_key(&edge.target_machine)
        {
            diagnostics.push(error(
                "GRAPH_EDGE_MACHINE_MISSING",
                format!("接続 `{}` の機能参照先がありません。", edge.edge_id),
                None,
            ));
        }
        if edge.source_machine == edge.target_machine {
            diagnostics.push(error(
                "GRAPH_SELF_EDGE",
                format!("機能 `{}` の自己接続は不要です。", edge.source_machine.0),
                None,
            ));
        }
        if let (Some(source), Some(target)) = (
            node_by_machine.get(&edge.source_machine),
            node_by_machine.get(&edge.target_machine),
        ) {
            if source.x >= target.x {
                diagnostics.push(error(
                    "GRAPH_DIRECTION_INVALID",
                    format!("接続 `{}` は左から右へ向けてください。", edge.edge_id),
                    None,
                ));
            }
        }
        if !exact_edges.insert((
            edge.source_machine.clone(),
            edge.target_machine.clone(),
            edge.kind.clone(),
            edge.output_id.clone(),
            edge.target_reference.clone(),
        )) {
            diagnostics.push(Diagnostic {
                code: "GRAPH_EDGE_DUPLICATE".to_owned(),
                severity: DiagnosticSeverity::Warning,
                message: format!("接続 `{}` は既存の接続と重複しています。", edge.edge_id),
                detail: None,
                source: None,
            });
        }
        if edge.kind == EdgeKind::PreviousOutput {
            let declared = project.outputs.iter().any(|output| {
                edge.output_id
                    .as_ref()
                    .is_some_and(|output_id| output.id == *output_id)
                    && output.owner_machine_id == edge.source_machine
            });
            if !declared {
                diagnostics.push(error(
                    "GRAPH_OUTPUT_BINDING_INVALID",
                    format!(
                        "PREVIOUS_OUTPUT接続 `{}` の出力宣言または所有機能が一致しません。",
                        edge.edge_id
                    ),
                    None,
                ));
            }
        }
    }
}

struct RequiredEdgeContext<'a> {
    project: &'a Project,
    machines: &'a HashMap<MachineId, &'a crate::Machine>,
    outputs: &'a HashMap<crate::OutputId, &'a crate::OutputDef>,
    diagnostics: &'a mut Vec<Diagnostic>,
}

fn validate_required_edges(
    expression: &GuardExpr,
    rule: &TransitionRule,
    context: &mut RequiredEdgeContext<'_>,
) {
    match expression {
        GuardExpr::Reference(ValueReference::State(machine_id)) => require_edge(
            machine_id,
            &rule.machine_id,
            EdgeKind::CurrentState,
            None,
            rule,
            context,
        ),
        GuardExpr::Reference(ValueReference::PreviousState(machine_id)) => require_edge(
            machine_id,
            &rule.machine_id,
            EdgeKind::PreviousState,
            None,
            rule,
            context,
        ),
        GuardExpr::Reference(ValueReference::PreviousOutput(output_id)) => {
            if let Some(output) = context.outputs.get(output_id) {
                require_edge(
                    &output.owner_machine_id,
                    &rule.machine_id,
                    EdgeKind::PreviousOutput,
                    Some(output_id),
                    rule,
                    context,
                );
            }
        }
        GuardExpr::Compare { left, right, .. } => {
            validate_required_edges(left, rule, context);
            validate_required_edges(right, rule, context);
        }
        GuardExpr::Not(value) => validate_required_edges(value, rule, context),
        GuardExpr::And(values) | GuardExpr::Or(values) => {
            for value in values {
                validate_required_edges(value, rule, context);
            }
        }
        GuardExpr::Literal(_)
        | GuardExpr::Reference(_)
        | GuardExpr::Rise(_)
        | GuardExpr::Fall(_) => {}
    }
}

fn require_edge(
    source: &MachineId,
    target: &MachineId,
    expected_kind: EdgeKind,
    output_id: Option<&crate::OutputId>,
    rule: &TransitionRule,
    context: &mut RequiredEdgeContext<'_>,
) {
    if source == target {
        return;
    }
    if !context.machines.contains_key(source) {
        return;
    }
    let connected = context.project.graph_edges.iter().any(|edge| {
        edge.source_machine == *source
            && edge.target_machine == *target
            && edge.kind == expected_kind
            && output_id
                .is_none_or(|output| edge.output_id.as_ref().is_some_and(|value| value == output))
    });
    if !connected {
        let requirement = match expected_kind {
            EdgeKind::CurrentState => "CURRENT",
            EdgeKind::PreviousState => "PREVIOUS STATE",
            EdgeKind::PreviousOutput => "PREVIOUS OUTPUT",
            EdgeKind::FlowOnly => "FLOW_ONLY",
        };
        context.diagnostics.push(rule_error(
            rule,
            "GRAPH_REFERENCE_UNRESOLVED",
            format!(
                "ガードが要求する `{}->{} ({requirement})` の手動接続がありません。",
                source.0, target.0
            ),
        ));
    }
}

fn validate_enum_literals(
    expression: &GuardExpr,
    project: &Project,
    inputs: &HashMap<InputId, &crate::InputDef>,
    outputs: &HashMap<crate::OutputId, &crate::OutputDef>,
) -> Result<(), String> {
    match expression {
        GuardExpr::Compare { left, right, .. } => {
            validate_enum_literal_pair(left, right, project, inputs, outputs)?;
            validate_enum_literal_pair(right, left, project, inputs, outputs)?;
            validate_enum_literals(left, project, inputs, outputs)?;
            validate_enum_literals(right, project, inputs, outputs)
        }
        GuardExpr::Not(value) => validate_enum_literals(value, project, inputs, outputs),
        GuardExpr::And(values) | GuardExpr::Or(values) => {
            for value in values {
                validate_enum_literals(value, project, inputs, outputs)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_enum_literal_pair(
    possible_literal: &GuardExpr,
    possible_reference: &GuardExpr,
    project: &Project,
    inputs: &HashMap<InputId, &crate::InputDef>,
    outputs: &HashMap<crate::OutputId, &crate::OutputDef>,
) -> Result<(), String> {
    let (GuardExpr::Literal(TypedValue::Enum(value)), GuardExpr::Reference(reference)) =
        (possible_literal, possible_reference)
    else {
        return Ok(());
    };
    let allowed = match reference {
        ValueReference::Input(input_id) => {
            inputs.get(input_id).and_then(|input| match &input.domain {
                ValueDomain::Enum(values) => Some(values.clone()),
                _ => None,
            })
        }
        ValueReference::State(machine_id)
        | ValueReference::BeforeState(machine_id)
        | ValueReference::PreviousState(machine_id) => Some(
            project
                .states
                .iter()
                .filter(|state| state.machine_id == *machine_id)
                .map(|state| state.id.0.clone())
                .collect(),
        ),
        ValueReference::PreviousOutput(output_id) => {
            outputs
                .get(output_id)
                .and_then(|output| match &output.domain {
                    ValueDomain::Enum(values) => Some(values.clone()),
                    _ => None,
                })
        }
    };
    if allowed
        .as_ref()
        .is_some_and(|values| !values.contains(value))
    {
        return Err(format!("ENUM値 `{value}` は比較対象の許容値にありません。"));
    }
    Ok(())
}

fn validate_scenario(
    project: &Project,
    inputs: &HashMap<InputId, &crate::InputDef>,
    outputs: &HashMap<crate::OutputId, &crate::OutputDef>,
    machines: &HashMap<MachineId, &crate::Machine>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (input_id, value) in &project.scenario.fixed_inputs {
        let Some(input) = inputs.get(input_id) else {
            diagnostics.push(error(
                "SCENARIO_INPUT_MISSING",
                format!("固定入力 `{}` がありません。", input_id.0),
                None,
            ));
            continue;
        };
        if let Some(message) = validate_typed_value(value, &input.value_type, &input.domain) {
            diagnostics.push(error(
                "SCENARIO_INPUT_INVALID",
                format!("固定入力 `{}`: {message}", input_id.0),
                None,
            ));
        }
    }
    for (machine_id, value) in &project.scenario.previous_states {
        let Some(machine) = machines.get(machine_id) else {
            diagnostics.push(error(
                "SCENARIO_PREVIOUS_STATE_MACHINE_MISSING",
                format!("PREV.STATEの機能 `{}` がありません。", machine_id.0),
                None,
            ));
            continue;
        };
        match value {
            TypedValue::Unknown => {}
            TypedValue::Enum(state_id)
                if project
                    .states
                    .iter()
                    .any(|state| state.machine_id == machine.id && state.id.0 == *state_id) => {}
            TypedValue::Enum(state_id) => diagnostics.push(error(
                "SCENARIO_PREVIOUS_STATE_INVALID",
                format!(
                    "PREV.STATE.{} の状態 `{state_id}` は定義されていません。",
                    machine_id.0
                ),
                None,
            )),
            _ => diagnostics.push(error(
                "SCENARIO_PREVIOUS_STATE_INVALID",
                format!(
                    "PREV.STATE.{} はENUM状態またはUNKNOWNにしてください。",
                    machine_id.0
                ),
                None,
            )),
        }
    }
    for (output_id, value) in &project.scenario.previous_outputs {
        let Some(output) = outputs.get(output_id) else {
            diagnostics.push(error(
                "SCENARIO_PREVIOUS_OUTPUT_MISSING",
                format!("PREV.OUTPUT `{}` がありません。", output_id.0),
                None,
            ));
            continue;
        };
        if let Some(message) = validate_typed_value(value, &output.value_type, &output.domain) {
            diagnostics.push(error(
                "SCENARIO_PREVIOUS_OUTPUT_INVALID",
                format!("PREV.OUTPUT.{}: {message}", output_id.0),
                None,
            ));
        }
    }
}

fn infer_type(
    expression: &GuardExpr,
    project: &Project,
    inputs: &HashMap<InputId, &crate::InputDef>,
    outputs: &HashMap<crate::OutputId, &crate::OutputDef>,
    machines: &HashMap<MachineId, &crate::Machine>,
) -> Result<ExpressionType, String> {
    match expression {
        GuardExpr::Literal(value) => Ok(value_type(value)),
        GuardExpr::Rise(input_id) | GuardExpr::Fall(input_id) => {
            let input = inputs
                .get(input_id)
                .ok_or_else(|| format!("入力 `{}` がありません。", input_id.0))?;
            if input.value_type != ValueType::Boolean {
                return Err(format!(
                    "RISE/FALLはBOOL入力のみ使えます。`{}` はBOOLではありません。",
                    input_id.0
                ));
            }
            Ok(ExpressionType::Boolean)
        }
        GuardExpr::Reference(reference) => match reference {
            ValueReference::Input(input_id) => inputs
                .get(input_id)
                .map(|input| value_type_from_definition(&input.value_type))
                .ok_or_else(|| format!("入力 `{}` がありません。", input_id.0)),
            ValueReference::State(machine_id)
            | ValueReference::BeforeState(machine_id)
            | ValueReference::PreviousState(machine_id) => {
                let machine = machines
                    .get(machine_id)
                    .ok_or_else(|| format!("機能 `{}` がありません。", machine_id.0))?;
                if !project
                    .states
                    .iter()
                    .any(|state| state.machine_id == machine.id)
                {
                    return Err(format!("機能 `{}` に状態がありません。", machine_id.0));
                }
                Ok(ExpressionType::Enum)
            }
            ValueReference::PreviousOutput(output_id) => outputs
                .get(output_id)
                .map(|output| value_type_from_definition(&output.value_type))
                .ok_or_else(|| format!("出力 `{}` がありません。", output_id.0)),
        },
        GuardExpr::Compare {
            operator,
            left,
            right,
        } => {
            let left_type = infer_type(left, project, inputs, outputs, machines)?;
            let right_type = infer_type(right, project, inputs, outputs, machines)?;
            let boolean_symbol = matches!(
                (left.as_ref(), right.as_ref()),
                (GuardExpr::Literal(TypedValue::Enum(value)), _) if matches!(value.as_str(), "ON" | "OFF")
            ) && right_type == ExpressionType::Boolean
                || matches!(
                    (left.as_ref(), right.as_ref()),
                    (_, GuardExpr::Literal(TypedValue::Enum(value))) if matches!(value.as_str(), "ON" | "OFF")
                ) && left_type == ExpressionType::Boolean;
            if left_type != ExpressionType::Unknown
                && right_type != ExpressionType::Unknown
                && left_type != right_type
                && !boolean_symbol
            {
                return Err(format!(
                    "比較する値の型が一致しません ({left_type:?} と {right_type:?})。"
                ));
            }
            if matches!(
                operator,
                CompareOperator::Less
                    | CompareOperator::LessOrEqual
                    | CompareOperator::Greater
                    | CompareOperator::GreaterOrEqual
            ) && !matches!(left_type, ExpressionType::Number | ExpressionType::Unknown)
            {
                return Err("大小比較はNUMBER値だけに使えます。".to_owned());
            }
            Ok(ExpressionType::Boolean)
        }
        GuardExpr::Not(value) => {
            require_boolean(
                infer_type(value, project, inputs, outputs, machines)?,
                "NOT",
            )?;
            Ok(ExpressionType::Boolean)
        }
        GuardExpr::And(values) | GuardExpr::Or(values) => {
            for value in values {
                require_boolean(
                    infer_type(value, project, inputs, outputs, machines)?,
                    "AND/OR",
                )?;
            }
            Ok(ExpressionType::Boolean)
        }
    }
}

fn require_boolean(value: ExpressionType, operator: &str) -> Result<(), String> {
    if matches!(value, ExpressionType::Boolean | ExpressionType::Unknown) {
        Ok(())
    } else {
        Err(format!("{operator}のオペランドにはBOOL値が必要です。"))
    }
}

fn value_type(value: &TypedValue) -> ExpressionType {
    match value {
        TypedValue::Boolean(_) => ExpressionType::Boolean,
        TypedValue::Enum(_) => ExpressionType::Enum,
        TypedValue::Number(_) => ExpressionType::Number,
        TypedValue::String(_) => ExpressionType::String,
        TypedValue::Unknown => ExpressionType::Unknown,
    }
}

fn value_type_from_definition(value_type: &ValueType) -> ExpressionType {
    match value_type {
        ValueType::Boolean => ExpressionType::Boolean,
        ValueType::Enum => ExpressionType::Enum,
        ValueType::Number => ExpressionType::Number,
        ValueType::String => ExpressionType::String,
    }
}

fn validate_typed_value(
    value: &TypedValue,
    value_type: &ValueType,
    domain: &ValueDomain,
) -> Option<String> {
    match (value, value_type, domain) {
        (TypedValue::Unknown, _, _) => None,
        (TypedValue::Boolean(_), ValueType::Boolean, ValueDomain::Boolean) => None,
        (TypedValue::Enum(value), ValueType::Enum, ValueDomain::Enum(values))
            if values.contains(value) =>
        {
            None
        }
        (
            TypedValue::Number(value),
            ValueType::Number,
            ValueDomain::Number { minimum, maximum },
        ) => {
            if !value.is_finite() {
                Some("NUMBERは有限値である必要があります。".to_owned())
            } else if minimum.is_some_and(|limit| *value < limit)
                || maximum.is_some_and(|limit| *value > limit)
            {
                Some("NUMBERが定義された範囲外です。".to_owned())
            } else {
                None
            }
        }
        (TypedValue::String(_), ValueType::String, ValueDomain::String) => None,
        (TypedValue::Boolean(_), ValueType::Boolean, _) => {
            Some("BOOLのallowed_valuesはBOOL domainである必要があります。".to_owned())
        }
        (TypedValue::Enum(value), ValueType::Enum, ValueDomain::Enum(_)) => {
            Some(format!("`{value}` は許容値ではありません。"))
        }
        _ => Some("値の型が定義と一致しません。".to_owned()),
    }
}

fn error(code: &str, message: String, source: Option<crate::SourceLocation>) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: DiagnosticSeverity::Error,
        message,
        detail: None,
        source,
    }
}

fn rule_error(rule: &TransitionRule, code: &str, message: String) -> Diagnostic {
    error(code, message, rule.source.clone())
}
