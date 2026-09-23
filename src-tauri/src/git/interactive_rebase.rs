use super::{ensure_clean_workdir, err_msg};
use git2::{Commit, Oid, Repository};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

const STATE_FILE: &str = "STASH_REBASE_TODO.json";

#[derive(Serialize, Clone)]
pub struct RebaseCommitInfo {
    oid: String,
    short_sha: String,
    summary: String,
    author: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RebaseStep {
    oid: String,
    action: String,
    message: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct RebaseState {
    original_branch_ref: String,
    original_tip_oid: String,
    onto_oid: String,
    current_head_oid: String,
    steps: Vec<RebaseStep>,
    current_index: usize,
    paused_reason: String,
}

fn state_path(repo: &Repository) -> PathBuf {
    repo.path().join(STATE_FILE)
}

pub(crate) fn load_state(repo: &Repository) -> Option<RebaseState> {
    let data = std::fs::read_to_string(state_path(repo)).ok()?;
    serde_json::from_str(&data).ok()
}

fn save_state(repo: &Repository, state: &RebaseState) -> Result<(), String> {
    let data = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(state_path(repo), data).map_err(|e| e.to_string())
}

fn clear_state(repo: &Repository) -> Result<(), String> {
    match std::fs::remove_file(state_path(repo)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Whether a rebase started by [`start_interactive_rebase`] is paused,
/// and why — checked by `conflicts::get_operation_status` before falling
/// back to `repo.state()`, since this engine never uses git2's own
/// Rebase/RebaseMerge state (see module docs below).
pub(crate) fn paused_reason(repo: &Repository) -> Option<String> {
    load_state(repo).map(|s| s.paused_reason)
}

fn find_commit<'a>(repo: &'a Repository, oid: Oid) -> Result<Commit<'a>, String> {
    repo.find_commit(oid).map_err(err_msg)
}

#[tauri::command]
pub fn get_rebase_commits(path: String, onto: String) -> Result<Vec<RebaseCommitInfo>, String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    let onto_oid = repo
        .revparse_single(&onto)
        .map_err(err_msg)?
        .peel_to_commit()
        .map_err(err_msg)?
        .id();
    let head_oid = repo.head().map_err(err_msg)?.peel_to_commit().map_err(err_msg)?.id();

    let mut walk = repo.revwalk().map_err(err_msg)?;
    walk.push(head_oid).map_err(err_msg)?;
    walk.hide(onto_oid).map_err(err_msg)?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE).map_err(err_msg)?;

    let mut commits = Vec::new();
    for oid in walk {
        let oid = oid.map_err(err_msg)?;
        let commit = repo.find_commit(oid).map_err(err_msg)?;
        let sha = oid.to_string();
        commits.push(RebaseCommitInfo {
            short_sha: sha[..7].to_string(),
            oid: sha,
            summary: commit.summary().ok().flatten().unwrap_or("").to_string(),
            author: commit.author().name().unwrap_or("").to_string(),
        });
    }
    Ok(commits)
}

/// Applies `step`'s action against a tree already resolved for it (either
/// a clean cherry-pick merge or a user-resolved conflict), producing the
/// new "current head" commit. Shared by the initial run and by resuming
/// after a conflict is resolved, so both paths agree on what pick/reword/
/// squash/fixup actually do.
fn finish_step(repo: &Repository, step: &RebaseStep, current_head_oid: Oid, tree_oid: Oid) -> Result<Oid, String> {
    let commit_oid = Oid::from_str(&step.oid).map_err(err_msg)?;
    let commit = find_commit(repo, commit_oid)?;
    let current_head_commit = find_commit(repo, current_head_oid)?;
    let sig = repo.signature().map_err(err_msg)?;
    let tree = repo.find_tree(tree_oid).map_err(err_msg)?;

    match step.action.as_str() {
        "pick" | "edit" => repo
            .commit(None, &commit.author(), &sig, commit.message().unwrap_or(""), &tree, &[&current_head_commit])
            .map_err(err_msg),
        "reword" => {
            let message = step.message.clone().unwrap_or_else(|| commit.message().unwrap_or("").to_string());
            repo.commit(None, &commit.author(), &sig, &message, &tree, &[&current_head_commit])
                .map_err(err_msg)
        }
        "squash" | "fixup" => {
            let parents: Vec<Commit> = current_head_commit.parents().collect();
            let parent_refs: Vec<&Commit> = parents.iter().collect();
            let message = if step.action == "fixup" {
                current_head_commit.message().unwrap_or("").to_string()
            } else {
                step.message.clone().unwrap_or_else(|| {
                    format!(
                        "{}\n\n{}",
                        current_head_commit.message().unwrap_or("").trim(),
                        commit.message().unwrap_or("").trim()
                    )
                })
            };
            repo.commit(None, &commit.author(), &sig, &message, &tree, &parent_refs).map_err(err_msg)
        }
        other => Err(format!("Acción de rebase desconocida: {other}")),
    }
}

