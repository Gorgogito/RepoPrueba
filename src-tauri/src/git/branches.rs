use super::{ensure_clean_workdir, err_msg};
use git2::{BranchType, Repository};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Serialize)]
pub struct BranchInfo {
    name: String,
    is_head: bool,
    upstream: Option<String>,
}

#[tauri::command]
pub fn list_branches(path: String) -> Result<Vec<BranchInfo>, String> {
    let repo = Repository::open(&path).map_err(err_msg)?;

    let mut branches = Vec::new();
    for item in repo.branches(Some(BranchType::Local)).map_err(err_msg)? {
        let (branch, _) = item.map_err(err_msg)?;
        let name = branch.name().map_err(err_msg)?.unwrap_or("").to_string();
        let upstream = branch
            .upstream()
            .ok()
            .and_then(|u| u.name().ok().flatten().map(|s| s.to_string()));

        branches.push(BranchInfo {
            is_head: branch.is_head(),
            name,
            upstream,
        });
    }

    branches.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(branches)
}

#[tauri::command]
pub fn create_branch(path: String, name: String) -> Result<(), String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    let target = repo.head().map_err(err_msg)?.peel_to_commit().map_err(err_msg)?;
    repo.branch(&name, &target, false).map_err(err_msg)?;
    Ok(())
}

fn checkout_ref(repo: &Repository, refname: &str) -> Result<(), String> {
    ensure_clean_workdir(repo)?;

    let target = repo.revparse_single(refname).map_err(err_msg)?;
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.force();
    repo.checkout_tree(&target, Some(&mut checkout)).map_err(err_msg)?;
    repo.set_head(refname).map_err(err_msg)?;

    // checkout_tree updates the working directory but, empirically, does not
    // reliably persist the index to match — do that explicitly so status
    // (and any later ensure_clean_workdir check) reflects reality.
    let target_tree = target.peel_to_tree().map_err(err_msg)?;
    let mut index = repo.index().map_err(err_msg)?;
    index.read_tree(&target_tree).map_err(err_msg)?;
    index.write().map_err(err_msg)?;

    Ok(())
}

#[tauri::command]
pub fn checkout_branch(path: String, name: String) -> Result<(), String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    checkout_ref(&repo, &format!("refs/heads/{name}"))
}

#[derive(Serialize)]
pub struct RemoteBranchInfo {
    name: String,
    remote: String,
    branch: String,
    has_local: bool,
}

/// Remote-tracking branches (`origin/feature-x`) that a plain `git fetch`
/// already pulled in but that never got a local branch of their own — the
/// common "a teammate pushed this, I never checked it out" case, which the
/// local-only branch list has no way to surface.
#[tauri::command]
pub fn list_remote_branches(path: String) -> Result<Vec<RemoteBranchInfo>, String> {
    let repo = Repository::open(&path).map_err(err_msg)?;

    let local_names: HashSet<String> = repo
        .branches(Some(BranchType::Local))
        .map_err(err_msg)?
        .filter_map(|item| item.ok())
        .filter_map(|(b, _)| b.name().ok().flatten().map(|s| s.to_string()))
        .collect();

    let mut result = Vec::new();
    for item in repo.branches(Some(BranchType::Remote)).map_err(err_msg)? {
        let (branch, _) = item.map_err(err_msg)?;
        // "origin/HEAD" and similar are symbolic pointers at another ref,
        // not a real branch to offer for checkout.
        if branch.get().symbolic_target_bytes().is_some() {
            continue;
        }
        let full_name = branch.name().ok().flatten().unwrap_or("").to_string();
        let Some((remote, short)) = full_name.split_once('/') else { continue };
        if full_name.is_empty() {
            continue;
        }

        result.push(RemoteBranchInfo {
            has_local: local_names.contains(short),
            name: full_name.clone(),
            remote: remote.to_string(),
            branch: short.to_string(),
        });
    }

    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
}

/// Creates a local branch named `local_name` tracking the remote branch
/// `remote_ref` (e.g. "origin/feature-x") and switches to it — the same
/// thing `git checkout feature-x` does automatically when no local branch
/// exists yet but exactly one remote has it.
#[tauri::command]
pub fn checkout_remote_branch(path: String, remote_ref: String, local_name: String) -> Result<(), String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    ensure_clean_workdir(&repo)?;

    let remote_branch = repo.find_branch(&remote_ref, BranchType::Remote).map_err(err_msg)?;
    let target = remote_branch.get().peel_to_commit().map_err(err_msg)?;

    let mut local_branch = repo.branch(&local_name, &target, false).map_err(err_msg)?;
    local_branch.set_upstream(Some(&remote_ref)).map_err(err_msg)?;

    checkout_ref(&repo, &format!("refs/heads/{local_name}"))
}

