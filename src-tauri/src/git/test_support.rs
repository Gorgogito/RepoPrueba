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
        // Fresh handle, like every real command — self.repo's in-memory index
        // can go stale after disk writes made through other fresh handles
        // (e.g. our checkout_branch), and reusing it here would silently
        // resurrect entries that were already removed on disk.
        let repo = Repository::open(self.dir.path()).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new(relative)).unwrap();
        index.write().unwrap();
    }

    pub fn commit(&self, message: &str) -> git2::Oid {
        let repo = Repository::open(self.dir.path()).unwrap();
        let sig = Signature::now("Test", "test@example.com").unwrap();
        let mut index = repo.index().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();

        let parents: Vec<git2::Commit> = match repo.head() {
            Ok(head) => vec![head.peel_to_commit().unwrap()],
            Err(_) => vec![],
        };
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
            .unwrap()
    }
}

/// A bare repo on disk, usable as a local "remote" for push/fetch/pull tests
/// without any network access or credentials.
pub fn init_bare_remote() -> TempDir {
    let dir = TempDir::new().unwrap();
    Repository::init_bare(dir.path()).unwrap();
    dir
}

/// Repo with a single commit ("initial.txt"), ready for status/branch experiments.
pub fn init_repo_with_commit() -> TestRepo {
    let dir = TempDir::new().unwrap();
    let repo = Repository::init(dir.path()).unwrap();

    // Repo-local identity so `repo.signature()` works without depending on
    // whatever global git config happens to be present on the test machine.
    let mut config = repo.config().unwrap();
    config.set_str("user.name", "Test").unwrap();
    config.set_str("user.email", "test@example.com").unwrap();
    // Keep line endings byte-for-byte regardless of the host's global
    // core.autocrlf, so assertions on file contents are deterministic.
    config.set_str("core.autocrlf", "false").unwrap();

    let test_repo = TestRepo { dir, repo };

    test_repo.write("initial.txt", "hello\n");
    test_repo.stage("initial.txt");
    test_repo.commit("initial commit");

    test_repo
}
