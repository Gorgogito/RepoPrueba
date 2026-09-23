use super::err_msg;
use git2::{BranchType, Repository};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct CommitInfo {
    sha: String,
    short_sha: String,
    summary: String,
    author_name: String,
    author_email: String,
    timestamp: i64,
    parents: Vec<String>,
    refs: Vec<String>,
}

#[tauri::command]
pub fn get_commit_log(path: String, limit: usize) -> Result<Vec<CommitInfo>, String> {
    let repo = Repository::open(&path).map_err(err_msg)?;

    let mut ref_map: HashMap<String, Vec<String>> = HashMap::new();
    for r in repo.references().map_err(err_msg)?.flatten() {
        if !(r.is_branch() || r.is_tag() || r.is_remote()) {
            continue;
        }
        if let Some(target) = r.target() {
            let name = r.shorthand().unwrap_or("").to_string();
            if !name.is_empty() {
                ref_map.entry(target.to_string()).or_default().push(name);
            }
        }
    }

    let mut revwalk = repo.revwalk().map_err(err_msg)?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME).map_err(err_msg)?;

    // Include history reachable from every local branch, not just HEAD, so
    // the graph shows unmerged branches too.
    if let Ok(head) = repo.head() {
        if let Some(oid) = head.target() {
            revwalk.push(oid).map_err(err_msg)?;
        }
    }
    for item in repo.branches(Some(BranchType::Local)).map_err(err_msg)? {
        let (branch, _) = item.map_err(err_msg)?;
        if let Some(oid) = branch.get().target() {
            let _ = revwalk.push(oid);
        }
    }

    let mut commits = Vec::new();
    for oid in revwalk.take(limit) {
        let oid = oid.map_err(err_msg)?;
        let commit = repo.find_commit(oid).map_err(err_msg)?;
        let author = commit.author();
        let sha = oid.to_string();
        let short_sha = sha.chars().take(7).collect();

        commits.push(CommitInfo {
            summary: commit.summary().ok().flatten().unwrap_or("").to_string(),
            author_name: author.name().unwrap_or("").to_string(),
            author_email: author.email().unwrap_or("").to_string(),
            timestamp: commit.time().seconds(),
            parents: commit.parent_ids().map(|p| p.to_string()).collect(),
            refs: ref_map.get(&sha).cloned().unwrap_or_default(),
            short_sha,
            sha,
        });
    }

    Ok(commits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::branches::{checkout_branch, create_branch};
    use crate::git::test_support::init_repo_with_commit;

    #[test]
    fn log_lists_commits_newest_first_with_refs_and_parents() {
        let test_repo = init_repo_with_commit();
        let base = test_repo.repo.head().unwrap().shorthand().unwrap().to_string();
        let first_oid = test_repo.repo.head().unwrap().target().unwrap().to_string();

        test_repo.write("second.txt", "second\n");
        test_repo.stage("second.txt");
        let second_oid = test_repo.commit("second commit").to_string();

        let log = get_commit_log(test_repo.path(), 50).unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].sha, second_oid);
        assert_eq!(log[0].summary, "second commit");
        assert_eq!(log[0].parents, vec![first_oid.clone()]);
        assert!(log[0].refs.contains(&base));

        assert_eq!(log[1].sha, first_oid);
        assert!(log[1].parents.is_empty());
    }

    #[test]
    fn log_includes_unmerged_branch_commits() {
        let test_repo = init_repo_with_commit();
        let base = test_repo.repo.head().unwrap().shorthand().unwrap().to_string();

        create_branch(test_repo.path(), "feature".to_string()).unwrap();
        checkout_branch(test_repo.path(), "feature".to_string()).unwrap();
        test_repo.write("feature.txt", "feature\n");
        test_repo.stage("feature.txt");
        let feature_oid = test_repo.commit("feature commit").to_string();
        checkout_branch(test_repo.path(), base).unwrap();

        let log = get_commit_log(test_repo.path(), 50).unwrap();
        assert!(log.iter().any(|c| c.sha == feature_oid));
        let feature_commit = log.iter().find(|c| c.sha == feature_oid).unwrap();
        assert!(feature_commit.refs.contains(&"feature".to_string()));
    }

    #[test]
    fn log_respects_limit() {
        let test_repo = init_repo_with_commit();
        for i in 0..5 {
            test_repo.write("loop.txt", &format!("{i}\n"));
            test_repo.stage("loop.txt");
            test_repo.commit(&format!("commit {i}"));
        }

        let log = get_commit_log(test_repo.path(), 3).unwrap();
        assert_eq!(log.len(), 3);
    }
}
