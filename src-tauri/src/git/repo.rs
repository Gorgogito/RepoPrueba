use git2::{Repository, StatusOptions};
use serde::Serialize;

#[derive(Serialize)]
pub struct RepoInfo {
    path: String,
    name: String,
    current_branch: String,
}

#[derive(Serialize)]
pub struct FileChange {
    path: String,
    staged: bool,
    status: String,
}

#[derive(Serialize)]
pub struct RepoStatus {
    is_clean: bool,
    ahead: usize,
    behind: usize,
    changes: Vec<FileChange>,
}

fn current_branch_name(repo: &Repository) -> String {
    match repo.head() {
        Ok(head) if head.is_branch() => head
            .shorthand()
            .unwrap_or("(unknown)")
            .to_string(),
        Ok(_) => "(detached HEAD)".to_string(),
        Err(_) => "(no commits yet)".to_string(),
    }
}

#[tauri::command]
pub fn open_repository(path: String) -> Result<RepoInfo, String> {
    let repo = Repository::open(&path).map_err(|e| e.message().to_string())?;

    let name = std::path::Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&path)
        .to_string();

    Ok(RepoInfo {
        current_branch: current_branch_name(&repo),
        path,
        name,
    })
}

#[tauri::command]
pub fn get_repo_status(path: String) -> Result<RepoStatus, String> {
    let repo = Repository::open(&path).map_err(|e| e.message().to_string())?;

    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| e.message().to_string())?;

    let mut changes = Vec::new();
    for entry in statuses.iter() {
        let file_path = entry.path().unwrap_or("").to_string();
        let s = entry.status();

        if s.is_index_new() || s.is_index_modified() || s.is_index_deleted() || s.is_index_renamed() {
            changes.push(FileChange {
                path: file_path.clone(),
                staged: true,
                status: index_status_label(s).to_string(),
            });
        }
        if s.is_wt_new() || s.is_wt_modified() || s.is_wt_deleted() || s.is_wt_renamed() || s.is_conflicted() {
            changes.push(FileChange {
                path: file_path,
                staged: false,
                status: worktree_status_label(s).to_string(),
            });
        }
    }

    let (ahead, behind) = ahead_behind(&repo).unwrap_or((0, 0));

    Ok(RepoStatus {
        is_clean: changes.is_empty(),
        ahead,
        behind,
        changes,
    })
}

fn index_status_label(s: git2::Status) -> &'static str {
    if s.is_index_new() {
        "added"
    } else if s.is_index_deleted() {
        "deleted"
    } else if s.is_index_renamed() {
        "renamed"
    } else {
        "modified"
    }
}

fn worktree_status_label(s: git2::Status) -> &'static str {
    if s.is_conflicted() {
        "conflicted"
    } else if s.is_wt_new() {
        "untracked"
    } else if s.is_wt_deleted() {
        "deleted"
    } else if s.is_wt_renamed() {
        "renamed"
    } else {
        "modified"
    }
}

fn ahead_behind(repo: &Repository) -> Option<(usize, usize)> {
    let head = repo.head().ok()?;
    let branch = git2::Branch::wrap(head);
    let local_oid = branch.get().target()?;
    let upstream = branch.upstream().ok()?;
    let upstream_oid = upstream.get().target()?;
    repo.graph_ahead_behind(local_oid, upstream_oid).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> String {
        // cargo test's cwd is the crate root (src-tauri); its parent is the repo root
        std::env::current_dir()
            .unwrap()
            .parent()
            .unwrap()
            .to_string_lossy()
            .to_string()
    }

    #[test]
    fn open_repository_reads_this_repo() {
        let info = open_repository(workspace_root()).expect("should open repo");
        assert_eq!(info.name, "Stash");
        assert_eq!(info.current_branch, "master");
    }

    #[test]
    fn get_repo_status_matches_git_cli() {
        let status = get_repo_status(workspace_root()).expect("should read status");

        assert!(!status.is_clean);
        assert_eq!(status.ahead, 0);
        assert_eq!(status.behind, 0);

        let staged: Vec<_> = status.changes.iter().filter(|c| c.staged).collect();
        let unstaged: Vec<_> = status.changes.iter().filter(|c| !c.staged).collect();

        assert_eq!(staged.len(), 0, "nothing should be staged in this test run");

        let modified: Vec<_> = unstaged.iter().filter(|c| c.status == "modified").collect();
        let untracked: Vec<_> = unstaged.iter().filter(|c| c.status == "untracked").collect();
        assert_eq!(modified.len(), 8, "expected 8 modified tracked files");
        assert_eq!(untracked.len(), 2, "expected 2 untracked files under src-tauri/src/git");

        let paths: Vec<_> = unstaged.iter().map(|c| c.path.as_str()).collect();
        assert!(paths.contains(&"src-tauri/src/git/mod.rs"));
        assert!(paths.contains(&"src-tauri/src/git/repo.rs"));
    }
}
