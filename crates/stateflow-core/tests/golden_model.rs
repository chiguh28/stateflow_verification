use stateflow_core::{
    CellStatus, EdgeKind, GraphEdge, MachineId, OutputId, Project, StateId, TypedValue,
    evaluate_step, import_excel_files, parse_guard, validate_project,
};
use std::path::PathBuf;

fn golden_project() -> Project {
    let workbook = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("golden-workbook.xlsx");
    let result = import_excel_files(&[workbook]).expect("golden workbook should import");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "GRAPH_REFERENCE_UNRESOLVED"),
        "import must leave cross-machine references unresolved until the user wires edges"
    );
    assert!(
        result.project.graph_edges.is_empty(),
        "Excel import must not create edges"
    );
    assert_eq!(
        result
            .project
            .graph_nodes
            .iter()
            .map(|node| node.x)
            .collect::<Vec<_>>(),
        vec![0.0, 340.0, 680.0],
        "default graph positions should leave room to drag edges between node handles"
    );
    let mut project = result.project;
    project.graph_edges = vec![
        GraphEdge {
            edge_id: "edge-ac-link-current".to_owned(),
            source_machine: MachineId("AC".to_owned()),
            target_machine: MachineId("LINK".to_owned()),
            kind: EdgeKind::CurrentState,
            output_id: None,
            target_reference: None,
        },
        GraphEdge {
            edge_id: "edge-link-heater-current".to_owned(),
            source_machine: MachineId("LINK".to_owned()),
            target_machine: MachineId("HEATER".to_owned()),
            kind: EdgeKind::CurrentState,
            output_id: None,
            target_reference: None,
        },
    ];
    let errors: Vec<_> = validate_project(&project)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == stateflow_core::DiagnosticSeverity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "golden model should validate after manual wiring: {errors:#?}"
    );
    project
}

fn initial_states(project: &Project) -> std::collections::BTreeMap<MachineId, StateId> {
    project
        .machines
        .iter()
        .map(|machine| (machine.id.clone(), machine.initial_state.clone()))
        .collect()
}

fn first_event(project: &Project, id: &str) -> stateflow_core::EventDef {
    project
        .events
        .iter()
        .find(|event| event.id.0 == id)
        .expect("event should exist")
        .clone()
}

#[test]
fn golden_a_links_current_states_sequentially() {
    let project = golden_project();
    let result = evaluate_step(
        &project,
        &first_event(&project, "AUTO_SW_UP"),
        &initial_states(&project),
    )
    .expect("step should evaluate");
    assert_eq!(
        result.next_states[&MachineId("AC".to_owned())],
        Some(StateId("AUTO".to_owned()))
    );
    assert_eq!(
        result.next_states[&MachineId("LINK".to_owned())],
        Some(StateId("ON".to_owned()))
    );
    assert_eq!(
        result.next_states[&MachineId("HEATER".to_owned())],
        Some(StateId("ON".to_owned()))
    );
    assert_eq!(
        result
            .evaluations
            .iter()
            .map(|value| value.machine_id.0.as_str())
            .collect::<Vec<_>>(),
        ["AC", "LINK", "HEATER"]
    );
}

#[test]
fn golden_b_previous_output_changes_heater_result_and_is_explained() {
    let mut project = golden_project();
    project
        .scenario
        .previous_outputs
        .insert(OutputId("HEATER_CMD".to_owned()), TypedValue::Boolean(true));
    let result = evaluate_step(
        &project,
        &first_event(&project, "AUTO_SW_UP"),
        &initial_states(&project),
    )
    .expect("step should evaluate");
    let heater = result
        .evaluations
        .iter()
        .find(|value| value.machine_id.0 == "HEATER")
        .unwrap();
    assert_eq!(heater.next_state, Some(StateId("OFF".to_owned())));
    assert_eq!(heater.status, CellStatus::ImplicitHold);
    assert!(heater.evaluations[0].expressions.iter().any(|expression| {
        expression
            .expression
            .contains("PREV.OUTPUT.HEATER_CMD == OFF")
            && expression.value == "FALSE"
    }));
}

