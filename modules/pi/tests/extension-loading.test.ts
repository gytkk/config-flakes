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
    encoding: "utf8", timeout: 30_000,
    env: { ...process.env, PI_CODING_AGENT_DIR: agent, JITI_FS_CACHE: "0", PI_SUBAGENT_CHILD: "0" },
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`Pi loader exited ${result.status}: ${result.stderr}\n${result.stdout}`);
  const loaded = JSON.parse(result.stdout);
  expect(loaded.errors).toEqual([]);
  expect(loaded.warnings).toEqual([]);
  return loaded.extensions as Array<{ path: string; tools: string[]; commands: string[]; messageRenderers: string[] }>;
}

test("discovers prompt-ui once, without loading its helpers as extensions", async () => {
  const { agent, cwd } = await fixture();
  await symlink(join(moduleRoot, "files/extensions/prompt-ui"), join(agent, "extensions/prompt-ui"));
  let extensions = load(agent, cwd);
  expect(extensions).toHaveLength(1);
  expect(extensions[0].path).toEndWith("prompt-ui/index.ts");
  expect(extensions[0].commands).toEqual([]);
  await symlink(join(moduleRoot, "files/extensions/codex-fast-mode.ts"), join(agent, "extensions/codex-fast-mode.ts"));
  extensions = load(agent, cwd);
  expect(extensions).toHaveLength(2);
  expect(extensions.flatMap((extension) => extension.commands)).toEqual(["fast"]);
}, 60_000);

const shellPackage = process.env.PI_INTERACTIVE_SHELL_PACKAGE_DIR;
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

const lspPackage = process.env.PI_LSP_PACKAGE_DIR;
test.skipIf(!lspPackage)("loads patched pi-lsp with npm dependencies and registers refactoring once", async () => {
  const source = resolve(lspPackage!);
  const pin = settings.packages.find((pkg: { source?: string }) => pkg.source?.startsWith("npm:pi-lsp@"));
  expect(pin.extensions).toEqual([]);
  const manifest = JSON.parse(await readFile(join(source, "package.json"), "utf8"));
  expect(`npm:${manifest.name}@${manifest.version}`).toBe(pin.source);

  const { root, agent, cwd } = await fixture();
  const patched = join(root, "patched");
  await mkdir(patched);
  await cp(join(source, "extensions"), join(patched, "extensions"), { recursive: true });
  const patch = spawnSync("patch", ["--batch", "--forward", "-p1", "-i", join(moduleRoot, "lsp-refactor.patch")], {
    cwd: patched, encoding: "utf8", timeout: 10_000,
  });
  if (patch.error) throw patch.error;
  if (patch.status !== 0) throw new Error(`LSP patch failed: ${patch.stdout}\n${patch.stderr}`);
  await cp(join(moduleRoot, "files/extensions/lsp-refactor"), join(patched, "extensions/pi-lsp/refactor"), { recursive: true });
  await writeFile(join(patched, "extensions/index.ts"), 'export { default } from "./pi-lsp/index";\n');
  await symlink(join(patched, "extensions"), join(agent, "extensions/pi-lsp"));
  await mkdir(join(agent, "npm"));
  await symlink(dirname(source), join(agent, "npm/node_modules"));

  await symlink(join(moduleRoot, "files/extensions/rounded-tools.ts"), join(agent, "extensions/rounded-tools.ts"));
  const extensions = load(agent, cwd);
  expect(extensions).toHaveLength(2);
  for (const name of ["lsp_rename", "lsp_code_actions", "lsp_apply_edit", "lsp_hover", "lsp_diagnostics"]) {
    expect(extensions.flatMap((extension) => extension.tools).filter((tool) => tool === name)).toHaveLength(1);
  }
  const owners = extensions.filter((extension) => extension.messageRenderers.includes("pi-lsp-diagnostics"));
  expect(owners).toHaveLength(1);
  expect(owners[0].path).toEndWith("rounded-tools.ts");
}, 60_000);
