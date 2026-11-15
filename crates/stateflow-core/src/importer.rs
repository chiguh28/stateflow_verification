use crate::{
    Diagnostic, DiagnosticSeverity, EventDef, EventId, GraphNode, ImportSource, InputDef, InputId,
    Machine, MachineId, OutputDef, OutputId, Project, ReviewDecision, ReviewRuleId, ReviewSettings,
    Scenario, SourceLocation, State, StateId, TransitionRule, TypedValue, ValueDomain, ValueType,
};
use calamine::{Data, DataType, Reader, open_workbook_auto};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

const REQUIRED_SHEETS: [&str; 6] = [
    "Machines",
    "States",
    "Inputs",
    "Outputs",
    "Events",
    "Transitions",
];
const GRAPH_NODE_SPACING: f64 = 340.0;
const MAX_IMPORT_BYTES: u64 = 100 * 1024 * 1024;
const MAX_IMPORT_ROWS: usize = 100_000;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ImportResult {
    pub project: Project,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug)]
struct SheetRow {
    file_name: String,
    sheet_name: String,
    excel_row: u32,
    headers: HashMap<String, usize>,
    values: Vec<String>,
}

impl SheetRow {
    fn value(&self, name: &str) -> Option<&str> {
        let index = *self.headers.get(name)?;
        let value = self.values.get(index)?.trim();
        (!value.is_empty()).then_some(value)
    }

    fn source(&self, field: &str) -> SourceLocation {
        let column = self.headers.get(field).copied().map(column_label);
        SourceLocation {
            file_name: self.file_name.clone(),
            sheet: self.sheet_name.clone(),
            row: self.excel_row,
            column,
        }
    }
}

