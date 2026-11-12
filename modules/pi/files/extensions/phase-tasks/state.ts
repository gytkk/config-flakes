export const SNAPSHOT_TYPE = "phase-tasks.snapshot.v1";
export const MAX_PHASES = 32;
export const MAX_TASKS = 200;
export const MAX_NAME = 160;
export const MAX_STATE_BYTES = 32_000;
export const statuses = ["pending", "in_progress", "completed", "abandoned"] as const;
export type Status = (typeof statuses)[number];
export interface Task { id: string; name: string; status: Status }
export interface Phase { id: string; name: string; tasks: Task[] }
export interface TaskState { version: 1; nextId: number; phases: Phase[] }
export interface PhaseInput { name: string; tasks: string[] }
export interface TaskInput {
  action: "init" | "start" | "done" | "drop" | "append" | "remove" | "view";
  phases?: PhaseInput[];
  phase?: string;
  tasks?: string[];
  target?: string;
  replace?: boolean;
}
export const emptyState = (): TaskState => ({ version: 1, nextId: 1, phases: [] });
export const cloneState = (state: TaskState): TaskState => ({
  ...state, phases: state.phases.map(p => ({ ...p, tasks: p.tasks.map(t => ({ ...t })) })),
});
export const allTasks = (state: TaskState): Task[] => state.phases.flatMap(p => p.tasks);
export const unfinished = (task: Task): boolean => task.status === "pending" || task.status === "in_progress";

