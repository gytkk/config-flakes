import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { homedir, tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { workerCompletion } from "../files/extensions/agent-hub/completion";
import { foregroundRows, restoreRecords } from "../files/extensions/agent-hub/records";
import { settings } from "./module-config";

const completed = { contract: "pi-worker-completion-v1", status: "completed", checks: [{ command: "node --test fixture.test.mjs", result: "passed", evidence: "Fixture: 2 passed, 0 failed" }], incomplete: [] };
const cases: Array<{ value: unknown; valid: boolean }> = [
  { value: completed, valid: true },
  { value: { ...completed, checks: [] }, valid: true },
  { value: { ...completed, status: "blocked", checks: [{ command: "npm test", result: "not-run", evidence: "Needs approval" }], incomplete: ["Required test not run"] }, valid: true },
  { value: { ...completed, status: "failed", checks: [{ command: "npm test", result: "failed", evidence: "Assertion failed" }], incomplete: ["Fix assertion"], summary: "Partial change" }, valid: true },
  { value: { ...completed, incomplete: ["Required integration test still pending"] }, valid: false },
  { value: { ...completed, status: "success" }, valid: false },
  { value: { ...completed, contract: "another-contract" }, valid: false },
  { value: { status: "completed" }, valid: false },
  { value: "completed", valid: false },
  { value: null, valid: false },
  { value: { ...completed, checks: [{ ...completed.checks[0], result: "success" }] }, valid: false },
  { value: { ...completed, checks: [{ command: "npm test", result: "passed" }] }, valid: false },
  { value: { ...completed, checks: [{ ...completed.checks[0], command: " " }] }, valid: false },
  { value: { ...completed, checks: [{ ...completed.checks[0], evidence: "" }] }, valid: false },
  { value: { ...completed, checks: [{ ...completed.checks[0], command: "x".repeat(1001) }] }, valid: false },
  { value: { ...completed, checks: [{ ...completed.checks[0], evidence: "x".repeat(1001) }] }, valid: false },
  { value: { ...completed, checks: Array(17).fill(completed.checks[0]) }, valid: false },
  { value: { ...completed, status: "blocked", incomplete: Array(17).fill("pending") }, valid: false },
  { value: { ...completed, status: "blocked", incomplete: ["x".repeat(1001)] }, valid: false },
  { value: { ...completed, status: "blocked", incomplete: [" "] }, valid: false },
  { value: { ...completed, summary: "x".repeat(2001) }, valid: false },
  { value: { ...completed, summary: " " }, valid: false },
  { value: { ...completed, summary: "😀".repeat(2000) }, valid: true },
  { value: { ...completed, extra: true }, valid: false },
  { value: { ...completed, checks: [{ ...completed.checks[0], extra: true }] }, valid: false },
  { value: { ...completed, summary: "한".repeat(2000), checks: Array(16).fill({ command: "x".repeat(1000), result: "passed", evidence: "x".repeat(1000) }) }, valid: true },
];

test("installed 0.84 discovery, structured_output, acceptance and configured /run routing work without launching a child", () => {
  const installed = process.env.PI_SUBAGENTS_PACKAGE_DIR ?? join(homedir(), ".pi/agent/npm/node_modules/pi-subagents");
  expect(JSON.parse(readFileSync(join(installed, "package.json"), "utf8")).version).toBe("0.84.0");
  const root = mkdtempSync(join(tmpdir(), "pi-worker-contract-"));
  try {
    const pkg = join(root, "package");
    mkdirSync(pkg);
    for (const name of ["src", "agents", "package.json"]) cpSync(join(installed, name), join(pkg, name), { recursive: true });
    const deps = join(pkg, "node_modules");
    mkdirSync(deps);
    const hostModules = fileURLToPath(new URL("../node_modules/", import.meta.url));
    symlinkSync(join(hostModules, "@earendil-works"), join(deps, "@earendil-works"));
    symlinkSync(join(hostModules, "typebox"), join(deps, "typebox"));
    const requireInstalled = createRequire(join(installed, "package.json"));
    for (const name of ["yaml", "acorn", "jiti", "undici", "@js-temporal/polyfill"]) {
      mkdirSync(dirname(join(deps, name)), { recursive: true });
      symlinkSync(dirname(requireInstalled.resolve(`${name}/package.json`)), join(deps, name));
    }
    const agent = join(root, "agent");
    mkdirSync(join(agent, "agents"), { recursive: true });
    cpSync(fileURLToPath(new URL("../agents/worker.md", import.meta.url)), join(agent, "agents/worker.md"));
    mkdirSync(join(agent, "extensions/subagent"), { recursive: true });
    cpSync(fileURLToPath(new URL("../files/extensions/subagent/config.json", import.meta.url)), join(agent, "extensions/subagent/config.json"));
    // Only model/context overrides, with package discovery disabled: no live settings or package installs.
    writeFileSync(join(agent, "settings.json"), JSON.stringify({ subagents: settings.subagents }));
    writeFileSync(join(root, "cases.json"), JSON.stringify(cases));
    const script = `
      import assert from 'node:assert/strict';
      import { EventEmitter } from 'node:events';
      import { SessionManager } from './package/node_modules/@earendil-works/pi-coding-agent/dist/index.js';
      import { loadConfig } from './package/src/extension/config.js';
      import { resolveDisabledFeatureSurface, disabledFeatureUseError } from './package/src/shared/disabled-features.js';
      import { registerSlashCommands } from './package/src/slash/slash-commands.js';
      import { readFileSync } from 'node:fs';
      import { discoverAgents } from './package/src/agents/agents.js';
      import { compactForegroundDetails } from './package/src/shared/utils.js';
      import { resolveStepBehavior, resolveEffectiveOutputSchema } from './package/src/runs/shared/child-launch-plan.js';
      import { resolveEffectiveAcceptance, resolveAcceptanceReportMode } from './package/src/runs/shared/acceptance.js';
      import { createStructuredOutputRuntime, createStructuredOutputFileCapture, clearStructuredOutputCaptures, readStructuredOutput, readStructuredOutputAcceptanceReport } from './package/src/runs/shared/structured-output.js';
      import registerRuntime from './package/src/runs/shared/subagent-prompt-runtime.js';
      const discovery = discoverAgents(process.cwd(), 'user', undefined, { globalNpmRoot: process.cwd() + '/empty-npm' });
      assert.deepEqual(discovery.agentDiagnostics, []);
      const worker = discovery.agents.find(a => a.name === 'worker');
      assert.equal(worker.source, 'user');
      assert.equal(worker.acceptanceRole, 'writer');
      assert.equal(worker.model, 'openai/gpt-6.1-sol');
      assert.equal(worker.thinking, 'medium');
      assert.equal(worker.inheritProjectContext, true);
      assert.equal(worker.inheritGlobalContext, false);
      assert.equal(worker.defaultContext, 'fresh');
      assert.equal(worker.defaultProgress, false);
      assert.equal(worker.defaultReads, undefined);
      const schema = worker.outputSchema;
      assert.equal(schema.properties.contract.const, 'pi-worker-completion-v1');
      const behavior = resolveStepBehavior(worker, {});
      assert.equal(behavior.output, false);
      assert.equal(behavior.progress, false);
      assert.equal(behavior.reads, false);
      assert.deepEqual(behavior.outputSchema, schema);
      assert.equal(resolveEffectiveOutputSchema(worker, false), undefined);
      assert.deepEqual(resolveEffectiveOutputSchema(worker, { type: 'string' }), { type: 'string' });
      const acceptance = resolveEffectiveAcceptance({ acceptanceRole: worker.acceptanceRole, async: false });
      assert.equal(acceptance.level, 'checked');
      assert.deepEqual(acceptance.verify, []);
      assert.equal(resolveAcceptanceReportMode(undefined), 'optional');
      assert.equal(resolveAcceptanceReportMode({ report: 'on' }), 'required');
      assert.equal(resolveAcceptanceReportMode({ report: 'off' }), 'off');
      const runtime = createStructuredOutputRuntime(schema, process.cwd(), { acceptanceReport: 'required' });
      assert.match((await readStructuredOutput(runtime)).error, /Missing structured_output/);
      const tools = new Map();
      // Register only native protocol hooks; no session factory, event firing, provider or command execution.
      registerRuntime({ on() {}, events: { on() {} }, registerTool(t) { tools.set(t.name, t); } }, {
        waitTool: { enabled: false }, structuredOutput: { schema, acceptanceReport: 'required', capture: createStructuredOutputFileCapture(runtime) }
      });
      const tool = tools.get('structured_output');
      assert(tool);
      assert.deepEqual(tool.parameters.required, ['value', 'acceptanceReport']);
      const report = { changedFiles: [], testsAddedOrUpdated: [], commandsRun: [{ command: 'fixture check', result: 'passed', summary: 'Fixture evidence only' }], residualRisks: [], noStagedFiles: true };
      const cases = JSON.parse(readFileSync('cases.json', 'utf8'));
      await assert.rejects(tool.execute('missing-report', { value: cases[0].value }), /Missing acceptanceReport/);
      assert.match((await readStructuredOutput(runtime)).error, /Missing structured_output/);
      const validities = [];
      for (const entry of cases) {
        assert.equal(clearStructuredOutputCaptures(runtime), undefined);
        try {
          const response = await tool.execute('fixture', { value: entry.value, acceptanceReport: report });
          assert.equal(response.terminate, true);
          assert.deepEqual((await readStructuredOutput(runtime)).value, entry.value);
          assert.deepEqual(readStructuredOutputAcceptanceReport(runtime).value, report);
          validities.push(true);
        } catch (error) {
          if (!String(error).includes('Structured output validation failed')) throw error;
          assert.match((await readStructuredOutput(runtime)).error, /Missing structured_output/);
          validities.push(false);
        }
      }
      const nativeDetails = compactForegroundDetails({ mode: 'single', runId: 'fixture-run', results: [{ index: 0, agent: 'worker', task: 'Fixture task', exitCode: 0, structuredOutput: cases[0].value, messages: [] }] });
      assert.equal(nativeDetails.results[0].task, '[prompt redacted]');
      assert.deepEqual(nativeDetails.results[0].structuredOutput, cases[0].value);
      const config = loadConfig();
      const disabled = resolveDisabledFeatureSurface(config);
      assert.deepEqual(config.disabledFeatures, ['workflow-scripts']);
      assert.equal(config.autoResume, false);
      assert.equal(disabledFeatureUseError({ action: 'resume', id: 'fixture-run' }, disabled), undefined);
      assert.match(disabledFeatureUseError({ workflowScript: 'return null' }, disabled), /workflow-scripts/);
      const commands = new Map();
      const bus = new EventEmitter();
      const slashMessages = [];
      const slashSession = SessionManager.inMemory(process.cwd());
      const requests = [];
      let finishSlash;
      const finishedSlash = new Promise(resolve => { finishSlash = resolve; });
      const pi = {
        on() {}, registerTool() {},
        registerCommand(name, command) { commands.set(name, command); },
        events: {
          on(name, listener) { bus.on(name, listener); return () => bus.off(name, listener); },
          emit(name, value) { bus.emit(name, value); },
        },
        sendMessage(message) {
          slashMessages.push(structuredClone(message));
          slashSession.appendCustomMessageEntry(message.customType, message.content, message.display, message.details);
          if (slashMessages.length === 2) finishSlash();
        },
      };
      // Mock only the executor bridge: real config loading, feature resolution,
      // slash registration/handler, discovery and message producers run unchanged.
      bus.on('subagent:slash:request', ({ requestId, params }) => {
        requests.push(params);
        bus.emit('subagent:slash:started', { requestId });
        bus.emit('subagent:slash:response', { requestId, result: { content: [], details: nativeDetails } });
      });
      const registration = registerSlashCommands(pi, { baseCwd: process.cwd() }, {
        workflowScriptsDisabled: disabled.features.has('workflow-scripts'),
      });
      try {
        await commands.get('run').handler('worker Fixture task', {
          cwd: process.cwd(), hasUI: false,
          ui: { notify(message) { throw new Error(message); } },
        });
        assert.deepEqual(requests, [{ agent: 'worker', task: 'Fixture task', agentScope: 'both', async: false }]);
        assert.equal(disabledFeatureUseError(requests[0], disabled), undefined);
        await finishedSlash;
        assert.equal(slashMessages[0].details.result.details.mode, 'single');
        assert.equal(slashMessages[1].details.result.details.mode, 'single');
        assert.equal(slashMessages[1].details.result.details.runId, 'fixture-run');
        assert.equal(slashMessages[0].details.requestId, slashMessages[1].details.requestId);
      } finally { registration.dispose(); }
      console.log(JSON.stringify({ validities, schema, acceptance: acceptance.level, nativeDetails, slashEntries: slashSession.getBranch() }));
    `;
    writeFileSync(join(root, "probe.mjs"), script);
    const probe = spawnSync("node", [join(root, "probe.mjs")], {
      cwd: root, encoding: "utf8", timeout: 30000,
      env: { ...process.env, HOME: root, PI_CODING_AGENT_DIR: agent, PI_SUBAGENT_EXTRA_AGENT_DIRS: "", PI_SUBAGENTS_PI_CODING_AGENT_PACKAGE_ROOT: join(hostModules, "@earendil-works/pi-coding-agent") },
    });
    expect(probe.stderr).toBe("");
    expect(probe.status).toBe(0);
    const output = JSON.parse(probe.stdout);
    expect(output.validities).toEqual(cases.map(entry => entry.valid));
    for (const entry of cases) expect(!!workerCompletion(entry.value)).toBe(entry.valid);
    expect(output.acceptance).toBe("checked");
    expect(foregroundRows(output.nativeDetails, true)[0].completion).toEqual(completed);
    const slashRows = [...restoreRecords(output.slashEntries, { sessionId: "fixture-parent", sessionFile: "/sessions/fixture-parent.jsonl" }, false).children.values()];
    expect(slashRows).toHaveLength(1);
    expect(slashRows[0]).toMatchObject({ runId: "fixture-run", index: 0, agent: "worker", task: "Fixture task", completion: completed });
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 60000);
