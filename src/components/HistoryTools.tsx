import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Props {
  repoPath: string;
  onChanged: () => void;
  onError: (message: string) => void;
}

function HistoryTools({ repoPath, onChanged, onError }: Props) {
  const [sha, setSha] = useState("");
  const [busy, setBusy] = useState(false);

  async function run(command: string) {
    const commitSha = sha.trim();
    if (!commitSha) return;
    setBusy(true);
    try {
      await invoke(command, { path: repoPath, commitSha });
      setSha("");
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(false);
      // A conflict is a failed invoke() but a real state change — always
      // refresh so the conflict-resolution panel can pick it up.
      onChanged();
    }
  }

  return (
    <section className="history-tools">
      <h3>Cherry-pick / Revert por SHA</h3>
      <p className="hint small">
        Provisional hasta tener el grafo de commits: pega el SHA de un commit para aplicarlo o revertirlo.
      </p>
      <div className="history-tools-row">
        <input
          value={sha}
          onChange={(e) => setSha(e.currentTarget.value)}
          placeholder="SHA del commit"
        />
        <button className="secondary" disabled={busy || !sha.trim()} onClick={() => run("cherry_pick")}>
          Cherry-pick
        </button>
        <button className="secondary" disabled={busy || !sha.trim()} onClick={() => run("revert_commit")}>
          Revert
        </button>
      </div>
    </section>
  );
}

export default HistoryTools;
