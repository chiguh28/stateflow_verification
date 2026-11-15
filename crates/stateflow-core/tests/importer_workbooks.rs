use stateflow_core::{EdgeKind, GraphEdge, MachineId, StateId, evaluate_step, import_excel_files};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn fixture_path(file_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(file_name)
}

fn repository_path(relative_path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("stateflow-core crate should have a parent directory")
        .parent()
        .expect("crates directory should have a repository root")
        .join(relative_path)
}

#[test]
fn standard_v1_template_imports_as_a_wired_by_user_sample() {
    let imported = import_excel_files(&[repository_path("templates/stateflow_input_v1.xlsx")])
        .expect("standard input template should import");

    assert_eq!(imported.project.machines.len(), 3);
    assert_eq!(imported.project.states.len(), 6);
    assert_eq!(imported.project.inputs.len(), 3);
    assert_eq!(imported.project.outputs.len(), 1);
    assert_eq!(imported.project.events.len(), 4);
    assert_eq!(imported.project.transitions.len(), 6);
    assert!(imported.project.graph_edges.is_empty());
    assert_eq!(
        imported
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "GRAPH_REFERENCE_UNRESOLVED")
            .count(),
        4,
        "the sample's two cross-machine CURRENT edges remain unwired; each is required by two rules"
    );
    assert!(imported.project.events.iter().all(|event| event.enabled));

    let mut project = imported.project;
    project.graph_edges = vec![current_edge("AC", "LINK"), current_edge("LINK", "HEATER")];
    let event = project
        .events
        .iter()
        .find(|event| event.id.0 == "AUTO_SW_UP")
        .expect("sample should define the AUTO_SW rising event");
    let initial_states = BTreeMap::from([
        (MachineId("AC".to_owned()), StateId("MANUAL".to_owned())),
        (MachineId("LINK".to_owned()), StateId("OFF".to_owned())),
        (MachineId("HEATER".to_owned()), StateId("OFF".to_owned())),
    ]);
    let step =
        evaluate_step(&project, event, &initial_states).expect("wired sample should evaluate");

    assert_eq!(
        step.next_states.get(&MachineId("AC".to_owned())),
        Some(&Some(StateId("AUTO".to_owned())))
    );
    assert_eq!(
        step.next_states.get(&MachineId("LINK".to_owned())),
        Some(&Some(StateId("ON".to_owned())))
    );
    assert_eq!(
        step.next_states.get(&MachineId("HEATER".to_owned())),
        Some(&Some(StateId("ON".to_owned())))
    );
}

fn current_edge(source: &str, target: &str) -> GraphEdge {
    GraphEdge {
        edge_id: format!("{source}-to-{target}"),
        source_machine: MachineId(source.to_owned()),
        target_machine: MachineId(target.to_owned()),
        kind: EdgeKind::CurrentState,
        output_id: None,
        target_reference: None,
    }
}

#[test]
fn multiple_workbooks_merge_distinct_models() {
    let result = import_excel_files(&[
        fixture_path("golden-workbook.xlsx"),
        fixture_path("second-workbook.xlsx"),
    ])
    .expect("both workbooks should import");

    assert_eq!(result.project.machines.len(), 6);
    assert_eq!(result.project.states.len(), 12);
    assert_eq!(result.project.inputs.len(), 6);
    assert_eq!(result.project.outputs.len(), 2);
    assert_eq!(result.project.events.len(), 8);
    assert_eq!(result.project.transitions.len(), 12);
    assert_eq!(result.project.graph_edges.len(), 0);
    assert_eq!(result.project.imports.len(), 2);
    assert!(
        result
            .project
            .machines
            .iter()
            .any(|machine| machine.id.0 == "FAN")
    );
    assert!(
        result
            .project
            .machines
            .iter()
            .any(|machine| machine.id.0 == "PUMP")
    );
}

#[test]
fn missing_required_column_reports_workbook_sheet_and_header_row() {
    let result = import_excel_files(&[fixture_path("invalid-missing-machine-name.xlsx")])
        .expect("invalid workbook should return diagnostics");

    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "IMPORT_SCHEMA_ERROR")
        .expect("missing column should be reported");
    let source = diagnostic
        .source
        .as_ref()
        .expect("schema error should contain a source location");

    assert_eq!(source.file_name, "invalid-missing-machine-name.xlsx");
    assert_eq!(source.sheet, "Machines");
    assert_eq!(source.row, 1);
}
