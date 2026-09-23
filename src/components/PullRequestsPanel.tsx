import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import ProviderConnectModal from "./ProviderConnectModal";
import CreatePullRequestModal from "./CreatePullRequestModal";
import { IconClose } from "./icons";

export interface PullRequestSummary {
  number: number;
  title: string;
  author: string;
  state: string;
  draft: boolean;
  base: string;
  head: string;
  html_url: string;
  created_at: string;
  updated_at: string;
}

export interface PullRequestDetail extends PullRequestSummary {
  body: string;
  merged: boolean;
  mergeable: boolean | null;
  mergeable_state: string;
  additions: number;
  deletions: number;
  changed_files: number;
  commits: number;
}

interface ProviderStatus {
  provider: "github" | "bitbucket" | null;
  provider_label: string | null;
  has_token: boolean;
  owner: string | null;
  repo: string | null;
}

interface Props {
  repoPath: string;
  branchNames: string[];
  currentBranch: string;
  refreshToken: number;
  onError: (message: string) => void;
}

type StateFilter = "open" | "closed" | "all";

function PullRequestsPanel({ repoPath, branchNames, currentBranch, refreshToken, onError }: Props) {
  const [status, setStatus] = useState<ProviderStatus | null>(null);
  const [stateFilter, setStateFilter] = useState<StateFilter>("open");
  const [prs, setPrs] = useState<PullRequestSummary[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [selected, setSelected] = useState<PullRequestDetail | null>(null);
  const [connectOpen, setConnectOpen] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [mergeMethod, setMergeMethod] = useState("merge");
  const [merging, setMerging] = useState(false);

  async function loadStatus() {
    try {
      const s = await invoke<ProviderStatus>("provider_status", { path: repoPath });
      setStatus(s);
      return s;
    } catch (err) {
      onError(String(err));
      return null;
    }
  }

  async function loadPulls(state: StateFilter) {
    setLoading(true);
    try {
      const list = await invoke<PullRequestSummary[]>("provider_list_pull_requests", { path: repoPath, state });
      setPrs(list);
    } catch (err) {
      onError(String(err));
      setPrs(null);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadStatus().then((s) => {
      if (s?.has_token && s.owner) loadPulls(stateFilter);
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repoPath, refreshToken]);

  useEffect(() => {
    if (status?.has_token && status.owner) loadPulls(stateFilter);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stateFilter]);

  async function openDetail(number: number) {
    try {
      const detail = await invoke<PullRequestDetail>("provider_get_pull_request", { path: repoPath, number });
      setSelected(detail);
      setMergeMethod("merge");
    } catch (err) {
      onError(String(err));
    }
  }

  async function handleMerge() {
    if (!selected) return;
    setMerging(true);
    try {
      await invoke("provider_merge_pull_request", { path: repoPath, number: selected.number, method: mergeMethod });
      setSelected(null);
      await loadPulls(stateFilter);
    } catch (err) {
      onError(String(err));
    } finally {
      setMerging(false);
    }
  }

  if (!status) {
    return <p className="hint">Cargando estado del proveedor remoto...</p>;
  }

  if (!status.provider) {
    return (
      <p className="hint">
        El remoto 'origin' de este repositorio no es de un proveedor soportado todavía (GitHub o Bitbucket).
      </p>
    );
  }

  if (!status.has_token) {
    return (
      <div className="pr-empty-state">
        <p className="hint">Conectá tu cuenta de {status.provider_label} para ver, crear y fusionar Pull Requests.</p>
        <button onClick={() => setConnectOpen(true)}>Conectar {status.provider_label}</button>
        {connectOpen && (
          <ProviderConnectModal
            provider={status.provider}
            providerLabel={status.provider_label ?? ""}
            onClose={() => setConnectOpen(false)}
            onConnected={() => {
              setConnectOpen(false);
              loadStatus().then((s) => {
                if (s?.has_token && s.owner) loadPulls(stateFilter);
              });
            }}
          />
        )}
      </div>
    );
  }

  return (
    <div className="pr-panel">
      <div className="pr-toolbar">
        <div className="pr-filter">
          {(["open", "closed", "all"] as StateFilter[]).map((f) => (
            <button
              key={f}
              className={`secondary pr-filter-button ${stateFilter === f ? "active" : ""}`}
              onClick={() => setStateFilter(f)}
            >
              {f === "open" ? "Abiertos" : f === "closed" ? "Cerrados" : "Todos"}
            </button>
          ))}
        </div>
        <button onClick={() => setCreateOpen(true)}>Crear Pull Request</button>
      </div>

      {loading && <p className="hint">Cargando pull requests...</p>}

      {!loading && prs && prs.length === 0 && <p className="hint">No hay pull requests {stateFilter === "open" ? "abiertos" : ""}.</p>}

      {!loading && prs && prs.length > 0 && (
        <ul className="pr-list">
          {prs.map((pr) => (
            <li key={pr.number} className="pr-row">
              <button className="pr-row-main" onClick={() => openDetail(pr.number)}>
                <span className="pr-number">#{pr.number}</span>
                <span className="pr-title">{pr.title}</span>
                {pr.draft && <span className="ref-badge">borrador</span>}
                <span className="pr-branches">
                  {pr.head} → {pr.base}
                </span>
                <span className="pr-author">{pr.author}</span>
              </button>
            </li>
          ))}
        </ul>
      )}

      {createOpen && (
        <CreatePullRequestModal
          repoPath={repoPath}
          branchNames={branchNames}
          currentBranch={currentBranch}
          onClose={() => setCreateOpen(false)}
          onCreated={(pr) => {
            setCreateOpen(false);
            loadPulls(stateFilter);
            setSelected(pr);
          }}
          onError={onError}
        />
      )}

      {selected && (
        <div className="modal-backdrop" onClick={() => setSelected(null)}>
          <div className="modal-panel pr-detail-modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h2>
                #{selected.number} {selected.title}
              </h2>
              <button className="icon-button" onClick={() => setSelected(null)} aria-label="Cerrar">
                <IconClose />
              </button>
            </div>

            <div className="diff-modal-body pr-detail-body">
              <p className="pr-detail-meta">
                {selected.author} · {selected.head} → {selected.base}
                {status.provider === "github" && (
                  <>
                    {" "}
                    · +{selected.additions} -{selected.deletions} · {selected.changed_files} archivos ·{" "}
                    {selected.commits} commits
                  </>
                )}
              </p>

              {selected.body && <p className="pr-detail-description">{selected.body}</p>}

              <button className="link-button" onClick={() => openUrl(selected.html_url)}>
                Ver en {status.provider_label}
              </button>

              {selected.merged ? (
                <p className="clean">Ya fue fusionado.</p>
              ) : selected.state !== "open" ? (
                <p className="hint">Este pull request está cerrado.</p>
              ) : (
                <div className="pr-merge-row">
                  <select value={mergeMethod} onChange={(e) => setMergeMethod(e.currentTarget.value)} disabled={merging}>
                    <option value="merge">Merge commit</option>
                    <option value="squash">Squash and merge</option>
                    <option value="rebase">
                      {status.provider === "bitbucket" ? "Fast-forward merge" : "Rebase and merge"}
                    </option>
                  </select>
                  <button onClick={handleMerge} disabled={merging || selected.mergeable === false}>
                    {merging ? "Fusionando..." : "Fusionar"}
                  </button>
                  {selected.mergeable === false && <span className="error">Tiene conflictos, no se puede fusionar</span>}
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default PullRequestsPanel;
