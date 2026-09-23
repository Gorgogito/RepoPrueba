use super::{ensure_clean_workdir, err_msg};
use git2::{BranchType, Repository};

fn current_branch_name(repo: &Repository) -> Result<String, String> {
    let head = repo.head().map_err(err_msg)?;
    Ok(head.shorthand().map_err(err_msg)?.to_string())
}

/// Merges `branch` into the current branch. Returns "up-to-date",
/// "fast-forward" or "merge-commit" describing what happened.
#[tauri::command]
pub fn merge_branch(path: String, branch: String) -> Result<String, String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    ensure_clean_workdir(&repo)?;
    let their_branch = repo.find_branch(&branch, BranchType::Local).map_err(err_msg)?;
    let their_oid = their_branch
        .get()
        .target()
        .ok_or_else(|| "La rama no tiene commits".to_string())?;
    let their_commit = repo.find_annotated_commit(their_oid).map_err(err_msg)?;

    let (analysis, _) = repo.merge_analysis(&[&their_commit]).map_err(err_msg)?;

    if analysis.is_up_to_date() {
        return Ok("up-to-date".to_string());
    }

    let branch_name = current_branch_name(&repo)?;

    if analysis.is_fast_forward() {
        let their_commit_obj = repo.find_commit(their_oid).map_err(err_msg)?;
        let mut checkout = git2::build::CheckoutBuilder::new();
        checkout.force();
        repo.checkout_tree(their_commit_obj.as_object(), Some(&mut checkout))
            .map_err(err_msg)?;

        let refname = format!("refs/heads/{branch_name}");
        let mut reference = repo.find_reference(&refname).map_err(err_msg)?;
        reference.set_target(their_oid, "stash: fast-forward merge").map_err(err_msg)?;

        let target_tree = their_commit_obj.tree().map_err(err_msg)?;
        let mut index = repo.index().map_err(err_msg)?;
        index.read_tree(&target_tree).map_err(err_msg)?;
        index.write().map_err(err_msg)?;

        return Ok("fast-forward".to_string());
    }

    let mut merge_checkout = git2::build::CheckoutBuilder::new();
    merge_checkout.force();
    repo.merge(&[&their_commit], None, Some(&mut merge_checkout)).map_err(err_msg)?;

    let mut index = repo.index().map_err(err_msg)?;
    if index.has_conflicts() {
        return Err(format!(
            "El merge de '{branch}' tiene conflictos que resolver manualmente"
        ));
    }

    let tree_oid = index.write_tree().map_err(err_msg)?;
    let tree = repo.find_tree(tree_oid).map_err(err_msg)?;
    let sig = repo.signature().map_err(err_msg)?;
    let head_commit = repo.head().map_err(err_msg)?.peel_to_commit().map_err(err_msg)?;
    let their_commit_obj = repo.find_commit(their_oid).map_err(err_msg)?;

    let message = format!("Merge branch '{branch}' into {branch_name}");
    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        &message,
        &tree,
        &[&head_commit, &their_commit_obj],
    )
    .map_err(err_msg)?;
    repo.cleanup_state().map_err(err_msg)?;

    Ok("merge-commit".to_string())
}

#[tauri::command]
pub fn cherry_pick(path: String, commit_sha: String) -> Result<(), String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    let oid = git2::Oid::from_str(&commit_sha).map_err(err_msg)?;
    let commit = repo.find_commit(oid).map_err(err_msg)?;

    repo.cherrypick(&commit, None).map_err(err_msg)?;

    let mut index = repo.index().map_err(err_msg)?;
    if index.has_conflicts() {
        return Err("El cherry-pick tiene conflictos que resolver manualmente".to_string());
    }

    let tree_oid = index.write_tree().map_err(err_msg)?;
    let tree = repo.find_tree(tree_oid).map_err(err_msg)?;
    let sig = repo.signature().map_err(err_msg)?;
    let head_commit = repo.head().map_err(err_msg)?.peel_to_commit().map_err(err_msg)?;

    let message = commit.message().unwrap_or("cherry-pick").to_string();
    repo.commit(Some("HEAD"), &commit.author(), &sig, &message, &tree, &[&head_commit])
        .map_err(err_msg)?;
    repo.cleanup_state().map_err(err_msg)?;
    Ok(())
}

