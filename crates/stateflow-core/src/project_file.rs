use crate::Project;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Write};
use std::path::Path;

const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProjectFileError {
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
}

pub fn save_project_file(path: &Path, project: &Project) -> Result<(), ProjectFileError> {
    if project.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(ProjectFileError {
            code: "UNSUPPORTED_SCHEMA_VERSION".to_owned(),
            message: format!(
                "プロジェクト形式 v{} は保存できません。対応形式はv{}です。",
                project.schema_version, CURRENT_SCHEMA_VERSION
            ),
            detail: None,
        });
    }

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| {
        io_error(
            "PROJECT_DIRECTORY_ERROR",
            "保存先フォルダーを作成できません。",
            error.to_string(),
        )
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project.stateflow.json");
    let temporary_path = parent.join(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(project).map_err(|error| {
        io_error(
            "PROJECT_SERIALIZE_ERROR",
            "プロジェクトJSONを作成できません。",
            error.to_string(),
        )
    })?;

    let write_result = (|| {
        let mut temporary_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)?;
        temporary_file.write_all(&bytes)?;
        temporary_file.flush()?;
        temporary_file.sync_all()?;
        fs::rename(&temporary_path, path)?;
        Ok::<(), std::io::Error>(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary_path);
        return Err(io_error(
            "PROJECT_WRITE_ERROR",
            "プロジェクトを安全に保存できませんでした。",
            error.to_string(),
        ));
    }
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
    Ok(())
}

pub fn load_project_file(path: &Path) -> Result<Project, ProjectFileError> {
    let file = File::open(path).map_err(|error| {
        io_error(
            "PROJECT_OPEN_ERROR",
            "プロジェクトを開けません。",
            error.to_string(),
        )
    })?;
    let project: Project = serde_json::from_reader(BufReader::new(file)).map_err(|error| {
        io_error(
            "PROJECT_JSON_INVALID",
            "プロジェクトJSONが不正です。",
            error.to_string(),
        )
    })?;
    if project.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(ProjectFileError {
            code: "UNSUPPORTED_SCHEMA_VERSION".to_owned(),
            message: format!(
                "プロジェクト形式 v{} は現在のアプリで開けません。",
                project.schema_version
            ),
            detail: None,
        });
    }
    Ok(project)
}

fn io_error(code: &str, message: &str, detail: String) -> ProjectFileError {
    ProjectFileError {
        code: code.to_owned(),
        message: message.to_owned(),
        detail: Some(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::{load_project_file, save_project_file};
    use crate::{
        ReviewFilters, ReviewStatus, get_review_page, import_excel_files, save_review_decision,
    };
    use std::path::PathBuf;

    #[test]
    fn saved_project_retains_graph_and_scenario() {
        let workbook = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("golden-workbook.xlsx");
        let mut project = import_excel_files(&[workbook])
            .expect("workbook should load")
            .project;
        project.graph_edges.push(crate::GraphEdge {
            edge_id: "current-ac-link".to_owned(),
            source_machine: crate::MachineId("AC".to_owned()),
            target_machine: crate::MachineId("LINK".to_owned()),
            kind: crate::EdgeKind::CurrentState,
            output_id: None,
            target_reference: None,
        });
        project.graph_edges.push(crate::GraphEdge {
            edge_id: "current-link-heater".to_owned(),
            source_machine: crate::MachineId("LINK".to_owned()),
            target_machine: crate::MachineId("HEATER".to_owned()),
            kind: crate::EdgeKind::CurrentState,
            output_id: None,
            target_reference: None,
        });
        let candidate =
            get_review_page(&project, &ReviewFilters::default(), 0, 1, project.revision)
                .expect("review candidates should load")
                .candidates
                .into_iter()
                .next()
                .expect("golden project should produce a review candidate");
        save_review_decision(
            &mut project,
            &candidate.fingerprint,
            &candidate.evidence_hash,
            ReviewStatus::ExpectedByDesign,
            "仕様通り",
        )
        .expect("review decision should save");
        let directory = std::env::temp_dir().join(format!("stateflow-{}", uuid::Uuid::new_v4()));
        let path = directory.join("golden.stateflow.json");
        save_project_file(&path, &project).expect("project should save");
        let loaded = load_project_file(&path).expect("saved project should reopen");
        assert_eq!(loaded.graph_edges, project.graph_edges);
        assert_eq!(loaded.scenario, project.scenario);
        assert_eq!(loaded.review_settings, project.review_settings);
        assert_eq!(loaded.review_decisions, project.review_decisions);
        std::fs::remove_dir_all(directory).expect("test directory should be removed");
    }
}
