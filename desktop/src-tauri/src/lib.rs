pub mod config;
pub mod db;
pub mod errors;
pub mod extractor;
pub mod chunker;
pub mod indexer;
pub mod llm;
pub mod platform;
#[path = "search.rs"]
pub mod search_engine;

use std::path::PathBuf;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use tauri::Emitter;
use crate::config::RootPayload;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletedJobPayload {
    pub job_id: i64,
    pub root_id: i64,
    pub files_total: i64,
    pub files_added: i64,
    pub files_updated: i64,
    pub files_moved: i64,
    pub files_deleted: i64,
    pub error_count: i64,
    pub completed_at: i64,
}

pub struct AppState {
    pub db_path: PathBuf,
    pub ollama_url: String,
    pub cancel_flag: Arc<AtomicBool>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            use tauri::Manager;

            let main_window = app
                .get_webview_window("main")
                .ok_or("main window not found")?;

            #[cfg(target_os = "macos")]
            {
                use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
                apply_vibrancy(&main_window, NSVisualEffectMaterial::HudWindow, None, Some(12.0))
                    .map_err(|e| e.to_string())?;
            }

            // Spotlight behaviour: hide the main window when it loses focus.
            let win = main_window.clone();
            main_window.on_window_event(move |event| {
                if let tauri::WindowEvent::Focused(false) = event {
                    let _ = win.hide();
                }
            });

            let app_data = app.path().app_data_dir()?;
            std::fs::create_dir_all(app_data.join("db"))?;
            let db_path = app_data.join("db").join("index.sqlite");
            db::open_and_migrate(&db_path)?;
            let ollama_url = llm::runtime::OLLAMA_BASE_URL.to_string();
            app.manage(AppState {
                db_path,
                ollama_url: ollama_url.clone(),
                cancel_flag: Arc::new(AtomicBool::new(false)),
            });
            std::thread::spawn(move || {
                let _ = llm::runtime::release_stale_models(&ollama_url);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search,
            get_recent_files,
            start_indexing,
            add_root,
            list_roots,
            remove_root,
            delete_index,
            open_file,
            get_runtime_status,
            parse_query,
            get_activity_log,
            cancel_indexing,
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
async fn list_roots(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<RootPayload>, String> {
    let db_path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db::open_and_migrate(&db_path).map_err(|e| e.to_string())?;
        config::list_roots(&conn).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn remove_root(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    let db_path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db::open_and_migrate(&db_path).map_err(|e| e.to_string())?;
        db::clear_root_index(&conn, id).map_err(|e| e.to_string())?;
        conn.execute("UPDATE roots SET active = 0 WHERE id = ?1", [id])
            .map_err(|e| e.to_string())?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| e.to_string())?;
    let _ = app.emit("index://changed", ());
    Ok(())
}

#[tauri::command]
async fn delete_index(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let db_path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db::open_and_migrate(&db_path).map_err(|e| e.to_string())?;
        db::clear_index(&conn).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?;
    let _ = app.emit("index://changed", ());
    Ok(())
}

#[tauri::command]
async fn start_indexing(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    root_id: i64,
) -> Result<i64, String> {
    let db_path = state.db_path.clone();
    let ollama_url = state.ollama_url.clone();
    let cancel_flag = state.cancel_flag.clone();
    cancel_flag.store(false, Ordering::Relaxed);
    tauri::async_runtime::spawn_blocking(move || {
        indexer::run(&app, &db_path, root_id, &ollama_url, &cancel_flag).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn cancel_indexing(state: tauri::State<'_, AppState>) -> Result<(), String> {
    log::info!("[cancel_indexing] setting cancel flag");
    state.cancel_flag.store(true, Ordering::Relaxed);
    log::info!("[cancel_indexing] flag is now: {}", state.cancel_flag.load(Ordering::Relaxed));
    Ok(())
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
async fn get_recent_files(
    state: tauri::State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<search_engine::FileSearchResult>, String> {
    let db_path = state.db_path.clone();
    let requested_limit = limit.unwrap_or(4);
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db::open_and_migrate(&db_path).map_err(|e| e.to_string())?;
        search_engine::recent_files(&conn, requested_limit).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn open_file(path: String) -> Result<(), String> {
    platform::open_file(&path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn parse_query(
    state: tauri::State<'_, AppState>,
    input: String,
) -> Result<llm::query_parser::ParsedQueryPayload, String> {
    let ollama_url = state.ollama_url.clone();
    tauri::async_runtime::spawn_blocking(move || {
        llm::query_parser::parse_query(&input, &ollama_url).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_activity_log(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<CompletedJobPayload>, String> {
    let db_path = state.db_path.clone();
    let limit = limit.unwrap_or(50);
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db::open_and_migrate(&db_path).map_err(|e| e.to_string())?;
        let jobs = db::list_completed_jobs(&conn, limit).map_err(|e| e.to_string())?;
        Ok(jobs.into_iter().map(|j| CompletedJobPayload {
            job_id: j.job_id,
            root_id: j.root_id,
            files_total: j.files_total,
            files_added: j.files_added,
            files_updated: j.files_updated,
            files_moved: j.files_moved,
            files_deleted: j.files_deleted,
            error_count: j.error_count,
            completed_at: j.completed_at,
        }).collect())
    })
    .await
    .map_err(|e| e.to_string())?
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
