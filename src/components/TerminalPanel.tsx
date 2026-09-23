import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";

interface Props {
  repoPath: string;
  onClose: () => void;
}

function base64ToUint8Array(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

function TerminalPanel({ repoPath, onClose }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    const isDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    const term = new Terminal({
      fontFamily: '"SF Mono", Consolas, monospace',
      fontSize: 13,
      cursorBlink: true,
      theme: isDark
        ? { background: "#1a1b1e", foreground: "#e5e7eb", cursor: "#e5e7eb" }
        : { background: "#f6f6f7", foreground: "#16181d", cursor: "#16181d" },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(containerRef.current);
    fit.fit();

    let unlistenOutput: UnlistenFn | undefined;
    let unlistenClosed: UnlistenFn | undefined;

    (async () => {
      unlistenOutput = await listen<string>("pty-output", (event) => {
        term.write(base64ToUint8Array(event.payload));
      });
      unlistenClosed = await listen("pty-closed", () => {
        term.write("\r\n\x1b[90m[proceso terminado]\x1b[0m\r\n");
      });
      await invoke("terminal_start", { path: repoPath, cols: term.cols, rows: term.rows });
    })();

    const dataDisposable = term.onData((data) => {
      invoke("terminal_write", { data }).catch(() => {});
    });

    const resizeDisposable = term.onResize(({ cols, rows }) => {
      invoke("terminal_resize", { cols, rows }).catch(() => {});
    });

    function handleWindowResize() {
      fit.fit();
    }
    window.addEventListener("resize", handleWindowResize);

    return () => {
      window.removeEventListener("resize", handleWindowResize);
      dataDisposable.dispose();
      resizeDisposable.dispose();
      unlistenOutput?.();
      unlistenClosed?.();
      invoke("terminal_stop").catch(() => {});
      term.dispose();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repoPath]);

  return (
    <div className="terminal-panel">
      <div className="terminal-panel-header">
        <span className="terminal-panel-title">Terminal — {repoPath}</span>
        <button className="secondary" onClick={onClose}>
          Cerrar
        </button>
      </div>
      <div className="terminal-container" ref={containerRef} />
    </div>
  );
}

export default TerminalPanel;
