import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { IconMerge, IconRebase, IconRebaseInteractive, IconClose, IconDot } from "./icons";

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
  onInteractiveRebase: (onto: string, ontoLabel: string) => void;
}

function BranchSidebar({ repoPath, branches, onChanged, onError, onInteractiveRebase }: Props) {
  const [newBranchName, setNewBranchName] = useState("");
  const [creating, setCreating] = useState(false);
  const [busyBranch, setBusyBranch] = useState<string | null>(null);

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

  async function handleMerge(name: string) {
    setBusyBranch(name);
    try {
      await invoke("merge_branch", { path: repoPath, branch: name });
    } catch (err) {
      onError(String(err));
    } finally {
      setBusyBranch(null);
      // A conflict is a failed invoke() but a real state change (files/index
      // now hold conflict markers) — always refresh, not just on success.
      onChanged();
    }
  }

  async function handleRebase(name: string) {
    setBusyBranch(name);
    try {
      await invoke("rebase_branch", { path: repoPath, ontoBranch: name });
    } catch (err) {
      onError(String(err));
    } finally {
      setBusyBranch(null);
      onChanged();
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
              {b.is_head && <IconDot className="current-dot" />}
              {b.name}
            </button>
            {!b.is_head && (
              <span className="branch-actions">
                <button
                  className="icon-button"
                  disabled={busyBranch !== null}
                  onClick={() => handleMerge(b.name)}
                  title={`Mezclar '${b.name}' en la rama actual`}
                  aria-label={`Mezclar '${b.name}' en la rama actual`}
                >
                  <IconMerge />
                </button>
                <button
                  className="icon-button"
                  disabled={busyBranch !== null}
                  onClick={() => handleRebase(b.name)}
                  title={`Rebasar la rama actual sobre '${b.name}'`}
                  aria-label={`Rebasar la rama actual sobre '${b.name}'`}
                >
                  <IconRebase />
                </button>
                <button
                  className="icon-button"
                  disabled={busyBranch !== null}
                  onClick={() => onInteractiveRebase(b.name, b.name)}
                  title={`Rebase interactivo sobre '${b.name}'`}
                  aria-label={`Rebase interactivo sobre '${b.name}'`}
                >
                  <IconRebaseInteractive />
                </button>
                <button
                  className="icon-button delete"
                  onClick={() => handleDelete(b.name)}
                  title="Eliminar rama"
                  aria-label="Eliminar rama"
                >
                  <IconClose />
                </button>
              </span>
            )}
          </li>
        ))}
      </ul>
    </aside>
  );
}

export default BranchSidebar;