fn checkout_clean_commit(repo: &Repository, oid: Oid) -> Result<(), String> {
    let commit = repo.find_commit(oid).map_err(err_msg)?;
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.force();
    repo.checkout_tree(commit.as_object(), Some(&mut checkout)).map_err(err_msg)?;
    let tree = commit.tree().map_err(err_msg)?;
    let mut index = repo.index().map_err(err_msg)?;
    index.read_tree(&tree).map_err(err_msg)?;
    index.write().map_err(err_msg)?;
    Ok(())
}

/// Runs `state.steps[state.current_index..]`. Returns `Ok(true)` once every
/// step has applied cleanly (the original branch is fast-forwarded to the
/// result and the state file is cleared), or `Ok(false)` if a step paused
/// (conflict, or an "edit" stop) — in which case the state file is left on
/// disk for `continue_after_resolution`/`abort` to pick up later.
///
/// Each non-dropped step is applied via `repo.cherrypick()` — the same
/// repo-level call `history_ops::cherry_pick` uses — against HEAD, which we
/// explicitly detach to `current_head_oid` first. That keeps everything on
/// the repository's real, on-disk index throughout (unlike the lower-level
/// `cherrypick_commit`, whose merge-result `Index` has no path and can't be
/// written — exactly what conflict resolution needs to do), so a paused
/// conflict looks and behaves just like any other conflicted operation.
fn run_steps(repo: &Repository, state: &mut RebaseState) -> Result<bool, String> {
    let mut current_head_oid = Oid::from_str(&state.current_head_oid).map_err(err_msg)?;
    let onto_oid = Oid::from_str(&state.onto_oid).map_err(err_msg)?;

    while state.current_index < state.steps.len() {
        let step = state.steps[state.current_index].clone();

        if step.action == "drop" {
            state.current_index += 1;
            continue;
        }
        if (step.action == "squash" || step.action == "fixup") && current_head_oid == onto_oid {
            return Err(
                "No se puede combinar el primer commit del plan: no hay un commit anterior".to_string(),
            );
        }

        checkout_clean_commit(repo, current_head_oid)?;
        repo.set_head_detached(current_head_oid).map_err(err_msg)?;

        let commit_oid = Oid::from_str(&step.oid).map_err(err_msg)?;
        let commit = find_commit(repo, commit_oid)?;
        repo.cherrypick(&commit, None).map_err(err_msg)?;

        let mut index = repo.index().map_err(err_msg)?;
        if index.has_conflicts() {
            state.current_head_oid = current_head_oid.to_string();
            state.paused_reason = "conflict".to_string();
            save_state(repo, state)?;
            return Ok(false);
        }

        let tree_oid = index.write_tree().map_err(err_msg)?;
        let new_oid = finish_step(repo, &step, current_head_oid, tree_oid)?;
        repo.cleanup_state().map_err(err_msg)?;
        state.current_index += 1;

        if step.action == "edit" {
            state.current_head_oid = new_oid.to_string();
            state.paused_reason = "edit".to_string();
            save_state(repo, state)?;
            checkout_clean_commit(repo, new_oid)?;
            repo.set_head_detached(new_oid).map_err(err_msg)?;
            return Ok(false);
        }

        current_head_oid = new_oid;
    }

    let mut branch_ref = repo.find_reference(&state.original_branch_ref).map_err(err_msg)?;
    branch_ref
        .set_target(current_head_oid, "stash: rebase interactivo")
        .map_err(err_msg)?;
    repo.set_head(&state.original_branch_ref).map_err(err_msg)?;
    checkout_clean_commit(repo, current_head_oid)?;
    clear_state(repo)?;
    Ok(true)
}

