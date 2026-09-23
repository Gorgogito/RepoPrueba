import { useEffect, useMemo, useRef, useState } from "react";

export interface PaletteCommand {
  id: string;
  label: string;
  group: string;
  run: () => void;
}

interface Props {
  commands: PaletteCommand[];
  onClose: () => void;
}

function CommandPalette({ commands, onClose }: Props) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    return commands.filter((c) => c.label.toLowerCase().includes(q) || c.group.toLowerCase().includes(q));
  }, [commands, query]);

  useEffect(() => {
    setSelected(0);
  }, [query]);

  useEffect(() => {
    const el = listRef.current?.children[selected] as HTMLElement | undefined;
    el?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  function execute(cmd: PaletteCommand | undefined) {
    if (!cmd) return;
    onClose();
    cmd.run();
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelected((s) => Math.min(s + 1, filtered.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelected((s) => Math.max(s - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      execute(filtered[selected]);
    } else if (e.key === "Escape") {
      onClose();
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-panel command-palette" onClick={(e) => e.stopPropagation()}>
        <input
          ref={inputRef}
          className="command-palette-input"
          value={query}
          onChange={(e) => setQuery(e.currentTarget.value)}
          onKeyDown={handleKeyDown}
          placeholder="Buscar rama, repositorio o acción..."
        />
        <ul className="command-palette-list" ref={listRef}>
          {filtered.length === 0 && <li className="hint small command-palette-empty">Sin resultados</li>}
          {filtered.map((c, i) => (
            <li
              key={c.id}
              className={i === selected ? "current" : ""}
              onMouseEnter={() => setSelected(i)}
              onClick={() => execute(c)}
            >
              <span className="command-group">{c.group}</span>
              <span className="command-label">{c.label}</span>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}

export default CommandPalette;