#[tauri::command]
pub fn delete_branch(path: String, name: String, force: bool) -> Result<(), String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    let mut branch = repo.find_branch(&name, BranchType::Local).map_err(err_msg)?;

    if branch.is_head() {
        return Err("No puedes eliminar la rama actual".to_string());
    }

    if !force {
        let head_oid = repo.head().ok().and_then(|h| h.target());
        let branch_oid = branch.get().target();
        if let (Some(head_oid), Some(branch_oid)) = (head_oid, branch_oid) {
            let is_merged =
                head_oid == branch_oid || repo.graph_descendant_of(head_oid, branch_oid).unwrap_or(false);
            if !is_merged {
                return Err(format!(
                    "La rama '{name}' tiene cambios sin fusionar. Usa 'force' para eliminarla de todos modos."
                ));
            }
        }
    }

    branch.delete().map_err(err_msg)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_support::init_repo_with_commit;

    #[test]
    fn create_lists_and_switches_branches() {
        let test_repo = init_repo_with_commit();
        let base_branch = test_repo.repo.head().unwrap().shorthand().unwrap().to_string();

        create_branch(test_repo.path(), "feature-x".to_string()).expect("create should succeed");

        let branches = list_branches(test_repo.path()).expect("list should succeed");
        assert_eq!(branches.len(), 2);
        assert!(branches.iter().any(|b| b.name == "feature-x" && !b.is_head));
        assert!(branches.iter().any(|b| b.name == base_branch && b.is_head));

        checkout_branch(test_repo.path(), "feature-x".to_string()).expect("checkout should succeed");
        let branches = list_branches(test_repo.path()).expect("list should succeed");
        assert!(branches.iter().any(|b| b.name == "feature-x" && b.is_head));
    }

    #[test]
    fn delete_refuses_current_branch() {
        let test_repo = init_repo_with_commit();
        let base_branch = test_repo.repo.head().unwrap().shorthand().unwrap().to_string();

        let err = delete_branch(test_repo.path(), base_branch, false).unwrap_err();
        assert!(err.contains("rama actual"));
    }

    #[test]
    fn delete_refuses_unmerged_branch_without_force() {
        let test_repo = init_repo_with_commit();
        let base_branch = test_repo.repo.head().unwrap().shorthand().unwrap().to_string();

        create_branch(test_repo.path(), "feature-x".to_string()).unwrap();
        checkout_branch(test_repo.path(), "feature-x".to_string()).unwrap();
        test_repo.write("more.txt", "content\n");
        test_repo.stage("more.txt");
        test_repo.commit("extra work on feature-x");

        checkout_branch(test_repo.path(), base_branch).unwrap();

        let err = delete_branch(test_repo.path(), "feature-x".to_string(), false).unwrap_err();
        assert!(err.contains("sin fusionar"));

        delete_branch(test_repo.path(), "feature-x".to_string(), true).expect("force delete should succeed");
    }

    #[test]
    fn lists_remote_branches_and_excludes_ones_already_tracked_locally() {
        let local = init_repo_with_commit();
        let base = local.repo.head().unwrap().shorthand().unwrap().to_string();
        let bare = crate::git::test_support::init_bare_remote();
        crate::git::remotes::add_remote(local.path(), "origin".to_string(), bare.path().to_string_lossy().to_string())
            .unwrap();
        crate::git::remotes::push(local.path(), "origin".to_string(), base.clone()).unwrap();

        create_branch(local.path(), "feature-x".to_string()).unwrap();
        crate::git::remotes::push(local.path(), "origin".to_string(), "feature-x".to_string()).unwrap();
        // Only exists on the remote now — the common "teammate pushed this,
        // I never checked it out" case.
        delete_branch(local.path(), "feature-x".to_string(), true).unwrap();
        crate::git::remotes::fetch(local.path(), "origin".to_string()).unwrap();

        let remotes = list_remote_branches(local.path()).unwrap();
        let feature = remotes.iter().find(|r| r.branch == "feature-x").expect("origin/feature-x should be listed");
        assert_eq!(feature.remote, "origin");
        assert!(!feature.has_local);

        let base_entry = remotes.iter().find(|r| r.branch == base).expect("origin/<base> should be listed");
        assert!(base_entry.has_local);
    }

    #[test]
    fn checkout_remote_branch_creates_and_switches_to_a_tracking_branch() {
        let local = init_repo_with_commit();
        let base = local.repo.head().unwrap().shorthand().unwrap().to_string();
        let bare = crate::git::test_support::init_bare_remote();
        crate::git::remotes::add_remote(local.path(), "origin".to_string(), bare.path().to_string_lossy().to_string())
            .unwrap();
        crate::git::remotes::push(local.path(), "origin".to_string(), base).unwrap();

        create_branch(local.path(), "feature-x".to_string()).unwrap();
        crate::git::remotes::push(local.path(), "origin".to_string(), "feature-x".to_string()).unwrap();
        delete_branch(local.path(), "feature-x".to_string(), true).unwrap();
        crate::git::remotes::fetch(local.path(), "origin".to_string()).unwrap();

        checkout_remote_branch(local.path(), "origin/feature-x".to_string(), "feature-x".to_string())
            .expect("checkout should succeed");

        let repo = Repository::open(local.path()).unwrap();
        assert_eq!(repo.head().unwrap().shorthand().unwrap(), "feature-x");
        let branch = repo.find_branch("feature-x", BranchType::Local).unwrap();
        let upstream = branch.upstream().unwrap();
        assert_eq!(upstream.name().unwrap().unwrap(), "origin/feature-x");
    }
}
