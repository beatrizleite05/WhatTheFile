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

use serde_json::Value;

pub fn run() {
    tauri::Builder::default()
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
async fn search(query: String) -> Result<Value, String> {
    todo!("Phase D: implement search command")
}

#[tauri::command]
async fn start_indexing(root_path: String) -> Result<(), String> {
    todo!("Phase B: implement start_indexing command")
}

#[tauri::command]
async fn add_root(path: String) -> Result<(), String> {
    todo!("Phase B: implement add_root command")
}

#[tauri::command]
async fn open_file(path: String) -> Result<(), String> {
    todo!("Phase E: implement open_file command")
}

#[tauri::command]
async fn get_runtime_status() -> Result<Value, String> {
    todo!("Phase D: implement get_runtime_status command")
}