#[tauri::command]
pub fn start_interactive_rebase(path: String, onto: String, steps: Vec<RebaseStep>) -> Result<(), String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    ensure_clean_workdir(&repo)?;

    if load_state(&repo).is_some() {
        return Err("Ya hay un rebase interactivo en curso".to_string());
    }
    if repo.state() != git2::RepositoryState::Clean {
        return Err("El repositorio tiene otra operación en curso".to_string());
    }
    if steps.is_empty() {
        return Err("El plan de rebase está vacío".to_string());
    }

    let onto_oid = repo
        .revparse_single(&onto)
        .map_err(err_msg)?
        .peel_to_commit()
        .map_err(err_msg)?
        .id();

    let head_ref = repo.head().map_err(err_msg)?;
    if !head_ref.is_branch() {
        return Err("Necesitás estar en una rama (no HEAD desacoplado) para iniciar un rebase interactivo".to_string());
    }
    let original_branch_ref = head_ref.name().map_err(err_msg)?.to_string();
    let original_tip_oid = head_ref.peel_to_commit().map_err(err_msg)?.id();

    let mut walk = repo.revwalk().map_err(err_msg)?;
    walk.push(original_tip_oid).map_err(err_msg)?;
    walk.hide(onto_oid).map_err(err_msg)?;
    let expected: HashSet<String> = walk
        .map(|oid| oid.map(|o| o.to_string()).map_err(err_msg))
        .collect::<Result<_, _>>()?;
    let provided: HashSet<String> = steps.iter().map(|s| s.oid.clone()).collect();
    if expected != provided {
        return Err("El plan no coincide con los commits pendientes de rebase".to_string());
    }

    let mut state = RebaseState {
        original_branch_ref,
        original_tip_oid: original_tip_oid.to_string(),
        onto_oid: onto_oid.to_string(),
        current_head_oid: onto_oid.to_string(),
        steps,
        current_index: 0,
        paused_reason: String::new(),
    };

    run_steps(&repo, &mut state)?;
    Ok(())
}

/// Resumes a paused interactive rebase: finishes the current step (using
/// the now-conflict-free index for a "conflict" pause, or amending the
/// paused commit with whatever's staged for an "edit" pause) and keeps
/// running the remaining steps, pausing again if another one needs it.
pub(crate) fn continue_after_resolution(repo: &Repository) -> Result<(), String> {
    let mut state = load_state(repo).ok_or_else(|| "No hay un rebase interactivo en curso".to_string())?;
    let index = repo.index().map_err(err_msg)?;
    if index.has_conflicts() {
        return Err("Todavía hay conflictos sin resolver".to_string());
    }

    match state.paused_reason.as_str() {
        "conflict" => {
            let step = state.steps[state.current_index].clone();
            let current_head_oid = Oid::from_str(&state.current_head_oid).map_err(err_msg)?;
            let tree_oid = repo.index().map_err(err_msg)?.write_tree().map_err(err_msg)?;
            let new_oid = finish_step(repo, &step, current_head_oid, tree_oid)?;
            repo.cleanup_state().map_err(err_msg)?;
            state.current_index += 1;

            if step.action == "edit" {
                state.current_head_oid = new_oid.to_string();
                state.paused_reason = "edit".to_string();
                save_state(repo, &state)?;
                checkout_clean_commit(repo, new_oid)?;
                repo.set_head_detached(new_oid).map_err(err_msg)?;
                return Ok(());
            }
            state.current_head_oid = new_oid.to_string();
        }
        "edit" => {
            let current_head_oid = Oid::from_str(&state.current_head_oid).map_err(err_msg)?;
            let current_head_commit = find_commit(repo, current_head_oid)?;
            let staged_tree_oid = repo.index().map_err(err_msg)?.write_tree().map_err(err_msg)?;
            if staged_tree_oid != current_head_commit.tree_id() {
                let tree = repo.find_tree(staged_tree_oid).map_err(err_msg)?;
                let sig = repo.signature().map_err(err_msg)?;
                let parents: Vec<Commit> = current_head_commit.parents().collect();
                let parent_refs: Vec<&Commit> = parents.iter().collect();
                let new_oid = repo
                    .commit(
                        None,
                        &current_head_commit.author(),
                        &sig,
                        current_head_commit.message().unwrap_or(""),
                        &tree,
                        &parent_refs,
                    )
                    .map_err(err_msg)?;
                state.current_head_oid = new_oid.to_string();
            }
        }
        other => return Err(format!("Estado de pausa desconocido: {other}")),
    }

    run_steps(repo, &mut state)?;
    Ok(())
}

