import { useEffect, useRef, useState } from "react";
import { IconChevronDown } from "./icons";

interface Props {
  onWorktrees: () => void;
  onSubmodules: () => void;
  onLfs: () => void;
}

function ToolsMenu({ onWorktrees, onSubmodules, onLfs }: Props) {
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
        Herramientas <IconChevronDown className="chevron" />
      </button>

      {open && (
        <div className="repo-switcher-menu">
          <button
            className="repo-switcher-browse"
            onClick={() => {
              setOpen(false);
              onWorktrees();
            }}
          >
            Worktrees...
          </button>
          <button
            className="repo-switcher-browse"
            onClick={() => {
              setOpen(false);
              onSubmodules();
            }}
          >
            Submódulos...
          </button>
          <button
            className="repo-switcher-browse"
            onClick={() => {
              setOpen(false);
              onLfs();
            }}
          >
            Git LFS...
          </button>
        </div>
      )}
    </div>
  );
}

export default ToolsMenu;
