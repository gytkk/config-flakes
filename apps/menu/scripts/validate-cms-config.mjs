import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import Ajv from "ajv";
import addFormats from "ajv-formats";
import { parseDocument } from "yaml";

const require = createRequire(import.meta.url);
const cmsPackage = require.resolve("@sveltia/cms");
const schemaPath = resolve(dirname(cmsPackage), "../schema/sveltia-cms.json");
const schema = JSON.parse(readFileSync(schemaPath, "utf8"));

export const CMS_SECRET_PATTERN =
  /secret(?:_|-)?access(?:_|-)?key|private(?:_|-)?key|password|\btoken\b|github_pat_[a-z0-9_]+|\bgh[pousr]_[a-z0-9]{20,}\b|\b[a-f0-9]{40,64}\b/i;

export function validateConfig(path = "public/admin/config.yml") {
  const source = readFileSync(resolve(path), "utf8");
  if (CMS_SECRET_PATTERN.test(source)) {
    throw new Error("Secret-like value found in CMS configuration");
  }

  const document = parseDocument(source);
  if (document.errors.length > 0) {
    throw new Error(document.errors.map((error) => error.message).join("\n"));
  }

  const config = document.toJS();
  const ajv = new Ajv({ allErrors: true, strict: false });
  addFormats(ajv);
  ajv.addFormat("regex", true);
  const validate = ajv.compile(schema);
  if (!validate(config)) {
    throw new Error(
      validate.errors
        ?.map((error) => `${error.instancePath || "/"} ${error.message}`)
        .join("\n"),
    );
  }

  return config;
}

if (import.meta.main) {
  validateConfig(process.argv[2]);
}
