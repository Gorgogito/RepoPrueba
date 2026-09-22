use super::err_msg;
use git2::Repository;
use std::path::Path;

#[tauri::command]
pub fn stage_file(path: String, file: String) -> Result<(), String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    let mut index = repo.index().map_err(err_msg)?;

    let full_path = Path::new(&path).join(&file);
    if full_path.exists() {
        index.add_path(Path::new(&file)).map_err(err_msg)?;
    } else {
        index.remove_path(Path::new(&file)).map_err(err_msg)?;
    }
    index.write().map_err(err_msg)?;
    Ok(())
}

#[tauri::command]
pub fn unstage_file(path: String, file: String) -> Result<(), String> {
    let repo = Repository::open(&path).map_err(err_msg)?;

    match repo.head() {
        Ok(head) => {
            let head_commit = head.peel_to_commit().map_err(err_msg)?;
            repo.reset_default(Some(head_commit.as_object()), [file.as_str()])
                .map_err(err_msg)?;
        }
        Err(_) => {
            // No commits yet: unstaging just means dropping it from the index.
            let mut index = repo.index().map_err(err_msg)?;
            index.remove_path(Path::new(&file)).map_err(err_msg)?;
            index.write().map_err(err_msg)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn commit(path: String, message: String) -> Result<String, String> {
    let repo = Repository::open(&path).map_err(err_msg)?;
    let sig = repo.signature().map_err(err_msg)?;

    let mut index = repo.index().map_err(err_msg)?;
    let tree_oid = index.write_tree().map_err(err_msg)?;
    let tree = repo.find_tree(tree_oid).map_err(err_msg)?;

    let parent = match repo.head() {
        Ok(head) => Some(head.peel_to_commit().map_err(err_msg)?),
        Err(_) => None,
    };

    match &parent {
        Some(p) if p.tree_id() == tree_oid => {
            return Err("No hay cambios en stage para commitear".to_string());
        }
        None if index.is_empty() => {
            return Err("El área de stage está vacía".to_string());
        }
        _ => {}
    }

    let parents: Vec<&git2::Commit> = parent.iter().collect();
    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, &message, &tree, &parents)
        .map_err(err_msg)?;
    Ok(oid.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::repo::get_repo_status;
    use crate::git::test_support::init_repo_with_commit;

    #[test]
    fn stage_then_commit_clears_status() {
        let test_repo = init_repo_with_commit();
        test_repo.write("initial.txt", "changed\n");
        test_repo.write("new.txt", "new\n");

        stage_file(test_repo.path(), "initial.txt".to_string()).unwrap();
        stage_file(test_repo.path(), "new.txt".to_string()).unwrap();

        let status = get_repo_status(test_repo.path()).unwrap();
        assert_eq!(status.changes.iter().filter(|c| c.staged).count(), 2);

        let oid = commit(test_repo.path(), "stage and commit".to_string()).expect("commit should succeed");
        assert_eq!(oid.len(), 40);

        let status = get_repo_status(test_repo.path()).unwrap();
        assert!(status.is_clean);
    }

    #[test]
    fn unstage_reverts_to_head_version() {
        let test_repo = init_repo_with_commit();
        test_repo.write("initial.txt", "changed\n");
        stage_file(test_repo.path(), "initial.txt".to_string()).unwrap();

        unstage_file(test_repo.path(), "initial.txt".to_string()).unwrap();

        let status = get_repo_status(test_repo.path()).unwrap();
        let entry = status.changes.iter().find(|c| c.path == "initial.txt").unwrap();
        assert!(!entry.staged);
        assert_eq!(entry.status, "modified");
    }

    #[test]
    fn commit_without_staged_changes_fails() {
        let test_repo = init_repo_with_commit();
        let err = commit(test_repo.path(), "empty".to_string()).unwrap_err();
        assert!(err.contains("stage"));
    }

    #[test]
    fn staging_a_deleted_file_removes_it_from_index() {
        let test_repo = init_repo_with_commit();
        std::fs::remove_file(test_repo.dir.path().join("initial.txt")).unwrap();

        stage_file(test_repo.path(), "initial.txt".to_string()).unwrap();

        let status = get_repo_status(test_repo.path()).unwrap();
        let entry = status.changes.iter().find(|c| c.path == "initial.txt").unwrap();
        assert!(entry.staged);
        assert_eq!(entry.status, "deleted");
    }
}
