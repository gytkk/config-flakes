import { afterEach, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { cp, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { settings } from "./module-config";

const moduleRoot = fileURLToPath(new URL("../", import.meta.url));
const piEntry = fileURLToPath(import.meta.resolve("@earendil-works/pi-coding-agent"));
const roots: string[] = [];
afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

async function fixture() {
  const root = await mkdtemp(join(tmpdir(), "pi-loader-test-"));
  roots.push(root);
  const agent = join(root, "agent");
  const cwd = join(root, "project");
  await mkdir(join(agent, "extensions"), { recursive: true });
  await mkdir(cwd);
  return { root, agent, cwd };
}

function load(agent: string, cwd: string) {
  // Pi uses Node and jiti; Bun imports alone do not validate runtime discovery.
  const result = spawnSync("node", ["--input-type=module", "-e", `
    import { pathToFileURL } from "node:url";
    const { discoverAndLoadExtensions } = await import(pathToFileURL(process.argv[1]).href);
    const loaded = await discoverAndLoadExtensions([], process.argv[2], process.env.PI_CODING_AGENT_DIR);
    console.log(JSON.stringify({
      errors: loaded.errors,
      warnings: loaded.warnings ?? [],
      extensions: loaded.extensions.map(extension => ({
        path: extension.path,
        tools: [...extension.tools.keys()],
        commands: [...extension.commands.keys()],
        messageRenderers: [...extension.messageRenderers.keys()],
      })),
    }));
  `, piEntry, cwd], {
    cwd, encoding: "utf8", timeout: 30_000,
    env: { ...process.env, HOME: dirname(agent), XDG_CACHE_HOME: join(dirname(agent), "cache"), PI_CODING_AGENT_DIR: agent, JITI_FS_CACHE: "0", PI_SUBAGENT_CHILD: "0" },
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`Pi loader exited ${result.status}: ${result.stderr}\n${result.stdout}`);
  const loaded = JSON.parse(result.stdout);
  expect(loaded.errors).toEqual([]);
  expect(loaded.warnings).toEqual([]);
  return loaded.extensions as Array<{ path: string; tools: string[]; commands: string[]; messageRenderers: string[] }>;
}

const powerlinePackage = process.env.PI_POWERLINE_PACKAGE_DIR;
test("discovers exact Powerline once with status providers and no local prompt UI", async () => {
  if (!powerlinePackage) throw new Error("Set PI_POWERLINE_PACKAGE_DIR to the unpacked pi-powerline-footer@0.22.0 release.");
  const source = resolve(powerlinePackage);
  const manifest = JSON.parse(await readFile(join(source, "package.json"), "utf8"));
  expect(`npm:${manifest.name}@${manifest.version}`).toBe("npm:pi-powerline-footer@0.22.0");
  expect(settings.packages).toContain(`npm:${manifest.name}@${manifest.version}`);
  const { agent, cwd } = await fixture();
  await writeFile(join(agent, "settings.json"), JSON.stringify(settings));
  await symlink(source, join(agent, "extensions/powerline"));
  for (const name of ["codex-fast-mode.ts", "codex-usage.ts", "rounded-tools.ts"]) {
    await symlink(join(moduleRoot, "files/extensions", name), join(agent, "extensions", name));
  }
  const extensions = load(agent, cwd);
  expect(extensions).toHaveLength(4);
  expect(extensions.filter(extension => extension.commands.includes("powerline"))).toHaveLength(1);
  expect(extensions.some(extension => extension.path.includes("prompt-ui"))).toBe(false);
  const commands = extensions.flatMap(extension => extension.commands);
  expect(new Set(commands).size).toBe(commands.length);
  expect(commands).toContain("fast");
  expect(commands).toContain("codex-usage");
}, 60_000);

const lensPackage = process.env.PI_LENS_PACKAGE_DIR;
const shellPackage = process.env.PI_INTERACTIVE_SHELL_PACKAGE_DIR;
test.skipIf(!lensPackage || !shellPackage)("migration packages coexist without duplicate tools or commands", async () => {
  if (!powerlinePackage) throw new Error("Set PI_POWERLINE_PACKAGE_DIR to the exact release.");
  const { root, agent, cwd } = await fixture();
  await writeFile(join(agent, "settings.json"), JSON.stringify(settings));
  await cp(join(moduleRoot, "files/interactive-shell.json"), join(agent, "interactive-shell.json"));
  await mkdir(join(root, ".pi-lens"));
  await cp(join(moduleRoot, "files/lens.json"), join(root, ".pi-lens/config.json"));
  for (const [name, source] of [["powerline", powerlinePackage], ["interactive-shell", shellPackage!], ["lens", lensPackage!]]) {
    await symlink(resolve(source), join(agent, "extensions", name));
  }
  for (const name of ["codex-fast-mode.ts", "codex-usage.ts", "rounded-tools.ts"]) {
    await symlink(join(moduleRoot, "files/extensions", name), join(agent, "extensions", name));
  }
  const extensions = load(agent, cwd);
  expect(extensions).toHaveLength(6);
  const tools = extensions.flatMap(extension => extension.tools);
  const commands = extensions.flatMap(extension => extension.commands);
  expect(new Set(tools).size).toBe(tools.length);
  expect(new Set(commands).size).toBe(commands.length);
  for (const name of ["enable_interactive_shell", "interactive_shell", "lsp_navigation", "lens_diagnostics"]) expect(tools).toContain(name);
  for (const name of ["bash", "bash_output", "lsp_hover", "ast_apply"]) expect(tools).not.toContain(name);
  expect(commands).toContain("powerline");
  expect(commands).not.toContain("bg");
}, 60_000);

test.skipIf(!shellPackage)("loads interactive shell once alongside rounded tools without replacing bash", async () => {
  const source = resolve(shellPackage!);
  const pin = settings.packages.find((pkg: { source?: string }) => pkg.source?.startsWith("npm:pi-interactive-shell@"));
  const manifest = JSON.parse(await readFile(join(source, "package.json"), "utf8"));
  expect(`npm:${manifest.name}@${manifest.version}`).toBe(pin.source);
  expect(pin.skills).toEqual([]);
  expect(pin.prompts).toEqual([]);
  const { agent, cwd } = await fixture();
  await cp(join(moduleRoot, "files/interactive-shell.json"), join(agent, "interactive-shell.json"));
  await symlink(source, join(agent, "extensions/interactive-shell"));
  await symlink(join(moduleRoot, "files/extensions/rounded-tools.ts"), join(agent, "extensions/rounded-tools.ts"));
  const extensions = load(agent, cwd);
  expect(extensions).toHaveLength(2);
  const tools = extensions.flatMap(extension => extension.tools);
  expect(tools.sort()).toEqual(["enable_interactive_shell", "interactive_shell"]);
  expect(extensions.flatMap(extension => extension.commands)).not.toContain("bg");
}, 60_000);
