import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import BranchSidebar, { BranchInfo } from "./components/BranchSidebar";
import CommitPanel, { RepoStatus } from "./components/CommitPanel";
import RemoteControls, { RemoteInfo } from "./components/RemoteControls";
import StashPanel, { StashInfo } from "./components/StashPanel";
import ConflictResolver, { OperationStatus } from "./components/ConflictResolver";
import CommitGraph from "./components/CommitGraph";
import RepoSwitcher, { RepoEntry } from "./components/RepoSwitcher";
import DiffModal, { DiffRequest } from "./components/DiffModal";
import CommandPalette, { PaletteCommand } from "./components/CommandPalette";
import UndoRedoControls from "./components/UndoRedoControls";
import TerminalPanel from "./components/TerminalPanel";
import CloneModal from "./components/CloneModal";
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
  const [knownRepos, setKnownRepos] = useState<RepoEntry[]>([]);
  const [diffRequest, setDiffRequest] = useState<DiffRequest | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [terminalOpen, setTerminalOpen] = useState(false);
  const [cloneModalOpen, setCloneModalOpen] = useState(false);

  useEffect(() => {
    invoke<RepoEntry[]>("list_known_repos")
      .then(setKnownRepos)
      .catch((err) => setError(String(err)));
  }, [reposVersion]);

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen((v) => !v);
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "`") {
        e.preventDefault();
        setTerminalOpen((v) => !v);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

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



  async function forgetRepo(path: string) {
    try {
      const updated = await invoke<RepoEntry[]>("remove_known_repo", { path });
      setKnownRepos(updated);
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleCloned(path: string) {
    setCloneModalOpen(false);
    await openRepositoryAtPath(path);
  }

  const commands = useMemo<PaletteCommand[]>(() => {
    const cmds: PaletteCommand[] = [];

    cmds.push({ id: "open-dialog", group: "Repositorio", label: "Abrir repositorio...", run: openRepository });
    cmds.push({
      id: "clone-dialog",
      group: "Repositorio",
      label: "Clonar repositorio...",
      run: () => setCloneModalOpen(true),
    });

    for (const r of knownRepos) {
      if (repo?.path === r.path) continue;
      cmds.push({
        id: `repo-${r.path}`,
        group: "Repositorios",
        label: `Cambiar a repositorio: ${r.name}`,
        run: () => openRepositoryAtPath(r.path),
      });
    }

    if (repo) {
      cmds.push({ id: "tab-changes", group: "Navegación", label: "Ver cambios", run: () => setTab("changes") });
      cmds.push({ id: "tab-history", group: "Navegación", label: "Ver historial", run: () => setTab("history") });

      for (const b of branches) {
        if (b.is_head) continue;
        cmds.push({
          id: `checkout-${b.name}`,
          group: "Ramas",
          label: `Cambiar a rama: ${b.name}`,
          run: () => {
            invoke("checkout_branch", { path: repo.path, name: b.name })
              .catch((err) => setError(String(err)))
              .finally(refresh);
          },
        });
        cmds.push({
          id: `merge-${b.name}`,
          group: "Ramas",
          label: `Mezclar '${b.name}' en la rama actual`,
          run: () => {
            invoke("merge_branch", { path: repo.path, branch: b.name })
              .catch((err) => setError(String(err)))
              .finally(refresh);
          },
        });
      }

      const remote = remotes[0];
      if (remote) {
        cmds.push({
          id: "fetch",
          group: "Remoto",
          label: `Fetch (${remote.name})`,
          run: () => {
            invoke("fetch", { path: repo.path, remoteName: remote.name })
              .catch((err) => setError(String(err)))
              .finally(refresh);
          },
        });
        cmds.push({
          id: "pull",
          group: "Remoto",
          label: `Pull (${remote.name})`,
          run: () => {
            invoke("pull", { path: repo.path, remoteName: remote.name })
              .catch((err) => setError(String(err)))
              .finally(refresh);
          },
        });
        cmds.push({
          id: "push",
          group: "Remoto",
          label: `Push (${remote.name})`,
          run: () => {
            invoke("push", { path: repo.path, remoteName: remote.name, branch: repo.current_branch })
              .catch((err) => setError(String(err)))
              .finally(refresh);
          },
        });
      }

      if (status && !status.is_clean) {
        cmds.push({
          id: "stash-save",
          group: "Stash",
          label: "Guardar cambios en un stash",
          run: () => {
            invoke("stash_save", { path: repo.path, message: null })
              .catch((err) => setError(String(err)))
              .finally(refresh);
          },
        });
      }
    }

    return cmds;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repo, branches, remotes, knownRepos, status]);

  return (
    <div className="app-shell">
      <header className="toolbar">
        <RepoSwitcher
          currentName={repo?.name ?? null}
          repos={knownRepos}
          onSwitchRepo={openRepositoryAtPath}
          onBrowse={openRepository}
          onForget={forgetRepo}
          onClone={() => setCloneModalOpen(true)}
        />
        <button className="secondary palette-trigger" onClick={() => setPaletteOpen(true)} title="Paleta de comandos">
          Buscar <span className="palette-shortcut">Ctrl+K</span>
        </button>
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
          <UndoRedoControls
            repoPath={repo.path}
            refreshToken={historyVersion}
            onChanged={refresh}
            onError={setError}
          />
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
        {repo && (
          <button
            className={`secondary terminal-trigger ${terminalOpen ? "active" : ""}`}
            onClick={() => setTerminalOpen((v) => !v)}
            title="Consola (Ctrl+`)"
          >
            Consola
          </button>
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

      {repo && terminalOpen && (
        <TerminalPanel
          repoPath={repo.path}
          onClose={() => {
            setTerminalOpen(false);
            // Commands run by hand in the terminal (git or otherwise) can
            // change repo state we'd otherwise only learn about on the next
            // unrelated action — refresh as soon as the user steps away.
            refresh();
          }}
        />
      )}

      {repo && diffRequest && (
        <DiffModal repoPath={repo.path} request={diffRequest} onClose={() => setDiffRequest(null)} />
      )}

      {paletteOpen && <CommandPalette commands={commands} onClose={() => setPaletteOpen(false)} />}

      {cloneModalOpen && <CloneModal onClose={() => setCloneModalOpen(false)} onCloned={handleCloned} />}
    </div>
  );
}

export default App;
