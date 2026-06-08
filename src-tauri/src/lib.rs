use std::fs;
use std::time::UNIX_EPOCH;
use tauri::Manager;

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

#[tauri::command]
fn set_always_on_top(window: tauri::Window, on_top: bool) -> Result<(), String> {
    window.set_always_on_top(on_top).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_always_on_bottom(window: tauri::Window, on_bottom: bool) -> Result<(), String> {
    window.set_always_on_bottom(on_bottom).map_err(|e| e.to_string())
}

#[tauri::command]
fn log_deleted_task(path: String, entry_json: String) -> Result<(), String> {
    use std::fs::OpenOptions;
    use std::io::Write;
    
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
        
    writeln!(file, "{}", entry_json).map_err(|e| e.to_string())
}

#[tauri::command]
fn pop_deleted_task(path: String) -> Result<String, String> {
    if !std::path::Path::new(&path).exists() {
        return Err("No history".to_string());
    }
    
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
    
    if lines.is_empty() {
        return Err("No history".to_string());
    }
    
    let last_line = lines.pop().unwrap().to_string();
    
    let new_content = if lines.is_empty() {
        "".to_string()
    } else {
        lines.join("\n") + "\n"
    };
    
    fs::write(&path, new_content).map_err(|e| e.to_string())?;
    
    Ok(last_line)
}

#[tauri::command]
fn set_desktop_parent(window: tauri::Window, enable: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            FindWindowExW, GetShellWindow, SetParent, SetWindowPos, HWND_BOTTOM, SWP_NOACTIVATE,
            SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
        };

        let to_wide = |s: &str| -> Vec<u16> {
            s.encode_utf16().chain(std::iter::once(0)).collect()
        };

        let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as HWND;

        if enable {
            let shell_w = unsafe { GetShellWindow() };
            if shell_w == 0 {
                return Err("Default shell window not found".to_string());
            }

            let has_def_view_direct = unsafe {
                FindWindowExW(
                    shell_w,
                    0,
                    to_wide("SHELLDLL_DefView").as_ptr(),
                    std::ptr::null(),
                )
            };

            let mut parent_hwnd = 0;

            if has_def_view_direct != 0 {
                parent_hwnd = shell_w;
            } else {
                let mut worker_w = 0;
                unsafe {
                    while {
                        worker_w = FindWindowExW(
                            0,
                            worker_w,
                            to_wide("WorkerW").as_ptr(),
                            std::ptr::null(),
                        );
                        worker_w != 0
                    } {
                        let def_view = FindWindowExW(
                            worker_w,
                            0,
                            to_wide("SHELLDLL_DefView").as_ptr(),
                            std::ptr::null(),
                        );
                        if def_view != 0 {
                            parent_hwnd = worker_w;
                            break;
                        }
                    }
                }
            }

            if parent_hwnd == 0 {
                return Err("Could not find desktop shell parent window".to_string());
            }

            unsafe {
                SetParent(hwnd, parent_hwnd);
                SetWindowPos(
                    hwnd,
                    HWND_BOTTOM,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        } else {
            unsafe {
                SetParent(hwnd, 0);
            }
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
        let _ = enable;
        Ok(())
    }
}


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--silent"]),
        ))
        .setup(|app| {
            // Handle --silent flag to start hidden
            let args: Vec<String> = std::env::args().collect();
            if args.contains(&"--silent".to_string()) {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            // Setup System Tray
            use tauri::menu::{Menu, MenuItem};
            use tauri::tray::TrayIconBuilder;

            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>).map_err(|e| e.to_string())?;
            let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>).map_err(|e| e.to_string())?;
            let menu = Menu::with_items(app, &[&show, &quit]).map_err(|e| e.to_string())?;

            if let Some(icon) = app.default_window_icon() {
                let _tray = TrayIconBuilder::new()
                    .icon(icon.clone())
                    .menu(&menu)
                    .on_menu_event(|app: &tauri::AppHandle, event: tauri::menu::MenuEvent| match event.id.as_ref() {
                        "quit" => {
                            app.exit(0);
                        }
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        _ => {}
                    })
                    .build(app)
                    .map_err(|e: tauri::Error| e.to_string())?;
            }

            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            read_todo,
            write_todo,
            get_default_path,
            get_file_modified_time,
            set_always_on_top,
            set_always_on_bottom,
            log_deleted_task,
            pop_deleted_task,
            set_desktop_parent
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

