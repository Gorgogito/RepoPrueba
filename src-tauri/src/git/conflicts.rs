use super::err_msg;
use git2::{Repository, RepositoryState, ResetType};
use serde::Serialize;
use std::path::Path;

fn operation_kind(state: RepositoryState) -> &'static str {
    match state {
        RepositoryState::Merge => "merge",
        RepositoryState::CherryPick => "cherrypick",
        RepositoryState::Revert => "revert",
        RepositoryState::RebaseMerge | RepositoryState::Rebase | RepositoryState::RebaseInteractive => "rebase",
        RepositoryState::Clean => "none",
        _ => "other",
    }
}

#[derive(Serialize)]
pub struct OperationStatus {
    pub(crate) kind: String,
    pub(crate) message: String,
    pub(crate) conflicts: Vec<String>,
    pub(crate) paused_reason: Option<String>,
}

fn collect_conflicts(repo: &Repository) -> Result<Vec<String>, String> {
    let index = repo.index().map_err(err_msg)?;
    let mut conflicts = Vec::new();
    if index.has_conflicts() {
        for entry in index.conflicts().map_err(err_msg)? {
            let entry = entry.map_err(err_msg)?;
            let raw_path = entry
                .our
                .or(entry.their)
                .or(entry.ancestor)
                .map(|e| e.path);
            if let Some(raw_path) = raw_path {
                if let Ok(p) = String::from_utf8(raw_path) {
                    if !conflicts.contains(&p) {
                        conflicts.push(p);
                    }
                }
            }
        }
    }
    Ok(conflicts)
}

#[tauri::command]
pub fn get_operation_status(path: String) -> Result<OperationStatus, String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    let conflicts = collect_conflicts(&repo)?;

    // Our interactive rebase engine (git/interactive_rebase.rs) never uses
    // git2's own RebaseMerge state, so it has to be checked separately —
    // it takes priority since repo.state() reports "Clean" while paused.
    if let Some(reason) = super::interactive_rebase::paused_reason(&repo) {
        return Ok(OperationStatus {
            kind: "rebase".to_string(),
            message: String::new(),
            conflicts,
            paused_reason: Some(reason),
        });
    }

    let kind = operation_kind(repo.state()).to_string();
    let message = repo.message().unwrap_or_default();
    Ok(OperationStatus { kind, message, conflicts, paused_reason: None })
}

#[derive(Serialize)]
pub struct ConflictContent {
    ancestor: Option<String>,
    ours: Option<String>,
    theirs: Option<String>,
}

#[tauri::command]
pub fn get_conflict_content(path: String, file: String) -> Result<ConflictContent, String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    let index = repo.index().map_err(err_msg)?;
    let conflict = index.conflict_get(Path::new(&file)).map_err(err_msg)?;

    let read_blob = |entry: Option<git2::IndexEntry>| -> Option<String> {
        let entry = entry?;
        let blob = repo.find_blob(entry.id).ok()?;
        String::from_utf8(blob.content().to_vec()).ok()
    };

    Ok(ConflictContent {
        ancestor: read_blob(conflict.ancestor),
        ours: read_blob(conflict.our),
        theirs: read_blob(conflict.their),
    })
}

#[tauri::command]
pub fn read_working_file(path: String, file: String) -> Result<String, String> {
    std::fs::read_to_string(Path::new(&path).join(&file)).map_err(|e| e.to_string())
}

/// Writes the given content as the resolution for `file` and stages it,
/// clearing its conflict entries from the index.
#[tauri::command]
pub fn resolve_conflict(path: String, file: String, content: String) -> Result<(), String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    std::fs::write(Path::new(&path).join(&file), content).map_err(|e| e.to_string())?;

    let mut index = repo.index().map_err(err_msg)?;
    index.add_path(Path::new(&file)).map_err(err_msg)?;
    index.write().map_err(err_msg)?;
    Ok(())
}

