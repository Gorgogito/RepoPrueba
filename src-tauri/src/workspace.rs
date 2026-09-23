use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct RepoEntry {
    path: String,
    name: String,
}

fn repo_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string()
}

/// Adds `path` to the list if not already present. No-op if it is.
fn add_repo_entry(repos: &mut Vec<RepoEntry>, path: String) {
    if repos.iter().any(|r| r.path == path) {
        return;
    }
    repos.push(RepoEntry { name: repo_name(&path), path });
}

fn remove_repo_entry(repos: &mut Vec<RepoEntry>, path: &str) {
    repos.retain(|r| r.path != path);
}

fn store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("repos.json"))
}

fn read_repos(app: &AppHandle) -> Result<Vec<RepoEntry>, String> {
    let path = store_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&data).map_err(|e| e.to_string())
}

fn write_repos(app: &AppHandle, repos: &[RepoEntry]) -> Result<(), String> {
    let path = store_path(app)?;
    let data = serde_json::to_string_pretty(repos).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_known_repos(app: AppHandle) -> Result<Vec<RepoEntry>, String> {
    read_repos(&app)
}

#[tauri::command]
pub fn add_known_repo(app: AppHandle, path: String) -> Result<Vec<RepoEntry>, String> {
    let mut repos = read_repos(&app)?;
    add_repo_entry(&mut repos, path);
    write_repos(&app, &repos)?;
    Ok(repos)
}

#[tauri::command]
pub fn remove_known_repo(app: AppHandle, path: String) -> Result<Vec<RepoEntry>, String> {
    let mut repos = read_repos(&app)?;
    remove_repo_entry(&mut repos, &path);
    write_repos(&app, &repos)?;
    Ok(repos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_repo_entry_dedupes_by_path() {
        let mut repos = Vec::new();
        add_repo_entry(&mut repos, "C:\\repos\\one".to_string());
        add_repo_entry(&mut repos, "C:\\repos\\one".to_string());
        add_repo_entry(&mut repos, "C:\\repos\\two".to_string());

        assert_eq!(repos.len(), 2);
        assert_eq!(repos[0].name, "one");
        assert_eq!(repos[1].name, "two");
    }

    #[test]
    fn remove_repo_entry_removes_only_the_matching_path() {
        let mut repos = vec![
            RepoEntry { path: "C:\\repos\\one".to_string(), name: "one".to_string() },
            RepoEntry { path: "C:\\repos\\two".to_string(), name: "two".to_string() },
        ];
        remove_repo_entry(&mut repos, "C:\\repos\\one");

        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].path, "C:\\repos\\two");
    }
}
