mod commands;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::import_excel_files,
            commands::validate_project,
            commands::get_project,
            commands::open_project,
            commands::save_project,
            commands::update_graph,
            commands::update_scenario,
            commands::update_review_settings,
            commands::get_matrix_metadata,
            commands::get_matrix_window,
            commands::get_cell_trace,
            commands::get_review_page,
            commands::get_review_window,
            commands::save_review_decision,
            commands::export_reviews_csv,
            commands::export_matrix_csv,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run STATEFLOW desktop application");
}