/// Finishes the in-progress merge/cherry-pick/revert once all conflicts are
/// resolved, creating the appropriate commit and cleaning up operation state.
#[tauri::command]
pub fn continue_operation(path: String) -> Result<(), String> {
    let mut repo = Repository::open(&path).map_err(err_msg)?;

    if super::interactive_rebase::paused_reason(&repo).is_some() {
        return super::interactive_rebase::continue_after_resolution(&repo);
    }

    let mut index = repo.index().map_err(err_msg)?;
    if index.has_conflicts() {
        return Err("Todavía hay conflictos sin resolver".to_string());
    }

    let state = repo.state();

    // Gathered up front: mergehead_foreach needs `&mut repo`, which can't
    // coexist with the immutable borrows (`tree`, commits) held below.
    let mut merge_oids = Vec::new();
    if state == RepositoryState::Merge {
        repo.mergehead_foreach(|oid| {
            merge_oids.push(*oid);
            true
        })
        .map_err(err_msg)?;
    }

    let tree_oid = index.write_tree().map_err(err_msg)?;
    let tree = repo.find_tree(tree_oid).map_err(err_msg)?;
    let head_commit = repo.head().map_err(err_msg)?.peel_to_commit().map_err(err_msg)?;
    // git2 appends "#Conflicts:" hint lines to the prepared message, mirroring
    // git's own MERGE_MSG; strip comment lines like git does when finalizing.
    let raw_message = repo.message().unwrap_or_default();
    let message: String = raw_message
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let message = message.trim();
    let sig = repo.signature().map_err(err_msg)?;

    match state {
        RepositoryState::Merge => {
            let mut parents = vec![head_commit];
            for oid in merge_oids {
                parents.push(repo.find_commit(oid).map_err(err_msg)?);
            }
            let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
            let message = if message.is_empty() { "Merge" } else { message };
            repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
                .map_err(err_msg)?;
        }
        RepositoryState::CherryPick => {
            let cp_head = repo.find_reference("CHERRY_PICK_HEAD").map_err(err_msg)?;
            let picked = cp_head.peel_to_commit().map_err(err_msg)?;
            let message = if message.is_empty() {
                picked.message().unwrap_or("cherry-pick").to_string()
            } else {
                message.to_string()
            };
            repo.commit(Some("HEAD"), &picked.author(), &sig, &message, &tree, &[&head_commit])
                .map_err(err_msg)?;
        }
        RepositoryState::Revert => {
            let message = if message.is_empty() { "Revert" } else { message };
            repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&head_commit])
                .map_err(err_msg)?;
        }
        _ => return Err("No hay una operación en curso que se pueda continuar".to_string()),
    }

    repo.cleanup_state().map_err(err_msg)?;
    Ok(())
}

