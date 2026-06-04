use std::fs;
use std::time::UNIX_EPOCH;

#[tauri::command]
fn read_todo(path: String) -> Result<String, String> {
    if !std::path::Path::new(&path).exists() {
        let default_content = "- [ ] Welcome to your desktop to-do widget!\n- [ ] Double-click to edit this task.\n  - [ ] Use Tab to indent.\n  - [ ] Use Shift+Tab to outdent.\n";
        fs::write(&path, default_content).map_err(|e| e.to_string())?;
    }
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn write_todo(path: String, content: String) -> Result<(), String> {
    fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_default_path() -> String {
    if cfg!(debug_assertions) {
        if let Ok(cwd) = std::env::current_dir() {
            return cwd.join("todo.md").to_string_lossy().to_string();
        }
    } else if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            return parent.join("todo.md").to_string_lossy().to_string();
        }
    }
    "todo.md".to_string()
}

#[tauri::command]
fn get_file_modified_time(path: String) -> Result<u64, String> {
    fs::metadata(&path)
        .and_then(|m| m.modified())
        .map(|time| time.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            read_todo,
            write_todo,
            get_default_path,
            get_file_modified_time
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
