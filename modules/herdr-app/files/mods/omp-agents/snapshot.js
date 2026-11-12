// Runtime labels are full snapshots, never terminal records or parent status updates.
const TTL_MS = 15_000;
const RUNTIMES = {
  omp: { phases: new Set(["running", "completed", "failed", "aborted"]), retention: 5_000 },
  claude: { phases: new Set(["running", "stopping", "completed", "failed", "interrupted"]), retention: 10_000 },
  pi: { phases: new Set(["pending", "running", "completed", "failed", "interrupted"]), retention: 5_000 },
};
const STATUS = { pending: "idle", running: "working", stopping: "idle", completed: "done", failed: "blocked", aborted: "idle", interrupted: "idle" };
const record = value => value !== null && typeof value === "object" && !Array.isArray(value);
const timestamp = value => Number.isSafeInteger(value) && value >= 0;

// Match the existing sidebar policy: plain, single-line, at most 80 characters.
function text(value) {
  const plain = value.replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, "").replace(/[\x00-\x1f\x7f-\x9f]/g, " ").replace(/\s+/g, " ").trim();
  const chars = Array.from(plain);
  return chars.length <= 80 ? plain : `${chars.slice(0, 79).join("")}…`;
}

export function snapshotChildren(raw, parent, runtime, now = Date.now()) {
  const policy = Object.hasOwn(RUNTIMES, runtime) ? RUNTIMES[runtime] : null;
  if (!policy || typeof raw !== "string" || typeof parent !== "string" || !parent) return [];
  let snapshot;
  try { snapshot = JSON.parse(raw); } catch { return []; }
  if (!record(snapshot) || snapshot.version !== 1 || typeof snapshot.session !== "string" || !snapshot.session.trim()
    || !timestamp(snapshot.updatedAt) || snapshot.updatedAt > now || now - snapshot.updatedAt >= TTL_MS
    || typeof snapshot.model !== "string" || !Array.isArray(snapshot.children)) return [];
  const ids = new Set();
  const visible = [];
  for (const child of snapshot.children) {
    if (!record(child) || typeof child.id !== "string" || !child.id.trim() || ids.has(child.id)
      || !policy.phases.has(child.phase) || typeof child.model !== "string" || typeof child.activity !== "string"
      || (["pending", "running", "stopping"].includes(child.phase) ? child.endedAt !== undefined
        : !timestamp(child.endedAt) || child.endedAt > snapshot.updatedAt)) return [];
    if (runtime === "pi" && typeof child.name !== "string") return [];
    if (runtime === "claude" && (typeof child.name !== "string" || typeof child.unavailable !== "boolean"
      || (child.phase === "stopping" ? !timestamp(child.stoppedAt) || child.stoppedAt > snapshot.updatedAt
        : child.phase === "running" ? child.stoppedAt !== undefined
          : child.stoppedAt !== undefined && (!timestamp(child.stoppedAt) || child.stoppedAt > snapshot.updatedAt)))) return [];
    ids.add(child.id);
    const settledAt = child.phase === "stopping" ? child.stoppedAt : child.endedAt;
    if (settledAt !== undefined && now - settledAt >= policy.retention) continue;
    const unavailable = runtime === "claude" && child.unavailable;
    visible.push({
      // A UI-only key, scoped by runtime, exact machine-qualified parent and session.
      address: `${runtime}:${JSON.stringify([parent, snapshot.session, child.id])}`,
      parent,
      metadata: true,
      runtime,
      id: child.id,
      name: text(runtime === "omp" ? child.id : child.name),
      phase: child.phase,
      unavailable,
      status: unavailable ? "idle" : STATUS[child.phase],
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
    const visible = Object.keys(RUNTIMES).flatMap(runtime => snapshotChildren(terminal.labels?.[`${runtime}.subagents`], terminal.address, runtime, now));
    if (visible.length) children.set(terminal.address, visible);
  }
  return children;
}
