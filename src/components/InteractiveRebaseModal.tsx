import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { IconClose } from "./icons";

interface RebaseCommitInfo {
  oid: string;
  short_sha: string;
  summary: string;
  author: string;
}

type RebaseAction = "pick" | "reword" | "squash" | "fixup" | "drop" | "edit";

interface PlanEntry extends RebaseCommitInfo {
  action: RebaseAction;
  message: string;
}

interface RebaseStep {
  oid: string;
  action: string;
  message: string | null;
}

const ACTION_LABELS: Record<RebaseAction, string> = {
  pick: "Pick",
  reword: "Reword",
  squash: "Squash",
  fixup: "Fixup",
  drop: "Drop",
  edit: "Edit",
};

interface Props {
  repoPath: string;
  onto: string;
  ontoLabel: string;
  onClose: () => void;
  onStarted: () => void;
  onError: (message: string) => void;
}

function InteractiveRebaseModal({ repoPath, onto, ontoLabel, onClose, onStarted, onError }: Props) {
  const [plan, setPlan] = useState<PlanEntry[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<RebaseCommitInfo[]>("get_rebase_commits", { path: repoPath, onto })
      .then((commits) => setPlan(commits.map((c) => ({ ...c, action: "pick" as RebaseAction, message: "" }))))
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repoPath, onto]);

  function move(index: number, delta: number) {
    setPlan((current) => {
      if (!current) return current;
      const target = index + delta;
      if (target < 0 || target >= current.length) return current;
      const next = [...current];
      [next[index], next[target]] = [next[target], next[index]];
      return next;
    });
  }

  function setAction(index: number, action: RebaseAction) {
    setPlan((current) => {
      if (!current) return current;
      const next = [...current];
      next[index] = { ...next[index], action };
      return next;
    });
  }

  function setMessage(index: number, message: string) {
    setPlan((current) => {
      if (!current) return current;
      const next = [...current];
      next[index] = { ...next[index], message };
      return next;
    });
  }

  async function handleStart() {
    if (!plan) return;
    setStarting(true);
    setError(null);
    try {
      const steps: RebaseStep[] = plan.map((p) => ({
        oid: p.oid,
        action: p.action,
        message: p.message.trim() ? p.message.trim() : null,
      }));
      await invoke("start_interactive_rebase", { path: repoPath, onto, steps });
      onStarted();
    } catch (err) {
      setError(String(err));
      onError(String(err));
    } finally {
      setStarting(false);
    }
  }

  const firstKeptIndex = plan?.findIndex((p) => p.action !== "drop") ?? -1;
  const invalidFirstStep =
    firstKeptIndex >= 0 && plan
      ? plan[firstKeptIndex].action === "squash" || plan[firstKeptIndex].action === "fixup"
      : false;

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-panel rebase-plan-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>Rebase interactivo sobre {ontoLabel}</h2>
          <button className="icon-button" onClick={onClose} aria-label="Cerrar">
            <IconClose />
          </button>
        </div>

        <div className="diff-modal-body rebase-plan-body">
          {loading && <p className="hint">Cargando commits...</p>}
          {!loading && plan && plan.length === 0 && <p className="hint">No hay commits para reordenar.</p>}

          {!loading && plan && plan.length > 0 && (
            <ul className="rebase-plan-list">
              {plan.map((entry, index) => (
                <li key={entry.oid} className="rebase-plan-row">
                  <div className="rebase-plan-order">
                    <button
                      type="button"
                      className="icon-button"
                      onClick={() => move(index, -1)}
                      disabled={index === 0}
                      aria-label="Mover arriba"
                    >
                      ↑
                    </button>
                    <button
                      type="button"
                      className="icon-button"
                      onClick={() => move(index, 1)}
                      disabled={index === plan.length - 1}
                      aria-label="Mover abajo"
                    >
                      ↓
                    </button>
                  </div>

                  <select
                    className="rebase-plan-action"
                    value={entry.action}
                    onChange={(e) => setAction(index, e.currentTarget.value as RebaseAction)}
                  >
                    {(Object.keys(ACTION_LABELS) as RebaseAction[]).map((a) => (
                      <option key={a} value={a}>
                        {ACTION_LABELS[a]}
                      </option>
                    ))}
                  </select>

                  <span className="commit-sha">{entry.short_sha}</span>
                  <span className="rebase-plan-summary">{entry.summary}</span>
                  <span className="pr-author">{entry.author}</span>

                  {(entry.action === "reword" || entry.action === "squash") && (
                    <input
                      className="rebase-plan-message"
                      value={entry.message}
                      onChange={(e) => setMessage(index, e.currentTarget.value)}
                      placeholder={entry.action === "reword" ? entry.summary : "Mensaje combinado (opcional)"}
                    />
                  )}
                </li>
              ))}
            </ul>
          )}

          {invalidFirstStep && (
            <p className="error">El primer commit del plan no puede ser squash/fixup: no hay un commit anterior</p>
          )}
          {error && <p className="error">{error}</p>}

          <button
            onClick={handleStart}
            disabled={starting || loading || !plan || plan.length === 0 || invalidFirstStep}
          >
            {starting ? "Iniciando..." : "Iniciar rebase interactivo"}
          </button>
        </div>
      </div>
    </div>
  );
}

export default InteractiveRebaseModal;
