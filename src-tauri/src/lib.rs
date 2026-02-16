mod agents;
mod commands;
mod config;
mod db;
mod debate;
mod decisions;
mod llm;
mod profile;

use commands::AppState;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().expect("Failed to get app data dir");
            std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data dir");

            let db_path = app_data_dir.join("database.sqlite");
            let database = db::Database::new(db_path.to_str().unwrap())
                .expect("Failed to initialize database");

            app.manage(Mutex::new(AppState {
                db: database,
                app_data_dir,
                debate_cancel_flags: std::collections::HashMap::new(),
            }));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::send_message,
            commands::get_conversations,
            commands::get_messages,
            commands::get_settings,
            commands::save_settings,
            commands::get_profile_files,
            commands::open_profile_folder,
            commands::delete_conversation,
            commands::create_decision,
            commands::get_decisions,
            commands::get_decision,
            commands::get_decision_by_conversation,
            commands::update_decision_status,
            commands::get_profile_files_detailed,
            commands::update_profile_file,
            commands::remove_profile_file,
            commands::get_agent_files,
            commands::update_agent_file,
            commands::open_agents_folder,
            commands::start_debate,
            commands::get_debate,
            commands::cancel_debate,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
