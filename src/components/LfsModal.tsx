import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { IconClose } from "./icons";

interface LfsStatus {
  available: boolean;
  tracked_patterns: string[];
}

interface LfsFileInfo {
  path: string;
  oid: string;
  fetched: boolean;
}

interface Props {
  repoPath: string;
  onClose: () => void;
  onError: (message: string) => void;
}

function LfsModal({ repoPath, onClose, onError }: Props) {
  const [status, setStatus] = useState<LfsStatus | null>(null);
  const [files, setFiles] = useState<LfsFileInfo[]>([]);
  const [pattern, setPattern] = useState("");
  const [busy, setBusy] = useState<string | null>(null);

  async function refresh() {
    try {
      const s = await invoke<LfsStatus>("get_lfs_status", { path: repoPath });
      setStatus(s);
      if (s.available) {
        setFiles(await invoke<LfsFileInfo[]>("lfs_list_files", { path: repoPath }));
      }
    } catch (err) {
      onError(String(err));
    }
  }

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repoPath]);

  async function handleTrack(e: React.FormEvent) {
    e.preventDefault();
    if (!pattern.trim()) return;
    setBusy("track");
    try {
      await invoke("lfs_track", { path: repoPath, pattern: pattern.trim() });
      setPattern("");
      await refresh();
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleUntrack(p: string) {
    setBusy(p);
    try {
      await invoke("lfs_untrack", { path: repoPath, pattern: p });
      await refresh();
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleInstall() {
    setBusy("install");
    try {
      await invoke("lfs_install", { path: repoPath });
      await refresh();
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(null);
    }
  }

  async function handlePull() {
    setBusy("pull");
    try {
      await invoke("lfs_pull", { path: repoPath });
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
          <h2>Git LFS</h2>
          <button className="icon-button" onClick={onClose} aria-label="Cerrar">
            <IconClose />
          </button>
        </div>

        <div className="diff-modal-body tools-modal-body">
          {!status && <p className="hint">Cargando...</p>}

          {status && !status.available && (
            <div className="pr-empty-state">
              <p className="hint">
                Git LFS no está instalado en este equipo. Es una extensión aparte de git, necesaria para trackear
                archivos grandes.
              </p>
              <button className="link-button" onClick={() => openUrl("https://git-lfs.com")}>
                Instrucciones de instalación
              </button>
            </div>
          )}

          {status && status.available && (
            <>
              <button className="secondary" onClick={handleInstall} disabled={busy !== null}>
                {busy === "install" ? "Configurando..." : "Instalar hooks en este repo"}
              </button>

              <section>
                <h3>Patrones trackeados</h3>
                {status.tracked_patterns.length === 0 && <p className="hint small">Ningún patrón trackeado todavía.</p>}
                <ul className="tools-list">
                  {status.tracked_patterns.map((p) => (
                    <li key={p} className="tools-list-row">
                      <span className="pr-title commit-sha">{p}</span>
                      <button className="icon-button delete" disabled={busy !== null} onClick={() => handleUntrack(p)}>
                        <IconClose />
                      </button>
                    </li>
                  ))}
                </ul>
                <form className="clone-destination-row tools-track-form" onSubmit={handleTrack}>
                  <input
                    value={pattern}
                    onChange={(e) => setPattern(e.currentTarget.value)}
                    placeholder="*.psd"
                    disabled={busy !== null}
                  />
                  <button type="submit" disabled={busy !== null || !pattern.trim()}>
                    Trackear
                  </button>
                </form>
              </section>

              <section>
                <div className="section-header">
                  <h3>Archivos LFS</h3>
                  <button className="secondary" onClick={handlePull} disabled={busy !== null}>
                    {busy === "pull" ? "Descargando..." : "Pull LFS"}
                  </button>
                </div>
                {files.length === 0 && <p className="hint small">Sin archivos LFS en este repositorio.</p>}
                <ul className="tools-list">
                  {files.map((f) => (
                    <li key={f.path} className="tools-list-row">
                      <span className="pr-title">{f.path}</span>
                      <span className={f.fetched ? "clean" : "hint"}>{f.fetched ? "descargado" : "solo puntero"}</span>
                    </li>
                  ))}
                </ul>
              </section>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

export default LfsModal;
