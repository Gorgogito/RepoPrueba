import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { IconChevronDown, IconClose } from "./icons";

export interface RepoEntry {
  path: string;
  name: string;
}

interface Props {
  currentName: string | null;
  reposVersion: number;
  onSwitchRepo: (path: string) => void;
  onBrowse: () => void;
  onError: (message: string) => void;
}

function RepoSwitcher({ currentName, reposVersion, onSwitchRepo, onBrowse, onError }: Props) {
  const [repos, setRepos] = useState<RepoEntry[]>([]);
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    invoke<RepoEntry[]>("list_known_repos")
      .then(setRepos)
      .catch((err) => onError(String(err)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [reposVersion]);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  async function forget(path: string, e: React.MouseEvent) {
    e.stopPropagation();
    try {
      const updated = await invoke<RepoEntry[]>("remove_known_repo", { path });
      setRepos(updated);
    } catch (err) {
      onError(String(err));
    }
  }

  return (
    <div className="repo-switcher" ref={containerRef}>
      <button className="secondary repo-switcher-trigger" onClick={() => setOpen((v) => !v)}>
        {currentName ?? "Abrir repositorio"} <IconChevronDown className="chevron" />
      </button>

      {open && (
        <div className="repo-switcher-menu">
          {repos.length === 0 && <p className="hint small">Sin repositorios recientes</p>}
          <ul className="repo-switcher-list">
            {repos.map((r) => (
              <li key={r.path}>
                <button
                  className="branch-name"
                  onClick={() => {
                    setOpen(false);
                    onSwitchRepo(r.path);
                  }}
                  title={r.path}
                >
                  {r.name}
                </button>
                <button
                  className="icon-button delete"
                  onClick={(e) => forget(r.path, e)}
                  title="Quitar de la lista"
                  aria-label="Quitar de la lista"
                >
                  <IconClose />
                </button>
              </li>
            ))}
          </ul>
          <button
            className="repo-switcher-browse"
            onClick={() => {
              setOpen(false);
              onBrowse();
            }}
          >
            Explorar carpeta...
          </button>
        </div>
      )}
    </div>
  );
}

export default RepoSwitcher;
