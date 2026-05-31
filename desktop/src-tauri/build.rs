use std::env;
use std::fs;
use std::path::Path;

fn main() {
    generate_media_types();

    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new().commands(&[
                "search",
                "get_recent_files",
                "start_indexing",
                "add_root",
                "list_roots",
                "remove_root",
                "delete_index",
                "open_file",
                "get_runtime_status",
                "parse_query",
            ]),
        ),
    )
    .unwrap();
}

fn generate_media_types() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let json_path = Path::new(&manifest_dir).join("../src/core/mediaTypes.json");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../src/core/mediaTypes.json");

    let json = fs::read_to_string(&json_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", json_path.display()));

    let types: Vec<String> = serde_json::from_str(&json).unwrap_or_else(|e| {
        panic!("failed to parse {} as JSON string array: {e}", json_path.display())
    });

    let entries: Vec<String> = types.iter().map(|t| format!("\"{t}\"")).collect();
    let rust = format!(
        "pub const MEDIA_TYPES: &[&str] = &[{}];\n",
        entries.join(", "),
    );

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = Path::new(&out_dir).join("media_types.rs");
    fs::write(&out_path, rust)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", out_path.display()));
}
