import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

interface GithubUser {
  login: string;
  avatar_url: string;
}

interface Props {
  onClose: () => void;
  onConnected: (user: GithubUser) => void;
}

function GithubConnectModal({ onClose, onConnected }: Props) {
  const [token, setToken] = useState("");
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleConnect(e: React.FormEvent) {
    e.preventDefault();
    if (!token.trim()) return;
    setConnecting(true);
    setError(null);
    try {
      const user = await invoke<GithubUser>("github_connect", { token: token.trim() });
      onConnected(user);
    } catch (err) {
      setError(String(err));
    } finally {
      setConnecting(false);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-panel clone-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>Conectar GitHub</h2>
          <button className="secondary" onClick={onClose}>
            Cerrar
          </button>
        </div>

        <form className="clone-form" onSubmit={handleConnect}>
          <p className="hint small">
            Necesitás un Personal Access Token con permiso <code>repo</code>.{" "}
            <button
              type="button"
              className="link-button"
              onClick={() => openUrl("https://github.com/settings/tokens/new?scopes=repo&description=Stash")}
            >
              Generar uno en GitHub
            </button>
          </p>

          <label className="clone-form-label">
            Personal Access Token
            <input
              autoFocus
              type="password"
              value={token}
              onChange={(e) => setToken(e.currentTarget.value)}
              placeholder="ghp_..."
              disabled={connecting}
            />
          </label>

          {error && <p className="error">{error}</p>}

          <button type="submit" disabled={connecting || !token.trim()}>
            {connecting ? "Conectando..." : "Conectar"}
          </button>
        </form>
      </div>
    </div>
  );
}

export default GithubConnectModal;
export type { GithubUser };
