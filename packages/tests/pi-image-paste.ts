import { spawnSync } from "node:child_process";
import { chmod, cp, mkdtemp, readFile, readdir, realpath, rm, symlink, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

// Test patched editor code against installed Pi without changing the live runtime.
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
const manifest = JSON.parse(await readFile(join(installed, "@earendil-works/pi-coding-agent/package.json"), "utf8"));
const sourceManifest = JSON.parse(await readFile(join(source, "packages/coding-agent/package.json"), "utf8"));
if (manifest.version !== sourceManifest.version) throw new Error("Pi source and installed runtime versions differ.");
const temporary = await mkdtemp(join(tmpdir(), "pi-image-paste-"));
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
  const editorPath = "packages/tui/src/components/editor.ts";
  await mkdir(dirname(join(stagedSource, editorPath)), { recursive: true });
  await cp(join(source, editorPath), join(stagedSource, editorPath));
  await writable(stagedSource);
  run("patch", ["--batch", "--forward", "--fuzz=0", "-p1", "-i", join(repo, "packages/apps/pi/image-paste-labels.patch")], { cwd: stagedSource });
  const nodes = join(temporary, "node_modules");
  await mkdir(join(nodes, "@earendil-works"), { recursive: true });
  for (const entry of await readdir(installed)) {
    if (entry !== "@earendil-works") await symlink(join(installed, entry), join(nodes, entry));
  }
  for (const entry of await readdir(join(installed, "@earendil-works"))) {
    const destination = join(nodes, "@earendil-works", entry);
    if (["pi-tui", "pi-coding-agent"].includes(entry)) {
      await cp(join(installed, "@earendil-works", entry), destination, { recursive: true, dereference: true });
      await writable(destination);
    } else await symlink(join(installed, "@earendil-works", entry), destination);
  }
  const transpiler = new Bun.Transpiler({ loader: "ts", target: "node" });
  const ts = await readFile(join(stagedSource, editorPath), "utf8");
  const js = transpiler.transformSync(ts).replace(/(["']\.{1,2}\/[^"']+)\.ts(["'])/g, "$1.js$2");
  await writeFile(join(nodes, "@earendil-works/pi-tui/dist/components/editor.js"), js);
  run("bash", [join(repo, "modules/pi/tests/run.sh"), "image-paste.test.ts", "prompt-ui.test.ts", "prompt-statusline.test.ts"], {
    cwd: repo, env: { ...process.env, PI_TEST_NODE_MODULES: nodes },
  });
} finally {
  await rm(temporary, { recursive: true, force: true });
}
