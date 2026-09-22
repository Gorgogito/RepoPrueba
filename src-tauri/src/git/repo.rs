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
    pub(crate) path: String,
    pub(crate) staged: bool,
    pub(crate) status: String,
}

#[derive(Serialize)]
pub struct RepoStatus {
    pub(crate) is_clean: bool,
    ahead: usize,
    behind: usize,
    pub(crate) changes: Vec<FileChange>,
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
    let repo = Repository::open(&path).map_err(super::err_msg)?;

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
    let repo = Repository::open(&path).map_err(super::err_msg)?;

    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(super::err_msg)?;

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
    use crate::git::test_support::init_repo_with_commit;

    #[test]
    fn open_repository_reads_name_and_branch() {
        let test_repo = init_repo_with_commit();
        let expected_branch = test_repo.repo.head().unwrap().shorthand().unwrap().to_string();

        let info = open_repository(test_repo.path()).expect("should open repo");
        assert_eq!(info.current_branch, expected_branch);
    }

    #[test]
    fn get_repo_status_reports_clean_repo() {
        let test_repo = init_repo_with_commit();

        let status = get_repo_status(test_repo.path()).expect("should read status");
        assert!(status.is_clean);
        assert_eq!(status.ahead, 0);
        assert_eq!(status.behind, 0);
        assert!(status.changes.is_empty());
    }

    #[test]
    fn get_repo_status_reports_staged_modified_and_untracked() {
        let test_repo = init_repo_with_commit();

        // staged addition
        test_repo.write("staged.txt", "new\n");
        test_repo.stage("staged.txt");

        // unstaged modification of the tracked file
        test_repo.write("initial.txt", "changed\n");

        // untracked file
        test_repo.write("scratch.txt", "temp\n");

        let status = get_repo_status(test_repo.path()).expect("should read status");
        assert!(!status.is_clean);

        let staged: Vec<_> = status.changes.iter().filter(|c| c.staged).collect();
        let unstaged: Vec<_> = status.changes.iter().filter(|c| !c.staged).collect();

        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].path, "staged.txt");
        assert_eq!(staged[0].status, "added");

        assert_eq!(unstaged.len(), 2);
        assert!(unstaged.iter().any(|c| c.path == "initial.txt" && c.status == "modified"));
        assert!(unstaged.iter().any(|c| c.path == "scratch.txt" && c.status == "untracked"));
    }
}
