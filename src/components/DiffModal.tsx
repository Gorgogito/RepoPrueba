import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface DiffLine {
  origin: string;
  content: string;
  old_lineno: number | null;
  new_lineno: number | null;
}

interface DiffHunk {
  header: string;
  lines: DiffLine[];
}

interface FileDiff {
  path: string;
  old_path: string | null;
  status: string;
  is_binary: boolean;
  hunks: DiffHunk[];
}

export type DiffRequest =
  | { kind: "working"; file: string; staged: boolean }
  | { kind: "commit"; sha: string; label: string };

interface Props {
  repoPath: string;
  request: DiffRequest;
  onClose: () => void;
}

function lineClass(origin: string) {
  if (origin === "+") return "add";
  if (origin === "-") return "del";
  return "ctx";
}

function marker(origin: string) {
  return origin === "+" || origin === "-" ? origin : "";
}

function DiffModal({ repoPath, request, onClose }: Props) {
  const [files, setFiles] = useState<FileDiff[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setFiles(null);
    setError(null);
    if (request.kind === "working") {
      invoke<FileDiff>("get_working_diff", { path: repoPath, file: request.file, staged: request.staged })
        .then((f) => setFiles([f]))
        .catch((err) => setError(String(err)));
    } else {
      invoke<FileDiff[]>("get_commit_diff", { path: repoPath, sha: request.sha })
        .then(setFiles)
        .catch((err) => setError(String(err)));
    }
  }, [repoPath, request]);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  const title = request.kind === "working" ? request.file : request.label;

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-panel diff-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>{title}</h2>
          <button className="secondary" onClick={onClose}>
            Cerrar
          </button>
        </div>

        <div className="diff-modal-body">
          {error && <p className="error">{error}</p>}
          {!error && !files && <p className="hint">Cargando diff...</p>}

          {files?.map((f) => (
            <div key={f.path} className="file-diff">
              <div className="file-diff-header">
                <span className={`status-tag ${f.status}`}>{f.status}</span>
                <span className="file-diff-path">
                  {f.old_path && f.old_path !== f.path ? `${f.old_path} → ${f.path}` : f.path}
                </span>
              </div>

              {f.is_binary ? (
                <p className="hint small">Archivo binario</p>
              ) : f.hunks.length === 0 ? (
                <p className="hint small">Sin cambios de contenido</p>
              ) : (
                f.hunks.map((h, hi) => (
                  <div key={hi} className="diff-hunk">
                    <div className="diff-hunk-header">{h.header}</div>
                    {h.lines.map((l, li) => (
                      <div key={li} className={`diff-line diff-line-${lineClass(l.origin)}`}>
                        <span className="diff-lineno">{l.old_lineno ?? ""}</span>
                        <span className="diff-lineno">{l.new_lineno ?? ""}</span>
                        <span className="diff-marker">{marker(l.origin)}</span>
                        <span className="diff-content">{l.content}</span>
                      </div>
                    ))}
                  </div>
                ))
              )}
            </div>
          ))}

          {files && files.length === 0 && <p className="hint">Sin cambios.</p>}
        </div>
      </div>
    </div>
  );
}

export default DiffModal;
