import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { IconClose } from "./icons";

interface SubmoduleInfo {
  name: string;
  path: string;
  url: string | null;
  pinned_id: string | null;
  workdir_id: string | null;
  is_initialized: boolean;
  is_up_to_date: boolean;
}

interface Props {
  repoPath: string;
  onClose: () => void;
  onError: (message: string) => void;
}

function SubmodulesModal({ repoPath, onClose, onError }: Props) {
  const [submodules, setSubmodules] = useState<SubmoduleInfo[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [newUrl, setNewUrl] = useState("");
  const [newPath, setNewPath] = useState("");

  async function refresh() {
    try {
      setSubmodules(await invoke<SubmoduleInfo[]>("list_submodules", { path: repoPath }));
    } catch (err) {
      onError(String(err));
    }
  }

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repoPath]);

  async function handleAdd(e: React.FormEvent) {
    e.preventDefault();
    if (!newUrl.trim() || !newPath.trim()) return;
    setBusy("add");
    try {
      await invoke("add_submodule", { path: repoPath, url: newUrl.trim(), submodulePath: newPath.trim() });
      setNewUrl("");
      setNewPath("");
      await refresh();
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleUpdate(sm: SubmoduleInfo) {
    setBusy(sm.path);
    try {
      await invoke("update_submodules", { path: repoPath, init: true, recursive: true });
      await refresh();
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleUpdateAll() {
    setBusy("update-all");
    try {
      await invoke("update_submodules", { path: repoPath, init: true, recursive: true });
      await refresh();
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleSync() {
    setBusy("sync");
    try {
      await invoke("sync_submodules", { path: repoPath });
      await refresh();
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleDeinit(sm: SubmoduleInfo) {
    setBusy(sm.path);
    try {
      await invoke("deinit_submodule", { path: repoPath, submodulePath: sm.path, force: true });
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
          <h2>Submódulos</h2>
          <button className="icon-button" onClick={onClose} aria-label="Cerrar">
            <IconClose />
          </button>
        </div>

        <div className="diff-modal-body tools-modal-body">
          {submodules === null && <p className="hint">Cargando...</p>}
          {submodules?.length === 0 && <p className="hint">Este repositorio no tiene submódulos.</p>}

          {submodules && submodules.length > 0 && (
            <>
              <ul className="tools-list">
                {submodules.map((sm) => (
                  <li key={sm.path} className="tools-list-row">
                    <div className="tools-list-main">
                      <span className="pr-title">{sm.name}</span>
                      {!sm.is_initialized && <span className="ref-badge">sin inicializar</span>}
                      {sm.is_initialized && !sm.is_up_to_date && <span className="ref-badge">desactualizado</span>}
                      <span className="commit-sha">{sm.pinned_id?.slice(0, 7) ?? "?"}</span>
                    </div>
                    <span className="tools-list-path">{sm.path}</span>
                    <span className="tools-list-actions">
                      <button className="secondary" disabled={busy !== null} onClick={() => handleUpdate(sm)}>
                        {sm.is_initialized ? "Actualizar" : "Inicializar"}
                      </button>
                      {sm.is_initialized && (
                        <button className="secondary" disabled={busy !== null} onClick={() => handleDeinit(sm)}>
                          Deinit
                        </button>
                      )}
                    </span>
                  </li>
                ))}
              </ul>
              <div className="tools-bulk-actions">
                <button className="secondary" onClick={handleUpdateAll} disabled={busy !== null}>
                  {busy === "update-all" ? "Actualizando..." : "Inicializar/actualizar todos"}
                </button>
                <button className="secondary" onClick={handleSync} disabled={busy !== null}>
                  {busy === "sync" ? "Sincronizando..." : "Sincronizar URLs"}
                </button>
              </div>
            </>
          )}

          <form className="clone-form tools-add-form" onSubmit={handleAdd}>
            <label className="clone-form-label">
              URL del repositorio
              <input
                value={newUrl}
                onChange={(e) => setNewUrl(e.currentTarget.value)}
                placeholder="https://github.com/usuario/dependencia.git"
                disabled={busy !== null}
              />
            </label>
            <label className="clone-form-label">
              Carpeta dentro del repo
              <input
                value={newPath}
                onChange={(e) => setNewPath(e.currentTarget.value)}
                placeholder="vendor/dependencia"
                disabled={busy !== null}
              />
            </label>
            <button type="submit" disabled={busy !== null || !newUrl.trim() || !newPath.trim()}>
              {busy === "add" ? "Agregando..." : "Agregar submódulo"}
            </button>
          </form>
        </div>
      </div>
    </div>
  );
}

export default SubmodulesModal;
