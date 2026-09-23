import { useEffect, useRef, useState } from "react";
import { IconChevronDown, IconClose } from "./icons";

export interface RepoEntry {
  path: string;
  name: string;
}

interface Props {
  currentName: string | null;
  repos: RepoEntry[];
  onSwitchRepo: (path: string) => void;
  onBrowse: () => void;
  onForget: (path: string) => void;
}

function RepoSwitcher({ currentName, repos, onSwitchRepo, onBrowse, onForget }: Props) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

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
                  onClick={(e) => {
                    e.stopPropagation();
                    onForget(r.path);
                  }}
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
