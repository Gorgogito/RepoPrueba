use super::run_git;
use serde::Serialize;

#[derive(Serialize, Debug, PartialEq)]
pub struct WorktreeInfo {
    name: String,
    path: String,
    head: String,
    branch: Option<String>,
    is_locked: bool,
    is_prunable: bool,
    is_bare: bool,
    is_main: bool,
}

/// Parses `git worktree list --porcelain`: blank-line-separated entries,
/// each a handful of `key[ value]` lines (`worktree <path>`, `HEAD <sha>`,
/// `branch <ref>` or `detached`, optional `locked`/`prunable`/`bare`). Git
/// always lists the main worktree first.
#[allow(unused_assignments)] // the resets after the final flush!() are inert, not a bug
fn parse_worktree_list(output: &str) -> Vec<WorktreeInfo> {
    let mut worktrees = Vec::new();
    let mut path: Option<String> = None;
    let mut head = String::new();
    let mut branch: Option<String> = None;
    let mut is_locked = false;
    let mut is_prunable = false;
    let mut is_bare = false;

    macro_rules! flush {
        () => {
            if let Some(p) = path.take() {
                let name = std::path::Path::new(&p)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&p)
                    .to_string();
                worktrees.push(WorktreeInfo {
                    name,
                    path: p,
                    head: std::mem::take(&mut head),
                    branch: branch.take(),
                    is_locked,
                    is_prunable,
                    is_bare,
                    is_main: worktrees.is_empty(),
                });
                is_locked = false;
                is_prunable = false;
                is_bare = false;
            }
        };
    }

    for line in output.lines() {
        if line.is_empty() {
            flush!();
            continue;
        }
        if let Some(rest) = line.strip_prefix("worktree ") {
            path = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("HEAD ") {
            head = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("branch ") {
            branch = Some(rest.trim_start_matches("refs/heads/").to_string());
        } else if line == "detached" {
            branch = None;
        } else if line == "bare" {
            is_bare = true;
        } else if line.starts_with("locked") {
            is_locked = true;
        } else if line.starts_with("prunable") {
            is_prunable = true;
        }
    }
    flush!();

    worktrees
}

#[tauri::command]
pub fn list_worktrees(path: String) -> Result<Vec<WorktreeInfo>, String> {
    let output = run_git(&path, &["worktree", "list", "--porcelain"])?;
    Ok(parse_worktree_list(&output))
}

#[tauri::command]
pub fn add_worktree(path: String, worktree_path: String, branch: String, create_branch: bool) -> Result<(), String> {
    if create_branch {
        run_git(&path, &["worktree", "add", "-b", &branch, &worktree_path]).map(|_| ())
    } else {
        run_git(&path, &["worktree", "add", &worktree_path, &branch]).map(|_| ())
    }
}

#[tauri::command]
pub fn remove_worktree(path: String, worktree_path: String, force: bool) -> Result<(), String> {
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(&worktree_path);
    run_git(&path, &args).map(|_| ())
}

#[tauri::command]
pub fn prune_worktrees(path: String) -> Result<(), String> {
    run_git(&path, &["worktree", "prune"]).map(|_| ())
}

#[tauri::command]
pub fn lock_worktree(path: String, worktree_path: String, reason: Option<String>) -> Result<(), String> {
    let mut args = vec!["worktree", "lock"];
    if let Some(r) = &reason {
        args.push("--reason");
        args.push(r);
    }
    args.push(&worktree_path);
    run_git(&path, &args).map(|_| ())
}

#[tauri::command]
pub fn unlock_worktree(path: String, worktree_path: String) -> Result<(), String> {
    run_git(&path, &["worktree", "unlock", &worktree_path]).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_support::init_repo_with_commit;

    #[test]
    fn lists_only_the_main_worktree_initially() {
        let test_repo = init_repo_with_commit();
        let branch = test_repo.repo.head().unwrap().shorthand().unwrap().to_string();

        let worktrees = list_worktrees(test_repo.path()).unwrap();
        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].is_main);
        assert_eq!(worktrees[0].branch.as_deref(), Some(branch.as_str()));
    }

    #[test]
    fn add_creates_a_linked_worktree_with_a_new_branch() {
        let test_repo = init_repo_with_commit();
        let parent = tempfile::TempDir::new().unwrap();
        let wt_path = parent.path().join("feature-wt");

        add_worktree(
            test_repo.path(),
            wt_path.to_string_lossy().to_string(),
            "feature".to_string(),
            true,
        )
        .expect("add should succeed");

        let worktrees = list_worktrees(test_repo.path()).unwrap();
        assert_eq!(worktrees.len(), 2);
        let linked = worktrees.iter().find(|w| !w.is_main).unwrap();
        assert_eq!(linked.branch.as_deref(), Some("feature"));
        assert!(wt_path.join("initial.txt").exists());
    }

    #[test]
    fn remove_deletes_a_clean_linked_worktree() {
        let test_repo = init_repo_with_commit();
        let parent = tempfile::TempDir::new().unwrap();
        let wt_path = parent.path().join("feature-wt");

        add_worktree(
            test_repo.path(),
            wt_path.to_string_lossy().to_string(),
            "feature".to_string(),
            true,
        )
        .unwrap();
        assert_eq!(list_worktrees(test_repo.path()).unwrap().len(), 2);

        remove_worktree(test_repo.path(), wt_path.to_string_lossy().to_string(), false)
            .expect("remove should succeed");

        assert_eq!(list_worktrees(test_repo.path()).unwrap().len(), 1);
        assert!(!wt_path.exists());
    }

    #[test]
    fn prune_clears_a_worktree_deleted_from_disk_directly() {
        let test_repo = init_repo_with_commit();
        let parent = tempfile::TempDir::new().unwrap();
        let wt_path = parent.path().join("feature-wt");

        add_worktree(
            test_repo.path(),
            wt_path.to_string_lossy().to_string(),
            "feature".to_string(),
            true,
        )
        .unwrap();

        // Simulate someone deleting the worktree folder by hand instead of
        // going through `git worktree remove` — git's admin metadata under
        // .git/worktrees/ is left dangling until a prune.
        std::fs::remove_dir_all(&wt_path).unwrap();

        let before = list_worktrees(test_repo.path()).unwrap();
        assert!(before.iter().any(|w| w.is_prunable));

        prune_worktrees(test_repo.path()).expect("prune should succeed");

        let after = list_worktrees(test_repo.path()).unwrap();
        assert_eq!(after.len(), 1);
    }

    #[test]
    fn lock_and_unlock_round_trip() {
        let test_repo = init_repo_with_commit();
        let parent = tempfile::TempDir::new().unwrap();
        let wt_path = parent.path().join("feature-wt");
        let wt_path_str = wt_path.to_string_lossy().to_string();

        add_worktree(test_repo.path(), wt_path_str.clone(), "feature".to_string(), true).unwrap();

        lock_worktree(test_repo.path(), wt_path_str.clone(), Some("in use".to_string())).unwrap();
        let locked = list_worktrees(test_repo.path()).unwrap();
        assert!(locked.iter().find(|w| !w.is_main).unwrap().is_locked);

        // A locked worktree refuses removal without --force.
        assert!(remove_worktree(test_repo.path(), wt_path_str.clone(), false).is_err());

        unlock_worktree(test_repo.path(), wt_path_str.clone()).unwrap();
        let unlocked = list_worktrees(test_repo.path()).unwrap();
        assert!(!unlocked.iter().find(|w| !w.is_main).unwrap().is_locked);
    }
}
