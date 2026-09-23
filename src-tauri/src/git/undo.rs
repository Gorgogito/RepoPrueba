use super::{ensure_clean_workdir, err_msg};
use git2::{Oid, Repository, ResetType};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Clone, Debug)]
struct RedoEntry {
    branch: String,
    from: Oid,
    to: Oid,
    label: String,
}

/// Per-repo redo stack, keyed by repo path. In-memory only: undo/redo
/// history doesn't need to survive an app restart.
#[derive(Default)]
pub struct UndoState(Mutex<HashMap<String, Vec<RedoEntry>>>);

fn current_branch_name(repo: &Repository) -> Result<String, String> {
    let head = repo.head().map_err(err_msg)?;
    Ok(head.shorthand().map_err(err_msg)?.to_string())
}

fn clean_label(message: &str) -> String {
    message.trim().to_string()
}

fn hard_reset_to(repo: &Repository, oid: Oid) -> Result<(), String> {
    let commit = repo.find_commit(oid).map_err(err_msg)?;
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.force();
    repo.reset(commit.as_object(), ResetType::Hard, Some(&mut checkout))
        .map_err(err_msg)?;
    Ok(())
}

#[derive(Serialize)]
pub struct UndoPreview {
    label: Option<String>,
}

/// What undo_last_operation would do, for display (tooltip/disabled state)
/// without actually doing it.
#[tauri::command]
pub fn get_undo_preview(path: String) -> Result<UndoPreview, String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    let branch = match current_branch_name(&repo) {
        Ok(b) => b,
        Err(_) => return Ok(UndoPreview { label: None }),
    };
    let refname = format!("refs/heads/{branch}");
    let reflog = match repo.reflog(&refname) {
        Ok(r) => r,
        Err(_) => return Ok(UndoPreview { label: None }),
    };
    let label = reflog
        .get(0)
        .filter(|entry| !entry.id_old().is_zero())
        .map(|entry| clean_label(entry.message().ok().flatten().unwrap_or("(sin mensaje)")));
    Ok(UndoPreview { label })
}

#[tauri::command]
pub fn get_redo_preview(path: String, state: tauri::State<UndoState>) -> UndoPreview {
    let stack = state.0.lock().unwrap();
    let label = stack.get(&path).and_then(|s| s.last()).map(|e| e.label.clone());
    UndoPreview { label }
}

/// Resets the current branch back one step using its own reflog (not
/// HEAD's), so this never conflates with plain branch switches — only
/// operations that actually moved the current branch (commit, merge,
/// rebase, cherry-pick, revert, reset, fast-forward pull) are undoable.
fn undo_impl(path: &str) -> Result<RedoEntry, String> {
    let repo = Repository::open(path).map_err(err_msg)?;
    ensure_clean_workdir(&repo)?;

    let branch = current_branch_name(&repo)?;
    let refname = format!("refs/heads/{branch}");
    let reflog = repo.reflog(&refname).map_err(err_msg)?;
    let entry = reflog
        .get(0)
        .ok_or_else(|| "No hay nada que deshacer en esta rama".to_string())?;

    let from = entry.id_new();
    let to = entry.id_old();
    if to.is_zero() {
        return Err("No hay nada que deshacer en esta rama".to_string());
    }
    let label = clean_label(entry.message().ok().flatten().unwrap_or("(sin mensaje)"));

    hard_reset_to(&repo, to)?;

    Ok(RedoEntry { branch, from: to, to: from, label })
}

fn redo_impl(path: &str, entry: &RedoEntry) -> Result<(), String> {
    let repo = Repository::open(path).map_err(err_msg)?;
    ensure_clean_workdir(&repo)?;

    let branch = current_branch_name(&repo)?;
    if branch != entry.branch {
        return Err("Cambiaste de rama desde el último undo; no se puede rehacer".to_string());
    }

    let head_oid = repo
        .find_reference(&format!("refs/heads/{branch}"))
        .ok()
        .and_then(|r| r.target());
    if head_oid != Some(entry.from) {
        return Err("El estado del repositorio cambió desde el último undo; no se puede rehacer".to_string());
    }

    hard_reset_to(&repo, entry.to)
}