#[tauri::command]
pub fn revert_commit(path: String, commit_sha: String) -> Result<(), String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    let oid = git2::Oid::from_str(&commit_sha).map_err(err_msg)?;
    let commit = repo.find_commit(oid).map_err(err_msg)?;

    repo.revert(&commit, None).map_err(err_msg)?;

    let mut index = repo.index().map_err(err_msg)?;
    if index.has_conflicts() {
        return Err("El revert tiene conflictos que resolver manualmente".to_string());
    }

    let tree_oid = index.write_tree().map_err(err_msg)?;
    let tree = repo.find_tree(tree_oid).map_err(err_msg)?;
    let sig = repo.signature().map_err(err_msg)?;
    let head_commit = repo.head().map_err(err_msg)?.peel_to_commit().map_err(err_msg)?;

    let message = format!("Revert \"{}\"", commit.summary().ok().flatten().unwrap_or(""));
    repo.commit(Some("HEAD"), &sig, &sig, &message, &tree, &[&head_commit])
        .map_err(err_msg)?;
    repo.cleanup_state().map_err(err_msg)?;
    Ok(())
}

/// Rebases the current branch onto `onto_branch`. If any step conflicts,
/// the whole rebase is aborted (not left half-done) and an error is
/// returned — resolving mid-rebase conflicts needs the conflict-resolution
/// UI, which doesn't exist yet.
#[tauri::command]
pub fn rebase_branch(path: String, onto_branch: String) -> Result<(), String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    ensure_clean_workdir(&repo)?;

    let onto_ref = repo.find_branch(&onto_branch, BranchType::Local).map_err(err_msg)?;
    let onto_oid = onto_ref
        .get()
        .target()
        .ok_or_else(|| "La rama destino no tiene commits".to_string())?;
    let onto_annotated = repo.find_annotated_commit(onto_oid).map_err(err_msg)?;

    let mut rebase = repo.rebase(None, None, Some(&onto_annotated), None).map_err(err_msg)?;
    let sig = repo.signature().map_err(err_msg)?;

    while let Some(op) = rebase.next() {
        if let Err(e) = op {
            let _ = rebase.abort();
            return Err(err_msg(e));
        }

        let index = repo.index().map_err(err_msg)?;
        if index.has_conflicts() {
            let _ = rebase.abort();
            return Err(
                "El rebase tiene conflictos; se abortó para no dejar el repo a medio resolver".to_string(),
            );
        }

        match rebase.commit(None, &sig, None) {
            Ok(_) => {}
            Err(e) if e.code() == git2::ErrorCode::Applied => {}
            Err(e) => {
                let _ = rebase.abort();
                return Err(err_msg(e));
            }
        }
    }

    rebase.finish(Some(&sig)).map_err(err_msg)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::branches::{checkout_branch, create_branch};
    use crate::git::changes::{commit, stage_file};
    use crate::git::test_support::init_repo_with_commit;

    #[test]
    fn merge_fast_forwards_when_possible() {
        let test_repo = init_repo_with_commit();
        let base = test_repo.repo.head().unwrap().shorthand().unwrap().to_string();

        create_branch(test_repo.path(), "feature".to_string()).unwrap();
        checkout_branch(test_repo.path(), "feature".to_string()).unwrap();
        test_repo.write("feature.txt", "new\n");
        test_repo.stage("feature.txt");
        test_repo.commit("feature work");

        checkout_branch(test_repo.path(), base).unwrap();
        let result = merge_branch(test_repo.path(), "feature".to_string()).unwrap();
        assert_eq!(result, "fast-forward");
        assert!(test_repo.dir.path().join("feature.txt").exists());
    }

    #[test]
    fn merge_creates_merge_commit_on_divergent_history() {
        let test_repo = init_repo_with_commit();
        let base = test_repo.repo.head().unwrap().shorthand().unwrap().to_string();

        create_branch(test_repo.path(), "feature".to_string()).unwrap();
        checkout_branch(test_repo.path(), "feature".to_string()).unwrap();
        test_repo.write("feature.txt", "new\n");
        test_repo.stage("feature.txt");
        test_repo.commit("feature work");

        checkout_branch(test_repo.path(), base).unwrap();
        test_repo.write("base.txt", "base work\n");
        test_repo.stage("base.txt");
        test_repo.commit("base work");

        let result = merge_branch(test_repo.path(), "feature".to_string()).unwrap();
        assert_eq!(result, "merge-commit");
        assert!(test_repo.dir.path().join("feature.txt").exists());
        assert!(test_repo.dir.path().join("base.txt").exists());

        let head = test_repo.repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.parent_count(), 2);
    }

    #[test]
    fn merge_reports_conflicts_without_committing() {
        let test_repo = init_repo_with_commit();
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

        let head_before = test_repo.repo.head().unwrap().peel_to_commit().unwrap().id();
        let err = merge_branch(test_repo.path(), "feature".to_string()).unwrap_err();
        assert!(err.contains("conflictos"));

        // HEAD must not have moved / a merge commit must not have been created.
        let head_after = Repository::open(test_repo.path())
            .unwrap()
            .head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .id();
        assert_eq!(head_before, head_after);
    }

    #[test]
    fn cherry_pick_applies_commit_onto_current_branch() {
        let test_repo = init_repo_with_commit();
        let base = test_repo.repo.head().unwrap().shorthand().unwrap().to_string();

        create_branch(test_repo.path(), "feature".to_string()).unwrap();
        checkout_branch(test_repo.path(), "feature".to_string()).unwrap();
        test_repo.write("feature.txt", "cherry\n");
        test_repo.stage("feature.txt");
        let picked_oid = test_repo.commit("cherry commit");

        checkout_branch(test_repo.path(), base).unwrap();
        assert!(!test_repo.dir.path().join("feature.txt").exists());

        cherry_pick(test_repo.path(), picked_oid.to_string()).expect("cherry-pick should succeed");
        assert!(test_repo.dir.path().join("feature.txt").exists());

        let head = test_repo.repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.message().unwrap().trim(), "cherry commit");
    }

    #[test]
    fn revert_undoes_a_commit_with_a_new_commit() {
        let test_repo = init_repo_with_commit();
        test_repo.write("initial.txt", "changed\n");
        stage_file(test_repo.path(), "initial.txt".to_string()).unwrap();
        let oid = commit(test_repo.path(), "change initial.txt".to_string()).unwrap();

        revert_commit(test_repo.path(), oid).expect("revert should succeed");

        let contents = std::fs::read_to_string(test_repo.dir.path().join("initial.txt")).unwrap();
        assert_eq!(contents, "hello\n");

        let head = test_repo.repo.head().unwrap().peel_to_commit().unwrap();
        assert!(head.message().unwrap().starts_with("Revert"));
    }

    #[test]
    fn rebase_replays_commits_onto_new_base() {
        let test_repo = init_repo_with_commit();
        let base = test_repo.repo.head().unwrap().shorthand().unwrap().to_string();

        create_branch(test_repo.path(), "feature".to_string()).unwrap();
        checkout_branch(test_repo.path(), "feature".to_string()).unwrap();
        test_repo.write("feature.txt", "new\n");
        test_repo.stage("feature.txt");
        test_repo.commit("feature work");

        checkout_branch(test_repo.path(), base.clone()).unwrap();
        test_repo.write("base.txt", "base work\n");
        test_repo.stage("base.txt");
        test_repo.commit("base work");

        checkout_branch(test_repo.path(), "feature".to_string()).unwrap();
        rebase_branch(test_repo.path(), base).expect("rebase should succeed");

        assert!(test_repo.dir.path().join("base.txt").exists());
        assert!(test_repo.dir.path().join("feature.txt").exists());

        let head = test_repo.repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.message().unwrap().trim(), "feature work");
        assert_eq!(head.parent_count(), 1);
    }

    #[test]
    fn rebase_aborts_cleanly_on_conflict() {
        let test_repo = init_repo_with_commit();
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

        checkout_branch(test_repo.path(), "feature".to_string()).unwrap();
        let head_before = test_repo.repo.head().unwrap().peel_to_commit().unwrap().id();

        let err = rebase_branch(test_repo.path(), base).unwrap_err();
        assert!(err.contains("conflictos"));

        // The rebase must have been aborted: branch tip unchanged, no
        // in-progress rebase state left behind.
        let repo = Repository::open(test_repo.path()).unwrap();
        let head_after = repo.head().unwrap().peel_to_commit().unwrap().id();
        assert_eq!(head_before, head_after);
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
    }
}
