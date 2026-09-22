import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./App.css";

interface RepoInfo {
  path: string;
  name: string;
  current_branch: string;
}

interface FileChange {
  path: string;
  staged: boolean;
  status: string;
}

interface RepoStatus {
  is_clean: boolean;
  ahead: number;
  behind: number;
  changes: FileChange[];
}

function App() {
  const [repo, setRepo] = useState<RepoInfo | null>(null);
  const [status, setStatus] = useState<RepoStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function openRepository() {
    setError(null);
    const selected = await open({ directory: true, multiple: false });
    if (!selected || Array.isArray(selected)) return;

    try {
      const info = await invoke<RepoInfo>("open_repository", { path: selected });
      const repoStatus = await invoke<RepoStatus>("get_repo_status", { path: selected });
      setRepo(info);
      setStatus(repoStatus);
    } catch (err) {
      setRepo(null);
      setStatus(null);
      setError(String(err));
    }
  }

  const staged = status?.changes.filter((c) => c.staged) ?? [];
  const unstaged = status?.changes.filter((c) => !c.staged) ?? [];

  return (
    <main className="container">
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

      {error && <p className="error">{error}</p>}

      {repo && status && (
        <div className="status">
          <p className="repo-path">{repo.path}</p>
          {status.is_clean ? (
            <p className="clean">Sin cambios pendientes</p>
          ) : (
            <>
              {staged.length > 0 && (
                <section>
                  <h3>Staged ({staged.length})</h3>
                  <ul>
                    {staged.map((c) => (
                      <li key={`s-${c.path}`}>
                        <span className={`status-tag ${c.status}`}>{c.status}</span> {c.path}
                      </li>
                    ))}
                  </ul>
                </section>
              )}
              {unstaged.length > 0 && (
                <section>
                  <h3>Sin stage ({unstaged.length})</h3>
                  <ul>
                    {unstaged.map((c) => (
                      <li key={`u-${c.path}`}>
                        <span className={`status-tag ${c.status}`}>{c.status}</span> {c.path}
                      </li>
                    ))}
                  </ul>
                </section>
              )}
            </>
          )}
        </div>
      )}

      {!repo && !error && <p className="hint">Selecciona un repositorio local para ver su estado.</p>}
    </main>
  );
}

export default App;
