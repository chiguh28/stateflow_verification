use stateflow_core::import_excel_files;
use std::path::PathBuf;

fn fixture_path(file_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(file_name)
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
