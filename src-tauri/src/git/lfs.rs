use super::run_git;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct LfsStatus {
    available: bool,
    tracked_patterns: Vec<String>,
}

#[derive(Serialize)]
pub struct LfsFileInfo {
    path: String,
    oid: String,
    fetched: bool,
}

fn lfs_available() -> bool {
    std::process::Command::new("git")
        .args(["lfs", "version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn require_lfs() -> Result<(), String> {
    if lfs_available() {
        Ok(())
    } else {
        Err("Git LFS no está instalado. Instálalo desde git-lfs.com y vuelve a intentar".to_string())
    }
}

/// Reads `.gitattributes` directly instead of shelling to `git lfs track`,
/// since listing what's tracked never actually needs the extension
/// installed — only parsing the attributes file it writes does.
fn tracked_patterns_from_gitattributes(repo_path: &str) -> Vec<String> {
    let content = match std::fs::read_to_string(Path::new(repo_path).join(".gitattributes")) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let mut parts = line.split_whitespace();
            let pattern = parts.next()?;
            parts.any(|attr| attr == "filter=lfs").then(|| pattern.to_string())
        })
        .collect()
}

#[tauri::command]
pub fn get_lfs_status(path: String) -> LfsStatus {
    LfsStatus { available: lfs_available(), tracked_patterns: tracked_patterns_from_gitattributes(&path) }
}

#[tauri::command]
pub fn lfs_track(path: String, pattern: String) -> Result<(), String> {
    require_lfs()?;
    run_git(&path, &["lfs", "track", &pattern]).map(|_| ())
}

#[tauri::command]
pub fn lfs_untrack(path: String, pattern: String) -> Result<(), String> {
    require_lfs()?;
    run_git(&path, &["lfs", "untrack", &pattern]).map(|_| ())
}

#[tauri::command]
pub fn lfs_install(path: String) -> Result<(), String> {
    require_lfs()?;
    run_git(&path, &["lfs", "install", "--local"]).map(|_| ())
}

#[tauri::command]
pub fn lfs_pull(path: String) -> Result<(), String> {
    require_lfs()?;
    run_git(&path, &["lfs", "pull"]).map(|_| ())
}

/// Parses `git lfs ls-files -l`: `<oid> <status-char> <path>` per line,
/// where the status column holds `*` once the object's content has been
/// fetched and `-` when only the pointer is present locally.
fn parse_ls_files(output: &str) -> Vec<LfsFileInfo> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, ' ');
            let oid = parts.next()?.to_string();
            let status = parts.next()?;
            let path = parts.next()?.to_string();
            Some(LfsFileInfo { oid, fetched: status.contains('*'), path })
        })
        .collect()
}

#[tauri::command]
pub fn lfs_list_files(path: String) -> Result<Vec<LfsFileInfo>, String> {
    if !lfs_available() {
        return Ok(Vec::new());
    }
    let output = run_git(&path, &["lfs", "ls-files", "-l"])?;
    Ok(parse_ls_files(&output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_support::init_repo_with_commit;

    #[test]
    fn tracked_patterns_reads_filter_lfs_lines_from_gitattributes() {
        let test_repo = init_repo_with_commit();
        test_repo.write(
            ".gitattributes",
            "*.psd filter=lfs diff=lfs merge=lfs -text\n*.txt text\n# a comment\ndata/*.bin filter=lfs\n",
        );

        let status = get_lfs_status(test_repo.path());
        assert_eq!(status.tracked_patterns, vec!["*.psd".to_string(), "data/*.bin".to_string()]);
    }

    #[test]
    fn status_has_no_tracked_patterns_without_a_gitattributes_file() {
        let test_repo = init_repo_with_commit();
        let status = get_lfs_status(test_repo.path());
        assert!(status.tracked_patterns.is_empty());
    }

    #[test]
    fn parses_ls_files_output_into_oid_status_and_path() {
        let output = "4d7a2146b2 * data/large.bin\nabc123 - data/pointer-only.bin\n";
        let files = parse_ls_files(output);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "data/large.bin");
        assert!(files[0].fetched);
        assert!(!files[1].fetched);
    }

    #[test]
    fn mutating_lfs_commands_refuse_clearly_when_the_extension_is_missing() {
        // This machine doesn't have git-lfs installed; skip rather than
        // fail outright if the environment running this test does.
        if lfs_available() {
            return;
        }
        let test_repo = init_repo_with_commit();
        let err = lfs_track(test_repo.path(), "*.psd".to_string()).unwrap_err();
        assert!(err.contains("no está instalado"));
    }
}
