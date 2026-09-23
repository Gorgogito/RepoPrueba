import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { IconClose } from "./icons";

interface WorktreeInfo {
  name: string;
  path: string;
  head: string;
  branch: string | null;
  is_locked: boolean;
  is_prunable: boolean;
  is_bare: boolean;
  is_main: boolean;
}

interface Props {
  repoPath: string;
  onClose: () => void;
  onError: (message: string) => void;
}

function WorktreesModal({ repoPath, onClose, onError }: Props) {
  const [worktrees, setWorktrees] = useState<WorktreeInfo[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [newBranch, setNewBranch] = useState("");
  const [newPath, setNewPath] = useState("");
  const [createBranch, setCreateBranch] = useState(true);

  async function refresh() {
    try {
      setWorktrees(await invoke<WorktreeInfo[]>("list_worktrees", { path: repoPath }));
    } catch (err) {
      onError(String(err));
    }
  }

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repoPath]);

  async function pickDestination() {
    const selected = await open({ directory: true, multiple: false });
    if (!selected || Array.isArray(selected)) return;
    setNewPath(`${selected}\\${newBranch || "worktree"}`);
  }

  async function handleAdd(e: React.FormEvent) {
    e.preventDefault();
    if (!newBranch.trim() || !newPath.trim()) return;
    setBusy("add");
    try {
      await invoke("add_worktree", {
        path: repoPath,
        worktreePath: newPath.trim(),
        branch: newBranch.trim(),
        createBranch,
      });
      setNewBranch("");
      setNewPath("");
      await refresh();
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleRemove(wt: WorktreeInfo, force: boolean) {
    setBusy(wt.path);
    try {
      await invoke("remove_worktree", { path: repoPath, worktreePath: wt.path, force });
      await refresh();
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleToggleLock(wt: WorktreeInfo) {
    setBusy(wt.path);
    try {
      if (wt.is_locked) {
        await invoke("unlock_worktree", { path: repoPath, worktreePath: wt.path });
      } else {
        await invoke("lock_worktree", { path: repoPath, worktreePath: wt.path, reason: null });
      }
      await refresh();
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(null);
    }
  }

  async function handlePrune() {
    setBusy("prune");
    try {
      await invoke("prune_worktrees", { path: repoPath });
      await refresh();
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-panel tools-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>Worktrees</h2>
          <button className="icon-button" onClick={onClose} aria-label="Cerrar">
            <IconClose />
          </button>
        </div>

        <div className="diff-modal-body tools-modal-body">
          {worktrees === null && <p className="hint">Cargando...</p>}

          {worktrees && (
            <ul className="tools-list">
              {worktrees.map((wt) => (
                <li key={wt.path} className="tools-list-row">
                  <div className="tools-list-main">
                    <span className="pr-title">
                      {wt.is_main && <span className="ref-badge">principal</span>} {wt.name}
                    </span>
                    <span className="pr-branches">{wt.branch ?? "(detached)"}</span>
                    <span className="commit-sha">{wt.head.slice(0, 7)}</span>
                  </div>
                  <span className="tools-list-path">{wt.path}</span>
                  {!wt.is_main && (
                    <span className="tools-list-actions">
                      <button
                        className="secondary"
                        disabled={busy !== null}
                        onClick={() => handleToggleLock(wt)}
                      >
                        {wt.is_locked ? "Desbloquear" : "Bloquear"}
                      </button>
                      <button
                        className="secondary"
                        disabled={busy !== null}
                        onClick={() => handleRemove(wt, wt.is_locked)}
                      >
                        Quitar
                      </button>
                    </span>
                  )}
                </li>
              ))}
            </ul>
          )}

          {worktrees?.some((w) => w.is_prunable) && (
            <button className="secondary" onClick={handlePrune} disabled={busy !== null}>
              Limpiar worktrees eliminados manualmente
            </button>
          )}

          <form className="clone-form tools-add-form" onSubmit={handleAdd}>
            <label className="clone-form-label">
              Rama
              <input
                value={newBranch}
                onChange={(e) => setNewBranch(e.currentTarget.value)}
                placeholder="nombre-de-rama"
                disabled={busy !== null}
              />
            </label>
            <label className="pr-draft-checkbox">
              <input
                type="checkbox"
                checked={createBranch}
                onChange={(e) => setCreateBranch(e.currentTarget.checked)}
              />
              Crear rama nueva (si no, usa una existente)
            </label>
            <label className="clone-form-label">
              Carpeta destino
              <div className="clone-destination-row">
                <input value={newPath} onChange={(e) => setNewPath(e.currentTarget.value)} placeholder="Ruta del nuevo worktree" />
                <button type="button" className="secondary" onClick={pickDestination} disabled={busy !== null}>
                  Elegir...
                </button>
              </div>
            </label>
            <button type="submit" disabled={busy !== null || !newBranch.trim() || !newPath.trim()}>
              {busy === "add" ? "Creando..." : "Agregar worktree"}
            </button>
          </form>
        </div>
      </div>
    </div>
  );
}

export default WorktreesModal;
