#![cfg(test)]

use git2::{Repository, Signature};
use std::fs;
use tempfile::TempDir;

/// A throwaway repo on disk, deterministic and isolated from the real working tree.
pub struct TestRepo {
    pub dir: TempDir,
    pub repo: Repository,
}

impl TestRepo {
    pub fn path(&self) -> String {
        self.dir.path().to_string_lossy().to_string()
    }

    pub fn write(&self, relative: &str, contents: &str) {
        fs::write(self.dir.path().join(relative), contents).unwrap();
    }

    pub fn stage(&self, relative: &str) {
        let mut index = self.repo.index().unwrap();
        index.add_path(std::path::Path::new(relative)).unwrap();
        index.write().unwrap();
    }

    pub fn commit(&self, message: &str) -> git2::Oid {
        let sig = Signature::now("Test", "test@example.com").unwrap();
        let mut index = self.repo.index().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = self.repo.find_tree(tree_oid).unwrap();

        let parents: Vec<git2::Commit> = match self.repo.head() {
            Ok(head) => vec![head.peel_to_commit().unwrap()],
            Err(_) => vec![],
        };
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

        self.repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
            .unwrap()
    }
}

/// Repo with a single commit ("initial.txt"), ready for status/branch experiments.
pub fn init_repo_with_commit() -> TestRepo {
    let dir = TempDir::new().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    let test_repo = TestRepo { dir, repo };

    test_repo.write("initial.txt", "hello\n");
    test_repo.stage("initial.txt");
    test_repo.commit("initial commit");

    test_repo
}
