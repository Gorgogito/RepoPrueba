import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { IconStashPop, IconStashApply, IconClose } from "./icons";

export interface StashInfo {
  index: number;
  message: string;
}

interface Props {
  repoPath: string;
  stashes: StashInfo[];
  canStash: boolean;
  onChanged: () => void;
  onError: (message: string) => void;
}

function StashPanel({ repoPath, stashes, canStash, onChanged, onError }: Props) {
  const [message, setMessage] = useState("");
  const [saving, setSaving] = useState(false);

  async function handleSave(e: React.FormEvent) {
    e.preventDefault();
    setSaving(true);
    try {
      await invoke("stash_save", { path: repoPath, message: message.trim() || null });
      setMessage("");
      onChanged();
    } catch (err) {
      onError(String(err));
    } finally {
      setSaving(false);
    }
  }

  async function run(command: string, index: number) {
    try {
      await invoke(command, { path: repoPath, index });
      onChanged();
    } catch (err) {
      onError(String(err));
    }
  }

  return (
    <aside className="branch-sidebar">
      <div className="branch-sidebar-header">
        <h3>Stash</h3>
      </div>

      <form className="new-branch-form" onSubmit={handleSave}>
        <input
          value={message}
          onChange={(e) => setMessage(e.currentTarget.value)}
          placeholder="Mensaje (opcional)"
          disabled={!canStash}
        />
        <button type="submit" disabled={!canStash || saving} title="Guardar cambios en un stash">
          Guardar
        </button>
      </form>

      {stashes.length === 0 ? (
        <p className="hint small">Sin stashes guardados</p>
      ) : (
        <ul className="branch-list">
          {stashes.map((s) => (
            <li key={s.index}>
              <span className="branch-name stash-entry" title={s.message}>
                {s.message}
              </span>
              <span className="branch-actions">
                <button
                  className="icon-button"
                  onClick={() => run("stash_pop", s.index)}
                  title="Aplicar y quitar"
                  aria-label="Aplicar y quitar"
                >
                  <IconStashPop />
                </button>
                <button
                  className="icon-button"
                  onClick={() => run("stash_apply", s.index)}
                  title="Aplicar (mantener)"
                  aria-label="Aplicar (mantener)"
                >
                  <IconStashApply />
                </button>
                <button
                  className="icon-button delete"
                  onClick={() => run("stash_drop", s.index)}
                  title="Eliminar"
                  aria-label="Eliminar"
                >
                  <IconClose />
                </button>
              </span>
            </li>
          ))}
        </ul>
      )}
    </aside>
  );
}

export default StashPanel;
