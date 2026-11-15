//! Platform-independent Stateflow domain types and analysis contracts.

pub mod dsl;
pub mod engine;
pub mod importer;
pub mod model;
pub mod project_file;
pub mod review;
pub mod validation;

pub use dsl::{GuardParseError, parse_guard};
pub use engine::{
    CellStatus, ExpressionTrace, MachineEvaluation, MatrixCell, MatrixMetadata, MatrixWindow,
    RuleEvaluation, StepResult, TruthValue, combination_at, count_combinations, evaluate_step,
    get_matrix_metadata, get_matrix_window, ordered_machines,
};
pub use importer::{ImportResult, import_excel_files};
pub use model::*;
pub use project_file::{ProjectFileError, load_project_file, save_project_file};
pub use review::{
    ReviewCandidate, ReviewCellKey, ReviewFilters, ReviewPage, get_review_export_candidates,
    get_review_page, get_review_window, save_review_decision,
};
pub use validation::validate_project;
