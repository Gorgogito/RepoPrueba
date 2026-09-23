mod git;
mod github;
mod terminal;
mod workspace;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(git::undo::UndoState::default())
        .manage(terminal::TerminalState::default())
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
            git::remotes::clone_repository,
            git::history_ops::merge_branch,
            git::history_ops::cherry_pick,
            git::history_ops::revert_commit,
            git::history_ops::rebase_branch,
            git::interactive_rebase::get_rebase_commits,
            git::interactive_rebase::start_interactive_rebase,
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
            workspace::remove_known_repo,
            git::undo::get_undo_preview,
            git::undo::get_redo_preview,
            git::undo::undo_last_operation,
            git::undo::redo_last_undo,
            terminal::terminal_start,
            terminal::terminal_write,
            terminal::terminal_resize,
            terminal::terminal_stop,
            github::github_status,
            github::github_connect,
            github::github_disconnect,
            github::github_list_pull_requests,
            github::github_get_pull_request,
            github::github_create_pull_request,
            github::github_merge_pull_request
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                let state = app_handle.state::<terminal::TerminalState>();
                terminal::kill_session(&state);
            }
        });
}
