import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

interface ConnectedUser {
  login: string;
  avatar_url: string;
}

interface Props {
  provider: "github" | "bitbucket";
  providerLabel: string;
  onClose: () => void;
  onConnected: (user: ConnectedUser) => void;
}

const HELP_LINKS: Record<Props["provider"], { label: string; url: string }> = {
  github: {
    label: "Generar uno en GitHub",
    url: "https://github.com/settings/tokens/new?scopes=repo&description=Stash",
  },
  bitbucket: {
    label: "Crear un App Password en Bitbucket",
    url: "https://bitbucket.org/account/settings/app-passwords/",
  },
};

function ProviderConnectModal({ provider, providerLabel, onClose, onConnected }: Props) {
  const [token, setToken] = useState("");
  const [username, setUsername] = useState("");
  const [secret, setSecret] = useState("");
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const canSubmit = provider === "github" ? !!token.trim() : !!username.trim() && !!secret.trim();

  async function handleConnect(e: React.FormEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    setConnecting(true);
    setError(null);
    try {
      const user = await invoke<ConnectedUser>("provider_connect", {
        provider,
        token: provider === "github" ? token.trim() : null,
        username: provider === "bitbucket" ? username.trim() : null,
        secret: provider === "bitbucket" ? secret.trim() : null,
      });
      onConnected(user);
    } catch (err) {
      setError(String(err));
    } finally {
      setConnecting(false);
    }
  }

  const help = HELP_LINKS[provider];

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-panel clone-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>Conectar {providerLabel}</h2>
          <button className="secondary" onClick={onClose}>
            Cerrar
          </button>
        </div>

        <form className="clone-form" onSubmit={handleConnect}>
          {provider === "github" ? (
            <p className="hint small">
              Necesitás un Personal Access Token con permiso <code>repo</code>.{" "}
              <button type="button" className="link-button" onClick={() => openUrl(help.url)}>
                {help.label}
              </button>
            </p>
          ) : (
            <p className="hint small">
              Bitbucket usa tu usuario más un App Password (no tu contraseña normal), con permiso de lectura y
              escritura sobre Pull Requests.{" "}
              <button type="button" className="link-button" onClick={() => openUrl(help.url)}>
                {help.label}
              </button>
            </p>
          )}

          {provider === "github" ? (
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
          ) : (
            <>
              <label className="clone-form-label">
                Usuario
                <input
                  autoFocus
                  value={username}
                  onChange={(e) => setUsername(e.currentTarget.value)}
                  placeholder="usuario de Bitbucket"
                  disabled={connecting}
                />
              </label>
              <label className="clone-form-label">
                App Password
                <input
                  type="password"
                  value={secret}
                  onChange={(e) => setSecret(e.currentTarget.value)}
                  placeholder="App Password"
                  disabled={connecting}
                />
              </label>
            </>
          )}

          {error && <p className="error">{error}</p>}

          <button type="submit" disabled={connecting || !canSubmit}>
            {connecting ? "Conectando..." : "Conectar"}
          </button>
        </form>
      </div>
    </div>
  );
}

export default ProviderConnectModal;
