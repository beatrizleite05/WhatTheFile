mod config;
mod db;
mod errors;
mod extractor;
mod chunker;
mod indexer;
mod llm;
mod platform;
#[path = "search.rs"]
mod search_engine;

use std::path::PathBuf;
use crate::config::RootPayload;

pub struct AppState {
    pub db_path: PathBuf,
    pub ollama_url: String,
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            use tauri::Manager;
            let app_data = app.path().app_data_dir()?;
            std::fs::create_dir_all(app_data.join("db"))?;
            let db_path = app_data.join("db").join("index.sqlite");
            db::open_and_migrate(&db_path)?;
            let ollama_url = llm::runtime::OLLAMA_BASE_URL.to_string();
            app.manage(AppState {
                db_path,
                ollama_url: ollama_url.clone(),
            });
            std::thread::spawn(move || {
                let _ = llm::runtime::release_stale_models(&ollama_url);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search,
            start_indexing,
            add_root,
            open_file,
            get_runtime_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn add_root(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<RootPayload, String> {
    let db_path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db::open_and_migrate(&db_path).map_err(|e| e.to_string())?;
        config::add_root(&conn, &path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn start_indexing(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    root_id: i64,
) -> Result<i64, String> {
    let db_path = state.db_path.clone();
    let ollama_url = state.ollama_url.clone();
    tauri::async_runtime::spawn_blocking(move || {
        indexer::run(&app, &db_path, root_id, &ollama_url).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn search(
    state: tauri::State<'_, AppState>,
    query: search_engine::SearchQuery,
) -> Result<search_engine::SearchResponse, String> {
    let db_path = state.db_path.clone();
    let ollama_url = state.ollama_url.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db::open_and_migrate(&db_path).map_err(|e| e.to_string())?;
        search_engine::search_files(&conn, &query, &ollama_url).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn open_file(_path: String) -> Result<(), String> {
    todo!("Phase E: implement open_file command")
}

#[tauri::command]
async fn get_runtime_status(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let ollama_url = state.ollama_url.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let ollama_reachable = llm::runtime::check_ollama(&ollama_url).unwrap_or(false);
        let models_loaded = if ollama_reachable {
            llm::runtime::list_loaded_models(&ollama_url).unwrap_or_default()
        } else {
            vec![]
        };
        Ok::<serde_json::Value, String>(serde_json::json!({
            "ollamaReachable": ollama_reachable,
            "modelsLoaded": models_loaded,
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}
