use stateflow_core::{
    EventDef, EventId, GraphNode, InputDef, InputId, Machine, MachineId, Project, ReviewSettings,
    Scenario, State, StateId, TypedValue, ValueDomain, ValueType, get_matrix_metadata,
    get_matrix_window,
};
use std::collections::BTreeMap;
use std::time::Instant;

#[test]
fn ten_million_cell_model_is_read_through_a_bounded_matrix_window() {
    let started_at = Instant::now();
    let project = large_project();
    let metadata = get_matrix_metadata(&project).expect("metadata should be available");

    assert_eq!(metadata.machine_count, 8);
    assert_eq!(metadata.event_count, 20);
    assert_eq!(metadata.combination_count, "65536");

    let full_matrix = get_matrix_window(&project, 0, 20, 0, 65_536, project.revision);
    assert!(
        full_matrix.is_err(),
        "the full matrix must exceed the window cap"
    );

    let first_window = get_matrix_window(&project, 0, 1, 0, 2, project.revision)
        .expect("a small visible window should be available");
    assert_eq!(first_window.cells.len(), 16);
    assert_eq!(first_window.cells[0].combination_id, "COMBO-000000000001");
    assert_eq!(first_window.cells[8].combination_id, "COMBO-000000000002");
    println!(
        "8 machines × 4 states × 20 events = 10,485,760 theoretical cells; fetched 16 cells in {:?}",
        started_at.elapsed()
    );
}

fn large_project() -> Project {
    let machines: Vec<_> = (0..8)
        .map(|index| Machine {
            id: MachineId(format!("M{index}")),
            name: format!("Machine {index}"),
            initial_state: StateId("S0".to_owned()),
            description: None,
        })
        .collect();
    let machine_ids: Vec<_> = machines.iter().map(|machine| machine.id.clone()).collect();
    let states: Vec<_> = machines
        .iter()
        .flat_map(|machine| {
            (0..4).map(|index| State {
                machine_id: machine.id.clone(),
                id: StateId(format!("S{index}")),
                name: format!("State {index}"),
            })
        })
        .collect();
    let events: Vec<_> = (0..20)
        .map(|index| EventDef {
            id: EventId(format!("E{index}")),
            name: format!("Event {index}"),
            input_id: InputId("SW".to_owned()),
            from_value: TypedValue::Boolean(false),
            to_value: TypedValue::Boolean(true),
            enabled: true,
            source: None,
        })
        .collect();
    let graph_nodes: Vec<_> = machines
        .iter()
        .enumerate()
        .map(|(index, machine)| GraphNode {
            machine_id: machine.id.clone(),
            x: index as f64 * 340.0,
            y: 160.0,
        })
        .collect();

    Project {
        schema_version: 1,
        project_id: "lazy-scale-test".to_owned(),
        name: "Lazy Scale Test".to_owned(),
        revision: 1,
        machines,
        states,
        inputs: vec![InputDef {
            id: InputId("SW".to_owned()),
            name: "Switch".to_owned(),
            value_type: ValueType::Boolean,
            domain: ValueDomain::Boolean,
            default_value: TypedValue::Boolean(false),
            unit: None,
        }],
        outputs: Vec::new(),
        events,
        transitions: Vec::new(),
        graph_nodes,
        graph_edges: Vec::new(),
        scenario: Scenario {
            fixed_inputs: BTreeMap::from([(InputId("SW".to_owned()), TypedValue::Boolean(false))]),
            previous_states: machine_ids
                .iter()
                .map(|machine_id| (machine_id.clone(), TypedValue::Unknown))
                .collect(),
            previous_outputs: BTreeMap::new(),
        },
        review_settings: ReviewSettings {
            enabled_rules: Vec::new(),
            enabled_machines: machine_ids,
            enabled_events: (0..20).map(|index| EventId(format!("E{index}"))).collect(),
        },
        review_decisions: Vec::new(),
        imports: Vec::new(),
    }
}