#[tauri::command]
pub fn undo_last_operation(path: String, state: tauri::State<UndoState>) -> Result<String, String> {
    let entry = undo_impl(&path)?;
    let label = entry.label.clone();
    state.0.lock().unwrap().entry(path).or_default().push(entry);
    Ok(label)
}

#[tauri::command]
pub fn redo_last_undo(path: String, state: tauri::State<UndoState>) -> Result<String, String> {
    let entry = {
        let stack = state.0.lock().unwrap();
        stack
            .get(&path)
            .and_then(|s| s.last())
            .cloned()
            .ok_or_else(|| "Nada para rehacer".to_string())?
    };

    redo_impl(&path, &entry)?;

    let mut stack = state.0.lock().unwrap();
    if let Some(repo_stack) = stack.get_mut(&path) {
        repo_stack.pop();
    }
    Ok(entry.label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::changes::{commit, stage_file};
    use crate::git::test_support::init_repo_with_commit;

    #[test]
    fn undo_reverts_last_commit_and_redo_restores_it() {
        let test_repo = init_repo_with_commit();
        test_repo.write("initial.txt", "second version\n");
        stage_file(test_repo.path(), "initial.txt".to_string()).unwrap();
        let second_oid = commit(test_repo.path(), "second commit".to_string()).unwrap();

        let redo_entry = undo_impl(&test_repo.path()).expect("undo should succeed");
        assert!(redo_entry.label.contains("second commit"));

        let repo = Repository::open(test_repo.path()).unwrap();
        let head = repo.head().unwrap().target().unwrap();
        assert_ne!(head.to_string(), second_oid);
        let contents = std::fs::read_to_string(test_repo.dir.path().join("initial.txt")).unwrap();
        assert_eq!(contents, "hello\n");

        redo_impl(&test_repo.path(), &redo_entry).expect("redo should succeed");
        let repo = Repository::open(test_repo.path()).unwrap();
        let head = repo.head().unwrap().target().unwrap();
        assert_eq!(head.to_string(), second_oid);
        let contents = std::fs::read_to_string(test_repo.dir.path().join("initial.txt")).unwrap();
        assert_eq!(contents, "second version\n");
    }

    #[test]
    fn undo_refuses_when_nothing_to_undo() {
        let test_repo = init_repo_with_commit();
        let err = undo_impl(&test_repo.path()).unwrap_err();
        assert!(err.contains("deshacer"));
    }

    #[test]
    fn undo_refuses_with_uncommitted_changes() {
        let test_repo = init_repo_with_commit();
        test_repo.write("initial.txt", "v2\n");
        stage_file(test_repo.path(), "initial.txt".to_string()).unwrap();
        commit(test_repo.path(), "second commit".to_string()).unwrap();
        test_repo.write("initial.txt", "dirty\n");

        let err = undo_impl(&test_repo.path()).unwrap_err();
        assert!(err.contains("sin commitear"));
    }

    #[test]
    fn redo_refuses_after_switching_branch() {
        use crate::git::branches::{checkout_branch, create_branch};

        let test_repo = init_repo_with_commit();
        test_repo.write("initial.txt", "v2\n");
        stage_file(test_repo.path(), "initial.txt".to_string()).unwrap();
        commit(test_repo.path(), "second commit".to_string()).unwrap();

        let redo_entry = undo_impl(&test_repo.path()).unwrap();

        create_branch(test_repo.path(), "other".to_string()).unwrap();
        checkout_branch(test_repo.path(), "other".to_string()).unwrap();

        let err = redo_impl(&test_repo.path(), &redo_entry).unwrap_err();
        assert!(err.contains("rama"));
    }

    #[test]
    fn redo_refuses_if_state_moved_on() {
        let test_repo = init_repo_with_commit();
        test_repo.write("initial.txt", "v2\n");
        stage_file(test_repo.path(), "initial.txt".to_string()).unwrap();
        commit(test_repo.path(), "second commit".to_string()).unwrap();

        let redo_entry = undo_impl(&test_repo.path()).unwrap();

        // Something else moves the branch forward again before redo runs.
        test_repo.write("initial.txt", "different path\n");
        stage_file(test_repo.path(), "initial.txt".to_string()).unwrap();
        commit(test_repo.path(), "a different second commit".to_string()).unwrap();

        let err = redo_impl(&test_repo.path(), &redo_entry).unwrap_err();
        assert!(err.contains("cambió"));
    }
}
