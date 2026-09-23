import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { IconCherryPick, IconRevert, IconRebaseInteractive } from "./icons";

export interface CommitInfo {
  sha: string;
  short_sha: string;
  summary: string;
  author_name: string;
  author_email: string;
  timestamp: number;
  parents: string[];
  refs: string[];
}

interface GraphRow extends CommitInfo {
  lane: number;
}

interface Edge {
  fromLane: number;
  fromRow: number;
  toLane: number;
  toRow: number;
}

const LANE_COLORS = ["#396cd8", "#16a34a", "#d97706", "#dc2626", "#7c3aed", "#0891b2", "#db2777"];
const ROW_HEIGHT = 34;
const LANE_WIDTH = 18;
const GRAPH_PADDING = 10;

function laneColor(lane: number) {
  return LANE_COLORS[lane % LANE_COLORS.length];
}

/** Assigns each commit to a lane (column) and computes the parent-child
 * edges needed to draw connector lines, mirroring how `git log --graph`
 * lays branches out. */
function layoutGraph(commits: CommitInfo[]): { rows: GraphRow[]; edges: Edge[]; laneCount: number } {
  const active: (string | null)[] = [];
  const rows: GraphRow[] = [];
  const rowIndexBySha = new Map<string, number>();
  const laneAtRow: number[] = [];

  commits.forEach((commit, rowIndex) => {
    let lane = active.indexOf(commit.sha);
    if (lane === -1) {
      lane = active.indexOf(null);
      if (lane === -1) {
        lane = active.length;
        active.push(null);
      }
    }

    active[lane] = commit.parents[0] ?? null;
    for (let i = 1; i < commit.parents.length; i++) {
      const parentSha = commit.parents[i];
      if (active.includes(parentSha)) continue;
      const freeLane = active.indexOf(null);
      if (freeLane === -1) {
        active.push(parentSha);
      } else {
        active[freeLane] = parentSha;
      }
    }

    rows.push({ ...commit, lane });
    rowIndexBySha.set(commit.sha, rowIndex);
    laneAtRow.push(lane);
  });

  const edges: Edge[] = [];
  rows.forEach((row, rowIndex) => {
    for (const parentSha of row.parents) {
      const parentRow = rowIndexBySha.get(parentSha);
      if (parentRow === undefined) continue; // parent outside the loaded window
      edges.push({
        fromLane: row.lane,
        fromRow: rowIndex,
        toLane: laneAtRow[parentRow],
        toRow: parentRow,
      });
    }
  });

  return { rows, edges, laneCount: active.length };
}

function formatDate(unixSeconds: number) {
  return new Date(unixSeconds * 1000).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

interface Props {
  repoPath: string;
  refreshToken: number;
  onChanged: () => void;
  onError: (message: string) => void;
  onViewDiff: (sha: string, label: string) => void;
  onInteractiveRebase: (onto: string, ontoLabel: string) => void;
}

function CommitGraph({ repoPath, refreshToken, onChanged, onError, onViewDiff, onInteractiveRebase }: Props) {
  const [commits, setCommits] = useState<CommitInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [busySha, setBusySha] = useState<string | null>(null);

  useEffect(() => {
    setLoading(true);
    invoke<CommitInfo[]>("get_commit_log", { path: repoPath, limit: 300 })
      .then(setCommits)
      .catch((err) => onError(String(err)))
      .finally(() => setLoading(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repoPath, refreshToken]);

  const { rows, edges, laneCount } = useMemo(() => layoutGraph(commits), [commits]);
  const graphWidth = GRAPH_PADDING * 2 + laneCount * LANE_WIDTH;
  const graphHeight = rows.length * ROW_HEIGHT;

  function laneX(lane: number) {
    return GRAPH_PADDING + lane * LANE_WIDTH + LANE_WIDTH / 2;
  }
  function rowY(row: number) {
    return row * ROW_HEIGHT + ROW_HEIGHT / 2;
  }

  async function runAction(command: string, sha: string) {
    setBusySha(sha);
    try {
      await invoke(command, { path: repoPath, commitSha: sha });
    } catch (err) {
      onError(String(err));
    } finally {
      setBusySha(null);
      onChanged();
    }
  }

  if (loading && rows.length === 0) {
    return <p className="hint">Cargando historial...</p>;
  }
  if (rows.length === 0) {
    return <p className="hint">Sin commits todavía.</p>;
  }

  return (
    <div className="commit-graph">
      <svg width={graphWidth} height={graphHeight} className="commit-graph-svg">
        {edges.map((e, i) => {
          const x1 = laneX(e.fromLane);
          const y1 = rowY(e.fromRow);
          const x2 = laneX(e.toLane);
          const y2 = rowY(e.toRow);
          const path =
            e.fromLane === e.toLane
              ? `M ${x1} ${y1} L ${x2} ${y2}`
              : `M ${x1} ${y1} C ${x1} ${(y1 + y2) / 2}, ${x2} ${(y1 + y2) / 2}, ${x2} ${y2}`;
          return <path key={i} d={path} stroke={laneColor(e.toLane)} strokeWidth={2} fill="none" />;
        })}
        {rows.map((row, i) => (
          <circle key={row.sha} cx={laneX(row.lane)} cy={rowY(i)} r={5} fill={laneColor(row.lane)} />
        ))}
      </svg>

      <div className="commit-list">
        {rows.map((row) => (
          <div key={row.sha} className="commit-row" style={{ height: ROW_HEIGHT }}>
            <div className="commit-summary">
              {row.refs.map((ref) => (
                <span key={ref} className="ref-badge">
                  {ref}
                </span>
              ))}
              <button className="commit-message" onClick={() => onViewDiff(row.sha, row.summary)}>
                {row.summary}
              </button>
            </div>
            <span className="commit-author">{row.author_name}</span>
            <span className="commit-date">{formatDate(row.timestamp)}</span>
            <span className="commit-sha">{row.short_sha}</span>
            <span className="commit-row-actions">
              <button
                className="icon-button"
                disabled={busySha !== null}
                onClick={() => runAction("cherry_pick", row.sha)}
                title="Cherry-pick este commit sobre la rama actual"
                aria-label="Cherry-pick este commit sobre la rama actual"
              >
                <IconCherryPick />
              </button>
              <button
                className="icon-button"
                disabled={busySha !== null}
                onClick={() => runAction("revert_commit", row.sha)}
                title="Revertir este commit"
                aria-label="Revertir este commit"
              >
                <IconRevert />
              </button>
              <button
                className="icon-button"
                disabled={busySha !== null || row.parents.length === 0}
                onClick={() =>
                  onInteractiveRebase(
                    row.parents[0],
                    rows.find((r) => r.sha === row.parents[0])?.short_sha ?? row.parents[0].slice(0, 7)
                  )
                }
                title="Rebase interactivo desde aquí"
                aria-label="Rebase interactivo desde aquí"
              >
                <IconRebaseInteractive />
              </button>
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

export default CommitGraph;
