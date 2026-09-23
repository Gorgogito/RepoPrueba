mod git;
mod workspace;

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
            git::branches::delete_branch,
            git::changes::stage_file,
            git::changes::unstage_file,
            git::changes::commit,
            git::remotes::list_remotes,
            git::remotes::add_remote,
            git::remotes::fetch,
            git::remotes::pull,
            git::remotes::push,
            git::history_ops::merge_branch,
            git::history_ops::cherry_pick,
            git::history_ops::revert_commit,
            git::history_ops::rebase_branch,
            git::stash::stash_save,
            git::stash::stash_list,
            git::stash::stash_apply,
            git::stash::stash_pop,
            git::stash::stash_drop,
            git::conflicts::get_operation_status,
            git::conflicts::get_conflict_content,
            git::conflicts::read_working_file,
            git::conflicts::resolve_conflict,
            git::conflicts::continue_operation,
            git::conflicts::abort_operation,
            git::log::get_commit_log,
            git::diff::get_working_diff,
            git::diff::get_commit_diff,
            workspace::list_known_repos,
            workspace::add_known_repo,
            workspace::remove_known_repo
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