/// Discards a paused interactive rebase entirely, restoring the branch to
/// exactly where it was before `start_interactive_rebase` touched it.
pub(crate) fn abort(repo: &Repository) -> Result<(), String> {
    let state = load_state(repo).ok_or_else(|| "No hay un rebase interactivo en curso".to_string())?;
    let original_tip = Oid::from_str(&state.original_tip_oid).map_err(err_msg)?;
    let commit = repo.find_commit(original_tip).map_err(err_msg)?;

    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.force();
    repo.reset(commit.as_object(), git2::ResetType::Hard, Some(&mut checkout))
        .map_err(err_msg)?;
    repo.set_head(&state.original_branch_ref).map_err(err_msg)?;
    clear_state(repo)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::branches::checkout_branch;
    use crate::git::conflicts::{continue_operation, get_operation_status, resolve_conflict};
    use crate::git::test_support::init_repo_with_commit;

    fn step(oid: Oid, action: &str) -> RebaseStep {
        RebaseStep { oid: oid.to_string(), action: action.to_string(), message: None }
    }

    fn reword_step(oid: Oid, message: &str) -> RebaseStep {
        RebaseStep { oid: oid.to_string(), action: "reword".to_string(), message: Some(message.to_string()) }
    }

    #[test]
    fn lists_commits_between_onto_and_head_oldest_first() {
        let test_repo = init_repo_with_commit();
        let base = test_repo.repo.head().unwrap().peel_to_commit().unwrap().id().to_string();
        test_repo.write("a.txt", "a\n");
        test_repo.stage("a.txt");
        let first = test_repo.commit("first");
        test_repo.write("b.txt", "b\n");
        test_repo.stage("b.txt");
        let second = test_repo.commit("second");

        let commits = get_rebase_commits(test_repo.path(), base).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].oid, first.to_string());
        assert_eq!(commits[1].oid, second.to_string());
    }

    #[test]
    fn pick_reword_and_drop_apply_in_order() {
        let test_repo = init_repo_with_commit();
        let base_oid = test_repo.repo.head().unwrap().peel_to_commit().unwrap().id();
        let base = base_oid.to_string();

        test_repo.write("a.txt", "a\n");
        test_repo.stage("a.txt");
        let first = test_repo.commit("first");
        test_repo.write("b.txt", "b\n");
        test_repo.stage("b.txt");
        let second = test_repo.commit("second");
        test_repo.write("c.txt", "c\n");
        test_repo.stage("c.txt");
        let third = test_repo.commit("third");

        let steps = vec![step(first, "pick"), reword_step(second, "renamed second"), step(third, "drop")];
        start_interactive_rebase(test_repo.path(), base, steps).expect("rebase should complete");

        let repo = Repository::open(test_repo.path()).unwrap();
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        assert!(load_state(&repo).is_none());

        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.message().unwrap().trim(), "renamed second");
        // The re-picked "first" is a fresh commit (its committer timestamp
        // is set at rebase time), so compare identity by message/lineage
        // rather than assuming it reuses the pre-rebase oid.
        let parent = head.parent(0).unwrap();
        assert_eq!(parent.message().unwrap().trim(), "first");
        assert_eq!(parent.parent_id(0).unwrap(), base_oid);

        assert!(!test_repo.dir.path().join("c.txt").exists());
        assert!(test_repo.dir.path().join("a.txt").exists());
        assert!(test_repo.dir.path().join("b.txt").exists());
    }

    #[test]
    fn squash_combines_into_the_previous_step() {
        let test_repo = init_repo_with_commit();
        let base = test_repo.repo.head().unwrap().peel_to_commit().unwrap().id().to_string();

        test_repo.write("a.txt", "a\n");
        test_repo.stage("a.txt");
        let first = test_repo.commit("first");
        test_repo.write("b.txt", "b\n");
        test_repo.stage("b.txt");
        let second = test_repo.commit("second");

        start_interactive_rebase(test_repo.path(), base, vec![step(first, "pick"), step(second, "squash")])
            .expect("rebase should complete");

        let repo = Repository::open(test_repo.path()).unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.parent_count(), 1);
        assert!(head.message().unwrap().contains("first"));
        assert!(head.message().unwrap().contains("second"));
        assert!(test_repo.dir.path().join("a.txt").exists());
        assert!(test_repo.dir.path().join("b.txt").exists());
    }

    #[test]
    fn conflicting_pick_pauses_then_resolves_via_shared_conflict_ui() {
        let test_repo = init_repo_with_commit();
        let base_name = test_repo.repo.head().unwrap().shorthand().unwrap().to_string();

        crate::git::branches::create_branch(test_repo.path(), "feature".to_string()).unwrap();
        checkout_branch(test_repo.path(), "feature".to_string()).unwrap();
        test_repo.write("initial.txt", "from feature\n");
        test_repo.stage("initial.txt");
        let feature_commit = test_repo.commit("feature edits initial.txt");

        checkout_branch(test_repo.path(), base_name.clone()).unwrap();
        test_repo.write("initial.txt", "from base\n");
        test_repo.stage("initial.txt");
        test_repo.commit("base edits initial.txt");

        checkout_branch(test_repo.path(), "feature".to_string()).unwrap();

        let steps = vec![step(feature_commit, "pick")];
        start_interactive_rebase(test_repo.path(), base_name, steps).expect("start should not error");

        let status = get_operation_status(test_repo.path()).unwrap();
        assert_eq!(status.kind, "rebase");
        assert_eq!(status.paused_reason.as_deref(), Some("conflict"));
        assert_eq!(status.conflicts, vec!["initial.txt".to_string()]);

        resolve_conflict(test_repo.path(), "initial.txt".to_string(), "resolved\n".to_string()).unwrap();
        continue_operation(test_repo.path()).expect("continue should finish the rebase");

        let repo = Repository::open(test_repo.path()).unwrap();
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        assert!(load_state(&repo).is_none());
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.message().unwrap().trim(), "feature edits initial.txt");
        let contents = std::fs::read_to_string(test_repo.dir.path().join("initial.txt")).unwrap();
        assert_eq!(contents, "resolved\n");
    }

    #[test]
    fn edit_pauses_after_applying_then_amends_on_continue() {
        let test_repo = init_repo_with_commit();
        let base = test_repo.repo.head().unwrap().peel_to_commit().unwrap().id().to_string();

        test_repo.write("a.txt", "a\n");
        test_repo.stage("a.txt");
        let first = test_repo.commit("first");

        start_interactive_rebase(test_repo.path(), base, vec![step(first, "edit")]).expect("start should not error");

        let status = get_operation_status(test_repo.path()).unwrap();
        assert_eq!(status.kind, "rebase");
        assert_eq!(status.paused_reason.as_deref(), Some("edit"));
        assert!(status.conflicts.is_empty());

        // Amend the paused commit's content before continuing.
        test_repo.write("a.txt", "amended\n");
        crate::git::changes::stage_file(test_repo.path(), "a.txt".to_string()).unwrap();

        continue_operation(test_repo.path()).expect("continue should finish the rebase");

        let repo = Repository::open(test_repo.path()).unwrap();
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        assert!(load_state(&repo).is_none());
        let contents = std::fs::read_to_string(test_repo.dir.path().join("a.txt")).unwrap();
        assert_eq!(contents, "amended\n");
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.message().unwrap().trim(), "first");
    }

    #[test]
    fn abort_restores_the_branch_to_its_original_tip() {
        let test_repo = init_repo_with_commit();
        let base_name = test_repo.repo.head().unwrap().shorthand().unwrap().to_string();

        crate::git::branches::create_branch(test_repo.path(), "feature".to_string()).unwrap();
        checkout_branch(test_repo.path(), "feature".to_string()).unwrap();
        test_repo.write("initial.txt", "from feature\n");
        test_repo.stage("initial.txt");
        let feature_commit = test_repo.commit("feature edits initial.txt");
        let tip_before = test_repo.repo.head().unwrap().peel_to_commit().unwrap().id();

        checkout_branch(test_repo.path(), base_name.clone()).unwrap();
        test_repo.write("initial.txt", "from base\n");
        test_repo.stage("initial.txt");
        test_repo.commit("base edits initial.txt");

        checkout_branch(test_repo.path(), "feature".to_string()).unwrap();

        let steps = vec![step(feature_commit, "pick")];
        start_interactive_rebase(test_repo.path(), base_name, steps).expect("start should not error");
        assert_eq!(get_operation_status(test_repo.path()).unwrap().paused_reason.as_deref(), Some("conflict"));

        crate::git::conflicts::abort_operation(test_repo.path()).expect("abort should succeed");

        let repo = Repository::open(test_repo.path()).unwrap();
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        assert!(load_state(&repo).is_none());
        assert_eq!(repo.head().unwrap().shorthand().unwrap(), "feature");
        assert_eq!(repo.head().unwrap().peel_to_commit().unwrap().id(), tip_before);
        let status = crate::git::repo::get_repo_status(test_repo.path()).unwrap();
        assert!(status.is_clean);
    }
}
