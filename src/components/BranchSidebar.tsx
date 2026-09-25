import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { IconMerge, IconRebase, IconRebaseInteractive, IconCheckout, IconClose, IconDot } from "./icons";

export interface BranchInfo {
  name: string;
  is_head: boolean;
  upstream: string | null;
}

interface RemoteBranchInfo {
  name: string;
  remote: string;
  branch: string;
  has_local: boolean;
}

interface Props {
  repoPath: string;
  branches: BranchInfo[];
  refreshToken: number;
  onChanged: () => void;
  onError: (message: string) => void;
  onInteractiveRebase: (onto: string, ontoLabel: string) => void;
}

function BranchSidebar({ repoPath, branches, refreshToken, onChanged, onError, onInteractiveRebase }: Props) {
  const [newBranchName, setNewBranchName] = useState("");
  const [creating, setCreating] = useState(false);
  const [busyBranch, setBusyBranch] = useState<string | null>(null);
  const [remoteBranches, setRemoteBranches] = useState<RemoteBranchInfo[]>([]);

  async function loadRemoteBranches() {
    try {
      const all = await invoke<RemoteBranchInfo[]>("list_remote_branches", { path: repoPath });
      setRemoteBranches(all.filter((r) => !r.has_local));
    } catch (err) {
      onError(String(err));
    }
  }

  useEffect(() => {
    loadRemoteBranches();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repoPath, refreshToken]);

  async function handleSwitch(name: string) {
    setBusyBranch(name);
    try {
      await invoke("checkout_branch", { path: repoPath, name });
    } catch (err) {
      onError(String(err));
    } finally {
      setBusyBranch(null);
      onChanged();
    }
  }

  async function handleSwitchRemote(r: RemoteBranchInfo) {
    setBusyBranch(r.name);
    try {
      await invoke("checkout_remote_branch", { path: repoPath, remoteRef: r.name, localName: r.branch });
    } catch (err) {
      onError(String(err));
    } finally {
      setBusyBranch(null);
      onChanged();
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
            {b.is_head ? (
              <span className="branch-name branch-name-current">
                <IconDot className="current-dot" />
                {b.name}
              </span>
            ) : (
              <button
                className="branch-name"
                onClick={() => handleSwitch(b.name)}
                disabled={busyBranch !== null}
                title={`Cambiar a la rama '${b.name}'`}
              >
                <IconCheckout className="branch-switch-icon" />
                {b.name}
              </button>
            )}
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

      {remoteBranches.length > 0 && (
        <>
          <div className="branch-sidebar-header branch-sidebar-subheader">
            <h3>Ramas remotas</h3>
          </div>
          <ul className="branch-list">
            {remoteBranches.map((r) => (
              <li key={r.name}>
                <button
                  className="branch-name"
                  onClick={() => handleSwitchRemote(r)}
                  disabled={busyBranch !== null}
                  title={`Crear rama local '${r.branch}' desde '${r.name}' y cambiar a ella`}
                >
                  <IconCheckout className="branch-switch-icon" />
                  {r.name}
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
    </aside>
  );
}

export default BranchSidebar;