function fail(message: string): never { throw new Error(`task_list: ${message}`); }
function name(value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > MAX_NAME ||
      /[\x00-\x1f\x7f-\x9f\u2028\u2029\u202a-\u202e\u2066-\u2069]/u.test(value)) {
    fail(`names must be nonempty, single-line text of at most ${MAX_NAME} characters without terminal controls`);
  }
  return value.trim();
}
function record(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
function only(value: Record<string, unknown>, keys: string[]): void {
  if (Object.keys(value).some(k => !keys.includes(k))) fail("unexpected argument for this action");
}

/** Decode untrusted persisted data without retaining references to session entries. */
export function decodeState(value: unknown): TaskState {
  if (!record(value) || value.version !== 1 || !Number.isSafeInteger(value.nextId) ||
      (value.nextId as number) < 1 || !Array.isArray(value.phases) || value.phases.length > MAX_PHASES) {
    fail("invalid task snapshot");
  }
  const ids = new Set<string>();
  let count = 0;
  let active = 0;
  const nextId = value.nextId as number;
  const id = (value: unknown, prefix: string): string => {
    if (typeof value !== "string" || !new RegExp(`^${prefix}[1-9][0-9]*$`).test(value) ||
        !Number.isSafeInteger(Number(value.slice(1))) || Number(value.slice(1)) >= nextId || ids.has(value)) {
      fail("invalid or duplicate snapshot id");
    }
    ids.add(value);
    return value;
  };
  const phases: Phase[] = value.phases.map(p => {
    if (!record(p) || !Array.isArray(p.tasks) || p.tasks.length === 0 || p.tasks.length > MAX_TASKS) fail("invalid snapshot phase");
    const phaseId = id(p.id, "p");
    const phaseName = name(p.name);
    const tasks: Task[] = p.tasks.map(t => {
      if (++count > MAX_TASKS || !record(t) || !statuses.includes(t.status as Status)) fail("invalid snapshot task");
      if (t.status === "in_progress" && ++active > 1) fail("only one task can be in progress");
      return { id: id(t.id, "t"), name: name(t.name), status: t.status as Status };
    });
    return { id: phaseId, name: phaseName, tasks };
  });
  const state: TaskState = { version: 1, nextId: value.nextId as number, phases };
  if (Buffer.byteLength(JSON.stringify(state), "utf8") > MAX_STATE_BYTES) fail(`task state exceeds ${MAX_STATE_BYTES} bytes`);
  return state;
}

function resolve<T extends { id: string; name: string }>(items: T[], ref: unknown, kind: string): T {
  if (typeof ref !== "string" || !ref || ref.length > MAX_NAME) fail(`${kind} needs an id or exact name`);
  // IDs take precedence, so every task remains addressable even if another
  // task happens to have an ID-looking name. Names are never fuzzy-matched.
  const byId = items.find(item => item.id === ref);
  if (byId) return byId;
  const matches = items.filter(item => item.name === ref);
  if (matches.length !== 1) fail(matches.length ? `ambiguous ${kind}; use a unique id or phase-qualified exact name` : `${kind} not found`);
  return matches[0];
}

export function phaseStatus(phase: Phase): Status {
  if (phase.tasks.some(t => t.status === "in_progress")) return "in_progress";
  if (phase.tasks.every(t => t.status === "pending")) return "pending";
  if (phase.tasks.some(t => t.status === "pending")) return "in_progress";
  return phase.tasks.some(t => t.status === "abandoned") ? "abandoned" : "completed";
}

/** Pure transaction: validation and all edits happen on a private candidate. */
export function transition(previous: TaskState, input: TaskInput): TaskState {
  if (!record(input)) fail("expected action object");
  const allowed: Record<TaskInput["action"], string[]> = {
    init: ["action", "phases", "replace"], append: ["action", "phases", "phase", "tasks"],
    start: ["action", "target", "phase"], done: ["action", "target", "phase"],
    drop: ["action", "target", "phase"], remove: ["action", "target", "phase"], view: ["action"],
  };
  if (!Object.hasOwn(allowed, input.action)) fail("unknown action");
  only(input, allowed[input.action]);
  const state = cloneState(previous);
  if (input.action === "view") return state;
  const allocate = (prefix: string): string => {
    if (state.nextId >= Number.MAX_SAFE_INTEGER) fail("task id space exhausted");
    return `${prefix}${state.nextId++}`;
  };
  const makeTasks = (values: unknown): Task[] => {
    if (!Array.isArray(values) || !values.length || values.length > MAX_TASKS) fail(`supply 1–${MAX_TASKS} tasks`);
    return values.map(value => ({ id: allocate("t"), name: name(value), status: "pending" }));
  };
  const makePhases = (values: unknown): Phase[] => {
    if (!Array.isArray(values) || !values.length || values.length > MAX_PHASES) fail(`supply 1–${MAX_PHASES} phases`);
    return values.map(value => {
      if (!record(value)) fail("invalid phase");
      only(value, ["name", "tasks"]);
      return { id: allocate("p"), name: name(value.name), tasks: makeTasks(value.tasks) };
    });
  };
  if (input.action === "init") {
    if (input.replace !== undefined && typeof input.replace !== "boolean") fail("replace must be boolean");
    if (state.phases.length && input.replace !== true) fail("a plan already exists; use replace:true to explicitly replace it");
    state.phases = makePhases(input.phases);
  } else if (input.action === "append") {
    if (input.phases !== undefined) {
      if (input.phase !== undefined || input.tasks !== undefined) fail("append accepts phases OR phase + tasks, not both");
      state.phases.push(...makePhases(input.phases));
    } else {
      resolve(state.phases, input.phase, "phase").tasks.push(...makeTasks(input.tasks));
    }
  } else {
    const tasks = input.phase === undefined ? allTasks(state) : resolve(state.phases, input.phase, "phase").tasks;
    const active = allTasks(state).find(t => t.status === "in_progress");
    const task = input.target !== undefined ? resolve(tasks, input.target, "task")
      : input.action === "start" ? tasks.find(t => t.status === "pending")
      : input.action === "done" ? tasks.find(t => t.status === "in_progress") : undefined;
    if (!task) fail("no matching task; drop/remove require target, start needs pending work, done needs in-progress work");
    if (input.action === "start") {
      if (task.status !== "pending") fail("start requires a pending task");
      if (active) fail("finish or drop the current task before starting another");
      task.status = "in_progress";
    } else {
      if (input.action === "done") {
        if (task.status !== "in_progress") fail("done requires the in-progress task");
        task.status = "completed";
      } else if (input.action === "drop") {
        if (!unfinished(task)) fail("drop requires a pending or in-progress task");
        task.status = "abandoned";
      } else {
        for (const phase of state.phases) phase.tasks = phase.tasks.filter(t => t.id !== task.id);
        state.phases = state.phases.filter(p => p.tasks.length > 0);
      }
      // Only consuming the active task advances execution. Editing a pending
      // list must not start work the user/model has not started yet.
      if (active?.id === task.id) {
        const next = allTasks(state).find(t => t.status === "pending");
        if (next) next.status = "in_progress";
      }
    }
  }
  return decodeState(state);
}

export function summary(state: TaskState): string {
  const tasks = allTasks(state);
  const remaining = tasks.filter(unfinished).length;
  return `${tasks.filter(t => t.status === "completed").length}/${tasks.length} completed · ${remaining} remaining · ${tasks.filter(t => t.status === "abandoned").length} abandoned`;
}
export function treeText(state: TaskState): string {
  if (!state.phases.length) return "No tasks. Use task_list init to create a phased plan.";
  return [summary(state), ...state.phases.flatMap(phase => [
    `${phase.id} ${phase.name} [${phaseStatus(phase)}]`,
    ...phase.tasks.map(task => `  ${task.id} [${task.status}] ${task.name}`),
  ])].join("\n");
}
export function currentTask(state: TaskState): { phase: Phase; task: Task; remaining: number } | undefined {
  const tasks = allTasks(state);
  const task = tasks.find(t => t.status === "in_progress") ?? tasks.find(t => t.status === "pending");
  if (!task) return undefined;
  return { phase: state.phases.find(p => p.tasks.includes(task))!, task, remaining: tasks.filter(unfinished).length };
}
