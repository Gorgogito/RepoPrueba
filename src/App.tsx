import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import BranchSidebar, { BranchInfo } from "./components/BranchSidebar";
import CommitPanel, { RepoStatus } from "./components/CommitPanel";
import RemoteControls, { RemoteInfo } from "./components/RemoteControls";
import StashPanel, { StashInfo } from "./components/StashPanel";
import ConflictResolver, { OperationStatus } from "./components/ConflictResolver";
import CommitGraph from "./components/CommitGraph";
import RepoSwitcher from "./components/RepoSwitcher";
import DiffModal, { DiffRequest } from "./components/DiffModal";
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
  const [remotes, setRemotes] = useState<RemoteInfo[]>([]);
  const [stashes, setStashes] = useState<StashInfo[]>([]);
  const [operation, setOperation] = useState<OperationStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<"changes" | "history">("changes");
  const [historyVersion, setHistoryVersion] = useState(0);
  const [reposVersion, setReposVersion] = useState(0);
  const [diffRequest, setDiffRequest] = useState<DiffRequest | null>(null);

  async function loadRepoData(path: string) {
    const [info, repoStatus, branchList, remoteList, stashList, operationStatus] = await Promise.all([
      invoke<RepoInfo>("open_repository", { path }),
      invoke<RepoStatus>("get_repo_status", { path }),
      invoke<BranchInfo[]>("list_branches", { path }),
      invoke<RemoteInfo[]>("list_remotes", { path }),
      invoke<StashInfo[]>("stash_list", { path }),
      invoke<OperationStatus>("get_operation_status", { path }),
    ]);
    setRepo(info);
    setStatus(repoStatus);
    setBranches(branchList);
    setRemotes(remoteList);
    setStashes(stashList);
    setOperation(operationStatus.kind === "none" ? null : operationStatus);
  }

  async function openRepositoryAtPath(path: string) {
    setError(null);
    try {
      await loadRepoData(path);
      setHistoryVersion((v) => v + 1);
      setTab("changes");
      await invoke("add_known_repo", { path });
      setReposVersion((v) => v + 1);
    } catch (err) {
      setRepo(null);
      setStatus(null);
      setBranches([]);
      setRemotes([]);
      setStashes([]);
      setOperation(null);
      setError(String(err));
    }
  }

  async function openRepository() {
    const selected = await open({ directory: true, multiple: false });
    if (!selected || Array.isArray(selected)) return;
    await openRepositoryAtPath(selected as string);
  }

  async function refresh() {
    if (!repo) return;
    try {
      await loadRepoData(repo.path);
      setHistoryVersion((v) => v + 1);
    } catch (err) {
      setError(String(err));
    }
  }



  return (
    <div className="app-shell">
      <header className="toolbar">
        <RepoSwitcher
          currentName={repo?.name ?? null}
          reposVersion={reposVersion}
          onSwitchRepo={openRepositoryAtPath}
          onBrowse={openRepository}
          onError={setError}
        />
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
        {repo && (
          <RemoteControls
            repoPath={repo.path}
            remotes={remotes}
            currentBranch={repo.current_branch}
            onChanged={refresh}
            onError={setError}
          />
        )}
      </header>

      <div className="app-body">
        {repo && (
          <>
            <BranchSidebar repoPath={repo.path} branches={branches} onChanged={refresh} onError={setError} />
            <StashPanel
              repoPath={repo.path}
              stashes={stashes}
              canStash={!!status && !status.is_clean}
              onChanged={refresh}
              onError={setError}
            />
          </>
        )}

        <main className="container">
          {error && <p className="error">{error}</p>}

          {repo && status && (
            <>
              <p className="repo-path">{repo.path}</p>

              <div className="tab-bar">
                <button
                  className={`tab-button ${tab === "changes" ? "active" : ""}`}
                  onClick={() => setTab("changes")}
                >
                  Cambios
                </button>
                <button
                  className={`tab-button ${tab === "history" ? "active" : ""}`}
                  onClick={() => setTab("history")}
                >
                  Historial
                </button>
              </div>

              {operation ? (
                <ConflictResolver repoPath={repo.path} operation={operation} onChanged={refresh} onError={setError} />
              ) : tab === "changes" ? (
                <CommitPanel
                  repoPath={repo.path}
                  status={status}
                  onChanged={refresh}
                  onError={setError}
                  onViewDiff={(file, staged) => setDiffRequest({ kind: "working", file, staged })}
                />
              ) : (
                <CommitGraph
                  repoPath={repo.path}
                  refreshToken={historyVersion}
                  onChanged={refresh}
                  onError={setError}
                  onViewDiff={(sha, label) => setDiffRequest({ kind: "commit", sha, label })}
                />
              )}
            </>
          )}

          {!repo && !error && <p className="hint">Selecciona un repositorio local para ver su estado.</p>}
        </main>
      </div>

      {repo && diffRequest && (
        <DiffModal repoPath={repo.path} request={diffRequest} onClose={() => setDiffRequest(null)} />
      )}
    </div>
  );
}

export default App;
