import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface FileChange {
  path: string;
  staged: boolean;
  status: string;
}

export interface RepoStatus {
  is_clean: boolean;
  ahead: number;
  behind: number;
  changes: FileChange[];
}

interface Props {
  repoPath: string;
  status: RepoStatus;
  onChanged: () => void;
  onError: (message: string) => void;
}

function CommitPanel({ repoPath, status, onChanged, onError }: Props) {
  const [message, setMessage] = useState("");
  const [committing, setCommitting] = useState(false);

  const staged = status.changes.filter((c) => c.staged);
  const unstaged = status.changes.filter((c) => !c.staged);

  async function toggleStage(file: FileChange) {
    try {
      const command = file.staged ? "unstage_file" : "stage_file";
      await invoke(command, { path: repoPath, file: file.path });
      onChanged();
    } catch (err) {
      onError(String(err));
    }
  }

  async function stageAll() {
    try {
      await Promise.all(unstaged.map((c) => invoke("stage_file", { path: repoPath, file: c.path })));
      onChanged();
    } catch (err) {
      onError(String(err));
    }
  }

  async function unstageAll() {
    try {
      await Promise.all(staged.map((c) => invoke("unstage_file", { path: repoPath, file: c.path })));
      onChanged();
    } catch (err) {
      onError(String(err));
    }
  }

  async function handleCommit(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = message.trim();
    if (!trimmed) return;
    setCommitting(true);
    try {
      await invoke("commit", { path: repoPath, message: trimmed });
      setMessage("");
      onChanged();
    } catch (err) {
      onError(String(err));
    } finally {
      setCommitting(false);
    }
  }

  return (
    <div className="status">
      {status.is_clean ? (
        <p className="clean">Sin cambios pendientes</p>
      ) : (
        <>
          {staged.length > 0 && (
            <section>
              <div className="section-header">
                <h3>Staged ({staged.length})</h3>
                <button className="link-button" onClick={unstageAll}>
                  Quitar todo
                </button>
              </div>
              <ul>
                {staged.map((c) => (
                  <li key={`s-${c.path}`}>
                    <button className="file-row" onClick={() => toggleStage(c)}>
                      <span className={`status-tag ${c.status}`}>{c.status}</span> {c.path}
                    </button>
                  </li>
                ))}
              </ul>
            </section>
          )}
          {unstaged.length > 0 && (
            <section>
              <div className="section-header">
                <h3>Sin stage ({unstaged.length})</h3>
                <button className="link-button" onClick={stageAll}>
                  Agregar todo
                </button>
              </div>
              <ul>
                {unstaged.map((c) => (
                  <li key={`u-${c.path}`}>
                    <button className="file-row" onClick={() => toggleStage(c)}>
                      <span className={`status-tag ${c.status}`}>{c.status}</span> {c.path}
                    </button>
                  </li>
                ))}
              </ul>
            </section>
          )}
        </>
      )}

      <form className="commit-form" onSubmit={handleCommit}>
        <textarea
          value={message}
          onChange={(e) => setMessage(e.currentTarget.value)}
          placeholder="Mensaje de commit"
          rows={3}
        />
        <button type="submit" disabled={committing || !message.trim() || staged.length === 0}>
          {committing ? "Commiteando..." : `Commit (${staged.length})`}
        </button>
      </form>
    </div>
  );
}

export default CommitPanel;
