use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

/// Holds the watcher for whichever repo is currently open. Replacing it
/// (on switching repos) drops the old one, which stops it — only one repo
/// is ever open at a time in the UI, so that's all we need.
#[derive(Default)]
pub struct WatcherState(Mutex<Option<RecommendedWatcher>>);

/// Only HEAD, refs/heads/*, refs/remotes/* and packed-refs actually mean
/// "the branch list or current branch may have changed" — everything else
/// under .git (the index, objects, lock files that come and go with every
/// git command) is noise we don't want to trigger a refresh for.
fn is_relevant(path: &std::path::Path) -> bool {
    if matches!(path.file_name().and_then(|f| f.to_str()), Some("HEAD") | Some("packed-refs")) {
        return true;
    }
    let comps: Vec<String> = path.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    comps.windows(2).any(|w| w[0] == "refs" && (w[1] == "heads" || w[1] == "remotes"))
}

/// Starts watching `path`'s `.git` directory for branch/HEAD changes,
/// emitting a `repo-changed` event to the frontend whenever one happens —
/// so switching branches from an external terminal, or any other tool,
/// shows up in the UI without the user having to do anything.
#[tauri::command]
pub fn watch_repository(app: AppHandle, state: tauri::State<WatcherState>, path: String) -> Result<(), String> {
    let git_dir = std::path::Path::new(&path).join(".git");
    if !git_dir.exists() {
        return Ok(());
    }

    let app_for_events = app.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        let Ok(event) = res else { return };
        if !matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)) {
            return;
        }
        if event.paths.iter().any(|p| is_relevant(p)) {
            let _ = app_for_events.emit("repo-changed", ());
        }
    })
    .map_err(|e| e.to_string())?;

    watcher.watch(&git_dir, RecursiveMode::Recursive).map_err(|e| e.to_string())?;

    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    *guard = Some(watcher);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn head_and_packed_refs_are_relevant() {
        assert!(is_relevant(&PathBuf::from("/repo/.git/HEAD")));
        assert!(is_relevant(&PathBuf::from("/repo/.git/packed-refs")));
    }

    #[test]
    fn branch_and_remote_refs_are_relevant() {
        assert!(is_relevant(&PathBuf::from("/repo/.git/refs/heads/feature-x")));
        assert!(is_relevant(&PathBuf::from("/repo/.git/refs/remotes/origin/feature-x")));
    }

    #[test]
    fn index_and_object_churn_is_not_relevant() {
        assert!(!is_relevant(&PathBuf::from("/repo/.git/index")));
        assert!(!is_relevant(&PathBuf::from("/repo/.git/index.lock")));
        assert!(!is_relevant(&PathBuf::from("/repo/.git/objects/ab/cdef1234")));
        assert!(!is_relevant(&PathBuf::from("/repo/.git/refs/tags/v1.0.0")));
    }
}
