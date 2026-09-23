import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PullRequestDetail } from "./PullRequestsPanel";

interface Props {
  repoPath: string;
  term: string;
  branchNames: string[];
  currentBranch: string;
  onClose: () => void;
  onCreated: (pr: PullRequestDetail) => void;
  onError: (message: string) => void;
}

function CreatePullRequestModal({ repoPath, term, branchNames, currentBranch, onClose, onCreated, onError }: Props) {
  const [head, setHead] = useState(currentBranch);
  const [base, setBase] = useState(branchNames.find((b) => b !== currentBranch) ?? currentBranch);
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [draft, setDraft] = useState(false);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    if (!title.trim() || head === base) return;
    setCreating(true);
    setError(null);
    try {
      const pr = await invoke<PullRequestDetail>("provider_create_pull_request", {
        path: repoPath,
        title: title.trim(),
        body,
        head,
        base,
        draft,
      });
      onCreated(pr);
    } catch (err) {
      setError(String(err));
      onError(String(err));
    } finally {
      setCreating(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-panel clone-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>Crear {term}</h2>
          <button className="secondary" onClick={onClose}>
            Cerrar
          </button>
        </div>

        <form className="clone-form" onSubmit={handleCreate}>
          <div className="pr-branch-row">
            <label className="clone-form-label">
              Base
              <select value={base} onChange={(e) => setBase(e.currentTarget.value)} disabled={creating}>
                {branchNames.map((b) => (
                  <option key={b} value={b}>
                    {b}
                  </option>
                ))}
              </select>
            </label>
            <span className="pr-branch-arrow">←</span>
            <label className="clone-form-label">
              Head
              <select value={head} onChange={(e) => setHead(e.currentTarget.value)} disabled={creating}>
                {branchNames.map((b) => (
                  <option key={b} value={b}>
                    {b}
                  </option>
                ))}
              </select>
            </label>
          </div>

          {head === base && <p className="error">La rama base y la rama head deben ser distintas</p>}

          <label className="clone-form-label">
            Título
            <input
              autoFocus
              value={title}
              onChange={(e) => setTitle(e.currentTarget.value)}
              placeholder={`Título del ${term.toLowerCase()}`}
              disabled={creating}
            />
          </label>

          <label className="clone-form-label">
            Descripción
            <textarea
              className="pr-body-textarea"
              value={body}
              onChange={(e) => setBody(e.currentTarget.value)}
              placeholder="Describe los cambios (opcional)"
              disabled={creating}
            />
          </label>

          <label className="pr-draft-checkbox">
            <input type="checkbox" checked={draft} onChange={(e) => setDraft(e.currentTarget.checked)} disabled={creating} />
            Crear como borrador
          </label>

          {error && <p className="error">{error}</p>}

          <button type="submit" disabled={creating || !title.trim() || head === base}>
            {creating ? "Creando..." : `Crear ${term}`}
          </button>
        </form>
      </div>
    </div>
  );
}

export default CreatePullRequestModal;
