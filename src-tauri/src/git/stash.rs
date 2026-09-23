use super::err_msg;
use git2::Repository;
use serde::Serialize;

#[derive(Serialize)]
pub struct StashInfo {
    index: usize,
    message: String,
}

#[tauri::command]
pub fn stash_save(path: String, message: Option<String>) -> Result<(), String> {
    let mut repo = Repository::open(&path).map_err(err_msg)?;
    let sig = repo.signature().map_err(err_msg)?;
    repo.stash_save(&sig, message.as_deref().unwrap_or("WIP"), None)
        .map_err(err_msg)?;
    Ok(())
}

#[tauri::command]
pub fn stash_list(path: String) -> Result<Vec<StashInfo>, String> {
    let mut repo = Repository::open(&path).map_err(err_msg)?;
    let mut stashes = Vec::new();
    repo.stash_foreach(|index, message, _oid| {
        stashes.push(StashInfo {
            index,
            message: message.to_string(),
        });
        true
    })
    .map_err(err_msg)?;
    Ok(stashes)
}

#[tauri::command]
pub fn stash_apply(path: String, index: usize) -> Result<(), String> {
    let mut repo = Repository::open(&path).map_err(err_msg)?;
    repo.stash_apply(index, None).map_err(err_msg)
}

#[tauri::command]
pub fn stash_pop(path: String, index: usize) -> Result<(), String> {
    let mut repo = Repository::open(&path).map_err(err_msg)?;
    repo.stash_pop(index, None).map_err(err_msg)
}

#[tauri::command]
pub fn stash_drop(path: String, index: usize) -> Result<(), String> {
    let mut repo = Repository::open(&path).map_err(err_msg)?;
    repo.stash_drop(index).map_err(err_msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::repo::get_repo_status;
    use crate::git::test_support::init_repo_with_commit;

    #[test]
    fn save_clears_working_tree_and_list_shows_it() {
        let test_repo = init_repo_with_commit();
        test_repo.write("initial.txt", "changed\n");

        stash_save(test_repo.path(), Some("wip work".to_string())).unwrap();

        let status = get_repo_status(test_repo.path()).unwrap();
        assert!(status.is_clean, "working tree should be clean after stashing");

        let stashes = stash_list(test_repo.path()).unwrap();
        assert_eq!(stashes.len(), 1);
        assert!(stashes[0].message.contains("wip work"));
    }

    #[test]
    fn pop_restores_the_change_and_removes_the_stash() {
        let test_repo = init_repo_with_commit();
        test_repo.write("initial.txt", "changed\n");
        stash_save(test_repo.path(), None).unwrap();

        stash_pop(test_repo.path(), 0).unwrap();

        let status = get_repo_status(test_repo.path()).unwrap();
        assert!(!status.is_clean);
        assert!(stash_list(test_repo.path()).unwrap().is_empty());
    }

    #[test]
    fn drop_removes_without_restoring() {
        let test_repo = init_repo_with_commit();
        test_repo.write("initial.txt", "changed\n");
        stash_save(test_repo.path(), None).unwrap();

        stash_drop(test_repo.path(), 0).unwrap();

        let status = get_repo_status(test_repo.path()).unwrap();
        assert!(status.is_clean, "drop should not restore the change");
        assert!(stash_list(test_repo.path()).unwrap().is_empty());
    }
}
