pub mod branches;
pub mod changes;
pub mod conflicts;
pub mod diff;
pub mod history_ops;
pub mod interactive_rebase;
pub mod log;
pub mod remotes;
pub mod repo;
pub mod stash;
pub mod undo;
#[cfg(test)]
mod test_support;

pub(crate) fn err_msg(e: git2::Error) -> String {
    e.message().to_string()
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
