use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Revision = u64;

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct MachineId(pub String);

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct StateId(pub String);

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct InputId(pub String);

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct OutputId(pub String);

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct EventId(pub String);

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuleId(pub String);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Project {
    pub schema_version: u32,
    pub project_id: String,
    pub name: String,
    pub revision: Revision,
    pub machines: Vec<Machine>,
    pub states: Vec<State>,
    pub inputs: Vec<InputDef>,
    pub outputs: Vec<OutputDef>,
    pub events: Vec<EventDef>,
    pub transitions: Vec<TransitionRule>,
    pub graph_nodes: Vec<GraphNode>,
    pub graph_edges: Vec<GraphEdge>,
    pub scenario: Scenario,
    pub review_settings: ReviewSettings,
    pub review_decisions: Vec<ReviewDecision>,
    pub imports: Vec<ImportSource>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Machine {
    pub id: MachineId,
    pub name: String,
    pub initial_state: StateId,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct State {
    pub machine_id: MachineId,
    pub id: StateId,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct InputDef {
    pub id: InputId,
    pub name: String,
    pub value_type: ValueType,
    pub domain: ValueDomain,
    pub default_value: TypedValue,
    pub unit: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OutputDef {
    pub id: OutputId,
    pub owner_machine_id: MachineId,
    pub name: String,
    pub value_type: ValueType,
    pub domain: ValueDomain,
    pub default_previous_value: Option<TypedValue>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EventDef {
    pub id: EventId,
    pub name: String,
    pub input_id: InputId,
    pub from_value: TypedValue,
    pub to_value: TypedValue,
    pub enabled: bool,
    pub source: Option<SourceLocation>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TransitionRule {
    pub id: RuleId,
    pub machine_id: MachineId,
    pub from_state: StateId,
    pub guard_source: String,
    pub guard: Option<GuardExpr>,
    pub to_state: StateId,
    pub priority: u32,
    pub source_ref: Option<String>,
    pub source: Option<SourceLocation>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceLocation {
    pub file_name: String,
    pub sheet: String,
    pub row: u32,
    pub column: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub detail: Option<String>,
    pub source: Option<SourceLocation>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueType {
    Boolean,
    Enum,
    Number,
    String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "values", rename_all = "snake_case")]
pub enum ValueDomain {
    Boolean,
    Enum(Vec<String>),
    Number {
        minimum: Option<f64>,
        maximum: Option<f64>,
    },
    String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum TypedValue {
    Boolean(bool),
    Enum(String),
    Number(f64),
    String(String),
    Unknown,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum GuardExpr {
    Literal(TypedValue),
    Reference(ValueReference),
    Rise(InputId),
    Fall(InputId),
    Compare {
        operator: CompareOperator,
        left: Box<GuardExpr>,
        right: Box<GuardExpr>,
    },
    Not(Box<GuardExpr>),
    And(Vec<GuardExpr>),
    Or(Vec<GuardExpr>),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "scope", content = "id", rename_all = "snake_case")]
pub enum ValueReference {
    Input(InputId),
    State(MachineId),
    BeforeState(MachineId),
    PreviousState(MachineId),
    PreviousOutput(OutputId),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareOperator {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GraphNode {
    pub machine_id: MachineId,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GraphEdge {
    pub edge_id: String,
    pub source_machine: MachineId,
    pub target_machine: MachineId,
    pub kind: EdgeKind,
    pub output_id: Option<OutputId>,
    pub target_reference: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    CurrentState,
    PreviousState,
    PreviousOutput,
    FlowOnly,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Scenario {
    pub fixed_inputs: BTreeMap<InputId, TypedValue>,
    pub previous_states: BTreeMap<MachineId, TypedValue>,
    pub previous_outputs: BTreeMap<OutputId, TypedValue>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReviewSettings {
    pub enabled_rules: Vec<ReviewRuleId>,
    pub enabled_machines: Vec<MachineId>,
    pub enabled_events: Vec<EventId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewRuleId {
    UpstreamChangedDownstreamHeld,
    PreviousValueSuppressed,
    OrderMissedUpdate,
    ImplicitHold,
    AllMachinesHeld,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReviewDecision {
    pub fingerprint: String,
    #[serde(default)]
    pub evidence_hash: String,
    pub status: ReviewStatus,
    pub note: String,
    pub updated_at: String,
    pub needs_recheck: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Pending,
    ExpectedByDesign,
    NeedsCorrection,
    Deferred,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ImportSource {
    pub file_name: String,
    pub sha256: String,
}

#[cfg(test)]
mod tests {
    use super::{EdgeKind, ValueReference, ValueType};
    use crate::MachineId;

    #[test]
    fn value_types_serialize_as_simple_names() {
        assert_eq!(
            serde_json::to_string(&ValueType::Boolean).unwrap(),
            "\"boolean\""
        );
        assert_eq!(serde_json::to_string(&ValueType::Enum).unwrap(), "\"enum\"");
    }

    #[test]
    fn before_state_and_previous_state_serialize_as_distinct_references() {
        let machine_id = MachineId("AC".to_owned());
        let before_state = serde_json::to_value(ValueReference::BeforeState(machine_id.clone()))
            .expect("BEFORE.STATE should serialize");
        let previous_state = serde_json::to_value(ValueReference::PreviousState(machine_id))
            .expect("PREV.STATE should serialize");

        assert_ne!(before_state, previous_state);
        assert_eq!(before_state["scope"], "before_state");
        assert_eq!(previous_state["scope"], "previous_state");
    }

    #[test]
    fn graph_edge_kinds_have_explicit_serialized_names() {
        assert_eq!(
            serde_json::to_string(&EdgeKind::CurrentState).unwrap(),
            "\"current_state\""
        );
        assert_eq!(
            serde_json::to_string(&EdgeKind::PreviousState).unwrap(),
            "\"previous_state\""
        );
        assert_eq!(
            serde_json::to_string(&EdgeKind::PreviousOutput).unwrap(),
            "\"previous_output\""
        );
        assert_eq!(
            serde_json::to_string(&EdgeKind::FlowOnly).unwrap(),
            "\"flow_only\""
        );
    }
}
