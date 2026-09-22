mod git;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            git::repo::open_repository,
            git::repo::get_repo_status,
            git::branches::list_branches,
            git::branches::create_branch,
            git::branches::checkout_branch,
            git::branches::delete_branch
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
