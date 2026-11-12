import { spawnSync } from "node:child_process";
import { chmod, cp, mkdtemp, readFile, readdir, realpath, rm, symlink, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

// Exercise the source patch with the installed runtime's exact dependencies, without building or installing Pi.
const repo = process.cwd();
const source = process.env.PI_SOURCE;
if (!source) throw new Error("Set PI_SOURCE to an unpatched Pi source tree matching the repository pin.");
function run(command: string, args: string[], options: { cwd?: string; env?: NodeJS.ProcessEnv } = {}) {
  const result = spawnSync(command, args, { stdio: "inherit", ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} failed with status ${result.status}`);
}
const which = spawnSync("which", ["pi"], { encoding: "utf8" });
if (!process.env.PI_TEST_NODE_MODULES && which.status !== 0) throw new Error("Pi is not on PATH; set PI_TEST_NODE_MODULES.");
const installed = process.env.PI_TEST_NODE_MODULES ?? join(dirname(dirname(await realpath(which.stdout.trim()))), "lib/pi/node_modules");
const packagePath = "@earendil-works/pi-coding-agent";
const manifest = JSON.parse(await readFile(join(installed, packagePath, "package.json"), "utf8"));
const sourceManifest = JSON.parse(await readFile(join(source, "packages/coding-agent/package.json"), "utf8"));
if (manifest.version !== sourceManifest.version) throw new Error("Pi source and installed runtime versions differ.");
const temporary = await mkdtemp(join(tmpdir(), "pi-message-wrappers-"));
async function writable(path: string) {
  await chmod(path, 0o700);
  for (const entry of await readdir(path, { withFileTypes: true })) {
    const child = join(path, entry.name);
    if (entry.isDirectory()) await writable(child);
    else if (entry.isFile()) await chmod(child, 0o600);
  }
}
try {
  const stagedSource = join(temporary, "source");
  await cp(join(source, "packages/coding-agent/src"), join(stagedSource, "packages/coding-agent/src"), { recursive: true });
  await writable(stagedSource);
  run("patch", ["--batch", "--forward", "--fuzz=0", "-p1", "-i", join(repo, "packages/apps/pi/message-renderer-wrappers.patch")], { cwd: stagedSource });
  const nodes = join(temporary, "node_modules");
  await mkdir(join(nodes, "@earendil-works"), { recursive: true });
  for (const entry of await readdir(installed)) {
    if (entry !== "@earendil-works") await symlink(join(installed, entry), join(nodes, entry));
  }
  for (const entry of await readdir(join(installed, "@earendil-works"))) {
    if (entry !== "pi-coding-agent") await symlink(join(installed, "@earendil-works", entry), join(nodes, "@earendil-works", entry));
  }
  const runtime = join(nodes, packagePath);
  await cp(join(installed, packagePath), runtime, { recursive: true, dereference: true });
  await writable(runtime);
  const transpiler = new Bun.Transpiler({ loader: "ts", target: "node" });
  for (const path of ["core/extensions/loader", "core/extensions/runner", "modes/interactive/components/custom-message", "modes/interactive/interactive-mode"]) {
    const ts = await readFile(join(stagedSource, "packages/coding-agent/src", `${path}.ts`), "utf8");
    const js = transpiler.transformSync(ts).replace(/(["']\.{1,2}\/[^"']+)\.ts(["'])/g, "$1.js$2");
    await writeFile(join(runtime, "dist", `${path}.js`), js);
  }
  const wiring = await readFile(join(runtime, "dist/modes/interactive/interactive-mode.js"), "utf8");
  if (!wiring.includes("getMessageRendererWrappers()")) throw new Error("Interactive mode does not pass message wrappers.");
  const tests = process.argv.length > 2 ? process.argv.slice(2) : ["custom-message-frames.test.ts", "rounded-tools.test.ts", "lsp-diagnostics-rendering.test.ts"];
  run("bash", [join(repo, "modules/pi/tests/run.sh"), ...tests], {
    cwd: repo, env: { ...process.env, PI_TEST_NODE_MODULES: resolve(nodes) },
  });
} finally {
  await rm(temporary, { recursive: true, force: true });
}
