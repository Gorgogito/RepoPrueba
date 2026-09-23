import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface RemoteInfo {
  name: string;
  url: string;
}

interface Props {
  repoPath: string;
  remotes: RemoteInfo[];
  currentBranch: string;
  onChanged: () => void;
  onError: (message: string) => void;
}

type Busy = "fetch" | "pull" | "push" | null;

function RemoteControls({ repoPath, remotes, currentBranch, onChanged, onError }: Props) {
  const [busy, setBusy] = useState<Busy>(null);
  const [addingRemote, setAddingRemote] = useState(false);
  const [remoteUrl, setRemoteUrl] = useState("");

  const remote = remotes[0];

  async function run(action: Busy, command: string, args: Record<string, unknown>) {
    setBusy(action);
    try {
      await invoke(command, args);
      onChanged();
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleAddRemote(e: React.FormEvent) {
    e.preventDefault();
    const url = remoteUrl.trim();
    if (!url) return;
    try {
      await invoke("add_remote", { path: repoPath, name: "origin", url });
      setRemoteUrl("");
      setAddingRemote(false);
      onChanged();
    } catch (err) {
      onError(String(err));
    }
  }

  if (!remote) {
    return addingRemote ? (
      <form className="remote-form" onSubmit={handleAddRemote}>
        <input
          autoFocus
          value={remoteUrl}
          onChange={(e) => setRemoteUrl(e.currentTarget.value)}
          placeholder="https://github.com/usuario/repo.git"
        />
        <button type="submit">Agregar</button>
      </form>
    ) : (
      <button className="secondary" onClick={() => setAddingRemote(true)}>
        Agregar remoto
      </button>
    );
  }

  return (
    <div className="remote-controls">
      <button
        className="secondary"
        disabled={busy !== null}
        onClick={() => run("fetch", "fetch", { path: repoPath, remoteName: remote.name })}
      >
        {busy === "fetch" ? "Fetch..." : "Fetch"}
      </button>
      <button
        className="secondary"
        disabled={busy !== null}
        onClick={() => run("pull", "pull", { path: repoPath, remoteName: remote.name })}
      >
        {busy === "pull" ? "Pull..." : "Pull"}
      </button>
      <button
        disabled={busy !== null}
        onClick={() => run("push", "push", { path: repoPath, remoteName: remote.name, branch: currentBranch })}
      >
        {busy === "push" ? "Push..." : "Push"}
      </button>
    </div>
  );
}

export default RemoteControls;
