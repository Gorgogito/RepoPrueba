import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { IconUndo, IconRedo } from "./icons";

interface UndoPreview {
  label: string | null;
}

interface Props {
  repoPath: string;
  refreshToken: number;
  onChanged: () => void;
  onError: (message: string) => void;
}

function UndoRedoControls({ repoPath, refreshToken, onChanged, onError }: Props) {
  const [undoLabel, setUndoLabel] = useState<string | null>(null);
  const [redoLabel, setRedoLabel] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    invoke<UndoPreview>("get_undo_preview", { path: repoPath })
      .then((r) => setUndoLabel(r.label))
      .catch(() => setUndoLabel(null));
    invoke<UndoPreview>("get_redo_preview", { path: repoPath })
      .then((r) => setRedoLabel(r.label))
      .catch(() => setRedoLabel(null));
  }, [repoPath, refreshToken]);

  async function run(command: string) {
    setBusy(true);
    try {
      await invoke(command, { path: repoPath });
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(false);
      onChanged();
    }
  }

  return (
    <div className="undo-redo-controls">
      <button
        className="secondary icon-button"
        disabled={busy || !undoLabel}
        onClick={() => run("undo_last_operation")}
        title={undoLabel ? `Deshacer: ${undoLabel}` : "Nada para deshacer"}
        aria-label="Deshacer"
      >
        <IconUndo />
      </button>
      <button
        className="secondary icon-button"
        disabled={busy || !redoLabel}
        onClick={() => run("redo_last_undo")}
        title={redoLabel ? `Rehacer: ${redoLabel}` : "Nada para rehacer"}
        aria-label="Rehacer"
      >
        <IconRedo />
      </button>
    </div>
  );
}

export default UndoRedoControls;