pub fn import_excel_files(paths: &[PathBuf]) -> Result<ImportResult, String> {
    if paths.is_empty() {
        return Err("Excelファイルを1件以上選択してください。".to_owned());
    }
    let mut diagnostics = Vec::new();
    let mut rows_by_sheet: HashMap<String, Vec<SheetRow>> = HashMap::new();
    let mut imports = Vec::new();
    let mut project_name = None;

    for path in paths {
        let metadata = std::fs::metadata(path)
            .map_err(|error| format!("{} を読み取れません: {error}", path.display()))?;
        if metadata.len() > MAX_IMPORT_BYTES {
            return Err(format!(
                "{} は100MBの取込上限を超えています。",
                path.display()
            ));
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("workbook.xlsx")
            .to_owned();
        project_name.get_or_insert_with(|| {
            path.file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("STATEFLOW Project")
                .to_owned()
        });
        let hash = hash_file(path)?;
        let mut workbook = open_workbook_auto(path)
            .map_err(|error| format!("{file_name} をExcelブックとして開けません: {error}"))?;
        let sheet_names = workbook.sheet_names();
        for sheet_name in sheet_names {
            let normalized_sheet = sheet_name.trim().to_lowercase();
            let expected_sheet = REQUIRED_SHEETS
                .iter()
                .find(|name| name.to_lowercase() == normalized_sheet);
            let Some(expected_sheet) = expected_sheet else {
                diagnostics.push(Diagnostic {
                    code: "UNKNOWN_SHEET".to_owned(),
                    severity: DiagnosticSeverity::Warning,
                    message: format!(
                        "{file_name} のシート `{sheet_name}` は取込対象ではないため無視しました。"
                    ),
                    detail: None,
                    source: Some(SourceLocation {
                        file_name: file_name.clone(),
                        sheet: sheet_name,
                        row: 1,
                        column: None,
                    }),
                });
                continue;
            };
            let range = workbook
                .worksheet_range(&sheet_name)
                .map_err(|error| format!("{file_name} / {sheet_name} を読み取れません: {error}"))?;
            if range.height() == 0 || range.width() == 0 {
                diagnostics.push(import_error(
                    &file_name,
                    expected_sheet,
                    "シートが空です。",
                    1,
                ));
                continue;
            }
            let start_row = range.start().map(|(row, _)| row).unwrap_or(0);
            let mut row_iter = range.rows();
            let Some(header_cells) = row_iter.next() else {
                diagnostics.push(import_error(
                    &file_name,
                    expected_sheet,
                    "列見出しがありません。",
                    1,
                ));
                continue;
            };
            let mut headers = HashMap::new();
            for (column, cell) in header_cells.iter().enumerate() {
                let header = cell_text(cell).trim().to_lowercase();
                if !header.is_empty() && headers.insert(header.clone(), column).is_some() {
                    diagnostics.push(import_error(
                        &file_name,
                        expected_sheet,
                        &format!("列名 `{header}` が重複しています。"),
                        start_row + 1,
                    ));
                }
            }
            for required in required_columns(expected_sheet) {
                if !headers.contains_key(*required) {
                    diagnostics.push(import_error(
                        &file_name,
                        expected_sheet,
                        &format!("必須列 `{required}` がありません。"),
                        start_row + 1,
                    ));
                }
            }
            let table = rows_by_sheet
                .entry((*expected_sheet).to_owned())
                .or_default();
            for (offset, cells) in row_iter.enumerate() {
                if cells.iter().all(DataType::is_empty) {
                    continue;
                }
                if table.len() >= MAX_IMPORT_ROWS {
                    return Err(format!(
                        "{expected_sheet} は行数上限{MAX_IMPORT_ROWS}件を超えています。"
                    ));
                }
                table.push(SheetRow {
                    file_name: file_name.clone(),
                    sheet_name: sheet_name.clone(),
                    excel_row: start_row + u32::try_from(offset).unwrap_or(u32::MAX) + 2,
                    headers: headers.clone(),
                    values: cells.iter().map(cell_text).collect(),
                });
            }
        }
        imports.push(ImportSource {
            file_name,
            sha256: hash,
        });
    }

    for sheet in REQUIRED_SHEETS {
        if !rows_by_sheet.contains_key(sheet) {
            diagnostics.push(import_error("", sheet, "必須シートがありません。", 1));
        }
    }

    let mut machines = Vec::new();
    let mut machine_orders = HashMap::new();
    for row in rows_by_sheet.get("Machines").into_iter().flatten() {
        let Some(id) = required_value(row, "machine_id", &mut diagnostics) else {
            continue;
        };
        let Some(name) = required_value(row, "machine_name", &mut diagnostics) else {
            continue;
        };
        let Some(initial_state) = required_value(row, "initial_state_id", &mut diagnostics) else {
            continue;
        };
        let step_order = row
            .value("step_order")
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(machines.len() as f64 + 1.0);
        machine_orders.insert(id.to_owned(), step_order);
        machines.push(Machine {
            id: MachineId(id.to_owned()),
            name: name.to_owned(),
            initial_state: StateId(initial_state.to_owned()),
            description: row.value("description").map(str::to_owned),
        });
    }

    let mut states = Vec::new();
    for row in rows_by_sheet.get("States").into_iter().flatten() {
        let Some(machine_id) = required_value(row, "machine_id", &mut diagnostics) else {
            continue;
        };
        let Some(id) = required_value(row, "state_id", &mut diagnostics) else {
            continue;
        };
        let Some(name) = required_value(row, "state_name", &mut diagnostics) else {
            continue;
        };
        states.push(State {
            machine_id: MachineId(machine_id.to_owned()),
            id: StateId(id.to_owned()),
            name: name.to_owned(),
        });
    }

    let mut inputs = Vec::new();
    for row in rows_by_sheet.get("Inputs").into_iter().flatten() {
        let Some(id) = required_value(row, "input_id", &mut diagnostics) else {
            continue;
        };
        let Some(name) = required_value(row, "input_name", &mut diagnostics) else {
            continue;
        };
        let Some(type_name) = required_value(row, "input_type", &mut diagnostics) else {
            continue;
        };
        let Some(allowed) = required_value(row, "allowed_values", &mut diagnostics) else {
            continue;
        };
        let Some(default) = required_value(row, "default_value", &mut diagnostics) else {
            continue;
        };
        let Ok((value_type, domain)) = parse_type_and_domain(type_name, allowed) else {
            diagnostics.push(import_error_at(
                row,
                "input_type",
                &format!("型またはallowed_valuesが不正です: {type_name} / {allowed}"),
            ));
            continue;
        };
        let default_value = parse_value(default, &value_type, &domain).unwrap_or_else(|message| {
            diagnostics.push(import_error_at(row, "default_value", &message));
            TypedValue::Unknown
        });
        inputs.push(InputDef {
            id: InputId(id.to_owned()),
            name: name.to_owned(),
            value_type,
            domain,
            default_value,
            unit: row.value("unit").map(str::to_owned),
        });
    }

    let mut outputs = Vec::new();
    for row in rows_by_sheet.get("Outputs").into_iter().flatten() {
        let Some(id) = required_value(row, "output_id", &mut diagnostics) else {
            continue;
        };
        let Some(owner) = required_value(row, "owner_machine_id", &mut diagnostics) else {
            continue;
        };
        let Some(name) = required_value(row, "output_name", &mut diagnostics) else {
            continue;
        };
        let Some(type_name) = required_value(row, "output_type", &mut diagnostics) else {
            continue;
        };
        let Some(allowed) = required_value(row, "allowed_values", &mut diagnostics) else {
            continue;
        };
        let Ok((value_type, domain)) = parse_type_and_domain(type_name, allowed) else {
            diagnostics.push(import_error_at(
                row,
                "output_type",
                &format!("型またはallowed_valuesが不正です: {type_name} / {allowed}"),
            ));
            continue;
        };
        let default_previous_value = row.value("default_previous_value").and_then(|value| {
            match parse_value(value, &value_type, &domain) {
                Ok(value) => Some(value),
                Err(message) => {
                    diagnostics.push(import_error_at(row, "default_previous_value", &message));
                    None
                }
            }
        });
        outputs.push(OutputDef {
            id: OutputId(id.to_owned()),
            owner_machine_id: MachineId(owner.to_owned()),
            name: name.to_owned(),
            value_type,
            domain,
            default_previous_value,
        });
    }

    let input_by_id: HashMap<_, _> = inputs
        .iter()
        .map(|input| (input.id.clone(), input))
        .collect();
    let mut events = Vec::new();
    for row in rows_by_sheet.get("Events").into_iter().flatten() {
        let Some(id) = required_value(row, "event_id", &mut diagnostics) else {
            continue;
        };
        let Some(name) = required_value(row, "event_name", &mut diagnostics) else {
            continue;
        };
        let Some(input_id) = required_value(row, "input_id", &mut diagnostics) else {
            continue;
        };
        let Some(from) = required_value(row, "from_value", &mut diagnostics) else {
            continue;
        };
        let Some(to) = required_value(row, "to_value", &mut diagnostics) else {
            continue;
        };
        let Some(enabled_text) = required_value(row, "enabled", &mut diagnostics) else {
            continue;
        };
        let Some(input) = input_by_id.get(&InputId(input_id.to_owned())) else {
            diagnostics.push(import_error_at(
                row,
                "input_id",
                &format!("入力 `{input_id}` がInputsにありません。"),
            ));
            continue;
        };
        let from_value =
            parse_value(from, &input.value_type, &input.domain).unwrap_or_else(|message| {
                diagnostics.push(import_error_at(row, "from_value", &message));
                TypedValue::Unknown
            });
        let to_value =
            parse_value(to, &input.value_type, &input.domain).unwrap_or_else(|message| {
                diagnostics.push(import_error_at(row, "to_value", &message));
                TypedValue::Unknown
            });
        let enabled = match parse_boolean(enabled_text) {
            Ok(value) => value,
            Err(message) => {
                diagnostics.push(import_error_at(row, "enabled", &message));
                false
            }
        };
        events.push(EventDef {
            id: EventId(id.to_owned()),
            name: name.to_owned(),
            input_id: InputId(input_id.to_owned()),
            from_value,
            to_value,
            enabled,
            source: Some(row.source("event_id")),
        });
    }

    let mut transitions = Vec::new();
    for row in rows_by_sheet.get("Transitions").into_iter().flatten() {
        let Some(id) = required_value(row, "rule_id", &mut diagnostics) else {
            continue;
        };
        let Some(machine_id) = required_value(row, "machine_id", &mut diagnostics) else {
            continue;
        };
        let Some(from_state) = required_value(row, "from_state_id", &mut diagnostics) else {
            continue;
        };
        let Some(guard_source) = required_value(row, "guard", &mut diagnostics) else {
            continue;
        };
        let Some(to_state) = required_value(row, "to_state_id", &mut diagnostics) else {
            continue;
        };
        let Some(priority_value) = required_value(row, "priority", &mut diagnostics) else {
            continue;
        };
        let priority = match priority_value.parse::<f64>() {
            Ok(value)
                if value.is_finite()
                    && value >= 0.0
                    && value.fract() == 0.0
                    && value <= u32::MAX as f64 =>
            {
                value as u32
            }
            _ => {
                diagnostics.push(import_error_at(
                    row,
                    "priority",
                    "priorityは0以上の整数で指定してください。",
                ));
                continue;
            }
        };
        let guard = match crate::parse_guard(guard_source) {
            Ok(expression) => Some(expression),
            Err(error) => {
                diagnostics.push(import_error_at(
                    row,
                    "guard",
                    &format!("ガード式を解析できません: {error}"),
                ));
                None
            }
        };
        transitions.push(TransitionRule {
            id: crate::RuleId(id.to_owned()),
            machine_id: MachineId(machine_id.to_owned()),
            from_state: StateId(from_state.to_owned()),
            guard_source: guard_source.to_owned(),
            guard,
            to_state: StateId(to_state.to_owned()),
            priority,
            source_ref: row.value("source_ref").map(str::to_owned),
            source: Some(row.source("guard")),
            description: row.value("description").map(str::to_owned),
        });
    }

    let mut ordered_machines = machines.clone();
    ordered_machines.sort_by(|left, right| {
        machine_orders
            .get(&left.id.0)
            .copied()
            .unwrap_or(f64::MAX)
            .total_cmp(&machine_orders.get(&right.id.0).copied().unwrap_or(f64::MAX))
            .then_with(|| left.id.cmp(&right.id))
    });
    let graph_nodes: Vec<GraphNode> = ordered_machines
        .iter()
        .enumerate()
        .map(|(index, machine)| GraphNode {
            machine_id: machine.id.clone(),
            x: index as f64 * GRAPH_NODE_SPACING,
            y: 160.0,
        })
        .collect();
    let scenario = Scenario {
        fixed_inputs: inputs
            .iter()
            .map(|input| (input.id.clone(), input.default_value.clone()))
            .collect(),
        previous_states: machines
            .iter()
            .map(|machine| (machine.id.clone(), TypedValue::Unknown))
            .collect(),
        previous_outputs: outputs
            .iter()
            .map(|output| {
                (
                    output.id.clone(),
                    output
                        .default_previous_value
                        .clone()
                        .unwrap_or(TypedValue::Unknown),
                )
            })
            .collect(),
    };
    let review_settings = ReviewSettings {
        enabled_rules: vec![
            ReviewRuleId::UpstreamChangedDownstreamHeld,
            ReviewRuleId::PreviousValueSuppressed,
            ReviewRuleId::OrderMissedUpdate,
        ],
        enabled_machines: ordered_machines
            .iter()
            .map(|machine| machine.id.clone())
            .collect(),
        enabled_events: events.iter().map(|event| event.id.clone()).collect(),
    };
    let project = Project {
        schema_version: 1,
        project_id: uuid::Uuid::new_v4().to_string(),
        name: project_name.unwrap_or_else(|| "STATEFLOW Project".to_owned()),
        revision: 1,
        machines,
        states,
        inputs,
        outputs,
        events,
        transitions,
        graph_nodes,
        graph_edges: Vec::new(),
        scenario,
        review_settings,
        review_decisions: Vec::<ReviewDecision>::new(),
        imports,
    };
    diagnostics.extend(crate::validate_project(&project));
    Ok(ImportResult {
        project,
        diagnostics,
    })
}

fn required_columns(sheet: &str) -> &'static [&'static str] {
    match sheet {
        "Machines" => &["machine_id", "machine_name", "initial_state_id"],
        "States" => &["machine_id", "state_id", "state_name"],
        "Inputs" => &[
            "input_id",
            "input_name",
            "input_type",
            "allowed_values",
            "default_value",
        ],
        "Outputs" => &[
            "output_id",
            "owner_machine_id",
            "output_name",
            "output_type",
            "allowed_values",
        ],
        "Events" => &[
            "event_id",
            "event_name",
            "input_id",
            "from_value",
            "to_value",
            "enabled",
        ],
        "Transitions" => &[
            "rule_id",
            "machine_id",
            "from_state_id",
            "guard",
            "to_state_id",
            "priority",
        ],
        _ => &[],
    }
}

