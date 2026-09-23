use super::err_msg;
use git2::Repository;
use serde::Serialize;
use std::process::Command;

#[derive(Serialize)]
pub struct RemoteInfo {
    name: String,
    url: String,
}

#[tauri::command]
pub fn list_remotes(path: String) -> Result<Vec<RemoteInfo>, String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    let names = repo.remotes().map_err(err_msg)?;

    let mut remotes = Vec::new();
    for name in names.iter() {
        let Ok(Some(name)) = name else { continue };
        if let Ok(remote) = repo.find_remote(name) {
            remotes.push(RemoteInfo {
                name: name.to_string(),
                url: remote.url().unwrap_or("").to_string(),
            });
        }
    }
    Ok(remotes)
}

#[tauri::command]
pub fn add_remote(path: String, name: String, url: String) -> Result<(), String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    repo.remote(&name, &url).map_err(err_msg)?;
    Ok(())
}

/// Network operations (fetch/pull/push) shell out to the system `git`
/// instead of driving libgit2's transport directly. This means credential
/// helpers (Git Credential Manager's GitHub/OAuth login on Windows), SSH
/// agents, proxies and .netrc all just work exactly as they do from a
/// terminal, with no need to vendor OpenSSL/libssh2 into this binary.
fn run_git(path: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
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

#[tauri::command]
pub fn fetch(path: String, remote_name: String) -> Result<(), String> {
    run_git(&path, &["fetch", &remote_name]).map(|_| ())
}

#[tauri::command]
pub fn pull(path: String, remote_name: String) -> Result<(), String> {
    // --ff-only: never silently create a merge commit or clobber local
    // work. A real divergence gets surfaced as an error for now; merging
    // it is the next milestone.
    run_git(&path, &["pull", "--ff-only", &remote_name]).map(|_| ())
}

#[tauri::command]
pub fn push(path: String, remote_name: String, branch: String) -> Result<(), String> {
    run_git(&path, &["push", "-u", &remote_name, &branch]).map(|_| ())
}

fn repo_name_from_url(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/').trim_end_matches(".git");
    let name = trimmed.rsplit(['/', '\\']).next().unwrap_or("");
    if name.is_empty() {
        "repo".to_string()
    } else {
        name.to_string()
    }
}

/// Clones `url` into a new folder (named after the repo) inside
/// `parent_dir`. Returns the resulting local path so the caller can open
/// it directly, without needing to parse git's "Cloning into '...'"
/// output to find out where it landed.
#[tauri::command]
pub fn clone_repository(url: String, parent_dir: String) -> Result<String, String> {
    let name = repo_name_from_url(&url);
    let dest = std::path::Path::new(&parent_dir).join(&name);
    if dest.exists() {
        return Err(format!("Ya existe una carpeta '{name}' en el destino"));
    }

    run_git(&parent_dir, &["clone", &url, &name])?;
    Ok(dest.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_support::{init_bare_remote, init_repo_with_commit, TestRepo};

    fn clone_bare(bare_path: &std::path::Path) -> TestRepo {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::clone(&bare_path.to_string_lossy(), dir.path()).unwrap();
        TestRepo { dir, repo }
    }

    #[test]
    fn add_and_list_remotes() {
        let test_repo = init_repo_with_commit();
        add_remote(test_repo.path(), "origin".to_string(), "https://example.invalid/repo.git".to_string())
            .unwrap();

        let remotes = list_remotes(test_repo.path()).unwrap();
        assert_eq!(remotes.len(), 1);
        assert_eq!(remotes[0].name, "origin");
        assert_eq!(remotes[0].url, "https://example.invalid/repo.git");
    }

    #[test]
    fn push_then_fetch_round_trip_via_local_bare_remote() {
        let local = init_repo_with_commit();
        let branch = local.repo.head().unwrap().shorthand().unwrap().to_string();

        let bare = init_bare_remote();
        add_remote(local.path(), "origin".to_string(), bare.path().to_string_lossy().to_string()).unwrap();

        push(local.path(), "origin".to_string(), branch.clone()).expect("push should succeed");

        let clone = clone_bare(bare.path());
        let clone_head = clone.repo.head().unwrap().peel_to_commit().unwrap();
        let local_head = local.repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(clone_head.id(), local_head.id());
    }

    #[test]
    fn pull_fast_forwards_when_behind() {
        let local = init_repo_with_commit();
        let branch = local.repo.head().unwrap().shorthand().unwrap().to_string();
        let bare = init_bare_remote();
        add_remote(local.path(), "origin".to_string(), bare.path().to_string_lossy().to_string()).unwrap();
        push(local.path(), "origin".to_string(), branch.clone()).unwrap();

        // Simulate a teammate pushing a new commit via a second clone.
        let teammate = clone_bare(bare.path());
        teammate.write("teammate.txt", "from teammate\n");
        teammate.stage("teammate.txt");
        teammate.commit("teammate's commit");
        push(teammate.path(), "origin".to_string(), branch.clone()).unwrap();

        pull(local.path(), "origin".to_string()).expect("pull should fast-forward");

        let local_head = local.repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(local_head.message().unwrap().trim(), "teammate's commit");
        assert!(local.dir.path().join("teammate.txt").exists());
    }

    #[test]
    fn pull_refuses_to_clobber_on_divergence() {
        let local = init_repo_with_commit();
        let branch = local.repo.head().unwrap().shorthand().unwrap().to_string();
        let bare = init_bare_remote();
        add_remote(local.path(), "origin".to_string(), bare.path().to_string_lossy().to_string()).unwrap();
        push(local.path(), "origin".to_string(), branch.clone()).unwrap();

        // Teammate pushes a commit...
        let teammate = clone_bare(bare.path());
        teammate.write("teammate.txt", "from teammate\n");
        teammate.stage("teammate.txt");
        teammate.commit("teammate's commit");
        push(teammate.path(), "origin".to_string(), branch.clone()).unwrap();

        // ...while we also commit locally, so histories diverge.
        local.write("mine.txt", "local work\n");
        local.stage("mine.txt");
        local.commit("my local commit");

        assert!(pull(local.path(), "origin".to_string()).is_err());

        // Local commit must still be intact (nothing was clobbered).
        let local_head = local.repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(local_head.message().unwrap().trim(), "my local commit");
    }

    #[test]
    fn repo_name_from_url_handles_common_forms() {
        assert_eq!(repo_name_from_url("https://github.com/user/repo.git"), "repo");
        assert_eq!(repo_name_from_url("https://github.com/user/repo"), "repo");
        assert_eq!(repo_name_from_url("git@github.com:user/repo.git"), "repo");
        assert_eq!(repo_name_from_url("https://github.com/user/repo/"), "repo");
        assert_eq!(repo_name_from_url("C:\\repos\\local-repo"), "local-repo");
    }

    #[test]
    fn clone_repository_creates_a_working_copy_at_the_returned_path() {
        let source = init_repo_with_commit();
        let bare = init_bare_remote();
        let branch = source.repo.head().unwrap().shorthand().unwrap().to_string();
        add_remote(source.path(), "origin".to_string(), bare.path().to_string_lossy().to_string()).unwrap();
        push(source.path(), "origin".to_string(), branch).unwrap();

        let parent = tempfile::TempDir::new().unwrap();
        let bare_url = bare.path().to_string_lossy().to_string();
        let result_path = clone_repository(bare_url, parent.path().to_string_lossy().to_string()).unwrap();

        let cloned = Repository::open(&result_path).unwrap();
        let cloned_head = cloned.head().unwrap().peel_to_commit().unwrap();
        let source_head = source.repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(cloned_head.id(), source_head.id());
        assert!(std::path::Path::new(&result_path).join("initial.txt").exists());
    }

    #[test]
    fn clone_repository_refuses_when_destination_already_exists() {
        let bare = init_bare_remote();
        let parent = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(parent.path().join(bare.path().file_name().unwrap())).unwrap();

        let bare_url = bare.path().to_string_lossy().to_string();
        let err = clone_repository(bare_url, parent.path().to_string_lossy().to_string()).unwrap_err();
        assert!(err.contains("Ya existe"));
    }
}
