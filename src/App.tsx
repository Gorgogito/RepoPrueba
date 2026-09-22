import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import BranchSidebar, { BranchInfo } from "./components/BranchSidebar";
import CommitPanel, { RepoStatus } from "./components/CommitPanel";
import "./App.css";

interface RepoInfo {
  path: string;
  name: string;
  current_branch: string;
}

function App() {
  const [repo, setRepo] = useState<RepoInfo | null>(null);
  const [status, setStatus] = useState<RepoStatus | null>(null);
  const [branches, setBranches] = useState<BranchInfo[]>([]);
  const [error, setError] = useState<string | null>(null);

  async function loadRepoData(path: string) {
    const [info, repoStatus, branchList] = await Promise.all([
      invoke<RepoInfo>("open_repository", { path }),
      invoke<RepoStatus>("get_repo_status", { path }),
      invoke<BranchInfo[]>("list_branches", { path }),
    ]);
    setRepo(info);
    setStatus(repoStatus);
    setBranches(branchList);
  }

  async function openRepository() {
    setError(null);
    const selected = await open({ directory: true, multiple: false });
    if (!selected || Array.isArray(selected)) return;

    try {
      await loadRepoData(selected);
    } catch (err) {
      setRepo(null);
      setStatus(null);
      setBranches([]);
      setError(String(err));
    }
  }

  async function refresh() {
    if (!repo) return;
    try {
      await loadRepoData(repo.path);
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <div className="app-shell">
      <header className="toolbar">
        <button onClick={openRepository}>Abrir repositorio</button>
        {repo && (
          <div className="repo-summary">
            <strong>{repo.name}</strong>
            <span className="branch">{repo.current_branch}</span>
            {status && (status.ahead > 0 || status.behind > 0) && (
              <span className="sync">
                {status.ahead > 0 && `↑${status.ahead}`} {status.behind > 0 && `↓${status.behind}`}
              </span>
            )}
          </div>
        )}
      </header>

      <div className="app-body">
        {repo && (
          <BranchSidebar repoPath={repo.path} branches={branches} onChanged={refresh} onError={setError} />
        )}

        <main className="container">
          {error && <p className="error">{error}</p>}

          {repo && status && (
            <>
              <p className="repo-path">{repo.path}</p>
              <CommitPanel repoPath={repo.path} status={status} onChanged={refresh} onError={setError} />
            </>
          )}

          {!repo && !error && <p className="hint">Selecciona un repositorio local para ver su estado.</p>}
        </main>
      </div>
    </div>
  );
}

export default App;
