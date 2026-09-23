use super::err_msg;
use git2::{Delta, Diff, DiffOptions, Repository};
use serde::Serialize;

#[derive(Serialize)]
pub struct DiffLine {
    origin: char,
    content: String,
    old_lineno: Option<u32>,
    new_lineno: Option<u32>,
}

#[derive(Serialize)]
pub struct DiffHunk {
    header: String,
    lines: Vec<DiffLine>,
}

#[derive(Serialize)]
pub struct FileDiff {
    path: String,
    old_path: Option<String>,
    status: String,
    is_binary: bool,
    hunks: Vec<DiffHunk>,
}

fn status_label(status: Delta) -> &'static str {
    match status {
        Delta::Added => "added",
        Delta::Deleted => "deleted",
        Delta::Renamed => "renamed",
        Delta::Copied => "copied",
        Delta::Typechange => "typechange",
        Delta::Untracked => "untracked",
        _ => "modified",
    }
}

fn diff_to_file_diffs(diff: &Diff) -> Result<Vec<FileDiff>, String> {
    let mut results = Vec::new();

    for i in 0..diff.deltas().count() {
        let delta = diff.get_delta(i).ok_or("delta desapareció")?;
        let new_path = delta.new_file().path().map(|p| p.to_string_lossy().to_string());
        let old_path = delta.old_file().path().map(|p| p.to_string_lossy().to_string());
        let path = new_path.clone().or_else(|| old_path.clone()).unwrap_or_default();
        let is_binary = delta.new_file().is_binary() || delta.old_file().is_binary();

        let mut hunks = Vec::new();
        if !is_binary {
            if let Some(patch) = git2::Patch::from_diff(diff, i).map_err(err_msg)? {
                for h in 0..patch.num_hunks() {
                    let (hunk, line_count) = patch.hunk(h).map_err(err_msg)?;
                    let header = String::from_utf8_lossy(hunk.header()).trim_end().to_string();

                    let mut lines = Vec::new();
                    for l in 0..line_count {
                        let line = patch.line_in_hunk(h, l).map_err(err_msg)?;
                        let mut content = String::from_utf8_lossy(line.content()).to_string();
                        while content.ends_with('\n') || content.ends_with('\r') {
                            content.pop();
                        }
                        lines.push(DiffLine {
                            origin: line.origin(),
                            content,
                            old_lineno: line.old_lineno(),
                            new_lineno: line.new_lineno(),
                        });
                    }
                    hunks.push(DiffHunk { header, lines });
                }
            }
        }

        results.push(FileDiff {
            old_path: if old_path != new_path { old_path } else { None },
            status: status_label(delta.status()).to_string(),
            is_binary,
            path,
            hunks,
        });
    }

    Ok(results)
}

/// Diff for a single working-tree file: staged = HEAD..index (what's
/// staged), unstaged = index..workdir (what's not, including untracked).
#[tauri::command]
pub fn get_working_diff(path: String, file: String, staged: bool) -> Result<FileDiff, String> {
    let repo = Repository::open(&path).map_err(err_msg)?;

    let mut opts = DiffOptions::new();
    opts.pathspec(&file);
    opts.include_untracked(true);
    opts.recurse_untracked_dirs(true);
    opts.show_untracked_content(true);

    let diff = if staged {
        let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
        repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))
            .map_err(err_msg)?
    } else {
        repo.diff_index_to_workdir(None, Some(&mut opts)).map_err(err_msg)?
    };

    diff_to_file_diffs(&diff)?
        .into_iter()
        .next()
        .ok_or_else(|| "Sin cambios para este archivo".to_string())
}

/// Full diff for a commit against its first parent (or against an empty
/// tree for a root commit).
#[tauri::command]
pub fn get_commit_diff(path: String, sha: String) -> Result<Vec<FileDiff>, String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    let oid = git2::Oid::from_str(&sha).map_err(err_msg)?;
    let commit = repo.find_commit(oid).map_err(err_msg)?;
    let tree = commit.tree().map_err(err_msg)?;
    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

    let diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
        .map_err(err_msg)?;
    diff_to_file_diffs(&diff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::changes::{commit, stage_file};
    use crate::git::test_support::init_repo_with_commit;

    #[test]
    fn unstaged_diff_shows_added_and_removed_lines() {
        let test_repo = init_repo_with_commit();
        test_repo.write("initial.txt", "goodbye\n");

        let diff = get_working_diff(test_repo.path(), "initial.txt".to_string(), false).unwrap();
        assert_eq!(diff.status, "modified");
        assert_eq!(diff.hunks.len(), 1);

        let origins: Vec<char> = diff.hunks[0].lines.iter().map(|l| l.origin).collect();
        assert!(origins.contains(&'+'));
        assert!(origins.contains(&'-'));
    }

    #[test]
    fn staged_diff_only_reflects_the_index() {
        let test_repo = init_repo_with_commit();
        test_repo.write("initial.txt", "staged change\n");
        stage_file(test_repo.path(), "initial.txt".to_string()).unwrap();
        // Further unstaged edit on top — should not appear in the staged diff.
        test_repo.write("initial.txt", "staged change\nplus more\n");

        let staged_diff = get_working_diff(test_repo.path(), "initial.txt".to_string(), true).unwrap();
        let staged_text: String = staged_diff.hunks.iter().flat_map(|h| h.lines.iter()).map(|l| l.content.clone()).collect();
        assert!(staged_text.contains("staged change"));
        assert!(!staged_text.contains("plus more"));

        let unstaged_diff = get_working_diff(test_repo.path(), "initial.txt".to_string(), false).unwrap();
        let unstaged_text: String = unstaged_diff.hunks.iter().flat_map(|h| h.lines.iter()).map(|l| l.content.clone()).collect();
        assert!(unstaged_text.contains("plus more"));
    }

    #[test]
    fn untracked_file_diff_is_all_additions() {
        let test_repo = init_repo_with_commit();
        test_repo.write("new.txt", "line one\nline two\n");

        let diff = get_working_diff(test_repo.path(), "new.txt".to_string(), false).unwrap();
        assert_eq!(diff.status, "untracked");
        assert!(diff.hunks[0].lines.iter().all(|l| l.origin == '+'));
    }

    #[test]
    fn commit_diff_lists_changed_files() {
        let test_repo = init_repo_with_commit();
        test_repo.write("initial.txt", "changed\n");
        stage_file(test_repo.path(), "initial.txt".to_string()).unwrap();
        test_repo.write("second.txt", "brand new\n");
        stage_file(test_repo.path(), "second.txt".to_string()).unwrap();
        let oid = commit(test_repo.path(), "two files".to_string()).unwrap();

        let diffs = get_commit_diff(test_repo.path(), oid).unwrap();
        assert_eq!(diffs.len(), 2);
        assert!(diffs.iter().any(|d| d.path == "initial.txt" && d.status == "modified"));
        assert!(diffs.iter().any(|d| d.path == "second.txt" && d.status == "added"));
    }

    #[test]
    fn commit_diff_on_root_commit_shows_everything_as_added() {
        let test_repo = init_repo_with_commit();
        let root_oid = test_repo.repo.head().unwrap().target().unwrap().to_string();

        let diffs = get_commit_diff(test_repo.path(), root_oid).unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].status, "added");
    }
}
