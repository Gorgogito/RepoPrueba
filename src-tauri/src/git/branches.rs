use super::err_msg;
use git2::{BranchType, Repository};
use serde::Serialize;

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

#[tauri::command]
pub fn checkout_branch(path: String, name: String) -> Result<(), String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    let refname = format!("refs/heads/{name}");

    let target = repo.revparse_single(&refname).map_err(err_msg)?;
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.safe();
    repo.checkout_tree(&target, Some(&mut checkout)).map_err(err_msg)?;
    repo.set_head(&refname).map_err(err_msg)?;
    Ok(())
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
}