/// Discards an in-progress merge/cherry-pick/revert, restoring the working
/// tree to HEAD (which was never moved while conflicted).
#[tauri::command]
pub fn abort_operation(path: String) -> Result<(), String> {
    let repo = Repository::open(&path).map_err(err_msg)?;

    if super::interactive_rebase::paused_reason(&repo).is_some() {
        return super::interactive_rebase::abort(&repo);
    }

    let head_commit = repo.head().map_err(err_msg)?.peel_to_commit().map_err(err_msg)?;

    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.force();
    repo.reset(head_commit.as_object(), ResetType::Hard, Some(&mut checkout))
        .map_err(err_msg)?;
    repo.cleanup_state().map_err(err_msg)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::branches::{checkout_branch, create_branch};
    use crate::git::history_ops::{cherry_pick, merge_branch};
    use crate::git::repo::get_repo_status;
    use crate::git::test_support::init_repo_with_commit;

    fn make_conflicting_branches(test_repo: &crate::git::test_support::TestRepo) -> (String, String) {
        let base = test_repo.repo.head().unwrap().shorthand().unwrap().to_string();

        create_branch(test_repo.path(), "feature".to_string()).unwrap();
        checkout_branch(test_repo.path(), "feature".to_string()).unwrap();
        test_repo.write("initial.txt", "from feature\n");
        test_repo.stage("initial.txt");
        test_repo.commit("feature edits initial.txt");

        checkout_branch(test_repo.path(), base.clone()).unwrap();
        test_repo.write("initial.txt", "from base\n");
        test_repo.stage("initial.txt");
        test_repo.commit("base edits initial.txt");

        (base, "feature".to_string())
    }

    #[test]
    fn merge_conflict_can_be_resolved_and_continued() {
        let test_repo = init_repo_with_commit();
        let (_base, feature) = make_conflicting_branches(&test_repo);

        let err = merge_branch(test_repo.path(), feature).unwrap_err();
        assert!(err.contains("conflictos"));

        let status = get_operation_status(test_repo.path()).unwrap();
        assert_eq!(status.kind, "merge");
        assert_eq!(status.conflicts, vec!["initial.txt".to_string()]);

        let content = get_conflict_content(test_repo.path(), "initial.txt".to_string()).unwrap();
        assert_eq!(content.ours.as_deref(), Some("from base\n"));
        assert_eq!(content.theirs.as_deref(), Some("from feature\n"));

        resolve_conflict(test_repo.path(), "initial.txt".to_string(), "resolved\n".to_string()).unwrap();

        let status = get_operation_status(test_repo.path()).unwrap();
        assert!(status.conflicts.is_empty());

        continue_operation(test_repo.path()).expect("continue should succeed once resolved");

        let repo = Repository::open(test_repo.path()).unwrap();
        assert_eq!(repo.state(), RepositoryState::Clean);
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.parent_count(), 2);

        let contents = std::fs::read_to_string(test_repo.dir.path().join("initial.txt")).unwrap();
        assert_eq!(contents, "resolved\n");

        let clean = get_repo_status(test_repo.path()).unwrap();
        assert!(clean.is_clean);
    }

    #[test]
    fn abort_operation_restores_clean_state() {
        let test_repo = init_repo_with_commit();
        let (base, feature) = make_conflicting_branches(&test_repo);
        let head_before = test_repo.repo.find_reference(&format!("refs/heads/{base}")).unwrap();
        let head_before_oid = head_before.target().unwrap();

        merge_branch(test_repo.path(), feature).unwrap_err();
        assert_ne!(get_operation_status(test_repo.path()).unwrap().kind, "none");

        abort_operation(test_repo.path()).expect("abort should succeed");

        let repo = Repository::open(test_repo.path()).unwrap();
        assert_eq!(repo.state(), RepositoryState::Clean);
        assert_eq!(repo.head().unwrap().target().unwrap(), head_before_oid);

        let status = get_repo_status(test_repo.path()).unwrap();
        assert!(status.is_clean);
    }

    #[test]
    fn cherry_pick_conflict_can_be_resolved_and_continued() {
        let test_repo = init_repo_with_commit();
        let base = test_repo.repo.head().unwrap().shorthand().unwrap().to_string();

        create_branch(test_repo.path(), "feature".to_string()).unwrap();
        checkout_branch(test_repo.path(), "feature".to_string()).unwrap();
        test_repo.write("initial.txt", "from feature\n");
        test_repo.stage("initial.txt");
        let picked = test_repo.commit("feature edits initial.txt");

        checkout_branch(test_repo.path(), base).unwrap();
        test_repo.write("initial.txt", "from base\n");
        test_repo.stage("initial.txt");
        test_repo.commit("base edits initial.txt");

        let err = cherry_pick(test_repo.path(), picked.to_string()).unwrap_err();
        assert!(err.contains("conflictos"));

        let status = get_operation_status(test_repo.path()).unwrap();
        assert_eq!(status.kind, "cherrypick");

        resolve_conflict(test_repo.path(), "initial.txt".to_string(), "resolved\n".to_string()).unwrap();
        continue_operation(test_repo.path()).expect("continue should succeed");

        let repo = Repository::open(test_repo.path()).unwrap();
        assert_eq!(repo.state(), RepositoryState::Clean);
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.parent_count(), 1);
        assert_eq!(head.message().unwrap().trim(), "feature edits initial.txt");
    }
}
