import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface OperationStatus {
  kind: string;
  message: string;
  conflicts: string[];
}

interface Props {
  repoPath: string;
  operation: OperationStatus;
  onChanged: () => void;
  onError: (message: string) => void;
}

interface ConflictContent {
  ancestor: string | null;
  ours: string | null;
  theirs: string | null;
}

const KIND_LABELS: Record<string, string> = {
  merge: "Merge en curso",
  cherrypick: "Cherry-pick en curso",
  revert: "Revert en curso",
  rebase: "Rebase en curso",
};

function ConflictResolver({ repoPath, operation, onChanged, onError }: Props) {
  const [selected, setSelected] = useState<string | null>(null);
  const [content, setContent] = useState<ConflictContent | null>(null);
  const [editorText, setEditorText] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setSelected(null);
    setContent(null);
  }, [operation.conflicts.join(",")]);

  async function openFile(file: string) {
    setSelected(file);
    try {
      const [current, sides] = await Promise.all([
        invoke<string>("read_working_file", { path: repoPath, file }),
        invoke<ConflictContent>("get_conflict_content", { path: repoPath, file }),
      ]);
      setEditorText(current);
      setContent(sides);
    } catch (err) {
      onError(String(err));
    }
  }

  async function markResolved() {
    if (!selected) return;
    setBusy(true);
    try {
      await invoke("resolve_conflict", { path: repoPath, file: selected, content: editorText });
      setSelected(null);
      setContent(null);
      onChanged();
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleContinue() {
    setBusy(true);
    try {
      await invoke("continue_operation", { path: repoPath });
      onChanged();
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleAbort() {
    setBusy(true);
    try {
      await invoke("abort_operation", { path: repoPath });
      onChanged();
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(false);
    }
  }

  const remaining = operation.conflicts.length;

  return (
    <div className="conflict-resolver">
      <div className="conflict-header">
        <div>
          <h2>{KIND_LABELS[operation.kind] ?? "Operación en curso"}</h2>
          {operation.message && <p className="conflict-message">{operation.message.split("\n")[0]}</p>}
        </div>
        <div className="conflict-actions">
          <button className="secondary" onClick={handleAbort} disabled={busy}>
            Abortar
          </button>
          <button onClick={handleContinue} disabled={busy || remaining > 0} title={remaining > 0 ? "Resuelve todos los conflictos primero" : undefined}>
            Continuar
          </button>
        </div>
      </div>

      <div className="conflict-body">
        <ul className="conflict-file-list">
          {operation.conflicts.map((file) => (
            <li key={file} className={file === selected ? "current" : ""}>
              <button className="branch-name" onClick={() => openFile(file)}>
                ⚠ {file}
              </button>
            </li>
          ))}
          {remaining === 0 && <li className="hint small">Todos los conflictos resueltos</li>}
        </ul>

        {selected && content && (
          <div className="conflict-editor">
            <div className="conflict-editor-toolbar">
              <span className="conflict-file-name">{selected}</span>
              <button className="secondary" onClick={() => setEditorText(content.ours ?? "")}>
                Usar la mía
              </button>
              <button className="secondary" onClick={() => setEditorText(content.theirs ?? "")}>
                Usar la otra
              </button>
            </div>
            <textarea
              className="conflict-textarea"
              value={editorText}
              onChange={(e) => setEditorText(e.currentTarget.value)}
              spellCheck={false}
            />
            <button onClick={markResolved} disabled={busy}>
              Marcar como resuelto
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

export default ConflictResolver;