fn required_value<'a>(
    row: &'a SheetRow,
    name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<&'a str> {
    match row.value(name) {
        Some(value) => Some(value),
        None => {
            diagnostics.push(import_error_at(
                row,
                name,
                &format!("必須値 `{name}` が空です。"),
            ));
            None
        }
    }
}

fn parse_type_and_domain(
    type_name: &str,
    allowed_values: &str,
) -> Result<(ValueType, ValueDomain), String> {
    match type_name.trim().to_ascii_uppercase().as_str() {
        "BOOL" | "BOOLEAN" => {
            let values: Vec<&str> = allowed_values.split('|').map(str::trim).collect();
            let normalized: Vec<String> = values
                .iter()
                .map(|value| value.to_ascii_uppercase())
                .collect();
            if normalized.len() != 2
                || !normalized.contains(&"OFF".to_owned())
                || !normalized.contains(&"ON".to_owned())
            {
                return Err("BOOLのallowed_valuesはOFF|ONまたはON|OFFにしてください。".to_owned());
            }
            Ok((ValueType::Boolean, ValueDomain::Boolean))
        }
        "ENUM" => {
            let values: Vec<String> = allowed_values
                .split('|')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect();
            if values.is_empty()
                || values
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    != values.len()
            {
                return Err(
                    "ENUMのallowed_valuesは空でない一意な値を`|`区切りで指定してください。"
                        .to_owned(),
                );
            }
            Ok((ValueType::Enum, ValueDomain::Enum(values)))
        }
        "NUMBER" => {
            let (minimum, maximum) =
                if let Some((minimum, maximum)) = allowed_values.split_once("..") {
                    let min = if minimum.trim().is_empty() {
                        None
                    } else {
                        Some(
                            minimum
                                .trim()
                                .parse::<f64>()
                                .map_err(|_| "NUMBER最小値が数値ではありません。".to_owned())?,
                        )
                    };
                    let max = if maximum.trim().is_empty() {
                        None
                    } else {
                        Some(
                            maximum
                                .trim()
                                .parse::<f64>()
                                .map_err(|_| "NUMBER最大値が数値ではありません。".to_owned())?,
                        )
                    };
                    if min.zip(max).is_some_and(|(min, max)| min > max) {
                        return Err("NUMBER範囲の最小値が最大値を超えています。".to_owned());
                    }
                    (min, max)
                } else {
                    let value = allowed_values
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| "NUMBER範囲は`最小..最大`で指定してください。".to_owned())?;
                    (Some(value), Some(value))
                };
            Ok((ValueType::Number, ValueDomain::Number { minimum, maximum }))
        }
        "STRING" => Ok((ValueType::String, ValueDomain::String)),
        _ => Err(format!("未知の値型 `{type_name}` です。")),
    }
}