#[test]
fn golden_c_disabled_link_input_explains_state_hold() {
    let mut project = golden_project();
    project.scenario.fixed_inputs.insert(
        stateflow_core::InputId("LINK_ENABLE".to_owned()),
        TypedValue::Boolean(false),
    );
    let result = evaluate_step(
        &project,
        &first_event(&project, "AUTO_SW_UP"),
        &initial_states(&project),
    )
    .expect("step should evaluate");
    assert_eq!(
        result.next_states[&MachineId("AC".to_owned())],
        Some(StateId("AUTO".to_owned()))
    );
    assert_eq!(
        result.next_states[&MachineId("LINK".to_owned())],
        Some(StateId("OFF".to_owned()))
    );
    assert_eq!(
        result.next_states[&MachineId("HEATER".to_owned())],
        Some(StateId("OFF".to_owned()))
    );
    assert_eq!(result.evaluations[1].status, CellStatus::ImplicitHold);
}

#[test]
fn golden_d_missing_cross_machine_edge_blocks_validation() {
    let mut project = golden_project();
    project.graph_edges.clear();
    let unresolved = validate_project(&project)
        .into_iter()
        .filter(|diagnostic| diagnostic.code == "GRAPH_REFERENCE_UNRESOLVED")
        .count();
    assert_eq!(
        unresolved, 4,
        "each of the two downstream rules declares the missing reference"
    );
}

#[test]
fn golden_e_same_priority_conflicting_destinations_are_not_guessed() {
    let mut project = golden_project();
    let mut conflicting_rule = project
        .transitions
        .iter()
        .find(|rule| rule.id.0 == "LINK_001")
        .unwrap()
        .clone();
    conflicting_rule.id = stateflow_core::RuleId("LINK_CONFLICT".to_owned());
    conflicting_rule.guard_source = "TRUE".to_owned();
    conflicting_rule.guard = Some(parse_guard("TRUE").unwrap());
    conflicting_rule.to_state = StateId("OFF".to_owned());
    project.transitions.push(conflicting_rule);
    let result = evaluate_step(
        &project,
        &first_event(&project, "AUTO_SW_UP"),
        &initial_states(&project),
    )
    .expect("step should return a conflict status");
    assert_eq!(result.evaluations[1].status, CellStatus::Conflict);
    assert_eq!(result.next_states[&MachineId("LINK".to_owned())], None);
    assert_eq!(result.evaluations[2].status, CellStatus::Indeterminate);
}

#[test]
fn golden_f_before_and_previous_state_remain_independent() {
    let mut project = golden_project();
    project.scenario.previous_states.insert(
        MachineId("AC".to_owned()),
        TypedValue::Enum("AUTO".to_owned()),
    );
    let ac_rule = project
        .transitions
        .iter_mut()
        .find(|rule| rule.id.0 == "AC_001")
        .unwrap();
    ac_rule.guard_source = "PREV.STATE.AC == AUTO".to_owned();
    ac_rule.guard = Some(parse_guard(&ac_rule.guard_source).unwrap());
    let result = evaluate_step(
        &project,
        &first_event(&project, "LINK_ENABLE_UP"),
        &initial_states(&project),
    )
    .expect("previous-state test should evaluate");
    assert_eq!(
        result.next_states[&MachineId("AC".to_owned())],
        Some(StateId("AUTO".to_owned()))
    );

    let ac_rule = project
        .transitions
        .iter_mut()
        .find(|rule| rule.id.0 == "AC_001")
        .unwrap();
    ac_rule.guard_source = "BEFORE.STATE.AC == MANUAL".to_owned();
    ac_rule.guard = Some(parse_guard(&ac_rule.guard_source).unwrap());
    let result = evaluate_step(
        &project,
        &first_event(&project, "LINK_ENABLE_UP"),
        &initial_states(&project),
    )
    .expect("before-state test should evaluate");
    assert_eq!(
        result.next_states[&MachineId("AC".to_owned())],
        Some(StateId("AUTO".to_owned()))
    );
}

#[test]
fn golden_g_each_event_uses_the_same_independent_starting_state() {
    let project = golden_project();
    let initial = initial_states(&project);
    for event in &project.events {
        let result =
            evaluate_step(&project, event, &initial).expect("each event should be independent");
        assert_eq!(result.initial_states, initial);
        assert_eq!(result.event_id, event.id.0);
    }
}

#[test]
fn legacy_workbook_is_rejected_with_missing_formal_sheets_reported() {
    let workbook = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("references")
        .join("standard_input_template_legacy.xlsx");
    let result = import_excel_files(&[workbook]).expect("legacy workbook should be inspectable");
    for sheet in ["Outputs", "Events"] {
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(sheet))
        );
    }
}
