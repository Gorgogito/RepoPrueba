import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

interface ConnectedUser {
  login: string;
  avatar_url: string;
}

type Provider = "github" | "bitbucket" | "gitlab";

interface Props {
  provider: Provider;
  providerLabel: string;
  onClose: () => void;
  onConnected: (user: ConnectedUser) => void;
}

const HELP_LINKS: Record<Provider, { label: string; url: string }> = {
  github: {
    label: "Generar uno en GitHub",
    url: "https://github.com/settings/tokens/new?scopes=repo&description=Stash",
  },
  gitlab: {
    label: "Generar uno en GitLab",
    url: "https://gitlab.com/-/user_settings/personal_access_tokens?scopes=api&name=Stash",
  },
  bitbucket: {
    label: "Crear un App Password en Bitbucket",
    url: "https://bitbucket.org/account/settings/app-passwords/",
  },
};

function ProviderConnectModal({ provider, providerLabel, onClose, onConnected }: Props) {
  const usesBasicAuth = provider === "bitbucket";

  const [token, setToken] = useState("");
  const [username, setUsername] = useState("");
  const [secret, setSecret] = useState("");
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const canSubmit = usesBasicAuth ? !!username.trim() && !!secret.trim() : !!token.trim();

  async function handleConnect(e: React.FormEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    setConnecting(true);
    setError(null);
    try {
      const user = await invoke<ConnectedUser>("provider_connect", {
        provider,
        token: usesBasicAuth ? null : token.trim(),
        username: usesBasicAuth ? username.trim() : null,
        secret: usesBasicAuth ? secret.trim() : null,
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
          {usesBasicAuth ? (
            <p className="hint small">
              Bitbucket usa tu usuario más un App Password (no tu contraseña normal), con permiso de lectura y
              escritura sobre Pull Requests.{" "}
              <button type="button" className="link-button" onClick={() => openUrl(help.url)}>
                {help.label}
              </button>
            </p>
          ) : (
            <p className="hint small">
              Necesitas un Personal Access Token con permiso <code>{provider === "github" ? "repo" : "api"}</code>.{" "}
              <button type="button" className="link-button" onClick={() => openUrl(help.url)}>
                {help.label}
              </button>
            </p>
          )}

          {usesBasicAuth ? (
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
          ) : (
            <label className="clone-form-label">
              Personal Access Token
              <input
                autoFocus
                type="password"
                value={token}
                onChange={(e) => setToken(e.currentTarget.value)}
                placeholder={provider === "github" ? "ghp_..." : "glpat-..."}
                disabled={connecting}
              />
            </label>
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
