import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";

const source = resolve("node_modules/@sveltia/cms/dist/sveltia-cms.js");
const destination = resolve("public/admin/sveltia-cms.js");

mkdirSync(dirname(destination), { recursive: true });
copyFileSync(source, destination);
