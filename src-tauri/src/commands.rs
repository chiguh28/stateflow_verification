use serde::Serialize;
use stateflow_core::{
    Diagnostic, DiagnosticSeverity, EventId, GraphEdge, GraphNode, MatrixMetadata, MatrixWindow,
    Project, ProjectFileError, ReviewCellKey, ReviewFilters, ReviewPage, ReviewSettings,
    ReviewStatus, Scenario, StepResult, get_matrix_metadata as core_get_matrix_metadata,
    get_matrix_window as core_get_matrix_window,
    get_review_export_candidates as core_get_review_export_candidates,
    get_review_page as core_get_review_page, get_review_window as core_get_review_window,
    import_excel_files as core_import_excel_files, load_project_file, save_project_file,
    save_review_decision as core_save_review_decision, validate_project as core_validate_project,
};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

const MAX_MATRIX_EXPORT_CELLS: u128 = 10_000_000;
const MAX_MATRIX_WINDOW_CELLS: usize = 50_000;

#[derive(Default)]
pub struct AppState {
    project: Mutex<Option<Project>>,
    saved_path: Mutex<Option<String>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
    pub source_location: Option<stateflow_core::SourceLocation>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProjectResponse {
    pub project: Project,
    pub diagnostics: Vec<Diagnostic>,
}

fn api_error(code: &str, message: impl Into<String>, detail: Option<String>) -> ApiError {
    ApiError {
        code: code.to_owned(),
        message: message.into(),
        detail,
        source_location: None,
    }
}

fn file_error(error: ProjectFileError) -> ApiError {
    ApiError {
        code: error.code,
        message: error.message,
        detail: error.detail,
        source_location: None,
    }
}

fn lock_error() -> ApiError {
    api_error("APP_STATE_LOCK", "アプリの状態を読み取れません。", None)
}

fn current_project(state: &AppState) -> Result<Project, ApiError> {
    state
        .project
        .lock()
        .map_err(|_| lock_error())?
        .clone()
        .ok_or_else(|| {
            api_error(
                "PROJECT_NOT_LOADED",
                "先にExcelまたはSTATEFLOWプロジェクトを開いてください。",
                None,
            )
        })
}

fn ensure_revision(project: &Project, requested: u64) -> Result<(), ApiError> {
    if project.revision != requested {
        return Err(api_error(
            "STALE_REVISION",
            "プロジェクトが更新されています。最新の状態を読み直してください。",
            Some(format!(
                "requested={requested}, current={}",
                project.revision
            )),
        ));
    }
    Ok(())
}

fn require_analyzable(project: &Project) -> Result<Vec<Diagnostic>, ApiError> {
    let diagnostics = core_validate_project(project);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .collect();
    if errors.is_empty() {
        Ok(diagnostics)
    } else {
        Err(api_error(
            "INVALID_MODEL",
            "Excelまたはグラフの検証エラーを修正してから解析してください。",
            Some(
                errors
                    .iter()
                    .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
        ))
    }
}

fn response(project: Project) -> ProjectResponse {
    let diagnostics = core_validate_project(&project);
    ProjectResponse {
        project,
        diagnostics,
    }
}

#[tauri::command]
pub fn import_excel_files(
    paths: Vec<String>,
    state: State<'_, AppState>,
) -> Result<stateflow_core::ImportResult, ApiError> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let mut result = core_import_excel_files(&paths).map_err(|error| {
        api_error(
            "IMPORT_ERROR",
            "Excelファイルを取り込めませんでした。",
            Some(error),
        )
    })?;
    result.project.revision = 1;
    *state.project.lock().map_err(|_| lock_error())? = Some(result.project.clone());
    *state.saved_path.lock().map_err(|_| lock_error())? = None;
    Ok(result)
}

#[tauri::command]
pub fn validate_project(state: State<'_, AppState>) -> Result<Vec<Diagnostic>, ApiError> {
    Ok(core_validate_project(&current_project(&state)?))
}

#[tauri::command]
pub fn get_project(state: State<'_, AppState>) -> Result<Project, ApiError> {
    current_project(&state)
}

#[tauri::command]
pub fn open_project(path: String, state: State<'_, AppState>) -> Result<ProjectResponse, ApiError> {
    let project = load_project_file(&PathBuf::from(&path)).map_err(file_error)?;
    *state.project.lock().map_err(|_| lock_error())? = Some(project.clone());
    *state.saved_path.lock().map_err(|_| lock_error())? = Some(path);
    Ok(response(project))
}

#[tauri::command]
pub fn save_project(path: String, state: State<'_, AppState>) -> Result<String, ApiError> {
    let project = current_project(&state)?;
    save_project_file(&PathBuf::from(&path), &project).map_err(file_error)?;
    *state.saved_path.lock().map_err(|_| lock_error())? = Some(path.clone());
    Ok(path)
}

#[tauri::command]
pub fn update_graph(
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
    base_revision: u64,
    state: State<'_, AppState>,
) -> Result<ProjectResponse, ApiError> {
    let mut projects = state.project.lock().map_err(|_| lock_error())?;
    let project = projects.as_mut().ok_or_else(|| {
        api_error(
            "PROJECT_NOT_LOADED",
            "先にプロジェクトを開いてください。",
            None,
        )
    })?;
    ensure_revision(project, base_revision)?;
    let mut candidate = project.clone();
    candidate.graph_nodes = nodes;
    candidate.graph_edges = edges;
    let diagnostics = core_validate_project(&candidate);
    let rejected_codes = [
        "GRAPH_NODE_MACHINE_MISSING",
        "GRAPH_NODE_DUPLICATE",
        "GRAPH_POSITION_INVALID",
        "GRAPH_ORDER_AMBIGUOUS",
        "GRAPH_NODE_MISSING",
        "GRAPH_EDGE_ID_DUPLICATE",
        "GRAPH_EDGE_MACHINE_MISSING",
        "GRAPH_SELF_EDGE",
        "GRAPH_DIRECTION_INVALID",
        "GRAPH_OUTPUT_BINDING_INVALID",
    ];
    let rejected: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| rejected_codes.contains(&diagnostic.code.as_str()))
        .collect();
    if !rejected.is_empty() {
        return Err(api_error(
            "GRAPH_UPDATE_INVALID",
            "接続またはノード配置を保存できません。",
            Some(
                rejected
                    .iter()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
        ));
    }
    candidate.revision = candidate.revision.saturating_add(1);
    *project = candidate.clone();
    Ok(response(candidate))
}

#[tauri::command]
pub fn update_scenario(
    scenario: Scenario,
    base_revision: u64,
    state: State<'_, AppState>,
) -> Result<ProjectResponse, ApiError> {
    let mut projects = state.project.lock().map_err(|_| lock_error())?;
    let project = projects.as_mut().ok_or_else(|| {
        api_error(
            "PROJECT_NOT_LOADED",
            "先にプロジェクトを開いてください。",
            None,
        )
    })?;
    ensure_revision(project, base_revision)?;
    project.scenario = scenario;
    project.revision = project.revision.saturating_add(1);
    Ok(response(project.clone()))
}

#[tauri::command]
pub fn update_review_settings(
    settings: ReviewSettings,
    base_revision: u64,
    state: State<'_, AppState>,
) -> Result<ProjectResponse, ApiError> {
    let mut projects = state.project.lock().map_err(|_| lock_error())?;
    let project = projects.as_mut().ok_or_else(|| {
        api_error(
            "PROJECT_NOT_LOADED",
            "先にプロジェクトを開いてください。",
            None,
        )
    })?;
    ensure_revision(project, base_revision)?;
    if settings
        .enabled_machines
        .iter()
        .any(|id| !project.machines.iter().any(|machine| machine.id == *id))
        || settings
            .enabled_events
            .iter()
            .any(|id| !project.events.iter().any(|event| event.id == *id))
    {
        return Err(api_error(
            "REVIEW_SETTINGS_INVALID",
            "レビュー設定に存在しない機能またはイベントがあります。",
            None,
        ));
    }
    project.review_settings = settings;
    project.revision = project.revision.saturating_add(1);
    Ok(response(project.clone()))
}

#[tauri::command]
pub fn get_matrix_metadata(state: State<'_, AppState>) -> Result<MatrixMetadata, ApiError> {
    let project = current_project(&state)?;
    require_analyzable(&project)?;
    core_get_matrix_metadata(&project)
        .map_err(|error| api_error("MATRIX_METADATA_ERROR", error, None))
}

#[tauri::command]
pub fn get_matrix_window(
    event_start: usize,
    event_count: usize,
    combination_start: String,
    combination_count: usize,
    revision: u64,
    state: State<'_, AppState>,
) -> Result<MatrixWindow, ApiError> {
    let project = current_project(&state)?;
    ensure_revision(&project, revision)?;
    require_analyzable(&project)?;
    let combination_start = combination_start.parse::<u128>().map_err(|_| {
        api_error(
            "MATRIX_RANGE_INVALID",
            "組み合わせ開始位置が不正です。",
            None,
        )
    })?;
    core_get_matrix_window(
        &project,
        event_start,
        event_count,
        combination_start,
        combination_count,
        revision,
    )
    .map_err(|error| api_error("MATRIX_WINDOW_ERROR", error, None))
}

#[tauri::command]
pub fn get_cell_trace(
    event_id: String,
    combination_index: String,
    revision: u64,
    state: State<'_, AppState>,
) -> Result<StepResult, ApiError> {
    let project = current_project(&state)?;
    ensure_revision(&project, revision)?;
    require_analyzable(&project)?;
    let event = project
        .events
        .iter()
        .find(|event| event.id == EventId(event_id.clone()))
        .ok_or_else(|| api_error("EVENT_NOT_FOUND", "指定したイベントがありません。", None))?;
    let index = combination_index.parse::<u128>().map_err(|_| {
        api_error(
            "MATRIX_RANGE_INVALID",
            "組み合わせインデックスが不正です。",
            None,
        )
    })?;
    let order = stateflow_core::ordered_machines(&project);
    let states = stateflow_core::combination_at(&project, &order, index)
        .map_err(|error| api_error("MATRIX_RANGE_INVALID", error, None))?;
    stateflow_core::evaluate_step(&project, event, &states)
        .map_err(|error| api_error("EVALUATION_ERROR", error, None))
}

#[tauri::command]
pub fn get_review_page(
    filters: ReviewFilters,
    offset: usize,
    limit: usize,
    revision: u64,
    state: State<'_, AppState>,
) -> Result<ReviewPage, ApiError> {
    let project = current_project(&state)?;
    ensure_revision(&project, revision)?;
    core_get_review_page(&project, &filters, offset, limit, revision)
        .map_err(|error| api_error("REVIEW_QUERY_ERROR", error, None))
}

#[tauri::command]
pub fn get_review_window(
    event_ids: Vec<String>,
    combination_start: String,
    combination_count: usize,
    revision: u64,
    state: State<'_, AppState>,
) -> Result<Vec<ReviewCellKey>, ApiError> {
    let project = current_project(&state)?;
    ensure_revision(&project, revision)?;
    let combination_start = combination_start.parse::<u128>().map_err(|_| {
        api_error(
            "MATRIX_RANGE_INVALID",
            "組み合わせ開始位置が不正です。",
            None,
        )
    })?;
    core_get_review_window(
        &project,
        event_ids,
        combination_start,
        combination_count,
        revision,
    )
    .map_err(|error| api_error("REVIEW_WINDOW_ERROR", error, None))
}

#[tauri::command]
pub fn save_review_decision(
    fingerprint: String,
    evidence_hash: String,
    status: ReviewStatus,
    note: String,
    base_revision: u64,
    state: State<'_, AppState>,
) -> Result<ProjectResponse, ApiError> {
    let mut projects = state.project.lock().map_err(|_| lock_error())?;
    let project = projects.as_mut().ok_or_else(|| {
        api_error(
            "PROJECT_NOT_LOADED",
            "先にプロジェクトを開いてください。",
            None,
        )
    })?;
    ensure_revision(project, base_revision)?;
    core_save_review_decision(project, &fingerprint, &evidence_hash, status, &note)
        .map_err(|error| api_error("REVIEW_DECISION_INVALID", error, None))?;
    Ok(response(project.clone()))
}

#[tauri::command]
pub fn export_reviews_csv(
    path: String,
    filters: ReviewFilters,
    revision: u64,
    state: State<'_, AppState>,
) -> Result<usize, ApiError> {
    let project = current_project(&state)?;
    ensure_revision(&project, revision)?;
    let candidates = core_get_review_export_candidates(&project, &filters, revision)
        .map_err(|error| api_error("REVIEW_EXPORT_ERROR", error, None))?;
    let file = File::create(path).map_err(|error| {
        api_error(
            "CSV_WRITE_ERROR",
            "CSV保存先を作成できません。",
            Some(error.to_string()),
        )
    })?;
    let mut writer = BufWriter::new(file);
    let candidate_count = candidates.len();
    write_csv_record(
        &mut writer,
        &[
            "candidate_id",
            "event",
            "combination",
            "machine",
            "current_state",
            "next_state",
            "review_rule",
            "source_rule",
            "reason",
            "decision",
            "note",
        ],
    )?;
    for candidate in candidates {
        let (decision, note) = candidate
            .decision
            .map(|decision| (status_label(&decision.status).to_owned(), decision.note))
            .unwrap_or_default();
        write_csv_record(
            &mut writer,
            &[
                candidate.fingerprint,
                candidate.event_name,
                candidate.combination_id,
                candidate.machine_id.0,
                state_label(&candidate.current_state),
                state_label(&candidate.next_state),
                candidate.rule_code,
                candidate.source_rule_id.unwrap_or_default(),
                candidate.reason,
                decision,
                note,
            ],
        )?;
    }
    writer.flush().map_err(|error| {
        api_error(
            "CSV_WRITE_ERROR",
            "レビューCSVを書き込めません。",
            Some(error.to_string()),
        )
    })?;
    Ok(candidate_count)
}

#[tauri::command]
pub fn export_matrix_csv(
    path: String,
    event_start: usize,
    event_count: usize,
    combination_start: String,
    combination_count: usize,
    revision: u64,
    state: State<'_, AppState>,
) -> Result<u128, ApiError> {
    let project = current_project(&state)?;
    ensure_revision(&project, revision)?;
    require_analyzable(&project)?;
    let combination_start = combination_start.parse::<u128>().map_err(|_| {
        api_error(
            "MATRIX_RANGE_INVALID",
            "組み合わせ開始位置が不正です。",
            None,
        )
    })?;
    let machine_count = project.machines.len();
    let export_cells = (event_count as u128)
        .saturating_mul(combination_count as u128)
        .saturating_mul(machine_count as u128);
    if export_cells > MAX_MATRIX_EXPORT_CELLS {
        return Err(api_error(
            "MATRIX_EXPORT_LIMIT",
            "1回のCSV出力は1,000万セルまでです。範囲を小さくしてください。",
            Some(format!("cells={export_cells}")),
        ));
    }
    let file = File::create(path).map_err(|error| {
        api_error(
            "CSV_WRITE_ERROR",
            "CSV保存先を作成できません。",
            Some(error.to_string()),
        )
    })?;
    let mut writer = BufWriter::new(file);
    write_csv_record(
        &mut writer,
        &[
            "event_id",
            "combination_id",
            "machine_id",
            "current_state",
            "next_state",
            "status",
        ],
    )?;
    let per_combination = event_count.saturating_mul(machine_count).max(1);
    let combination_chunk = (MAX_MATRIX_WINDOW_CELLS / per_combination).max(1);
    let mut offset = 0_usize;
    while offset < combination_count {
        let count = combination_chunk.min(combination_count - offset);
        let window = core_get_matrix_window(
            &project,
            event_start,
            event_count,
            combination_start + offset as u128,
            count,
            revision,
        )
        .map_err(|error| api_error("MATRIX_EXPORT_ERROR", error, None))?;
        for cell in window.cells {
            write_csv_record(
                &mut writer,
                &[
                    cell.event_id,
                    cell.combination_id,
                    cell.machine_id.0,
                    cell.current_state.0,
                    cell.next_state
                        .map(|state| state.0)
                        .unwrap_or_else(|| "UNKNOWN".to_owned()),
                    format!("{:?}", cell.status).to_ascii_uppercase(),
                ],
            )?;
        }
        offset += count;
    }
    writer.flush().map_err(|error| {
        api_error(
            "CSV_WRITE_ERROR",
            "マトリクスCSVを書き込めません。",
            Some(error.to_string()),
        )
    })?;
    Ok(export_cells)
}

fn write_csv_record(writer: &mut impl Write, fields: &[impl AsRef<str>]) -> Result<(), ApiError> {
    let line = fields
        .iter()
        .map(|field| csv_field(field.as_ref()))
        .collect::<Vec<_>>()
        .join(",");
    writeln!(writer, "{line}").map_err(|error| {
        api_error(
            "CSV_WRITE_ERROR",
            "CSVを書き込めません。",
            Some(error.to_string()),
        )
    })
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn state_label(state: &Option<stateflow_core::StateId>) -> String {
    state
        .as_ref()
        .map(|state| state.0.clone())
        .unwrap_or_else(|| "UNKNOWN".to_owned())
}

fn status_label(status: &ReviewStatus) -> &'static str {
    match status {
        ReviewStatus::Pending => "未判定",
        ReviewStatus::ExpectedByDesign => "仕様通り",
        ReviewStatus::NeedsCorrection => "要修正",
        ReviewStatus::Deferred => "保留",
    }
}