fn parse_value(
    value: &str,
    value_type: &ValueType,
    domain: &ValueDomain,
) -> Result<TypedValue, String> {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("UNKNOWN") {
        return Ok(TypedValue::Unknown);
    }
    let typed = match value_type {
        ValueType::Boolean => TypedValue::Boolean(parse_boolean(trimmed)?),
        ValueType::Enum => TypedValue::Enum(trimmed.to_owned()),
        ValueType::Number => TypedValue::Number(
            trimmed
                .parse::<f64>()
                .map_err(|_| format!("`{trimmed}` はNUMBERではありません。"))?,
        ),
        ValueType::String => TypedValue::String(trimmed.to_owned()),
    };
    if let (TypedValue::Enum(name), ValueDomain::Enum(values)) = (&typed, domain) {
        if !values.contains(name) {
            return Err(format!("`{name}` は許容値にありません。"));
        }
    }
    if let (TypedValue::Number(value), ValueDomain::Number { minimum, maximum }) = (&typed, domain)
    {
        if !value.is_finite()
            || minimum.is_some_and(|minimum| *value < minimum)
            || maximum.is_some_and(|maximum| *value > maximum)
        {
            return Err(format!("`{value}` はNUMBER許容範囲外です。"));
        }
    }
    Ok(typed)
}

