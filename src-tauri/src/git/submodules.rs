use super::{err_msg, run_git};
use git2::Repository;
use serde::Serialize;

#[derive(Serialize)]
pub struct SubmoduleInfo {
    name: String,
    path: String,
    url: Option<String>,
    /// The commit the parent repo currently pins this submodule to — from
    /// the index (so this reflects `add`/`update` immediately, without
    /// needing a commit first) rather than HEAD.
    pinned_id: Option<String>,
    workdir_id: Option<String>,
    is_initialized: bool,
    is_up_to_date: bool,
}

#[tauri::command]
pub fn list_submodules(path: String) -> Result<Vec<SubmoduleInfo>, String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    let submodules = repo.submodules().map_err(err_msg)?;

    let mut result = Vec::new();
    for sm in &submodules {
        let pinned_id = sm.index_id().map(|o| o.to_string());
        // workdir_id is None both when the submodule was never cloned into
        // the working tree AND when it's cloned but has no commits, so we
        // treat "has a checked-out working tree" as the initialized signal
        // instead (open() only succeeds once the submodule has a .git).
        let workdir_id = sm.workdir_id().map(|o| o.to_string());
        let is_initialized = sm.open().is_ok();
        result.push(SubmoduleInfo {
            name: sm.name().map_err(err_msg)?.to_string(),
            path: sm.path().to_string_lossy().to_string(),
            url: sm.url().map_err(err_msg)?.map(|s| s.to_string()),
            is_initialized,
            is_up_to_date: is_initialized && pinned_id.is_some() && pinned_id == workdir_id,
            pinned_id,
            workdir_id,
        });
    }
    Ok(result)
}

#[tauri::command]
pub fn add_submodule(path: String, url: String, submodule_path: String) -> Result<(), String> {
    run_git(&path, &["submodule", "add", &url, &submodule_path]).map(|_| ())
}

#[tauri::command]
pub fn update_submodules(path: String, init: bool, recursive: bool) -> Result<(), String> {
    let mut args = vec!["submodule", "update"];
    if init {
        args.push("--init");
    }
    if recursive {
        args.push("--recursive");
    }
    run_git(&path, &args).map(|_| ())
}

#[tauri::command]
pub fn sync_submodules(path: String) -> Result<(), String> {
    run_git(&path, &["submodule", "sync", "--recursive"]).map(|_| ())
}

#[tauri::command]
pub fn deinit_submodule(path: String, submodule_path: String, force: bool) -> Result<(), String> {
    let mut args = vec!["submodule", "deinit"];
    if force {
        args.push("--force");
    }
    args.push(&submodule_path);
    run_git(&path, &args).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_support::{init_bare_remote, init_repo_with_commit, TestRepo};
    use crate::git::remotes::push;

    /// Git refuses to clone submodules over the `file://`/local-path
    /// transport by default (CVE-2022-39253 hardening) — repo-local
    /// `protocol.file.allow` config isn't consulted by the nested clone
    /// `git submodule add`/`update` spawns, but the `GIT_ALLOW_PROTOCOL`
    /// env var is, so that's what a real user hitting this with a
    /// local-path submodule would also need to set. The env var is
    /// process-global, so a lock held for the guard's lifetime keeps this
    /// from racing with any other test using it concurrently.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[allow(dead_code)] // held only for its Drop / mutual-exclusion side effect
    struct AllowLocalSubmoduleTransport(std::sync::MutexGuard<'static, ()>);

    impl AllowLocalSubmoduleTransport {
        fn scoped() -> Self {
            let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            std::env::set_var("GIT_ALLOW_PROTOCOL", "file");
            AllowLocalSubmoduleTransport(guard)
        }
    }

    impl Drop for AllowLocalSubmoduleTransport {
        fn drop(&mut self) {
            std::env::remove_var("GIT_ALLOW_PROTOCOL");
        }
    }

    fn bare_url_with_a_commit() -> (String, tempfile::TempDir) {
        let source = init_repo_with_commit();
        let bare = init_bare_remote();
        let branch = source.repo.head().unwrap().shorthand().unwrap().to_string();
        crate::git::remotes::add_remote(source.path(), "origin".to_string(), bare.path().to_string_lossy().to_string())
            .unwrap();
        push(source.path(), "origin".to_string(), branch).unwrap();
        (bare.path().to_string_lossy().to_string(), bare)
    }

    #[test]
    fn add_submodule_clones_it_and_registers_it_as_up_to_date() {
        let parent = init_repo_with_commit();
        let (bare_url, _bare) = bare_url_with_a_commit();
        let _allow = AllowLocalSubmoduleTransport::scoped();

        add_submodule(parent.path(), bare_url, "vendor/lib".to_string()).expect("add should succeed");

        let submodules = list_submodules(parent.path()).unwrap();
        assert_eq!(submodules.len(), 1);
        assert_eq!(submodules[0].path, "vendor/lib");
        assert!(submodules[0].is_initialized);
        assert!(submodules[0].is_up_to_date);
        assert!(parent.dir.path().join("vendor/lib/initial.txt").exists());
    }

    #[test]
    fn update_init_populates_a_submodule_after_a_fresh_clone() {
        let parent = init_repo_with_commit();
        let (bare_url, _bare) = bare_url_with_a_commit();
        let _allow = AllowLocalSubmoduleTransport::scoped();
        add_submodule(parent.path(), bare_url, "vendor/lib".to_string()).unwrap();
        parent.stage(".gitmodules");
        parent.stage("vendor/lib");
        parent.commit("add vendor/lib submodule");

        // A plain clone never auto-initializes submodules, mirroring what
        // a teammate pulling this repo for the first time would see.
        let clone_dir = tempfile::TempDir::new().unwrap();
        let cloned = Repository::clone(&parent.path(), clone_dir.path()).unwrap();
        let clone_repo = TestRepo { dir: clone_dir, repo: cloned };

        let before = list_submodules(clone_repo.path()).unwrap();
        assert_eq!(before.len(), 1);
        assert!(!before[0].is_initialized);

        update_submodules(clone_repo.path(), true, false).expect("update --init should succeed");

        let after = list_submodules(clone_repo.path()).unwrap();
        assert!(after[0].is_initialized);
        assert!(after[0].is_up_to_date);
        assert!(clone_repo.dir.path().join("vendor/lib/initial.txt").exists());
    }
}
