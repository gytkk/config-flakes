// OMP labels are full snapshots, never terminal records or parent status updates.
const TTL_MS = 15_000;
const COMPLETED_MS = 5_000;
const PHASES = new Set(["running", "completed", "failed", "aborted"]);
const STATUS = { running: "working", completed: "done", failed: "blocked", aborted: "idle" };
const record = value => value !== null && typeof value === "object" && !Array.isArray(value);
const timestamp = value => Number.isSafeInteger(value) && value >= 0;

// Match the existing OMP sidebar policy: plain, single-line, at most 80 characters.
function text(value) {
  const plain = value.replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, "").replace(/[\x00-\x1f\x7f-\x9f]/g, " ").replace(/\s+/g, " ").trim();
  const chars = Array.from(plain);
  return chars.length <= 80 ? plain : `${chars.slice(0, 79).join("")}…`;
}

export function snapshotChildren(raw, parent, now = Date.now()) {
  if (typeof raw !== "string" || typeof parent !== "string" || !parent) return [];
  let snapshot;
  try { snapshot = JSON.parse(raw); } catch { return []; }
  if (!record(snapshot) || snapshot.version !== 1 || typeof snapshot.session !== "string" || !snapshot.session.trim()
    || !timestamp(snapshot.updatedAt) || snapshot.updatedAt > now || now - snapshot.updatedAt >= TTL_MS
    || typeof snapshot.model !== "string" || !Array.isArray(snapshot.children)) return [];
  const ids = new Set();
  const visible = [];
  for (const child of snapshot.children) {
    if (!record(child) || typeof child.id !== "string" || !child.id.trim() || ids.has(child.id)
      || !PHASES.has(child.phase) || typeof child.model !== "string" || typeof child.activity !== "string"
      || (child.phase === "running" ? child.endedAt !== undefined
        : !timestamp(child.endedAt) || child.endedAt > snapshot.updatedAt)) return [];
    ids.add(child.id);
    if (child.phase !== "running" && now - child.endedAt >= COMPLETED_MS) continue;
    visible.push({
      // A UI-only key, scoped by exact machine-qualified parent and session.
      address: `omp:${JSON.stringify([parent, snapshot.session, child.id])}`,
      parent,
      omp: true,
      name: text(child.id),
      phase: child.phase,
      status: STATUS[child.phase],
      model: text(child.model),
      activity: text(child.activity),
    });
  }
  return visible;
}

export function metadataChildren(terminals, now = Date.now()) {
  const children = new Map();
  for (const terminal of terminals) {
    if (terminal.state !== "live") continue;
    const visible = snapshotChildren(terminal.labels?.["omp.subagents"], terminal.address, now);
    if (visible.length) children.set(terminal.address, visible);
  }
  return children;
}
