pub mod branches;
pub mod changes;
pub mod conflicts;
pub mod diff;
pub mod history_ops;
pub mod interactive_rebase;
pub mod lfs;
pub mod log;
pub mod remotes;
pub mod repo;
pub mod stash;
pub mod submodules;
pub mod undo;
pub mod worktrees;
#[cfg(test)]
mod test_support;

pub(crate) fn err_msg(e: git2::Error) -> String {
    e.message().to_string()
}

/// Network/porcelain operations (fetch/pull/push/clone/worktree/submodule/
/// lfs) shell out to the system `git` instead of driving libgit2's
/// transport or replicating porcelain-only plumbing directly. This means
/// credential helpers (Git Credential Manager, SSH agents, proxies,
/// .netrc) all just work exactly as they do from a terminal, and we don't
/// have to reimplement behavior git's own CLI already gets right.
pub(crate) fn run_git(path: &str, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .map_err(|e| format!("No se pudo ejecutar git: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            format!("git {} falló", args.join(" "))
        } else {
            stderr
        })
    }
}

/// Refuses to proceed if the working tree or index has pending changes.
/// Used before operations that move HEAD/checkout a different tree
/// (branch switch, fast-forward, rebase) so we never depend on
/// checkout's own "safe" heuristics — we check, then force.
pub(crate) fn ensure_clean_workdir(repo: &git2::Repository) -> Result<(), String> {
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(false);
    let statuses = repo.statuses(Some(&mut opts)).map_err(err_msg)?;
    if statuses.is_empty() {
        Ok(())
    } else {
        Err("Hay cambios sin commitear: haz commit o stash antes de continuar".to_string())
    }
}
