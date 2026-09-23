import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

interface Props {
  onClose: () => void;
  onCloned: (path: string) => void;
}

function CloneModal({ onClose, onCloned }: Props) {
  const [url, setUrl] = useState("");
  const [parentDir, setParentDir] = useState<string | null>(null);
  const [cloning, setCloning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function pickDestination() {
    const selected = await open({ directory: true, multiple: false });
    if (!selected || Array.isArray(selected)) return;
    setParentDir(selected);
  }

  async function handleClone(e: React.FormEvent) {
    e.preventDefault();
    if (!url.trim() || !parentDir) return;
    setCloning(true);
    setError(null);
    try {
      const path = await invoke<string>("clone_repository", { url: url.trim(), parentDir });
      onCloned(path);
    } catch (err) {
      setError(String(err));
    } finally {
      setCloning(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-panel clone-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>Clonar repositorio</h2>
          <button className="secondary" onClick={onClose}>
            Cerrar
          </button>
        </div>

        <form className="clone-form" onSubmit={handleClone}>
          <label className="clone-form-label">
            URL del repositorio
            <input
              autoFocus
              value={url}
              onChange={(e) => setUrl(e.currentTarget.value)}
              placeholder="https://github.com/usuario/repo.git"
              disabled={cloning}
            />
          </label>

          <label className="clone-form-label">
            Carpeta destino
            <div className="clone-destination-row">
              <input value={parentDir ?? ""} readOnly placeholder="Elegí dónde guardarlo" />
              <button type="button" className="secondary" onClick={pickDestination} disabled={cloning}>
                Elegir...
              </button>
            </div>
          </label>

          {error && <p className="error">{error}</p>}

          <button type="submit" disabled={cloning || !url.trim() || !parentDir}>
            {cloning ? "Clonando..." : "Clonar"}
          </button>
        </form>
      </div>
    </div>
  );
}

export default CloneModal;
