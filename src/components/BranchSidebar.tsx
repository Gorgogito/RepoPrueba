import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface BranchInfo {
  name: string;
  is_head: boolean;
  upstream: string | null;
}

interface Props {
  repoPath: string;
  branches: BranchInfo[];
  onChanged: () => void;
  onError: (message: string) => void;
}

function BranchSidebar({ repoPath, branches, onChanged, onError }: Props) {
  const [newBranchName, setNewBranchName] = useState("");
  const [creating, setCreating] = useState(false);

  async function handleSwitch(name: string) {
    try {
      await invoke("checkout_branch", { path: repoPath, name });
      onChanged();
    } catch (err) {
      onError(String(err));
    }
  }

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    const name = newBranchName.trim();
    if (!name) return;
    try {
      await invoke("create_branch", { path: repoPath, name });
      await invoke("checkout_branch", { path: repoPath, name });
      setNewBranchName("");
      setCreating(false);
      onChanged();
    } catch (err) {
      onError(String(err));
    }
  }

  async function handleDelete(name: string) {
    try {
      await invoke("delete_branch", { path: repoPath, name, force: false });
      onChanged();
    } catch (err) {
      onError(String(err));
    }
  }

  return (
    <aside className="branch-sidebar">
      <div className="branch-sidebar-header">
        <h3>Ramas</h3>
        <button className="icon-button" onClick={() => setCreating((v) => !v)} title="Nueva rama">
          +
        </button>
      </div>

      {creating && (
        <form className="new-branch-form" onSubmit={handleCreate}>
          <input
            autoFocus
            value={newBranchName}
            onChange={(e) => setNewBranchName(e.currentTarget.value)}
            placeholder="nombre-de-rama"
          />
        </form>
      )}

      <ul className="branch-list">
        {branches.map((b) => (
          <li key={b.name} className={b.is_head ? "current" : ""}>
            <button className="branch-name" onClick={() => handleSwitch(b.name)} disabled={b.is_head}>
              {b.is_head ? "● " : ""}
              {b.name}
            </button>
            {!b.is_head && (
              <button className="icon-button delete" onClick={() => handleDelete(b.name)} title="Eliminar rama">
                ×
              </button>
            )}
          </li>
        ))}
      </ul>
    </aside>
  );
}

export default BranchSidebar;