fn parse_boolean(value: &str) -> Result<bool, String> {
    match value.trim().to_ascii_uppercase().as_str() {
        "ON" | "TRUE" | "1" => Ok(true),
        "OFF" | "FALSE" | "0" => Ok(false),
        _ => Err(format!(
            "`{value}` はON/OFFまたはTRUE/FALSEで指定してください。"
        )),
    }
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|error| format!("{} を開けません: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("{} を読み取れません: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::Int(value) => value.to_string(),
        Data::Float(value) => value.to_string(),
        Data::String(value) | Data::DateTimeIso(value) | Data::DurationIso(value) => value.clone(),
        Data::Bool(value) => value.to_string().to_ascii_uppercase(),
        Data::DateTime(value) => value.as_f64().to_string(),
        Data::Error(error) => format!("#ERROR:{error:?}"),
    }
}

fn column_label(index: usize) -> String {
    let mut number = index + 1;
    let mut letters = Vec::new();
    while number > 0 {
        let remainder = (number - 1) % 26;
        letters.push((b'A' + u8::try_from(remainder).unwrap_or(0)) as char);
        number = (number - 1) / 26;
    }
    letters.into_iter().rev().collect()
}

fn import_error(file_name: &str, sheet: &str, message: &str, row: u32) -> Diagnostic {
    Diagnostic {
        code: "IMPORT_SCHEMA_ERROR".to_owned(),
        severity: DiagnosticSeverity::Error,
        message: format!("{message} ({file_name} / {sheet}, {row}行)"),
        detail: None,
        source: Some(SourceLocation {
            file_name: file_name.to_owned(),
            sheet: sheet.to_owned(),
            row,
            column: None,
        }),
    }
}

fn import_error_at(row: &SheetRow, field: &str, message: &str) -> Diagnostic {
    Diagnostic {
        code: "IMPORT_VALUE_ERROR".to_owned(),
        severity: DiagnosticSeverity::Error,
        message: format!(
            "{message} ({} / {}, {}行 {})",
            row.file_name,
            row.sheet_name,
            row.excel_row,
            row.source(field).column.unwrap_or_default()
        ),
        detail: None,
        source: Some(row.source(field)),
    }
}
